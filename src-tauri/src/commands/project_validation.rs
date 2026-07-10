use crate::project_model::AppError;
use crate::project_validation::{
    validate_project as validate_project_impl, ProjectValidationResult,
};
use crate::schema_pack::build_schema_catalog;
use crate::services::def_index_cache;
use crate::settings_store::load_settings;
use tauri::AppHandle;

#[tauri::command]
pub fn validate_project(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectValidationResult, AppError> {
    let settings = load_settings(&app)?;
    let catalog_result = build_schema_catalog(&[], None);
    let def_index = def_index_cache::load_for_project(&app, &settings, &project_id, false)?;
    validate_project_impl(&settings, &project_id, &catalog_result.catalog, &def_index)
}
