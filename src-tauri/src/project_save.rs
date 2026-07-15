use crate::def_index::{
    apply_replacement_overlay, DefIndex, DefIndexReplacement, IndexedFileFingerprint,
};
#[cfg(test)]
use crate::def_index::{load_or_rebuild_def_index, DefIndexBuildOptions};
use crate::project_files::{validate_and_resolve, ProjectFileError};
use crate::project_model::{AppError, ProjectSettings};
use crate::rimworld_load_folders::read_load_folders_version_keys;
use crate::schema_pack::{build_schema_catalog, schema_pack_roots};
use crate::xml_document::{
    parse_to_document, parse_xml_document, validate_about_metadata_document, validate_document,
    ValidationContext, XmlDocumentProfile,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use std::io::Write;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePreview {
    pub project_id: String,
    pub relative_path: String,
    pub current_hash: String,
    pub proposed_hash: String,
    pub changed: bool,
    pub diff: Vec<DiffLine>,
    pub validation_token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub project_id: String,
    pub relative_path: String,
    pub backup_path: String,
    pub bytes_written: usize,
    pub current_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
    /// Set only for `DiffLineKind::Gap`: the number of elided unchanged lines this marker
    /// stands in for. Machine-readable so the frontend can pluralize/format it via `t()`
    /// instead of Rust assembling an English sentence (`text` is empty for gap rows).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    Unchanged,
    Added,
    Removed,
    /// A collapsed run of unchanged lines elided from the preview. `count` holds the number
    /// of elided lines; both line numbers are `None` and `text` is empty.
    Gap,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectSaveError {
    #[error("Project not found: {0}")]
    ProjectNotFound(String),
    #[error("Project is not editable: {0}")]
    ProjectNotEditable(String),
    #[error("File path is outside project root")]
    FileOutsideRoot,
    #[error("File type is not supported")]
    UnsupportedFile,
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Proposed XML is invalid: {0}")]
    InvalidXml(String),
    #[error("Backup failed: {0}")]
    BackupFailed(String),
    #[error("Temp write failed: {0}")]
    TempWriteFailed(String),
    #[error("Replace failed: {0}")]
    ReplaceFailed(String),
}

impl From<ProjectFileError> for ProjectSaveError {
    fn from(e: ProjectFileError) -> Self {
        match e {
            ProjectFileError::ProjectNotFound(s) => ProjectSaveError::ProjectNotFound(s),
            ProjectFileError::ProjectNotEditable(s) => ProjectSaveError::ProjectNotEditable(s),
            ProjectFileError::FileOutsideRoot => ProjectSaveError::FileOutsideRoot,
            ProjectFileError::UnsupportedFile => ProjectSaveError::UnsupportedFile,
            ProjectFileError::FileNotFound(s) => ProjectSaveError::FileNotFound(s),
            ProjectFileError::ScanFailed(s) => ProjectSaveError::BackupFailed(s),
            ProjectFileError::InvalidFileName(s) => ProjectSaveError::BackupFailed(s),
            ProjectFileError::PathAlreadyExists(s) => ProjectSaveError::BackupFailed(s),
            ProjectFileError::KindMismatch(s) => ProjectSaveError::BackupFailed(s),
            ProjectFileError::CannotModifyRoot => {
                ProjectSaveError::BackupFailed("Cannot modify root".to_string())
            }
        }
    }
}

impl From<ProjectSaveError> for AppError {
    fn from(e: ProjectSaveError) -> Self {
        let code = match &e {
            ProjectSaveError::ProjectNotFound(_) => "save_project_not_found",
            ProjectSaveError::ProjectNotEditable(_) => "save_project_not_editable",
            ProjectSaveError::FileOutsideRoot => "save_file_outside_root",
            ProjectSaveError::UnsupportedFile => "save_unsupported_file",
            ProjectSaveError::FileNotFound(_) => "save_file_not_found",
            ProjectSaveError::InvalidXml(_) => "save_invalid_xml",
            ProjectSaveError::BackupFailed(_) => "save_backup_failed",
            ProjectSaveError::TempWriteFailed(_) => "save_temp_write_failed",
            ProjectSaveError::ReplaceFailed(_) => "save_replace_failed",
        };
        // `ProjectNotFound`/`ProjectNotEditable`/`FileNotFound` carry clean literal identifiers;
        // the remaining variants wrap arbitrary IO-error text.
        let args = match &e {
            ProjectSaveError::ProjectNotFound(id) | ProjectSaveError::ProjectNotEditable(id) => {
                crate::diagnostics::diagnostic_args([("projectId", id.as_str().into())])
            }
            ProjectSaveError::FileNotFound(path) => {
                crate::diagnostics::diagnostic_args([("path", path.as_str().into())])
            }
            _ => crate::diagnostics::DiagnosticArgs::new(),
        };
        AppError {
            code: code.to_string(),
            message: e.to_string(),
            details: None,
            args,
        }
    }
}

#[cfg(test)]
mod diagnostic_ref_wire_tests {
    use super::*;

    #[test]
    fn file_not_found_carries_path_arg() {
        let err: AppError = ProjectSaveError::FileNotFound("Defs/Foo.xml".to_string()).into();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "save_file_not_found");
        assert_eq!(json["args"]["path"], "Defs/Foo.xml");
    }
}

/// Per-session secret used to generate and verify validation tokens.
/// Register as Tauri managed state so the secret is stable within one app session.
pub struct SaveValidationSecret([u8; 32]);

impl Default for SaveValidationSecret {
    fn default() -> Self {
        let mut secret = [0u8; 32];
        let u1 = uuid::Uuid::new_v4();
        let u2 = uuid::Uuid::new_v4();
        secret[..16].copy_from_slice(u1.as_bytes());
        secret[16..].copy_from_slice(u2.as_bytes());
        SaveValidationSecret(secret)
    }
}

impl SaveValidationSecret {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

pub fn hash_xml(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Computes a validation token that binds a specific (project, file, current, proposed, index)
/// tuple to the app-session secret. A matching token proves the preview validated this exact
/// payload against this exact index state.
pub fn compute_validation_token(
    secret: &[u8],
    project_id: &str,
    relative_path: &str,
    current_hash: &str,
    proposed_hash: &str,
    index_fp: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.update(b"\x00");
    hasher.update(project_id.as_bytes());
    hasher.update(b"\x00");
    hasher.update(relative_path.as_bytes());
    hasher.update(b"\x00");
    hasher.update(current_hash.as_bytes());
    hasher.update(b"\x00");
    hasher.update(proposed_hash.as_bytes());
    hasher.update(b"\x00");
    hasher.update(index_fp.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Hashes the stored file fingerprints into a single string that captures the index state.
/// Fingerprints must already be sorted (they are sorted in the cache by location_id/relative_path).
pub fn hash_file_fingerprints(fps: &[IndexedFileFingerprint]) -> String {
    let mut hasher = Sha256::new();
    for fp in fps {
        hasher.update(fp.location_id.as_bytes());
        hasher.update(b"\x00");
        hasher.update(fp.relative_path.as_bytes());
        hasher.update(b"\x00");
        hasher.update(fp.content_hash.as_bytes());
        hasher.update(b"\x01");
    }
    format!("{:x}", hasher.finalize())
}

pub fn backup_path_from_base(
    base_dir: &Path,
    project_id: &str,
    relative_path: &str,
    now: OffsetDateTime,
    suffix: &str,
) -> PathBuf {
    let rel = Path::new(relative_path);
    let parent = rel.parent().unwrap_or(Path::new(""));
    let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ts = format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    );
    let backup_dir = base_dir.join("backups").join(project_id).join(parent);
    backup_dir.join(format!("{}.{}.{}.xml", stem, ts, suffix))
}

fn check_xml_well_formed(relative_path: &str, proposed_xml: &str) -> Result<(), ProjectSaveError> {
    let result = parse_xml_document(relative_path, proposed_xml);
    if result.document.is_none() {
        let msg = result
            .parse_diagnostics
            .first()
            .map(|d| d.message.clone())
            .unwrap_or_else(|| "unknown parse error".to_string());
        return Err(ProjectSaveError::InvalidXml(msg));
    }
    Ok(())
}

/// Validates proposed XML using a pre-loaded def index, avoiding a redundant disk load.
pub fn validate_proposed_xml_with_index(
    settings: &ProjectSettings,
    base_index: &DefIndex,
    project_id: &str,
    relative_path: &str,
    proposed_xml: &str,
) -> Result<(), ProjectSaveError> {
    check_xml_well_formed(relative_path, proposed_xml)?;
    let mut doc = parse_to_document(relative_path, proposed_xml);

    if doc.profile == XmlDocumentProfile::About {
        let load_folders_versions = settings
            .locations
            .iter()
            .find(|l| l.id == project_id)
            .and_then(|l| read_load_folders_version_keys(std::path::Path::new(&l.root_path)));
        doc.validation_diagnostics =
            validate_about_metadata_document(&doc, load_folders_versions.as_deref());
    } else {
        // Same catalog-context policy as live document validation
        // (`services::validation::validate_doc_for_project`/`validate_doc_for_source`): every
        // registered location's root as a candidate external-schema-pack root, filtered by the
        // project's selected game version. Save preview/final save must not diverge from what
        // the live form/editor already validated against -- see Plan.md section 15's
        // "catalog-context mismatch" and issue 09.
        let roots = schema_pack_roots(settings);
        let catalog_result = build_schema_catalog(&roots, Some(&settings.game_version));
        let def_index = apply_replacement_overlay(
            base_index.clone(),
            settings,
            DefIndexReplacement {
                location_id: project_id,
                relative_path,
                source: proposed_xml,
            },
        );
        let context = ValidationContext {
            catalog: &catalog_result.catalog,
            def_index: &def_index,
        };
        doc.validation_diagnostics = validate_document(&doc, &context);
    }

    if let Some(diagnostic) = doc.validation_diagnostics.iter().find(|d| d.blocking) {
        return Err(ProjectSaveError::InvalidXml(diagnostic.message.clone()));
    }
    Ok(())
}

#[cfg(test)]
pub fn validate_proposed_xml(
    settings: &ProjectSettings,
    app_data_dir: &Path,
    project_id: &str,
    relative_path: &str,
    proposed_xml: &str,
) -> Result<(), ProjectSaveError> {
    let base_index = load_or_rebuild_def_index(
        app_data_dir,
        settings,
        DefIndexBuildOptions {
            project_id: Some(project_id),
            include_sources: true,
            replacement: None,
            force_rebuild: false,
        },
    )
    .map_err(|e| ProjectSaveError::InvalidXml(e.to_string()))?;
    validate_proposed_xml_with_index(
        settings,
        &base_index,
        project_id,
        relative_path,
        proposed_xml,
    )
}

fn normalize_line_endings_for_diff(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Number of unchanged context lines kept on each side of a change in the preview diff.
const DIFF_CONTEXT_LINES: usize = 3;

/// Hunked diff (default): only changed hunks plus a few context lines, with long unchanged
/// runs collapsed into a single Gap marker. Keeps the preview render and IPC payload
/// proportional to the size of the change, not the whole file.
pub fn generate_line_diff(old_xml: &str, proposed_xml: &str) -> Vec<DiffLine> {
    generate_line_diff_inner(old_xml, proposed_xml, true)
}

/// Full diff: every line, no collapsing. Used by the "Show full file" affordance.
pub fn generate_line_diff_full(old_xml: &str, proposed_xml: &str) -> Vec<DiffLine> {
    generate_line_diff_inner(old_xml, proposed_xml, false)
}

fn generate_line_diff_inner(old_xml: &str, proposed_xml: &str, collapse: bool) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(old_xml, proposed_xml);
    let mut lines = Vec::new();
    for change in diff.iter_all_changes() {
        let kind = match change.tag() {
            ChangeTag::Equal => DiffLineKind::Unchanged,
            ChangeTag::Delete => DiffLineKind::Removed,
            ChangeTag::Insert => DiffLineKind::Added,
        };
        let old_line = change.old_index().map(|i| i + 1);
        let new_line = change.new_index().map(|i| i + 1);
        let text = change.value().to_string();
        lines.push(DiffLine {
            kind,
            old_line,
            new_line,
            text,
            count: None,
        });
    }
    if collapse {
        collapse_unchanged_runs(lines, DIFF_CONTEXT_LINES)
    } else {
        lines
    }
}

fn collapse_unchanged_runs(lines: Vec<DiffLine>, context: usize) -> Vec<DiffLine> {
    let n = lines.len();
    let mut keep = vec![false; n];
    for (i, line) in lines.iter().enumerate() {
        if !matches!(line.kind, DiffLineKind::Unchanged) {
            let start = i.saturating_sub(context);
            let end = (i + context + 1).min(n);
            for slot in keep.iter_mut().take(end).skip(start) {
                *slot = true;
            }
        }
    }

    let mut out = Vec::new();
    for (i, line) in lines.into_iter().enumerate() {
        if keep[i] {
            out.push(line);
        } else if !matches!(
            out.last().map(|l: &DiffLine| &l.kind),
            Some(DiffLineKind::Gap)
        ) {
            // Start a new gap; extend its count as the run continues.
            out.push(DiffLine {
                kind: DiffLineKind::Gap,
                old_line: None,
                new_line: None,
                text: String::new(),
                count: Some(1),
            });
        } else if let Some(last) = out.last_mut() {
            last.count = Some(last.count.unwrap_or(1) + 1);
        }
    }

    out
}

fn perform_file_write(
    app_data_dir: &Path,
    project_id: &str,
    relative_path: &str,
    canonical: &Path,
    current_xml: &str,
    proposed_xml: &str,
) -> Result<SaveResult, ProjectSaveError> {
    let now = OffsetDateTime::now_utc();
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let bpath = backup_path_from_base(app_data_dir, project_id, relative_path, now, suffix);

    if let Some(bdir) = bpath.parent() {
        std::fs::create_dir_all(bdir)
            .map_err(|e| ProjectSaveError::BackupFailed(format!("create backup dir: {}", e)))?;
    }
    {
        let mut backup_file = std::fs::File::create(&bpath)
            .map_err(|e| ProjectSaveError::BackupFailed(format!("create backup: {}", e)))?;
        backup_file
            .write_all(current_xml.as_bytes())
            .map_err(|e| ProjectSaveError::BackupFailed(format!("write backup: {}", e)))?;
        backup_file
            .sync_all()
            .map_err(|e| ProjectSaveError::BackupFailed(format!("sync backup: {}", e)))?;
    }

    let target_dir = canonical.parent().ok_or_else(|| {
        ProjectSaveError::TempWriteFailed("target has no parent directory".to_string())
    })?;
    let mut temp = tempfile::NamedTempFile::new_in(target_dir)
        .map_err(|e| ProjectSaveError::TempWriteFailed(format!("create temp: {}", e)))?;
    temp.write_all(proposed_xml.as_bytes())
        .map_err(|e| ProjectSaveError::TempWriteFailed(format!("write temp: {}", e)))?;
    temp.flush()
        .map_err(|e| ProjectSaveError::TempWriteFailed(format!("flush temp: {}", e)))?;
    temp.as_file()
        .sync_all()
        .map_err(|e| ProjectSaveError::TempWriteFailed(format!("sync temp: {}", e)))?;

    temp.persist(canonical)
        .map_err(|e| ProjectSaveError::ReplaceFailed(format!("persist: {}", e.error)))?;

    let current_hash = hash_xml(proposed_xml);
    Ok(SaveResult {
        project_id: project_id.to_string(),
        relative_path: relative_path.to_string(),
        backup_path: bpath.to_string_lossy().to_string(),
        bytes_written: proposed_xml.len(),
        current_hash,
    })
}

/// Generates a save preview using a pre-loaded def index.
/// The `validation_token` field is left empty; the command layer sets it from the app secret.
pub fn preview_xml_save_with_index(
    settings: &ProjectSettings,
    base_index: &DefIndex,
    project_id: &str,
    relative_path: &str,
    proposed_xml: &str,
    collapse_diff: bool,
) -> Result<SavePreview, ProjectSaveError> {
    let canonical = validate_and_resolve(settings, project_id, relative_path)?;
    validate_proposed_xml_with_index(
        settings,
        base_index,
        project_id,
        relative_path,
        proposed_xml,
    )?;
    let current = std::fs::read_to_string(&canonical)
        .map_err(|e| ProjectSaveError::FileNotFound(format!("{}: {}", relative_path, e)))?;
    let current_hash = hash_xml(&current);
    let proposed_hash = hash_xml(proposed_xml);
    let changed = current_hash != proposed_hash;
    let current_normalized = normalize_line_endings_for_diff(&current);
    let proposed_normalized = normalize_line_endings_for_diff(proposed_xml);
    let diff = if collapse_diff {
        generate_line_diff(&current_normalized, &proposed_normalized)
    } else {
        generate_line_diff_full(&current_normalized, &proposed_normalized)
    };
    Ok(SavePreview {
        project_id: project_id.to_string(),
        relative_path: relative_path.to_string(),
        current_hash,
        proposed_hash,
        changed,
        diff,
        validation_token: String::new(),
    })
}

#[cfg(test)]
pub fn preview_xml_save(
    settings: &ProjectSettings,
    app_data_dir: &Path,
    project_id: &str,
    relative_path: &str,
    proposed_xml: &str,
) -> Result<SavePreview, ProjectSaveError> {
    let base_index = load_or_rebuild_def_index(
        app_data_dir,
        settings,
        DefIndexBuildOptions {
            project_id: Some(project_id),
            include_sources: true,
            replacement: None,
            force_rebuild: false,
        },
    )
    .map_err(|e| ProjectSaveError::InvalidXml(e.to_string()))?;
    preview_xml_save_with_index(
        settings,
        &base_index,
        project_id,
        relative_path,
        proposed_xml,
        true,
    )
}

/// Fast save path: verifies the validation token using pre-cached in-memory file fingerprints
/// (no project-wide file scan) and writes the file if the token matches.
///
/// Returns `Ok(Some(result))` when the token is valid and the file was written.
/// Returns `Ok(None)` when the token does not match - the caller should fall back to the slow
/// path (`save_project_xml_with_index`) which loads the index and re-validates.
/// Returns `Err` for definitive errors (path rejected, malformed XML, write failure).
#[allow(clippy::too_many_arguments)]
pub fn try_save_with_fast_token(
    settings: &ProjectSettings,
    app_data_dir: &Path,
    project_id: &str,
    relative_path: &str,
    proposed_xml: &str,
    validation_token: &str,
    secret: &[u8],
    index_fp: &str,
) -> Result<Option<SaveResult>, ProjectSaveError> {
    let canonical = validate_and_resolve(settings, project_id, relative_path)?;
    check_xml_well_formed(relative_path, proposed_xml)?;
    let current = std::fs::read_to_string(&canonical)
        .map_err(|e| ProjectSaveError::FileNotFound(format!("{}: {}", relative_path, e)))?;
    let current_hash = hash_xml(&current);
    let proposed_hash = hash_xml(proposed_xml);
    let expected = compute_validation_token(
        secret,
        project_id,
        relative_path,
        &current_hash,
        &proposed_hash,
        index_fp,
    );
    if validation_token != expected {
        return Ok(None);
    }
    let result = perform_file_write(
        app_data_dir,
        project_id,
        relative_path,
        &canonical,
        &current,
        proposed_xml,
    )?;
    Ok(Some(result))
}

/// Saves a project XML file using a pre-loaded def index, always running full semantic validation.
/// Call this on the fallback path when no valid token is available.
pub fn save_project_xml_with_index(
    settings: &ProjectSettings,
    app_data_dir: &Path,
    base_index: &DefIndex,
    project_id: &str,
    relative_path: &str,
    proposed_xml: &str,
) -> Result<SaveResult, ProjectSaveError> {
    let canonical = validate_and_resolve(settings, project_id, relative_path)?;
    validate_proposed_xml_with_index(
        settings,
        base_index,
        project_id,
        relative_path,
        proposed_xml,
    )?;
    let current = std::fs::read_to_string(&canonical)
        .map_err(|e| ProjectSaveError::FileNotFound(format!("{}: {}", relative_path, e)))?;
    perform_file_write(
        app_data_dir,
        project_id,
        relative_path,
        &canonical,
        &current,
        proposed_xml,
    )
}

#[cfg(test)]
pub fn save_project_xml(
    settings: &ProjectSettings,
    app_data_dir: &Path,
    project_id: &str,
    relative_path: &str,
    proposed_xml: &str,
) -> Result<SaveResult, ProjectSaveError> {
    let canonical = validate_and_resolve(settings, project_id, relative_path)?;
    validate_proposed_xml(
        settings,
        app_data_dir,
        project_id,
        relative_path,
        proposed_xml,
    )?;
    let current = std::fs::read_to_string(&canonical)
        .map_err(|e| ProjectSaveError::FileNotFound(format!("{}: {}", relative_path, e)))?;
    perform_file_write(
        app_data_dir,
        project_id,
        relative_path,
        &canonical,
        &current,
        proposed_xml,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};
    use std::fs;
    use time::OffsetDateTime;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rimedit_save_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_settings(root: &Path) -> ProjectSettings {
        ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![RegisteredLocation {
                id: "proj1".to_string(),
                display_name: "Test Project".to_string(),
                root_path: root.to_string_lossy().to_string(),
                kind: LocationKind::Project,
                source_type: SourceType::Folder,
                read_only: false,
                mod_id: None,
                game_version: None,
                expansion_name: None,
                created_at: OffsetDateTime::now_utc(),
                updated_at: OffsetDateTime::now_utc(),
            }],
            active_project_id: Some("proj1".to_string()),
        }
    }

    const VALID_XML: &str = "<Defs><ThingDef><defName>Rock</defName></ThingDef></Defs>";
    const VALID_XML_2: &str = "<Defs><ThingDef><defName>Stone</defName></ThingDef></Defs>";
    const INVALID_XML: &str = "<Defs><ThingDef><defName>Rock</defName></Defs>";

    #[test]
    fn backup_path_is_timestamped_and_unique() {
        let base = temp_dir();
        let now = OffsetDateTime::now_utc();
        let s1 = &uuid::Uuid::new_v4().to_string()[..8];
        let s2 = &uuid::Uuid::new_v4().to_string()[..8];
        let p1 = backup_path_from_base(&base, "proj1", "Defs/Items.xml", now, s1);
        let p2 = backup_path_from_base(&base, "proj1", "Defs/Items.xml", now, s2);

        let n1 = p1.file_name().unwrap().to_string_lossy();
        let _n2 = p2.file_name().unwrap().to_string_lossy();

        assert!(n1.starts_with("Items."), "stem missing: {}", n1);
        assert!(n1.ends_with(".xml"), "extension missing: {}", n1);
        assert!(n1.contains('T'), "timestamp missing: {}", n1);
        assert_ne!(p1, p2, "two paths with different suffix should differ");
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn safe_save_writes_backup_temp_then_replaces_original() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let result =
            save_project_xml(&settings, &app_data_dir, "proj1", "file.xml", VALID_XML_2).unwrap();

        let on_disk = fs::read_to_string(&file_path).unwrap();
        assert_eq!(on_disk, VALID_XML_2, "target should contain proposed XML");

        let backup_content = fs::read_to_string(&result.backup_path).unwrap();
        assert_eq!(backup_content, VALID_XML, "backup should contain old XML");

        let temp_count = fs::read_dir(&project_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name().to_string_lossy().starts_with(".tmp")
                    || e.path().extension().and_then(|x| x.to_str()) == Some("tmp")
            })
            .count();
        assert_eq!(temp_count, 0, "no temp files should remain");

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn line_diff_captures_added_removed_and_changed_lines() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nchanged2\nline3\nnewline4\n";
        let diff = generate_line_diff(old, new);

        let removed: Vec<_> = diff
            .iter()
            .filter(|d| matches!(d.kind, DiffLineKind::Removed))
            .collect();
        let added: Vec<_> = diff
            .iter()
            .filter(|d| matches!(d.kind, DiffLineKind::Added))
            .collect();

        assert!(!removed.is_empty(), "expected removed lines");
        assert!(!added.is_empty(), "expected added lines");

        let removed_texts: Vec<&str> = removed.iter().map(|d| d.text.trim()).collect();
        let added_texts: Vec<&str> = added.iter().map(|d| d.text.trim()).collect();
        assert!(
            removed_texts.contains(&"line2"),
            "line2 should be removed: {:?}",
            removed_texts
        );
        assert!(
            added_texts.contains(&"changed2"),
            "changed2 should be added: {:?}",
            added_texts
        );
        assert!(
            added_texts.contains(&"newline4"),
            "newline4 should be added: {:?}",
            added_texts
        );

        for d in diff
            .iter()
            .filter(|d| matches!(d.kind, DiffLineKind::Removed))
        {
            assert!(d.old_line.is_some(), "removed lines must have old_line");
        }
        for d in diff
            .iter()
            .filter(|d| matches!(d.kind, DiffLineKind::Added))
        {
            assert!(d.new_line.is_some(), "added lines must have new_line");
        }
    }

    #[test]
    fn line_diff_collapses_unchanged_runs_into_gaps() {
        // A small change in a large file should emit only the change + context, plus a Gap
        // marker for the elided unchanged lines - not the whole file.
        let mut old = String::new();
        for i in 0..200 {
            old.push_str(&format!("line{}\n", i));
        }
        let mut new = old.clone();
        new = new.replace("line100\n", "CHANGED100\n");

        let diff = generate_line_diff(&old, &new);

        // Far fewer rows than the 200+ lines of the file.
        assert!(
            diff.len() < 20,
            "expected a compact hunked diff, got {} rows",
            diff.len()
        );
        // The change is present.
        assert!(diff
            .iter()
            .any(|d| matches!(d.kind, DiffLineKind::Added) && d.text.trim() == "CHANGED100"));
        assert!(diff
            .iter()
            .any(|d| matches!(d.kind, DiffLineKind::Removed) && d.text.trim() == "line100"));
        // Gap markers stand in for the elided unchanged lines.
        let gaps: Vec<_> = diff
            .iter()
            .filter(|d| matches!(d.kind, DiffLineKind::Gap))
            .collect();
        assert_eq!(
            gaps.len(),
            2,
            "expected a leading and trailing gap: {:?}",
            gaps
        );
        assert!(gaps[0].count.is_some_and(|c| c > 0));
        assert!(gaps[0].text.is_empty());
        // Context lines immediately around the change are kept.
        assert!(diff
            .iter()
            .any(|d| matches!(d.kind, DiffLineKind::Unchanged) && d.text.trim() == "line99"));
        assert!(diff
            .iter()
            .any(|d| matches!(d.kind, DiffLineKind::Unchanged) && d.text.trim() == "line101"));
    }

    #[test]
    fn line_diff_full_keeps_every_line_without_gaps() {
        let mut old = String::new();
        for i in 0..200 {
            old.push_str(&format!("line{}\n", i));
        }
        let new = old.replace("line100\n", "CHANGED100\n");

        let full = generate_line_diff_full(&old, &new);

        // No collapsing: every original line is represented and there are no gap markers.
        assert!(
            full.len() >= 200,
            "full diff should keep every line, got {}",
            full.len()
        );
        assert!(
            !full.iter().any(|d| matches!(d.kind, DiffLineKind::Gap)),
            "full diff must not contain gap markers"
        );
        assert!(full
            .iter()
            .any(|d| matches!(d.kind, DiffLineKind::Unchanged) && d.text.trim() == "line0"));
        assert!(full
            .iter()
            .any(|d| matches!(d.kind, DiffLineKind::Unchanged) && d.text.trim() == "line199"));
    }

    #[test]
    fn invalid_xml_is_rejected_before_backup() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let err = save_project_xml(&settings, &app_data_dir, "proj1", "file.xml", INVALID_XML)
            .unwrap_err();

        assert!(
            matches!(err, ProjectSaveError::InvalidXml(_)),
            "expected InvalidXml, got {:?}",
            err
        );

        let on_disk = fs::read_to_string(&file_path).unwrap();
        assert_eq!(on_disk, VALID_XML, "original must be unchanged");

        let backup_dir = app_data_dir.join("backups");
        assert!(
            !backup_dir.exists(),
            "no backup directory should be created for invalid XML"
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn empty_content_is_rejected_before_backup() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let err = save_project_xml(&settings, &app_data_dir, "proj1", "file.xml", "").unwrap_err();
        assert!(
            matches!(err, ProjectSaveError::InvalidXml(_)),
            "empty content should be rejected: {:?}",
            err
        );
        assert!(!app_data_dir.join("backups").exists());
        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn multiple_root_elements_is_rejected_before_backup() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let err = save_project_xml(&settings, &app_data_dir, "proj1", "file.xml", "<A/><B/>")
            .unwrap_err();
        assert!(
            matches!(err, ProjectSaveError::InvalidXml(_)),
            "multiple root elements should be rejected: {:?}",
            err
        );
        assert!(!app_data_dir.join("backups").exists());
        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn outside_root_relative_paths_are_rejected() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        fs::write(project_dir.join("ok.xml"), VALID_XML).unwrap();
        let settings = make_settings(&project_dir);

        let err_traversal = save_project_xml(
            &settings,
            &app_data_dir,
            "proj1",
            "../outside.xml",
            VALID_XML_2,
        )
        .unwrap_err();
        assert!(
            matches!(err_traversal, ProjectSaveError::FileOutsideRoot),
            "expected FileOutsideRoot for .., got {:?}",
            err_traversal
        );

        let abs = project_dir.join("ok.xml").to_string_lossy().to_string();
        let err_abs =
            save_project_xml(&settings, &app_data_dir, "proj1", &abs, VALID_XML_2).unwrap_err();
        assert!(
            matches!(err_abs, ProjectSaveError::FileOutsideRoot),
            "expected FileOutsideRoot for absolute path, got {:?}",
            err_abs
        );

        assert!(
            !app_data_dir.join("backups").exists(),
            "no backup should be created"
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn preview_does_not_write_or_backup() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let preview =
            preview_xml_save(&settings, &app_data_dir, "proj1", "file.xml", VALID_XML_2).unwrap();

        assert!(preview.changed);
        let on_disk = fs::read_to_string(&file_path).unwrap();
        assert_eq!(on_disk, VALID_XML, "preview must not modify the file");
        assert!(
            !app_data_dir.join("backups").exists(),
            "preview must not create backups"
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn preview_allows_unnamed_def() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let proposed = "<Defs><ThingDef><label>rock</label></ThingDef></Defs>";
        let preview =
            preview_xml_save(&settings, &app_data_dir, "proj1", "file.xml", proposed).unwrap();

        assert!(preview.changed);

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn preview_allows_unknown_field_warnings() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let proposed =
            "<Defs><ThingDef><defName>Rock</defName><madeUp>1</madeUp></ThingDef></Defs>";
        let preview =
            preview_xml_save(&settings, &app_data_dir, "proj1", "file.xml", proposed).unwrap();

        assert!(preview.changed);

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    // Save validation now filters by `settings.game_version`
    // instead of the old version-blind `build_schema_catalog(&[], None)`. Could an existing
    // project whose configured game version doesn't resolve to ANY
    // installed schema pack (e.g. a stale/orphaned version left over from a removed external
    // pack) start seeing NEW save failures purely because of this filtering -- possibly even a
    // genuinely NEW *blocking* diagnostic, not just a non-blocking "unknown def type" warning, if
    // a conflicting version-agnostic pack were present? It cannot, because
    // `schema_pack::filter_packs_by_game_version` treats an unresolvable selected version as
    // equivalent to "no filter" (falls back to the full, unfiltered pack set) rather than
    // silently narrowing to just the version-agnostic packs -- see the
    // `schema_pack::tests::unresolvable_game_version_*` tests for the general mechanism,
    // including the conflicting-pack scenario. This test proves the concrete save-path
    // consequence: with only the built-in "1.6" pack installed and no external roots, the
    // fallback here means "9.9" still resolves the FULL built-in catalog (not an empty one), so
    // `ThingDef` is recognized normally and save succeeds cleanly -- consistent with what the
    // live editor/`validate_doc_for_project` already show for the same project (both fixed
    // together in issue 09) rather than a new, save-specific regression.
    #[test]
    fn save_validation_is_not_blocked_by_an_unresolvable_configured_game_version() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let mut settings = make_settings(&project_dir);
        // No installed schema pack targets "9.9" -- the built-in pack only targets "1.6", and no
        // external pack root is registered, so this game version is unresolvable and the
        // fallback (full, unfiltered catalog) applies.
        settings.game_version = "9.9".to_string();

        let result =
            validate_proposed_xml(&settings, &app_data_dir, "proj1", "file.xml", VALID_XML_2);

        assert!(
            result.is_ok(),
            "an unresolvable configured game version must not newly block save: {:?}",
            result
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn preview_allows_valid_about_xml() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let about_dir = project_dir.join("About");
        fs::create_dir_all(&about_dir).unwrap();
        let file_path = about_dir.join("About.xml");
        fs::write(
            &file_path,
            "<ModMetaData><packageId>foo.bar</packageId></ModMetaData>",
        )
        .unwrap();

        let settings = make_settings(&project_dir);
        let proposed = "<ModMetaData><packageId>foo.bar</packageId><name>Foo</name><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>";
        let preview = preview_xml_save(
            &settings,
            &app_data_dir,
            "proj1",
            "About/About.xml",
            proposed,
        )
        .unwrap();

        assert!(preview.changed);

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn preview_rejects_about_xml_with_invalid_package_id() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let about_dir = project_dir.join("About");
        fs::create_dir_all(&about_dir).unwrap();
        let file_path = about_dir.join("About.xml");
        fs::write(
            &file_path,
            "<ModMetaData><packageId>foo.bar</packageId></ModMetaData>",
        )
        .unwrap();

        let settings = make_settings(&project_dir);
        let proposed = "<ModMetaData><packageId>not_a_valid_id</packageId></ModMetaData>";
        let err = preview_xml_save(
            &settings,
            &app_data_dir,
            "proj1",
            "About/About.xml",
            proposed,
        )
        .unwrap_err();

        assert!(
            matches!(err, ProjectSaveError::InvalidXml(_)),
            "invalid packageId should block save preview: {:?}",
            err
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn preview_rejects_scalar_text_in_list_fields() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let proposed =
            "<Defs><ThingDef><defName>Rock</defName><recipes>MakeThing</recipes></ThingDef></Defs>";
        let err =
            preview_xml_save(&settings, &app_data_dir, "proj1", "file.xml", proposed).unwrap_err();

        assert!(
            matches!(err, ProjectSaveError::InvalidXml(_)),
            "scalar list text should block save preview: {:?}",
            err
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn unchanged_preview_returns_changed_false() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let preview =
            preview_xml_save(&settings, &app_data_dir, "proj1", "file.xml", VALID_XML).unwrap();

        assert!(
            !preview.changed,
            "same content should produce changed=false"
        );
        assert_eq!(preview.current_hash, preview.proposed_hash);

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn non_xml_target_is_rejected() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        fs::write(project_dir.join("data.txt"), "text").unwrap();

        let settings = make_settings(&project_dir);
        let err = save_project_xml(&settings, &app_data_dir, "proj1", "data.txt", VALID_XML_2)
            .unwrap_err();
        assert!(
            matches!(err, ProjectSaveError::UnsupportedFile),
            "expected UnsupportedFile, got {:?}",
            err
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn read_only_location_is_rejected() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        fs::write(project_dir.join("file.xml"), VALID_XML).unwrap();

        let mut settings = make_settings(&project_dir);
        settings.locations[0].read_only = true;

        let err = save_project_xml(&settings, &app_data_dir, "proj1", "file.xml", VALID_XML_2)
            .unwrap_err();
        assert!(
            matches!(err, ProjectSaveError::ProjectNotEditable(_)),
            "expected ProjectNotEditable, got {:?}",
            err
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn line_diff_ignores_line_ending_only_changes_when_normalized_for_preview() {
        let old = "a\r\nb\r\nc\r\n";
        let new = "a\nB\nc\n";
        let diff = generate_line_diff(
            &normalize_line_endings_for_diff(old),
            &normalize_line_endings_for_diff(new),
        );
        let removed: Vec<_> = diff
            .iter()
            .filter(|d| matches!(d.kind, DiffLineKind::Removed))
            .collect();
        let added: Vec<_> = diff
            .iter()
            .filter(|d| matches!(d.kind, DiffLineKind::Added))
            .collect();
        assert_eq!(
            removed.len(),
            1,
            "only line 2 should be removed: {:?}",
            removed
        );
        assert_eq!(added.len(), 1, "only line 2 should be added: {:?}", added);
        assert_eq!(removed[0].text.trim(), "b");
        assert_eq!(added[0].text.trim(), "B");
    }

    #[test]
    fn preview_allows_source_duplicate_warning() {
        let project_dir = temp_dir();
        let source_dir = temp_dir();
        let app_data_dir = temp_dir();

        fs::write(
            project_dir.join("file.xml"),
            "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
        )
        .unwrap();
        fs::write(
            source_dir.join("core.xml"),
            "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
        )
        .unwrap();

        let mut settings = make_settings(&project_dir);
        settings.locations.push(RegisteredLocation {
            id: "core".to_string(),
            display_name: "RimWorld Core".to_string(),
            root_path: source_dir.to_string_lossy().to_string(),
            kind: LocationKind::Source,
            source_type: SourceType::Folder,
            read_only: true,
            mod_id: None,
            game_version: None,
            expansion_name: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        });

        let proposed = "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>";
        let preview =
            preview_xml_save(&settings, &app_data_dir, "proj1", "file.xml", proposed).unwrap();

        assert!(
            !preview.changed,
            "source duplicate warning should not block save: {:?}",
            preview
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&source_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    // --- validation token tests ---

    const SEMANTICALLY_INVALID_XML: &str =
        "<Defs><ThingDef><defName>Rock</defName><recipes>MakeThing</recipes></ThingDef></Defs>";

    const TEST_INDEX_FP: &str = "test-index-fingerprint";

    fn make_fast_token(
        secret: &[u8],
        project_dir: &Path,
        project_id: &str,
        relative_path: &str,
        proposed_xml: &str,
        index_fp: &str,
    ) -> String {
        let canonical = project_dir.join(relative_path);
        let current = fs::read_to_string(&canonical).unwrap();
        let current_hash = hash_xml(&current);
        let proposed_hash = hash_xml(proposed_xml);
        compute_validation_token(
            secret,
            project_id,
            relative_path,
            &current_hash,
            &proposed_hash,
            index_fp,
        )
    }

    #[test]
    fn valid_token_skips_semantic_validation() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let secret = b"test-secret";
        let token = make_fast_token(
            secret,
            &project_dir,
            "proj1",
            "file.xml",
            SEMANTICALLY_INVALID_XML,
            TEST_INDEX_FP,
        );
        let result = try_save_with_fast_token(
            &settings,
            &app_data_dir,
            "proj1",
            "file.xml",
            SEMANTICALLY_INVALID_XML,
            &token,
            secret,
            TEST_INDEX_FP,
        );
        assert!(
            matches!(result, Ok(Some(_))),
            "valid token should write the file without semantic validation: {:?}",
            result
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn wrong_token_returns_none_for_fallback() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let result = try_save_with_fast_token(
            &settings,
            &app_data_dir,
            "proj1",
            "file.xml",
            SEMANTICALLY_INVALID_XML,
            "wrong-token",
            b"test-secret",
            TEST_INDEX_FP,
        )
        .unwrap();
        assert!(
            result.is_none(),
            "wrong token should return None so caller falls back to full validation"
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn mismatched_index_fp_returns_none_for_fallback() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let secret = b"test-secret";
        // Token was issued with one index fingerprintÃ¢â‚¬Â¦
        let token = make_fast_token(
            secret,
            &project_dir,
            "proj1",
            "file.xml",
            VALID_XML_2,
            "fp-at-preview",
        );
        // Ã¢â‚¬Â¦but at save time the index changed (different fingerprint)
        let result = try_save_with_fast_token(
            &settings,
            &app_data_dir,
            "proj1",
            "file.xml",
            VALID_XML_2,
            &token,
            secret,
            "fp-after-other-file-changed",
        )
        .unwrap();
        assert!(
            result.is_none(),
            "index_fp mismatch should return None so caller re-validates against current index"
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn stale_token_from_changed_target_file_returns_none() {
        let project_dir = temp_dir();
        let app_data_dir = temp_dir();
        let file_path = project_dir.join("file.xml");
        fs::write(&file_path, VALID_XML).unwrap();

        let settings = make_settings(&project_dir);
        let secret = b"test-secret";
        // Token computed against original file content
        let token = make_fast_token(
            secret,
            &project_dir,
            "proj1",
            "file.xml",
            SEMANTICALLY_INVALID_XML,
            TEST_INDEX_FP,
        );

        // Target file is modified externally before save completes
        fs::write(&file_path, VALID_XML_2).unwrap();

        let result = try_save_with_fast_token(
            &settings,
            &app_data_dir,
            "proj1",
            "file.xml",
            SEMANTICALLY_INVALID_XML,
            &token,
            secret,
            TEST_INDEX_FP,
        )
        .unwrap();
        assert!(
            result.is_none(),
            "stale token (target file changed on disk) should return None so caller re-validates: {:?}",
            result
        );

        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&app_data_dir).ok();
    }
}
