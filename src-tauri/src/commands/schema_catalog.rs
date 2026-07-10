use crate::project_model::AppError;
use crate::schema_pack::{
    build_schema_catalog, list_installed_schema_game_versions, SchemaCatalogLoadResult,
};
use std::path::PathBuf;
use tauri::AppHandle;

#[tauri::command]
pub fn load_schema_catalog(
    _app: AppHandle,
    extra_schema_roots: Option<Vec<String>>,
    game_version: Option<String>,
) -> Result<SchemaCatalogLoadResult, AppError> {
    let roots: Vec<PathBuf> = extra_schema_roots
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();
    Ok(build_schema_catalog(&roots, game_version.as_deref()))
}

#[tauri::command]
pub fn list_installed_schema_game_versions_cmd(
    _app: AppHandle,
    extra_schema_roots: Option<Vec<String>>,
) -> Result<Vec<String>, AppError> {
    let roots: Vec<PathBuf> = extra_schema_roots
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();
    Ok(list_installed_schema_game_versions(&roots))
}
