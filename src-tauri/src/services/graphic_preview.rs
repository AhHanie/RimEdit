mod asset_protocol;
mod labels;
mod locations;
mod model;
mod paths;
mod resolvers;
mod strategy;

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::project_model::{AppError, LocationKind, ProjectSettings, RegisteredLocation};

#[allow(unused_imports)]
pub(crate) use asset_protocol::{
    content_type_for_texture, extract_asset_token, is_browser_preview_supported, preview_asset_url,
    read_preview_asset, AssetTokenCache,
};
pub(crate) use model::{GraphicPreviewAssetResult, GraphicPreviewVariant};

use locations::build_search_locations;
use paths::{normalize_texture_path, verified_textures_root};
use resolvers::appearances::resolve_appearances;
use resolvers::exact::resolve_exact;
use resolvers::folder::resolve_folder_collection;
use resolvers::multi::resolve_multi;
use resolvers::special::resolve_special_wrapper;
use strategy::{strategy_for_graphic_class, GraphicPreviewStrategy};

pub(crate) fn resolve_graphic_preview_assets(
    settings: &ProjectSettings,
    asset_cache: &AssetTokenCache,
    project_id: &str,
    tex_path: &str,
    graphic_class: &str,
    _mask_path: Option<&str>,
) -> Result<GraphicPreviewAssetResult, AppError> {
    if !settings
        .locations
        .iter()
        .any(|l| l.id == project_id && l.kind == LocationKind::Project)
    {
        return Err(AppError {
            code: "project_not_found".to_string(),
            message: format!("No project location found for id '{}'.", project_id),
            details: None,
        });
    }

    let normalized = normalize_texture_path(tex_path)?;
    let mut warnings: Vec<String> = Vec::new();

    let search_locations = build_search_locations(settings, project_id);

    for loc in &search_locations {
        if verified_textures_root(Path::new(&loc.root_path)).is_none() {
            warnings.push(format!(
                "Location '{}' has no Textures directory; no textures will be found there.",
                loc.display_name,
            ));
        }
    }

    let strategy = strategy_for_graphic_class(graphic_class);
    let variants = match strategy {
        GraphicPreviewStrategy::Single => resolve_single(
            &normalized,
            "single",
            "Single",
            &search_locations,
            asset_cache,
            &mut warnings,
        ),
        GraphicPreviewStrategy::DirectionalMulti => {
            resolve_multi(&normalized, &search_locations, asset_cache, &mut warnings)
        }
        GraphicPreviewStrategy::FolderCollection => resolve_folder_collection(
            graphic_class,
            &normalized,
            &search_locations,
            asset_cache,
            &mut warnings,
        ),
        GraphicPreviewStrategy::Appearances => {
            resolve_appearances(&normalized, &search_locations, asset_cache, &mut warnings)
        }
        GraphicPreviewStrategy::SpecialWrapper => resolve_special_wrapper(
            graphic_class,
            &normalized,
            &search_locations,
            asset_cache,
            &mut warnings,
        ),
        GraphicPreviewStrategy::Unknown => {
            warnings.push(format!(
                "Unknown graphic class '{}'; falling back to single texture.",
                graphic_class
            ));
            resolve_single(
                &normalized,
                "single",
                "Single",
                &search_locations,
                asset_cache,
                &mut warnings,
            )
        }
    };

    Ok(GraphicPreviewAssetResult {
        tex_path: tex_path.to_string(),
        graphic_class: graphic_class.to_string(),
        variants,
        warnings,
    })
}

fn resolve_single(
    relative_base: &str,
    role: &str,
    label: &str,
    search_locations: &[&RegisteredLocation],
    asset_cache: &AssetTokenCache,
    warnings: &mut Vec<String>,
) -> Vec<GraphicPreviewVariant> {
    vec![resolve_exact(
        role,
        label,
        relative_base,
        search_locations,
        asset_cache,
        warnings,
    )]
}
