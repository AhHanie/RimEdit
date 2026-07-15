use super::error::FormViewStoreError;
use super::model::{
    CustomFormView, CustomFormViewUpdate, FormViewStoreWarning, FormViewTarget,
    LastSelectedFormView, NewCustomFormView, SelectedFormViewRef, UserFormViewStore,
    CURRENT_SCHEMA_VERSION,
};
use crate::project_model::AppError;
use crate::services::app_paths;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use time::OffsetDateTime;

/// Root directory for all project-scoped Form View stores, i.e. `{app storage}/form-views`.
fn form_views_root(app: &AppHandle) -> Result<PathBuf, AppError> {
    app_paths::app_storage_dir(app, "form_view_path_failed").map(|d| d.join("form-views"))
}

fn store_file_path(form_views_root: &Path, project_id: &str) -> PathBuf {
    form_views_root
        .join("projects")
        .join(project_id)
        .join("form-views.json")
}

/// Upgrades a parsed-but-not-yet-typed raw store value to the current `UserFormViewStore` shape,
/// dispatching on the store's own recorded `schemaVersion`. Plan.md section 12: "migrate store
/// versions before deserialize."
///
/// `CURRENT_SCHEMA_VERSION` is 1 -- the first version this store format has ever had -- so there
/// is no real prior format to migrate *from* yet, and the only arm below is the identity mapping
/// for version 1 itself (parse `raw` directly as `UserFormViewStore`). This function exists so a
/// real future migration has an obvious, already-wired-up place to land: a new version would gain
/// its own match arm here that reshapes `raw` (still an untyped `serde_json::Value`) into the
/// current shape before returning it, exactly like the existing arm does trivially for version 1.
/// Callers must route any `schema_version > CURRENT_SCHEMA_VERSION` to
/// `ReadOutcome::UnsupportedNewerVersion` before reaching this function -- migrating *forward*
/// from a version this build doesn't know about is never possible, only migrating older data
/// *up to* the current version is.
fn migrate_to_current(
    raw: serde_json::Value,
    version: u32,
) -> Result<UserFormViewStore, FormViewStoreError> {
    match version {
        CURRENT_SCHEMA_VERSION => serde_json::from_value(raw)
            .map_err(|e| FormViewStoreError::ReadFailed(format!("JSON parse error: {}", e))),
        // No schema version older than the current one has ever shipped yet. When one does,
        // add an explicit arm above this fallback (e.g. `0 => { ...transform `raw` into the v1
        // shape, then deserialize... }`) rather than folding it into this catch-all, which exists
        // only to fail safely (not silently accept unrecognized data) until a real migration is
        // written.
        other => Err(FormViewStoreError::ReadFailed(format!(
            "store schema version {} has no known migration path to version {}",
            other, CURRENT_SCHEMA_VERSION
        ))),
    }
}

enum ReadOutcome {
    Ready(UserFormViewStore),
    UnsupportedNewerVersion(u32),
}

fn read_store(path: &Path, project_id: &str) -> Result<ReadOutcome, FormViewStoreError> {
    if !path.exists() {
        return Ok(ReadOutcome::Ready(UserFormViewStore::empty(project_id)));
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| FormViewStoreError::ReadFailed(format!("{}: {}", path.display(), e)))?;

    let raw_value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        FormViewStoreError::ReadFailed(format!("JSON parse error in {}: {}", path.display(), e))
    })?;
    let schema_version = raw_value
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            FormViewStoreError::ReadFailed(format!(
                "missing or non-numeric schemaVersion in {}",
                path.display()
            ))
        })? as u32;
    if schema_version > CURRENT_SCHEMA_VERSION {
        return Ok(ReadOutcome::UnsupportedNewerVersion(schema_version));
    }

    let store = migrate_to_current(raw_value, schema_version)?;

    // Defense against a moved/copied store file: the directory layout already encodes the
    // project id (`projects/<projectId>/form-views.json`), so a mismatch means the file's
    // *content* disagrees with where it lives on disk. Refuse rather than silently adopting
    // or silently overwriting either id.
    if store.project_id != project_id {
        return Err(FormViewStoreError::ProjectIdMismatch(format!(
            "store at {} is recorded for project '{}', but was opened as project '{}'",
            path.display(),
            store.project_id,
            project_id
        )));
    }

    Ok(ReadOutcome::Ready(store))
}

/// Like `read_store`, but for mutation call sites: a newer-version store must never be written
/// to (that would destroy data this build cannot understand), so the soft "read-only" outcome
/// becomes a hard error here instead of an empty store + warning.
fn read_store_for_mutation(
    path: &Path,
    project_id: &str,
) -> Result<UserFormViewStore, FormViewStoreError> {
    match read_store(path, project_id)? {
        ReadOutcome::Ready(store) => Ok(store),
        ReadOutcome::UnsupportedNewerVersion(v) => {
            Err(FormViewStoreError::UnsupportedNewerVersion(v))
        }
    }
}

fn write_store(path: &Path, store: &UserFormViewStore) -> Result<(), FormViewStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| FormViewStoreError::WriteFailed(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| FormViewStoreError::WriteFailed(e.to_string()))?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)
        .map_err(|e| FormViewStoreError::WriteFailed(format!("{}: {}", tmp_path.display(), e)))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|e| FormViewStoreError::WriteFailed(format!("rename failed: {}", e)))?;
    Ok(())
}

/// Atomically claims a race-proof sibling backup path for `path`, distinguished by `timestamp`
/// (nanosecond-resolution Unix time, per `OffsetDateTime::unix_timestamp_nanos`).
///
/// An earlier version of this function picked a candidate path by checking `Path::exists()` and
/// then acting on it -- a classic TOCTOU (time-of-check-to-time-of-use) gap: two concurrent
/// resets (or a reset racing some other writer) could both see the same candidate as "free"
/// during their respective checks, then one could clobber the other during the subsequent
/// rename/write, with no OS-level guarantee between the check and the act. Nanosecond timestamps
/// make that window vanishingly unlikely but do not close it, since some platforms/clocks have
/// coarser real resolution than the type suggests.
///
/// This version instead *claims* each candidate with
/// `OpenOptions::new().write(true).create_new(true)`, whose check-and-create happens as one
/// atomic filesystem operation with no gap for a race: `create_new` fails with
/// `ErrorKind::AlreadyExists` if the path already exists (whether that path was created a moment
/// ago, by another thread mid-race, or was pre-existing). Looping through suffixes
/// (`.bak`, `.bak.1`, `.bak.2`, ...) and retrying on `AlreadyExists` is therefore guaranteed to
/// land on a path nothing else has touched, however many callers race for it concurrently.
///
/// Returns the open (empty, zero-byte, exclusively-owned) file handle together with its path.
/// The caller must write the actual backup content into that handle rather than `rename`-ing a
/// source file onto it: a second `rename` targeting an already-claimed path could in principle
/// still replace it on some platform/timing combinations, whereas writing into a handle this
/// process already atomically owns (and holds the only reference to) cannot be raced by anything
/// else. See `reset_store_in_at` below for that write-then-remove-original sequencing.
fn claim_backup_path(
    path: &Path,
    timestamp: i128,
) -> Result<(std::fs::File, PathBuf), FormViewStoreError> {
    let file_name = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("form-views.json");

    let mut suffix: u64 = 0;
    loop {
        let candidate = if suffix == 0 {
            path.with_file_name(format!("{}.corrupt-{}.bak", file_name, timestamp))
        } else {
            path.with_file_name(format!(
                "{}.corrupt-{}.bak.{}",
                file_name, timestamp, suffix
            ))
        };

        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((file, candidate)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                suffix += 1;
            }
            Err(e) => {
                return Err(FormViewStoreError::WriteFailed(format!(
                    "failed to claim backup path {}: {}",
                    candidate.display(),
                    e
                )))
            }
        }
    }
}

/// Recovers from a corrupt/unreadable store per Plan.md section 12 and issue 04 step 6:
/// "no overwrite; explicit backup-and-reset command/action." If a store file exists at all
/// (corrupt, project-id-mismatched, or otherwise), its content is copied into a race-proof `.bak`
/// sibling (see `claim_backup_path`) -- never deleted -- so a user can still recover data by hand
/// later, and only then is the original removed and a fresh empty v1 store written in its place.
/// Returns the backup path when one was created (`None` when there was nothing on disk to back
/// up, e.g. resetting an already-missing store is a no-op creation of an empty store).
fn reset_store_in(root: &Path, project_id: &str) -> Result<Option<PathBuf>, FormViewStoreError> {
    reset_store_in_at(
        root,
        project_id,
        OffsetDateTime::now_utc().unix_timestamp_nanos(),
    )
}

/// The actual reset logic behind `reset_store_in`, parameterized on the backup timestamp so
/// tests can force two resets to target the exact same instant deterministically rather than
/// hoping two real clock reads happen to coincide.
fn reset_store_in_at(
    root: &Path,
    project_id: &str,
    timestamp: i128,
) -> Result<Option<PathBuf>, FormViewStoreError> {
    let path = store_file_path(root, project_id);

    let backup_path = if path.exists() {
        // Read the original bytes before claiming a backup path: if another concurrent reset
        // races to remove `path` first (see below), this call still has its own copy of the
        // content it is responsible for preserving.
        let original_bytes = std::fs::read(&path).map_err(|e| {
            FormViewStoreError::ReadFailed(format!(
                "failed to read existing store at {} before backing it up: {}",
                path.display(),
                e
            ))
        })?;

        let (mut backup_file, backup_path) = claim_backup_path(&path, timestamp)?;
        backup_file.write_all(&original_bytes).map_err(|e| {
            FormViewStoreError::WriteFailed(format!(
                "failed to write backup content to {}: {}",
                backup_path.display(),
                e
            ))
        })?;
        backup_file.flush().map_err(|e| {
            FormViewStoreError::WriteFailed(format!(
                "failed to flush backup content to {}: {}",
                backup_path.display(),
                e
            ))
        })?;
        drop(backup_file);

        // Only remove the original after its content is safely durable in the claimed backup.
        // A concurrent reset could have already removed it (each racing caller independently
        // reads its own copy of the original bytes above and claims its own distinct backup
        // path, so this is harmless duplication, never data loss) -- ignore NotFound, but
        // surface any other removal failure.
        if let Err(e) = std::fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(FormViewStoreError::WriteFailed(format!(
                    "failed to remove original store at {} after backing it up to {}: {}",
                    path.display(),
                    backup_path.display(),
                    e
                )));
            }
        }

        Some(backup_path)
    } else {
        None
    };

    write_store(&path, &UserFormViewStore::empty(project_id))?;
    Ok(backup_path)
}

fn trimmed_name_conflicts(
    views: &[CustomFormView],
    target: &FormViewTarget,
    trimmed_name: &str,
    exclude_id: Option<&str>,
) -> bool {
    views.iter().any(|v| {
        exclude_id != Some(v.id.as_str()) && v.target == *target && v.name.trim() == trimmed_name
    })
}

fn list_views_in(
    root: &Path,
    project_id: &str,
    game_version: Option<&str>,
    def_type: Option<&str>,
) -> Result<(Vec<CustomFormView>, Option<FormViewStoreWarning>), FormViewStoreError> {
    let path = store_file_path(root, project_id);
    match read_store(&path, project_id)? {
        ReadOutcome::Ready(store) => {
            let views = store
                .custom_views
                .into_iter()
                .filter(|v| game_version.is_none_or(|gv| v.target.game_version == gv))
                .filter(|v| def_type.is_none_or(|dt| v.target.def_type == dt))
                .collect();
            Ok((views, None))
        }
        ReadOutcome::UnsupportedNewerVersion(v) => Ok((
            Vec::new(),
            Some(FormViewStoreWarning::unsupported_newer_version(v)),
        )),
    }
}

fn create_view_in(
    root: &Path,
    project_id: &str,
    new_view: NewCustomFormView,
) -> Result<CustomFormView, FormViewStoreError> {
    let path = store_file_path(root, project_id);
    let mut store = read_store_for_mutation(&path, project_id)?;

    let trimmed_name = new_view.name.trim();
    if trimmed_name.is_empty() {
        return Err(FormViewStoreError::BlankName);
    }
    if trimmed_name_conflicts(&store.custom_views, &new_view.target, trimmed_name, None) {
        return Err(FormViewStoreError::DuplicateName(trimmed_name.to_string()));
    }

    let now = OffsetDateTime::now_utc();
    let view = CustomFormView {
        id: uuid::Uuid::new_v4().to_string(),
        target: new_view.target,
        name: trimmed_name.to_string(),
        description: new_view.description,
        hidden_field_ids: new_view.hidden_field_ids,
        base_schema_view: new_view.base_schema_view,
        created_at: now,
        updated_at: now,
    };

    store.schema_version = CURRENT_SCHEMA_VERSION;
    store.project_id = project_id.to_string();
    store.custom_views.push(view.clone());
    write_store(&path, &store)?;
    Ok(view)
}

fn update_view_in(
    root: &Path,
    project_id: &str,
    view_id: &str,
    update: CustomFormViewUpdate,
) -> Result<CustomFormView, FormViewStoreError> {
    let path = store_file_path(root, project_id);
    let mut store = read_store_for_mutation(&path, project_id)?;

    let index = store
        .custom_views
        .iter()
        .position(|v| v.id == view_id)
        .ok_or_else(|| FormViewStoreError::ViewNotFound(view_id.to_string()))?;

    let trimmed_new_name = match &update.name {
        Some(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err(FormViewStoreError::BlankName);
            }
            Some(trimmed.to_string())
        }
        None => None,
    };

    if let Some(trimmed) = &trimmed_new_name {
        let target = store.custom_views[index].target.clone();
        if trimmed_name_conflicts(&store.custom_views, &target, trimmed, Some(view_id)) {
            return Err(FormViewStoreError::DuplicateName(trimmed.clone()));
        }
    }

    {
        let view = &mut store.custom_views[index];
        if let Some(trimmed) = trimmed_new_name {
            view.name = trimmed;
        }
        if let Some(hidden_field_ids) = update.hidden_field_ids {
            view.hidden_field_ids = hidden_field_ids;
        }
        if let Some(description) = update.description {
            view.description = description;
        }
        view.updated_at = OffsetDateTime::now_utc();
    }
    let updated = store.custom_views[index].clone();

    write_store(&path, &store)?;
    Ok(updated)
}

fn delete_view_in(root: &Path, project_id: &str, view_id: &str) -> Result<(), FormViewStoreError> {
    let path = store_file_path(root, project_id);
    let mut store = read_store_for_mutation(&path, project_id)?;

    let original_len = store.custom_views.len();
    store.custom_views.retain(|v| v.id != view_id);
    if store.custom_views.len() == original_len {
        return Err(FormViewStoreError::ViewNotFound(view_id.to_string()));
    }

    write_store(&path, &store)?;
    Ok(())
}

fn set_last_selected_in(
    root: &Path,
    project_id: &str,
    entry: LastSelectedFormView,
) -> Result<(), FormViewStoreError> {
    let path = store_file_path(root, project_id);
    let mut store = read_store_for_mutation(&path, project_id)?;

    store
        .preferences
        .last_selected
        .retain(|e| !(e.game_version == entry.game_version && e.def_type == entry.def_type));
    store.preferences.last_selected.push(entry);

    store.schema_version = CURRENT_SCHEMA_VERSION;
    store.project_id = project_id.to_string();
    write_store(&path, &store)?;
    Ok(())
}

fn get_last_selected_in(
    root: &Path,
    project_id: &str,
    game_version: &str,
    def_type: &str,
) -> Result<(Option<SelectedFormViewRef>, Option<FormViewStoreWarning>), FormViewStoreError> {
    let path = store_file_path(root, project_id);
    match read_store(&path, project_id)? {
        ReadOutcome::Ready(store) => {
            let found = store
                .preferences
                .last_selected
                .into_iter()
                .find(|e| e.game_version == game_version && e.def_type == def_type)
                .map(|e| e.view);
            Ok((found, None))
        }
        ReadOutcome::UnsupportedNewerVersion(v) => Ok((
            None,
            Some(FormViewStoreWarning::unsupported_newer_version(v)),
        )),
    }
}

pub fn list_custom_views(
    app: &AppHandle,
    project_id: &str,
    game_version: Option<&str>,
    def_type: Option<&str>,
) -> Result<(Vec<CustomFormView>, Option<FormViewStoreWarning>), AppError> {
    let root = form_views_root(app)?;
    list_views_in(&root, project_id, game_version, def_type).map_err(Into::into)
}

pub fn create_view(
    app: &AppHandle,
    project_id: &str,
    new_view: NewCustomFormView,
) -> Result<CustomFormView, AppError> {
    let root = form_views_root(app)?;
    create_view_in(&root, project_id, new_view).map_err(Into::into)
}

pub fn update_view(
    app: &AppHandle,
    project_id: &str,
    view_id: &str,
    update: CustomFormViewUpdate,
) -> Result<CustomFormView, AppError> {
    let root = form_views_root(app)?;
    update_view_in(&root, project_id, view_id, update).map_err(Into::into)
}

pub fn delete_view(app: &AppHandle, project_id: &str, view_id: &str) -> Result<(), AppError> {
    let root = form_views_root(app)?;
    delete_view_in(&root, project_id, view_id).map_err(Into::into)
}

/// Backs up (never deletes) any existing store file and writes a fresh empty v1 store in its
/// place. Returns the backup path, if a file existed to back up. See `reset_store_in` above.
pub fn reset_store(app: &AppHandle, project_id: &str) -> Result<Option<PathBuf>, AppError> {
    let root = form_views_root(app)?;
    reset_store_in(&root, project_id).map_err(Into::into)
}

pub fn set_last_selected(
    app: &AppHandle,
    project_id: &str,
    entry: LastSelectedFormView,
) -> Result<(), AppError> {
    let root = form_views_root(app)?;
    set_last_selected_in(&root, project_id, entry).map_err(Into::into)
}

pub fn get_last_selected(
    app: &AppHandle,
    project_id: &str,
    game_version: &str,
    def_type: &str,
) -> Result<(Option<SelectedFormViewRef>, Option<FormViewStoreWarning>), AppError> {
    let root = form_views_root(app)?;
    get_last_selected_in(&root, project_id, game_version, def_type).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form_views::model::FormViewOrigin;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rimedit_form_views_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_new_view(game_version: &str, def_type: &str, name: &str) -> NewCustomFormView {
        NewCustomFormView {
            target: FormViewTarget {
                game_version: game_version.to_string(),
                def_type: def_type.to_string(),
            },
            name: name.to_string(),
            description: None,
            hidden_field_ids: vec!["apparel".to_string(), "plant".to_string()],
            base_schema_view: None,
        }
    }

    #[test]
    fn listing_for_a_missing_store_returns_empty() {
        let root = temp_dir();
        let (views, warning) = list_views_in(&root, "proj1", None, None).unwrap();
        assert!(views.is_empty());
        assert!(warning.is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_then_list_round_trips() {
        let root = temp_dir();
        let created =
            create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();

        let (listed, _) = list_views_in(&root, "proj1", None, None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].name, "Weapon");
        assert_eq!(listed[0].hidden_field_ids, vec!["apparel", "plant"]);
        assert_eq!(listed[0].target.game_version, "1.6");
        assert_eq!(listed[0].target.def_type, "ThingDef");

        // The store file itself is pretty-printed JSON under the expected path, matching the
        // Plan.md section 6 shape (schemaVersion/projectId/customViews/preferences).
        let path = store_file_path(&root, "proj1");
        assert!(path.exists());
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\n"), "expected pretty-printed JSON");
        assert!(raw.contains("\"schemaVersion\""));
        assert!(raw.contains("\"customViews\""));
        assert!(raw.contains("\"preferences\""));
        assert!(raw.contains("\"hiddenFieldIds\""));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn listing_filters_by_game_version_and_def_type() {
        let root = temp_dir();
        create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();
        create_view_in(
            &root,
            "proj1",
            sample_new_view("1.5", "ThingDef", "Old weapon"),
        )
        .unwrap();
        create_view_in(
            &root,
            "proj1",
            sample_new_view("1.6", "PawnKindDef", "Pawn"),
        )
        .unwrap();

        let (thing_16, _) = list_views_in(&root, "proj1", Some("1.6"), Some("ThingDef")).unwrap();
        assert_eq!(thing_16.len(), 1);
        assert_eq!(thing_16[0].name, "Weapon");

        let (all_thing, _) = list_views_in(&root, "proj1", None, Some("ThingDef")).unwrap();
        assert_eq!(all_thing.len(), 2);

        let (all_16, _) = list_views_in(&root, "proj1", Some("1.6"), None).unwrap();
        assert_eq!(all_16.len(), 2);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn views_are_scoped_by_project() {
        let root = temp_dir();
        create_view_in(
            &root,
            "proj1",
            sample_new_view("1.6", "ThingDef", "Proj1 view"),
        )
        .unwrap();
        create_view_in(
            &root,
            "proj2",
            sample_new_view("1.6", "ThingDef", "Proj2 view"),
        )
        .unwrap();

        let (proj1, _) = list_views_in(&root, "proj1", None, None).unwrap();
        let (proj2, _) = list_views_in(&root, "proj2", None, None).unwrap();
        assert_eq!(proj1.len(), 1);
        assert_eq!(proj2.len(), 1);
        assert_eq!(proj1[0].name, "Proj1 view");
        assert_eq!(proj2[0].name, "Proj2 view");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn update_renames_and_bumps_updated_at() {
        let root = temp_dir();
        let created =
            create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();

        // Ensure a real timestamp delta is observable irrespective of clock resolution.
        std::thread::sleep(std::time::Duration::from_millis(5));

        let updated = update_view_in(
            &root,
            "proj1",
            &created.id,
            CustomFormViewUpdate {
                name: Some("Ranged weapon".to_string()),
                hidden_field_ids: None,
                description: None,
            },
        )
        .unwrap();

        assert_eq!(updated.name, "Ranged weapon");
        assert_eq!(updated.hidden_field_ids, vec!["apparel", "plant"]);
        assert!(updated.updated_at > created.updated_at);
        assert_eq!(updated.created_at, created.created_at);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn update_replaces_hidden_field_ids() {
        let root = temp_dir();
        let created =
            create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();

        let updated = update_view_in(
            &root,
            "proj1",
            &created.id,
            CustomFormViewUpdate {
                name: None,
                hidden_field_ids: Some(vec!["race".to_string()]),
                description: None,
            },
        )
        .unwrap();

        assert_eq!(updated.name, "Weapon");
        assert_eq!(updated.hidden_field_ids, vec!["race"]);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn update_unknown_view_id_errors() {
        let root = temp_dir();
        create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();

        let err = update_view_in(
            &root,
            "proj1",
            "does-not-exist",
            CustomFormViewUpdate {
                name: Some("New name".to_string()),
                hidden_field_ids: None,
                description: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, FormViewStoreError::ViewNotFound(_)));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_removes_only_the_matching_view_in_the_matching_project() {
        let root = temp_dir();
        let keep = create_view_in(
            &root,
            "proj1",
            sample_new_view("1.6", "ThingDef", "Keep me"),
        )
        .unwrap();
        let doomed = create_view_in(
            &root,
            "proj1",
            sample_new_view("1.6", "ThingDef", "Delete me"),
        )
        .unwrap();
        let other_project = create_view_in(
            &root,
            "proj2",
            sample_new_view("1.6", "ThingDef", "Untouched"),
        )
        .unwrap();

        delete_view_in(&root, "proj1", &doomed.id).unwrap();

        let (remaining, _) = list_views_in(&root, "proj1", None, None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, keep.id);

        let (other, _) = list_views_in(&root, "proj2", None, None).unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].id, other_project.id);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_unknown_view_id_errors() {
        let root = temp_dir();
        create_view_in(
            &root,
            "proj1",
            sample_new_view("1.6", "ThingDef", "Keep me"),
        )
        .unwrap();

        let err = delete_view_in(&root, "proj1", "does-not-exist").unwrap_err();
        assert!(matches!(err, FormViewStoreError::ViewNotFound(_)));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_rejects_blank_name() {
        let root = temp_dir();
        let err =
            create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "   ")).unwrap_err();
        assert!(matches!(err, FormViewStoreError::BlankName));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_rejects_duplicate_trimmed_name_within_the_same_scope() {
        let root = temp_dir();
        create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();

        let err = create_view_in(
            &root,
            "proj1",
            sample_new_view("1.6", "ThingDef", "  Weapon  "),
        )
        .unwrap_err();
        assert!(matches!(err, FormViewStoreError::DuplicateName(_)));

        // A different def type or game version is a different scope, so the same name is fine.
        assert!(create_view_in(
            &root,
            "proj1",
            sample_new_view("1.6", "PawnKindDef", "Weapon")
        )
        .is_ok());
        assert!(
            create_view_in(&root, "proj1", sample_new_view("1.5", "ThingDef", "Weapon")).is_ok()
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn update_rejects_renaming_into_a_duplicate_within_the_same_scope() {
        let root = temp_dir();
        create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();
        let second =
            create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Armor")).unwrap();

        let err = update_view_in(
            &root,
            "proj1",
            &second.id,
            CustomFormViewUpdate {
                name: Some("Weapon".to_string()),
                hidden_field_ids: None,
                description: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, FormViewStoreError::DuplicateName(_)));

        // Renaming a view to its own current name is not a conflict with itself.
        assert!(update_view_in(
            &root,
            "proj1",
            &second.id,
            CustomFormViewUpdate {
                name: Some("Armor".to_string()),
                hidden_field_ids: None,
                description: None,
            },
        )
        .is_ok());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn malformed_json_returns_a_read_error_and_never_overwrites() {
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not valid json").unwrap();

        let err = list_views_in(&root, "proj1", None, None).unwrap_err();
        assert!(matches!(err, FormViewStoreError::ReadFailed(_)));

        let err = create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon"))
            .unwrap_err();
        assert!(matches!(err, FormViewStoreError::ReadFailed(_)));

        // The corrupt file itself is left completely untouched.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw, "{ not valid json");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn project_id_mismatch_is_rejected_and_never_overwritten() {
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let foreign_store = UserFormViewStore::empty("some-other-project");
        std::fs::write(&path, serde_json::to_string_pretty(&foreign_store).unwrap()).unwrap();

        let err = list_views_in(&root, "proj1", None, None).unwrap_err();
        assert!(matches!(err, FormViewStoreError::ProjectIdMismatch(_)));

        let raw_before = std::fs::read_to_string(&path).unwrap();
        let err = create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon"))
            .unwrap_err();
        assert!(matches!(err, FormViewStoreError::ProjectIdMismatch(_)));
        let raw_after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            raw_before, raw_after,
            "mismatched store must not be rewritten"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_newer_unsupported_schema_version_opens_read_only_without_overwriting() {
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let newer_json = r#"{
            "schemaVersion": 999,
            "projectId": "proj1",
            "customViews": [{"someFutureShapeField": "unreadable-by-this-build"}],
            "preferences": {}
        }"#;
        std::fs::write(&path, newer_json).unwrap();

        // Listing degrades gracefully: empty result, with a warning, no error.
        let (views, warning) = list_views_in(&root, "proj1", None, None).unwrap();
        assert!(views.is_empty());
        let warning = warning.expect("expected a newer-version warning");
        // Same stable code as `FormViewStoreError::UnsupportedNewerVersion`'s `AppError`
        // conversion below (`form_view_unsupported_version`), and present in the frontend's
        // diagnostic catalog, so `renderDiagnostic` actually translates it instead of silently
        // falling back to the raw `message` text.
        assert_eq!(warning.code, "form_view_unsupported_version");
        assert_eq!(
            warning.args.get("schemaVersion"),
            Some(&crate::diagnostics::DiagnosticArgValue::Int(999))
        );

        // Mutations refuse outright rather than risk destroying data this build can't parse.
        let err = create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon"))
            .unwrap_err();
        assert!(matches!(
            err,
            FormViewStoreError::UnsupportedNewerVersion(999)
        ));

        let err = set_last_selected_in(
            &root,
            "proj1",
            LastSelectedFormView {
                game_version: "1.6".to_string(),
                def_type: "ThingDef".to_string(),
                view: SelectedFormViewRef {
                    origin: FormViewOrigin::Default,
                    id: "default".to_string(),
                },
            },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FormViewStoreError::UnsupportedNewerVersion(999)
        ));

        // The unreadable newer file is left completely untouched throughout.
        let raw_after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw_after, newer_json);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn version_1_is_read_through_the_identity_migration_arm() {
        // CURRENT_SCHEMA_VERSION is 1 -- the first version this store format has ever had -- so
        // there is no real prior format to migrate *from* yet. This pins down that
        // `migrate_to_current`'s `CURRENT_SCHEMA_VERSION` arm (a pure passthrough today) is
        // exercised for version 1, not that migration is skipped as a permanent design choice;
        // see `migrate_to_current`'s doc comment for where a real future migration would plug in.
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let v1_json = r#"{
            "schemaVersion": 1,
            "projectId": "proj1",
            "customViews": [],
            "preferences": { "lastSelected": [] }
        }"#;
        std::fs::write(&path, v1_json).unwrap();

        let (views, warning) = list_views_in(&root, "proj1", None, None).unwrap();
        assert!(views.is_empty());
        assert!(warning.is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_schema_version_with_no_known_migration_path_is_rejected_not_silently_accepted() {
        // No version below CURRENT_SCHEMA_VERSION has ever shipped, so `migrate_to_current`'s
        // fallback arm should reject this rather than guess -- proving the dispatch is a real
        // gate, not a no-op that would let arbitrary "older" data through unmigrated.
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let hypothetical_pre_v1_json = r#"{
            "schemaVersion": 0,
            "projectId": "proj1",
            "customViews": [],
            "preferences": { "lastSelected": [] }
        }"#;
        std::fs::write(&path, hypothetical_pre_v1_json).unwrap();

        let err = list_views_in(&root, "proj1", None, None).unwrap_err();
        assert!(matches!(err, FormViewStoreError::ReadFailed(_)));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn unknown_hidden_field_ids_and_missing_base_reference_survive_an_unrelated_update_on_disk() {
        // Plan.md section 6/12: a field ID no longer present in the live schema, or a base
        // schema view reference to a view/pack that no longer resolves, must never be pruned by
        // storage itself -- only a resolution-time (issue 05+) layer may compute compatibility.
        //
        // Hand-seed the raw JSON (rather than round-tripping through `create_view_in`, whose
        // return value alone wouldn't prove the *serializer* never prunes) with references that
        // cannot resolve against any real schema, perform a read-modify-write that changes only
        // an unrelated field (name), and inspect the raw bytes on disk afterward.
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let seed_json = r#"{
            "schemaVersion": 1,
            "projectId": "proj1",
            "customViews": [{
                "id": "view1",
                "target": { "gameVersion": "1.6", "defType": "ThingDef" },
                "name": "Weapon",
                "description": null,
                "hiddenFieldIds": ["apparel", "aFieldRemovedByALaterSchemaRelease"],
                "baseSchemaView": {
                    "viewId": "anUnresolvableViewId",
                    "packId": "rimedit.rimworld.core",
                    "packVersion": "1.6.0",
                    "declaredOnDefType": "ThingDef"
                },
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z"
            }],
            "preferences": { "lastSelected": [] }
        }"#;
        std::fs::write(&path, seed_json).unwrap();

        // A read-modify-write cycle that only renames the view, exactly like a real "rename" UI
        // action would trigger through `update_view_in`.
        let renamed = update_view_in(
            &root,
            "proj1",
            "view1",
            CustomFormViewUpdate {
                name: Some("Ranged weapon".to_string()),
                hidden_field_ids: None,
                description: None,
            },
        )
        .unwrap();
        assert_eq!(renamed.name, "Ranged weapon");

        // Read the raw bytes back off disk -- not the in-memory return value -- so a future
        // serialization-time pruning bug would actually be caught here.
        let raw_after = std::fs::read_to_string(&path).unwrap();
        assert!(raw_after.contains("\"aFieldRemovedByALaterSchemaRelease\""));
        assert!(raw_after.contains("\"anUnresolvableViewId\""));
        assert!(raw_after.contains("\"Ranged weapon\""));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn update_can_set_and_clear_description() {
        let root = temp_dir();
        let created =
            create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();
        assert_eq!(created.description, None);

        let with_description = update_view_in(
            &root,
            "proj1",
            &created.id,
            CustomFormViewUpdate {
                name: None,
                hidden_field_ids: None,
                description: Some(Some("My weapon view".to_string())),
            },
        )
        .unwrap();
        assert_eq!(
            with_description.description.as_deref(),
            Some("My weapon view")
        );

        // `None` (the outer Option) leaves the field untouched -- proven by updating a
        // different field (name) while omitting `description` entirely.
        let untouched = update_view_in(
            &root,
            "proj1",
            &created.id,
            CustomFormViewUpdate {
                name: Some("Ranged weapon".to_string()),
                hidden_field_ids: None,
                description: None,
            },
        )
        .unwrap();
        assert_eq!(untouched.description.as_deref(), Some("My weapon view"));

        // `Some(None)` explicitly clears it back to no description.
        let cleared = update_view_in(
            &root,
            "proj1",
            &created.id,
            CustomFormViewUpdate {
                name: None,
                hidden_field_ids: None,
                description: Some(None),
            },
        )
        .unwrap();
        assert_eq!(cleared.description, None);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reset_backs_up_a_corrupt_store_and_replaces_it_with_a_fresh_empty_one() {
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let corrupt_content = "{ not valid json, definitely not recoverable as-is";
        std::fs::write(&path, corrupt_content).unwrap();

        // Confirm the store is indeed unusable beforehand, matching real corruption recovery.
        let err = list_views_in(&root, "proj1", None, None).unwrap_err();
        assert!(matches!(err, FormViewStoreError::ReadFailed(_)));

        let backup_path = reset_store_in(&root, "proj1")
            .unwrap()
            .expect("a backup path is returned when a file existed to back up");

        // The original corrupt content is preserved verbatim in the backup, not destroyed.
        assert!(backup_path.exists());
        let backed_up = std::fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backed_up, corrupt_content);

        // The store is now a valid, empty v1 store at the original path.
        let (views, warning) = list_views_in(&root, "proj1", None, None).unwrap();
        assert!(views.is_empty());
        assert!(warning.is_none());
        let raw_reset = std::fs::read_to_string(&path).unwrap();
        assert!(raw_reset.contains("\"schemaVersion\": 1"));
        assert!(!raw_reset.contains(corrupt_content));

        // The store is fully usable again afterward.
        create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();
        let (views, _) = list_views_in(&root, "proj1", None, None).unwrap();
        assert_eq!(views.len(), 1);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reset_of_a_missing_store_creates_a_fresh_empty_one_with_no_backup() {
        let root = temp_dir();

        let backup_path = reset_store_in(&root, "proj1").unwrap();
        assert!(backup_path.is_none());

        let (views, warning) = list_views_in(&root, "proj1", None, None).unwrap();
        assert!(views.is_empty());
        assert!(warning.is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reset_of_a_project_id_mismatched_store_backs_it_up_too() {
        // A project-id mismatch is a different flavor of "unusable store" from corrupt JSON, but
        // reset must recover it the same safe way: back up, don't delete, then start fresh.
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let foreign_store = UserFormViewStore::empty("some-other-project");
        let foreign_json = serde_json::to_string_pretty(&foreign_store).unwrap();
        std::fs::write(&path, &foreign_json).unwrap();

        let backup_path = reset_store_in(&root, "proj1").unwrap().unwrap();
        let backed_up = std::fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backed_up, foreign_json);

        let (views, _) = list_views_in(&root, "proj1", None, None).unwrap();
        assert!(views.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claim_backup_path_skips_a_candidate_that_already_exists_at_claim_time() {
        // Proves the TOCTOU fix directly: a file sitting at the first candidate path *before*
        // `claim_backup_path` is even called (simulating another caller having already won that
        // exact candidate an instant earlier) must never be touched -- `create_new` fails with
        // `AlreadyExists` on it rather than this function checking `exists()` first and then
        // acting on a stale answer.
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let timestamp: i128 = 1_700_000_000_000_000_000;
        let file_name = path.file_name().unwrap().to_str().unwrap();
        let first_candidate =
            path.with_file_name(format!("{}.corrupt-{}.bak", file_name, timestamp));
        std::fs::write(&first_candidate, "already claimed by someone else").unwrap();

        let (file, claimed_path) = claim_backup_path(&path, timestamp).unwrap();
        drop(file);

        assert_ne!(
            claimed_path, first_candidate,
            "must not reuse a candidate that already exists"
        );
        assert!(claimed_path.to_string_lossy().ends_with(".bak.1"));
        // The pre-existing file is completely untouched -- not truncated, not renamed over.
        assert_eq!(
            std::fs::read_to_string(&first_candidate).unwrap(),
            "already claimed by someone else"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claim_backup_path_never_reuses_a_path_it_already_claimed_and_wrote_to() {
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let timestamp: i128 = 1_700_000_000_000_000_002;

        let (mut file1, path1) = claim_backup_path(&path, timestamp).unwrap();
        file1.write_all(b"first content").unwrap();
        drop(file1);

        let (mut file2, path2) = claim_backup_path(&path, timestamp).unwrap();
        file2.write_all(b"second content").unwrap();
        drop(file2);

        assert_ne!(path1, path2);
        assert_eq!(std::fs::read_to_string(&path1).unwrap(), "first content");
        assert_eq!(std::fs::read_to_string(&path2).unwrap(), "second content");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claim_backup_path_is_race_safe_under_real_concurrent_access() {
        // A genuine multi-thread stress test (not just sequential/deterministic collision
        // simulation): many real OS threads race to claim a backup path at the exact same
        // timestamp simultaneously (synchronized with a `Barrier` to maximize contention), each
        // writes its own distinct content, and every single claim must land on its own path with
        // its own content intact -- proving `create_new`'s atomicity actually holds under real
        // concurrent access, not merely when called one after another from one thread.
        use std::sync::{Arc, Barrier};
        use std::thread;

        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let shared_path = Arc::new(path);
        let timestamp: i128 = 1_700_000_000_000_000_003;

        const THREAD_COUNT: usize = 16;
        let barrier = Arc::new(Barrier::new(THREAD_COUNT));
        let handles: Vec<_> = (0..THREAD_COUNT)
            .map(|i| {
                let shared_path = Arc::clone(&shared_path);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let (mut file, claimed_path) =
                        claim_backup_path(&shared_path, timestamp).unwrap();
                    let content = format!("content-from-thread-{}", i);
                    file.write_all(content.as_bytes()).unwrap();
                    drop(file);
                    (claimed_path, content)
                })
            })
            .collect();

        let results: Vec<(PathBuf, String)> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();

        let mut claimed_paths: Vec<&PathBuf> = results.iter().map(|(p, _)| p).collect();
        claimed_paths.sort();
        let mut deduped = claimed_paths.clone();
        deduped.dedup();
        assert_eq!(
            deduped.len(),
            THREAD_COUNT,
            "every concurrently racing claim must land on a distinct path"
        );

        for (claimed_path, expected_content) in &results {
            let actual = std::fs::read_to_string(claimed_path).unwrap();
            assert_eq!(
                &actual, expected_content,
                "no concurrent claim may clobber another thread's committed content"
            );
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn resetting_twice_at_the_same_timestamp_preserves_both_backups_distinctly() {
        // The integration-level version of the collision test above: drives the actual
        // `reset_store_in_at` path (not just the path-picking helper) through two resets forced
        // to the same instant, and confirms both backups survive on disk with their own distinct
        // content -- never silently clobbering the first backup, which could itself have held
        // recoverable data.
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "first corrupt content").unwrap();

        let fixed_timestamp: i128 = 1_700_000_000_000_000_001;

        let first_backup = reset_store_in_at(&root, "proj1", fixed_timestamp)
            .unwrap()
            .expect("a backup path is returned when a file existed to back up");

        // Corrupt the freshly-reset store again (simulating recurring corruption) and reset a
        // second time at the exact same timestamp.
        std::fs::write(&path, "second corrupt content").unwrap();
        let second_backup = reset_store_in_at(&root, "proj1", fixed_timestamp)
            .unwrap()
            .expect("a backup path is returned when a file existed to back up");

        assert_ne!(
            first_backup, second_backup,
            "same-timestamp resets must not collide on one backup path"
        );
        assert!(first_backup.exists(), "the first backup must survive");
        assert!(second_backup.exists(), "the second backup must also exist");
        assert_eq!(
            std::fs::read_to_string(&first_backup).unwrap(),
            "first corrupt content"
        );
        assert_eq!(
            std::fs::read_to_string(&second_backup).unwrap(),
            "second corrupt content"
        );

        // The store itself is left in a valid, usable, empty state after both resets.
        let (views, warning) = list_views_in(&root, "proj1", None, None).unwrap();
        assert!(views.is_empty());
        assert!(warning.is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn atomic_write_leaves_no_tmp_file_behind_on_success() {
        let root = temp_dir();
        create_view_in(&root, "proj1", sample_new_view("1.6", "ThingDef", "Weapon")).unwrap();

        let path = store_file_path(&root, "proj1");
        let tmp_path = path.with_extension("json.tmp");
        assert!(path.exists());
        assert!(!tmp_path.exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_and_get_last_selected_round_trips_per_scope() {
        let root = temp_dir();

        set_last_selected_in(
            &root,
            "proj1",
            LastSelectedFormView {
                game_version: "1.6".to_string(),
                def_type: "ThingDef".to_string(),
                view: SelectedFormViewRef {
                    origin: FormViewOrigin::Schema,
                    id: "weapon".to_string(),
                },
            },
        )
        .unwrap();
        set_last_selected_in(
            &root,
            "proj1",
            LastSelectedFormView {
                game_version: "1.6".to_string(),
                def_type: "PawnKindDef".to_string(),
                view: SelectedFormViewRef {
                    origin: FormViewOrigin::Default,
                    id: "default".to_string(),
                },
            },
        )
        .unwrap();

        let (thing_selection, _) = get_last_selected_in(&root, "proj1", "1.6", "ThingDef").unwrap();
        let thing_selection = thing_selection.unwrap();
        assert_eq!(thing_selection.origin, FormViewOrigin::Schema);
        assert_eq!(thing_selection.id, "weapon");

        let (pawn_selection, _) =
            get_last_selected_in(&root, "proj1", "1.6", "PawnKindDef").unwrap();
        assert_eq!(pawn_selection.unwrap().origin, FormViewOrigin::Default);

        let (none_selection, _) = get_last_selected_in(&root, "proj1", "1.6", "RecipeDef").unwrap();
        assert!(none_selection.is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_new_selection_in_the_same_scope_replaces_the_previous_one() {
        let root = temp_dir();

        set_last_selected_in(
            &root,
            "proj1",
            LastSelectedFormView {
                game_version: "1.6".to_string(),
                def_type: "ThingDef".to_string(),
                view: SelectedFormViewRef {
                    origin: FormViewOrigin::Schema,
                    id: "weapon".to_string(),
                },
            },
        )
        .unwrap();
        set_last_selected_in(
            &root,
            "proj1",
            LastSelectedFormView {
                game_version: "1.6".to_string(),
                def_type: "ThingDef".to_string(),
                view: SelectedFormViewRef {
                    origin: FormViewOrigin::Custom,
                    id: "custom-uuid".to_string(),
                },
            },
        )
        .unwrap();

        let (selection, _) = get_last_selected_in(&root, "proj1", "1.6", "ThingDef").unwrap();
        let selection = selection.unwrap();
        assert_eq!(selection.origin, FormViewOrigin::Custom);
        assert_eq!(selection.id, "custom-uuid");

        // Only one entry exists for the scope, not two accumulated ones.
        let path = store_file_path(&root, "proj1");
        let raw = std::fs::read_to_string(&path).unwrap();
        let store: UserFormViewStore = serde_json::from_str(&raw).unwrap();
        assert_eq!(store.preferences.last_selected.len(), 1);

        std::fs::remove_dir_all(&root).ok();
    }
}
