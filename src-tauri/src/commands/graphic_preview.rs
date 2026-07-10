use tauri::{AppHandle, State};

use crate::project_model::AppError;
use crate::services::graphic_preview::{self, AssetTokenCache, GraphicPreviewAssetResult};
use crate::settings_store::load_settings;

#[tauri::command]
pub fn resolve_graphic_preview_assets(
    app: AppHandle,
    state: State<'_, AssetTokenCache>,
    project_id: String,
    tex_path: String,
    graphic_class: String,
    mask_path: Option<String>,
) -> Result<GraphicPreviewAssetResult, AppError> {
    let _span = crate::instrumentation::span(&app, "commands.resolveGraphicPreviewAssets");
    let settings = load_settings(&app)?;
    graphic_preview::resolve_graphic_preview_assets(
        &settings,
        &state,
        &project_id,
        &tex_path,
        &graphic_class,
        mask_path.as_deref(),
    )
}
