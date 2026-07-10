use crate::project_model::RegisteredLocation;
use crate::services::graphic_preview::asset_protocol::AssetTokenCache;
use crate::services::graphic_preview::model::GraphicPreviewVariant;
use crate::services::graphic_preview::resolvers::exact::resolve_exact;

pub(in crate::services::graphic_preview) fn resolve_multi(
    tex_path: &str,
    search_locations: &[&RegisteredLocation],
    asset_cache: &AssetTokenCache,
    warnings: &mut Vec<String>,
) -> Vec<GraphicPreviewVariant> {
    const DIRECTIONS: [(&str, &str); 4] = [
        ("north", "North"),
        ("east", "East"),
        ("south", "South"),
        ("west", "West"),
    ];

    let mut variants: Vec<GraphicPreviewVariant> = Vec::new();
    for (dir, label) in &DIRECTIONS {
        let base = format!("{}_{}", tex_path, dir);
        variants.push(resolve_exact(
            dir,
            label,
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
            "Single",
            tex_path,
            search_locations,
            asset_cache,
            warnings,
        );
        if fallback.missing != Some(true) {
            warnings.push(format!(
                "All directional textures missing for '{}'; fell back to single texture.",
                tex_path
            ));
            return vec![fallback];
        }
    }

    variants
}
