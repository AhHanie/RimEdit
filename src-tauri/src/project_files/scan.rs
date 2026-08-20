use super::{
    paths::{
        canonicalize_location_root, folder_path_of, infer_file_kind, is_xml_extension,
        relative_path_to_forward_slash,
    },
    LocationXmlFileScan, ProjectFileEntry, ProjectFileError, ProjectFileKind, ProjectFileScan,
    ProjectFolderEntry,
};
use crate::diagnostics::DiagnosticArgs;
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation};
use crate::rimworld_load_folders::{resolve_load_folders, LoadFolderDiagnostic};
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;

/// Renders a `walkdir::Error` without leaking the absolute path it embeds in its own `Display`
/// impl (e.g. `"IO error for operation on <absolute path>: <os message>"`) -- callers that
/// surface this text over the wire (e.g. `DefIndexError.message` via `get_def_index_errors`)
/// must never expose the location root or any absolute filesystem path.
/// Falls back to a generic, still-path-free message when the underlying `io::Error` isn't
/// available (e.g. a symlink loop, which `follow_links(false)` should make unreachable here).
fn safe_walk_error_message(e: &walkdir::Error) -> String {
    match e.io_error() {
        Some(io_err) => format!("I/O error while scanning for Def XML files: {}", io_err),
        None => "I/O error while scanning for Def XML files.".to_string(),
    }
}

fn build_file_entry(
    path: &Path,
    root: &Path,
    file_name: String,
    file_kind: ProjectFileKind,
) -> Result<ProjectFileEntry, ProjectFileError> {
    let rel = path
        .strip_prefix(root)
        .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot relativize path: {}", e)))?;
    let normalized = relative_path_to_forward_slash(rel);
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    let size_bytes = path
        .metadata()
        .map_err(|e| {
            ProjectFileError::ScanFailed(format!("Cannot read metadata for {}: {}", normalized, e))
        })?
        .len();
    let folder_path = folder_path_of(&normalized);

    Ok(ProjectFileEntry {
        relative_path: normalized,
        folder_path,
        file_name,
        extension,
        size_bytes,
        file_kind,
        active_for_game_version: None,
    })
}

fn scan_xml_files_in_root(root: &Path) -> Result<Vec<ProjectFileEntry>, ProjectFileError> {
    let mut files: Vec<ProjectFileEntry> = Vec::new();
    for result in WalkDir::new(root).follow_links(false) {
        let entry = result.map_err(|e| ProjectFileError::ScanFailed(e.to_string()))?;
        if !entry.file_type().is_file() || !is_xml_extension(entry.path()) {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        files.push(build_file_entry(
            entry.path(),
            root,
            file_name,
            ProjectFileKind::Xml,
        )?);
    }
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

pub(crate) fn scan_location_xml_files(
    location: &RegisteredLocation,
) -> Result<super::LocationXmlFileScan, ProjectFileError> {
    let root = canonicalize_location_root(location)?;
    let files = scan_xml_files_in_root(&root)?;

    Ok(super::LocationXmlFileScan {
        location_id: location.id.clone(),
        location_name: location.display_name.clone(),
        root_path: root.to_string_lossy().to_string(),
        source_kind: location.kind.clone(),
        source_type: location.source_type.clone(),
        read_only: location.read_only,
        mod_id: location.mod_id.clone(),
        files,
        diagnostics: Vec::new(),
    })
}

pub fn scan_xml_files(
    settings: &ProjectSettings,
    project_id: &str,
) -> Result<ProjectFileScan, ProjectFileError> {
    let location = settings
        .locations
        .iter()
        .find(|l| l.id == project_id)
        .ok_or_else(|| ProjectFileError::ProjectNotFound(project_id.to_string()))?;

    if location.kind != LocationKind::Project || location.read_only {
        return Err(ProjectFileError::ProjectNotEditable(project_id.to_string()));
    }

    let scan = scan_location_xml_files(location)?;

    Ok(ProjectFileScan {
        project_id: project_id.to_string(),
        project_root: scan.root_path,
        folders: vec![],
        files: scan.files,
    })
}

fn scan_project_tree_entries(
    root: &Path,
) -> Result<(Vec<ProjectFolderEntry>, Vec<ProjectFileEntry>), ProjectFileError> {
    let mut folders: Vec<ProjectFolderEntry> = Vec::new();
    let mut files: Vec<ProjectFileEntry> = Vec::new();

    for result in WalkDir::new(root).follow_links(false) {
        let entry = result.map_err(|e| ProjectFileError::ScanFailed(e.to_string()))?;

        if entry.path() == root {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot relativize path: {}", e)))?;
        let normalized = relative_path_to_forward_slash(rel);

        if entry.file_type().is_dir() {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            let parent_path = folder_path_of(&normalized);
            folders.push(ProjectFolderEntry {
                relative_path: normalized,
                folder_name,
                parent_path,
            });
        } else if entry.file_type().is_file() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let extension = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();
            let file_kind = infer_file_kind(&extension);
            files.push(build_file_entry(entry.path(), root, file_name, file_kind)?);
        }
    }

    folders.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok((folders, files))
}

/// Scan only the Def XML files that RimWorld would load for the given location and project
/// game version.
///
/// For each resolved load folder, this scans `<folder>/Defs/**/*.xml`.
/// The first file with a given relative-to-load-folder identity key wins (shadowing).
/// Returned `relative_path` values are relative to the location root.
pub(crate) fn scan_indexable_def_xml_files(
    settings: &ProjectSettings,
    location: &RegisteredLocation,
) -> Result<LocationXmlFileScan, ProjectFileError> {
    let resolution = resolve_load_folders(location, settings);
    let location_root = &resolution.root_path;

    let mut seen_keys: HashSet<(String, String)> = HashSet::new();
    let should_shadow = resolution.shadow_by_relative_path;
    let mut files: Vec<ProjectFileEntry> = Vec::new();
    let mut walk_diagnostics: Vec<LoadFolderDiagnostic> = Vec::new();

    for folder in &resolution.selected_folders {
        let defs_dir = folder.absolute_path.join("Defs");
        if !defs_dir.exists() || !defs_dir.is_dir() {
            continue;
        }

        for result in WalkDir::new(&defs_dir).follow_links(false) {
            let entry = match result {
                Ok(entry) => entry,
                Err(e) => {
                    // A single unreadable/unwalkable entry (permission denied, a too-long path,
                    // ...) must not abort the entire location's scan -- for a Steam Workshop
                    // collection that would silently index zero Defs from every item, not just
                    // the offending one. Record it as a non-fatal, isolated diagnostic instead
                    // and keep walking.
                    let relative_path = e
                        .path()
                        .and_then(|p| p.strip_prefix(location_root).ok())
                        .map(relative_path_to_forward_slash);
                    walk_diagnostics.push(LoadFolderDiagnostic {
                        code: "def_index_file_walk_failed".to_string(),
                        message: safe_walk_error_message(&e),
                        scope: folder.scope.clone(),
                        relative_path,
                        args: DiagnosticArgs::new(),
                    });
                    continue;
                }
            };
            if !entry.file_type().is_file() || !is_xml_extension(entry.path()) {
                continue;
            }

            if should_shadow {
                // Identity key: the folder's content-pack scope (empty for an ordinary source,
                // distinct per Workshop item for a SteamWorkshop collection -- see
                // `ResolvedLoadFolder::scope`) plus the path relative to the load folder (e.g.
                // `Defs/Things/Foo.xml`). Two different content packs never shadow each other.
                let identity_path = entry
                    .path()
                    .strip_prefix(&folder.absolute_path)
                    .map(relative_path_to_forward_slash)
                    .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());

                if !seen_keys.insert((folder.scope.clone(), identity_path)) {
                    // Shadowed by an earlier (higher-precedence) load folder in the same content pack.
                    continue;
                }
            }

            // Relative path from the location root (used for file opening and indexing).
            let rel_from_root = entry
                .path()
                .strip_prefix(location_root)
                .map(relative_path_to_forward_slash)
                .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());

            let file_name = entry.file_name().to_string_lossy().to_string();
            let folder_path = folder_path_of(&rel_from_root);
            let size_bytes = entry.path().metadata().map(|m| m.len()).unwrap_or(0);

            files.push(ProjectFileEntry {
                relative_path: rel_from_root,
                folder_path,
                file_name,
                extension: "xml".to_string(),
                size_bytes,
                file_kind: ProjectFileKind::Xml,
                active_for_game_version: None,
            });
        }
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut diagnostics = resolution.diagnostics.clone();
    diagnostics.extend(walk_diagnostics);

    Ok(LocationXmlFileScan {
        location_id: location.id.clone(),
        location_name: location.display_name.clone(),
        root_path: location_root.to_string_lossy().to_string(),
        source_kind: location.kind.clone(),
        source_type: location.source_type.clone(),
        read_only: location.read_only,
        mod_id: location.mod_id.clone(),
        files,
        diagnostics,
    })
}

pub fn scan_all_project_files(
    settings: &ProjectSettings,
    project_id: &str,
) -> Result<ProjectFileScan, ProjectFileError> {
    let location = settings
        .locations
        .iter()
        .find(|l| l.id == project_id)
        .ok_or_else(|| ProjectFileError::ProjectNotFound(project_id.to_string()))?;

    if location.kind != LocationKind::Project || location.read_only {
        return Err(ProjectFileError::ProjectNotEditable(project_id.to_string()));
    }

    let root = std::path::Path::new(&location.root_path)
        .canonicalize()
        .map_err(|e| {
            ProjectFileError::ScanFailed(format!("Cannot canonicalize project root: {}", e))
        })?;

    let (folders, mut files) = scan_project_tree_entries(&root)?;

    // Determine which XML files are active for the current game version.
    // A file is active if RimWorld would load it either as a Def or as a Patch.
    let active_paths: HashSet<String> = scan_indexable_def_xml_files(settings, location)
        .into_iter()
        .chain(crate::patches::scan_indexable_patch_xml_files(
            settings, location,
        ))
        .flat_map(|scan| scan.files.into_iter().map(|f| f.relative_path))
        .collect();

    for file in &mut files {
        if matches!(file.file_kind, ProjectFileKind::Xml) {
            file.active_for_game_version = Some(active_paths.contains(&file.relative_path));
        }
    }

    Ok(ProjectFileScan {
        project_id: project_id.to_string(),
        project_root: root.to_string_lossy().to_string(),
        folders,
        files,
    })
}
