use crate::project_model::RegisteredLocation;
use crate::services::graphic_preview::asset_protocol::AssetTokenCache;
use crate::services::graphic_preview::model::{
    Direction, GraphicPreviewLabel, GraphicPreviewVariant, GraphicPreviewWarning,
};
use crate::services::graphic_preview::resolvers::exact::resolve_exact;

pub(in crate::services::graphic_preview) fn resolve_multi(
    tex_path: &str,
    search_locations: &[&RegisteredLocation],
    asset_cache: &AssetTokenCache,
    warnings: &mut Vec<GraphicPreviewWarning>,
) -> Vec<GraphicPreviewVariant> {
    const DIRECTIONS: [(&str, Direction); 4] = [
        ("north", Direction::North),
        ("east", Direction::East),
        ("south", Direction::South),
        ("west", Direction::West),
    ];

    let mut variants: Vec<GraphicPreviewVariant> = Vec::new();
    for (dir, direction) in &DIRECTIONS {
        let base = format!("{}_{}", tex_path, dir);
        variants.push(resolve_exact(
            dir,
            GraphicPreviewLabel::Direction {
                direction: *direction,
            },
            &base,
            search_locations,
            asset_cache,
            warnings,
        ));
    }

    // If all four directions are missing, try the base texPath as a fallback.
    if variants.iter().all(|v| v.missing == Some(true)) {
        let fallback = resolve_exact(
            "single",
            GraphicPreviewLabel::Single,
            tex_path,
            search_locations,
            asset_cache,
            warnings,
        );
        if fallback.missing != Some(true) {
            warnings.push(
                GraphicPreviewWarning::new(
                    "graphic_preview_directional_fallback_to_single",
                    format!(
                        "All directional textures missing for '{}'; fell back to single texture.",
                        tex_path
                    ),
                )
                .with_args(crate::diagnostics::diagnostic_args([(
                    "texPath",
                    tex_path.into(),
                )])),
            );
            return vec![fallback];
        }
    }

    variants
}
