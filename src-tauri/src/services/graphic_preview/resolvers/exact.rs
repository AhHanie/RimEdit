use std::path::PathBuf;

use crate::project_model::RegisteredLocation;
use crate::services::graphic_preview::asset_protocol::{preview_asset_url, AssetTokenCache};
use crate::services::graphic_preview::model::{
    GraphicPreviewLabel, GraphicPreviewVariant, GraphicPreviewWarning,
};
use crate::services::graphic_preview::paths::{
    resolve_existing_texture_file, texture_relative_file_candidates, verified_textures_root,
};

pub(in crate::services::graphic_preview) fn resolve_exact(
    role: &str,
    label: GraphicPreviewLabel,
    relative_base: &str,
    search_locations: &[&RegisteredLocation],
    asset_cache: &AssetTokenCache,
    warnings: &mut Vec<GraphicPreviewWarning>,
) -> GraphicPreviewVariant {
    let candidates = texture_relative_file_candidates(relative_base);

    for location in search_locations {
        let location_root = PathBuf::from(&location.root_path);
        let canonical_textures = match verified_textures_root(&location_root) {
            Some(t) => t,
            None => continue,
        };
        for candidate in &candidates {
            if let Some(resolved) = resolve_existing_texture_file(&canonical_textures, candidate) {
                let ext = resolved
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "dds" {
                    warnings.push(
                        GraphicPreviewWarning::new(
                            "graphic_preview_dds_unsupported",
                            format!(
                                "Resolved DDS texture '{}'; browser preview may be unsupported until conversion is added.",
                                candidate
                            ),
                        )
                        .with_args(crate::diagnostics::diagnostic_args([(
                            "relativePath",
                            candidate.as_str().into(),
                        )])),
                    );
                }
                let relative_texture_path = format!("Textures/{}", candidate);
                let token = asset_cache.register(resolved);
                let asset_url = preview_asset_url(&token);
                return GraphicPreviewVariant {
                    id: format!("{}:{}:{}", role, location.id, relative_texture_path),
                    label: label.clone(),
                    role: role.to_string(),
                    source_location_id: location.id.clone(),
                    source_location_name: location.display_name.clone(),
                    relative_texture_path,
                    asset_url,
                    asset_token: Some(token),
                    missing: None,
                };
            }
        }
    }

    warnings.push(
        GraphicPreviewWarning::new(
            "graphic_preview_texture_not_found",
            format!("No loose texture found for {}.", relative_base),
        )
        .with_args(crate::diagnostics::diagnostic_args([(
            "relativePath",
            relative_base.into(),
        )])),
    );
    GraphicPreviewVariant {
        id: format!("missing:{}:{}", role, relative_base),
        label,
        role: role.to_string(),
        source_location_id: String::new(),
        source_location_name: String::new(),
        relative_texture_path: format!("Textures/{}", relative_base),
        asset_url: String::new(),
        asset_token: None,
        missing: Some(true),
    }
}
