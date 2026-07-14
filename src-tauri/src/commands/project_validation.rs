use crate::project_model::AppError;
use crate::project_validation::{
    validate_project as validate_project_impl, ProjectValidationResult,
};
use crate::schema_pack::{build_schema_catalog, schema_pack_roots};
use crate::services::def_index_cache;
use crate::settings_store::load_settings;
use tauri::AppHandle;

#[tauri::command]
pub fn validate_project(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectValidationResult, AppError> {
    let settings = load_settings(&app)?;
    // Same catalog-context policy as live document/save validation (`schema_pack_roots` +
    // the project's selected game version) -- project-wide validation must not diverge from
    // what per-document validation already reported. See Plan.md section 15's "catalog-context
    // mismatch" and issue 09.
    let roots = schema_pack_roots(&settings);
    let catalog_result = build_schema_catalog(&roots, Some(&settings.game_version));
    let def_index = def_index_cache::load_for_project(&app, &settings, &project_id, false)?;
    validate_project_impl(&settings, &project_id, &catalog_result.catalog, &def_index)
}
