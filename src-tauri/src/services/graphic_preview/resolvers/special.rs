use crate::project_model::RegisteredLocation;
use crate::services::graphic_preview::asset_protocol::AssetTokenCache;
use crate::services::graphic_preview::model::GraphicPreviewVariant;
use crate::services::graphic_preview::resolvers::exact::resolve_exact;
use crate::services::graphic_preview::resolvers::folder::resolve_folder_collection;

pub(in crate::services::graphic_preview) fn resolve_special_wrapper(
    graphic_class: &str,
    tex_path: &str,
    search_locations: &[&RegisteredLocation],
    asset_cache: &AssetTokenCache,
    warnings: &mut Vec<String>,
) -> Vec<GraphicPreviewVariant> {
    warnings.push(format!(
        "'{}' renders with runtime context that cannot be fully previewed as a loose texture.",
        graphic_class
    ));

    let is_linked = graphic_class.starts_with("Graphic_Linked");

    if is_linked {
        // Try base single first; accumulate its warnings separately so we only
        // emit them if folder collection also fails.
        let mut single_warnings: Vec<String> = Vec::new();
        let single = resolve_exact(
            "single",
            "Single",
            tex_path,
            search_locations,
            asset_cache,
            &mut single_warnings,
        );

        if single.missing != Some(true) {
            warnings.extend(single_warnings);
            return vec![single];
        }

        // Base single missing - try folder collection as fallback.
        let folder = resolve_folder_collection(
            graphic_class,
            tex_path,
            search_locations,
            asset_cache,
            warnings,
        );
        if folder.iter().any(|v| v.missing != Some(true)) {
            return folder;
        }

        // Nothing found; report the single-miss warning and return the placeholder.
        warnings.extend(single_warnings);
        return vec![single];
    }

    // For RandomRotated, Shadow, PawnBodySilhouette: base single with warning.
    vec![resolve_exact(
        "single",
        "Single",
        tex_path,
        search_locations,
        asset_cache,
        warnings,
    )]
}
