use crate::project_model::{
    AppError, ProjectSettings, ProjectSettingsLoadResult, RegisteredLocationDraft,
    RegisteredLocationUpdate,
};
use crate::schema_pack::{list_installed_schema_game_versions, SchemaCatalogCacheState};
use crate::services::indexing::{self, IndexJobReason};
use crate::services::project_settings as ps_service;
use crate::settings_store::{load_settings, save_settings};
use tauri::{AppHandle, Manager};

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
