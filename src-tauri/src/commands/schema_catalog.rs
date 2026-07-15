use crate::project_model::AppError;
use crate::schema_pack::{
    build_schema_catalog_with_locale, list_installed_schema_game_versions, SchemaCatalogLoadResult,
};
use std::path::PathBuf;
use tauri::AppHandle;

/// Load the schema catalog for display (issue 06). `locale` is the frontend's active UI locale
/// (`useLocale()`'s current value); it is resolved against the application locale registry inside
/// `build_schema_catalog_with_locale`, so an absent/unsupported value deterministically falls back
/// to `crate::locale::FALLBACK_LOCALE` rather than erroring -- this command never fails because of
/// a bad locale string. Recomputes the catalog on every call (no server-side caching here) exactly
/// as before locale was added; only the per-keystroke XPath-completion path
/// (`SchemaCatalogCacheState`) caches a built-in-only catalog.
#[tauri::command]
pub fn load_schema_catalog(
    _app: AppHandle,
    extra_schema_roots: Option<Vec<String>>,
    game_version: Option<String>,
    locale: Option<String>,
) -> Result<SchemaCatalogLoadResult, AppError> {
    let roots: Vec<PathBuf> = extra_schema_roots
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();
    Ok(build_schema_catalog_with_locale(
        &roots,
        game_version.as_deref(),
        locale.as_deref(),
    ))
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
