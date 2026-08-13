use crate::project_model::{
    AppError, ProjectSettings, ProjectSettingsLoadResult, RegisteredLocationDraft,
    RegisteredLocationUpdate, SourceType,
};
use crate::schema_pack::{list_installed_schema_game_versions, SchemaCatalogCacheState};
use crate::services::indexing::{self, IndexJobReason};
use crate::services::project_settings as ps_service;
use crate::settings_store::{load_settings, save_settings};
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Manager};

/// Bounded number of numeric-named child directories inspected for RimWorld content-pack
/// markers (`About/About.xml`, `Defs/`, `LoadFolders.xml`) when classifying a freshly-picked
/// source folder. Keeps classification cheap even for a collection with thousands of
/// subscribed items -- a handful of hits is already a confident signal.
const WORKSHOP_CLASSIFICATION_SAMPLE_LIMIT: usize = 20;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFolderClassification {
    pub suggested_source_type: SourceType,
    /// True only when there is more than one Workshop-item-shaped (all-digit-named) child
    /// directory AND at least one sampled child looks like an actual RimWorld content pack.
    /// A single numeric directory is too easily a coincidence to switch source type
    /// automatically -- callers should keep `folder` and surface a non-blocking reminder
    /// instead when this is false.
    pub high_confidence: bool,
    /// Count of immediate child directories whose name looks like a Steam published file id
    /// (all-digit). Never includes child paths or names -- aggregate count only.
    pub numeric_item_count: usize,
}

fn looks_like_workshop_item_id(name: &str) -> bool {
    name.len() >= 5 && name.bytes().all(|b| b.is_ascii_digit())
}

fn looks_like_content_pack(dir: &Path) -> bool {
    dir.join("About").join("About.xml").is_file()
        || dir.join("Defs").is_dir()
        || dir.join("LoadFolders.xml").is_file()
}

/// Classifies a freshly-picked source folder as a likely Steam Workshop collection root
/// (`.../steamapps/workshop/content/294100`, where every immediate child is one subscribed
/// mod) versus an ordinary single-mod folder, by inspecting only immediate children and a
/// bounded number of per-child marker paths -- never reads file contents or returns any child
/// path/name.
pub fn classify_source_folder(root_path: &Path) -> SourceFolderClassification {
    let mut numeric_item_count = 0usize;
    let mut content_pack_hits = 0usize;
    let mut sampled = 0usize;

    if let Ok(read_dir) = std::fs::read_dir(root_path) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if !looks_like_workshop_item_id(&name) {
                continue;
            }
            numeric_item_count += 1;
            if sampled < WORKSHOP_CLASSIFICATION_SAMPLE_LIMIT {
                sampled += 1;
                if looks_like_content_pack(&path) {
                    content_pack_hits += 1;
                }
            }
        }
    }

    let high_confidence = numeric_item_count >= 2 && content_pack_hits > 0;
    SourceFolderClassification {
        suggested_source_type: if high_confidence {
            SourceType::SteamWorkshop
        } else {
            SourceType::Folder
        },
        high_confidence,
        numeric_item_count,
    }
}

#[tauri::command]
pub fn classify_source_folder_cmd(path: String) -> SourceFolderClassification {
    classify_source_folder(Path::new(&path))
}

#[tauri::command]
pub fn get_project_settings(app: AppHandle) -> Result<ProjectSettingsLoadResult, AppError> {
    let mut settings = load_settings(&app)?;
    let active_before = settings.active_project_id.clone();
    let missing_active_project = ps_service::deactivate_missing_active_project(&mut settings);
    if settings.active_project_id != active_before {
        save_settings(&app, &settings)?;
        trigger_settings_reindex(&app, &settings);
    }
    Ok(ProjectSettingsLoadResult {
        settings,
        missing_active_project,
    })
}

fn trigger_settings_reindex(app: &AppHandle, settings: &ProjectSettings) {
    let _ = indexing::restart_for_settings(app, settings);
    if let Some(ref pid) = settings.active_project_id {
        indexing::enqueue_full_rebuild(app, Some(pid.clone()), IndexJobReason::SettingsChanged);
    }
    // Registered locations, game version, and active project all feed into the external schema
    // roots/game-version key `SchemaCatalogCacheState` builds catalogs from -- drop every cached
    // catalog so the next XPath completion (or built-in-only) request rebuilds against the new
    // settings rather than serving one built for the prior root set (Plan.md's cache invalidation
    // requirement).
    app.state::<SchemaCatalogCacheState>().invalidate_all();
}

#[tauri::command]
pub fn upsert_location(
    app: AppHandle,
    location: RegisteredLocationDraft,
) -> Result<ProjectSettings, AppError> {
    let mut settings = load_settings(&app)?;
    let added = ps_service::upsert_location(&mut settings, location)?;
    if added {
        save_settings(&app, &settings)?;
        trigger_settings_reindex(&app, &settings);
    }
    Ok(settings)
}

#[tauri::command]
pub fn remove_location(app: AppHandle, id: String) -> Result<ProjectSettings, AppError> {
    let mut settings = load_settings(&app)?;
    ps_service::remove_location(&mut settings, &id)?;
    save_settings(&app, &settings)?;
    trigger_settings_reindex(&app, &settings);
    Ok(settings)
}

#[tauri::command]
pub fn set_active_project(app: AppHandle, id: Option<String>) -> Result<ProjectSettings, AppError> {
    let mut settings = load_settings(&app)?;
    ps_service::set_active_project(&mut settings, id)?;
    save_settings(&app, &settings)?;
    trigger_settings_reindex(&app, &settings);
    Ok(settings)
}

#[tauri::command]
pub fn update_location(
    app: AppHandle,
    update: RegisteredLocationUpdate,
) -> Result<ProjectSettings, AppError> {
    let mut settings = load_settings(&app)?;
    ps_service::update_location(&mut settings, update)?;
    save_settings(&app, &settings)?;
    trigger_settings_reindex(&app, &settings);
    Ok(settings)
}

#[tauri::command]
pub fn update_app_locale(app: AppHandle, locale: String) -> Result<ProjectSettings, AppError> {
    let mut settings = load_settings(&app)?;
    ps_service::update_app_locale(&mut settings, locale)?;
    save_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn update_project_game_version(
    app: AppHandle,
    game_version: String,
    extra_schema_roots: Option<Vec<String>>,
) -> Result<ProjectSettings, AppError> {
    let roots: Vec<std::path::PathBuf> = extra_schema_roots
        .unwrap_or_default()
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect();
    let installed = list_installed_schema_game_versions(&roots);
    let mut settings = load_settings(&app)?;
    ps_service::update_project_game_version(&mut settings, game_version, &installed)?;
    save_settings(&app, &settings)?;
    trigger_settings_reindex(&app, &settings);
    Ok(settings)
}

#[cfg(test)]
mod classify_source_folder_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rimedit_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detects_a_high_confidence_workshop_collection() {
        let root = temp_dir();
        for id in ["1234567890", "2345678901"] {
            let item = root.join(id);
            fs::create_dir_all(item.join("Defs")).unwrap();
        }
        let result = classify_source_folder(&root);
        assert!(result.high_confidence);
        assert_eq!(result.suggested_source_type, SourceType::SteamWorkshop);
        assert_eq!(result.numeric_item_count, 2);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_single_numeric_directory_is_not_high_confidence() {
        let root = temp_dir();
        let item = root.join("1234567890");
        fs::create_dir_all(item.join("Defs")).unwrap();
        let result = classify_source_folder(&root);
        assert!(!result.high_confidence);
        assert_eq!(result.suggested_source_type, SourceType::Folder);
        assert_eq!(result.numeric_item_count, 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn numeric_directories_with_no_content_pack_markers_are_not_high_confidence() {
        let root = temp_dir();
        for id in ["1234567890", "2345678901"] {
            fs::create_dir_all(root.join(id)).unwrap();
        }
        let result = classify_source_folder(&root);
        assert!(!result.high_confidence);
        assert_eq!(result.suggested_source_type, SourceType::Folder);
        assert_eq!(result.numeric_item_count, 2);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_ordinary_single_mod_folder_is_not_a_workshop_collection() {
        let root = temp_dir();
        fs::create_dir_all(root.join("Defs")).unwrap();
        fs::create_dir_all(root.join("About")).unwrap();
        fs::write(root.join("About").join("About.xml"), "<ModMetaData/>").unwrap();
        let result = classify_source_folder(&root);
        assert!(!result.high_confidence);
        assert_eq!(result.suggested_source_type, SourceType::Folder);
        assert_eq!(result.numeric_item_count, 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn short_numeric_names_do_not_count_as_workshop_item_ids() {
        let root = temp_dir();
        // "1.6" and "123" are common non-Workshop directory names -- neither should count.
        fs::create_dir_all(root.join("1.6")).unwrap();
        fs::create_dir_all(root.join("123")).unwrap();
        let result = classify_source_folder(&root);
        assert_eq!(result.numeric_item_count, 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_directory_classifies_as_low_confidence_without_panicking() {
        let result = classify_source_folder(Path::new(
            "C:/definitely/not/a/real/path/rimedit_test_missing",
        ));
        assert!(!result.high_confidence);
        assert_eq!(result.suggested_source_type, SourceType::Folder);
        assert_eq!(result.numeric_item_count, 0);
    }
}
