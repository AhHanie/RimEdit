use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::project_model::RegisteredLocation;
use crate::services::graphic_preview::asset_protocol::{preview_asset_url, AssetTokenCache};
use crate::services::graphic_preview::labels::{appearance_label, is_mask_stem};
use crate::services::graphic_preview::model::GraphicPreviewVariant;
use crate::services::graphic_preview::paths::{verified_textures_root, TEXTURE_EXTENSIONS};

const FOLDER_VARIANT_CAP: usize = 64;

pub(in crate::services::graphic_preview) fn resolve_appearances(
    tex_path: &str,
    search_locations: &[&RegisteredLocation],
    asset_cache: &AssetTokenCache,
    warnings: &mut Vec<String>,
) -> Vec<GraphicPreviewVariant> {
    warnings.push(
        "Graphic_Appearances: StuffAppearanceDef.pathPrefix cross-reference is not implemented; \
         showing all candidate textures matching the path prefix."
            .to_string(),
    );

    // tex_path is a prefix like "Things/Stuff/Blocks"; scan the parent dir for files
    // whose stem starts with the base component ("Blocks").
    let (parent_dir, base_name) = tex_path
        .rsplit_once('/')
        .map_or(("", tex_path), |(p, b)| (p, b));
    let base_name_lower = base_name.to_lowercase();

    let mut found: BTreeMap<String, (String, String, PathBuf)> = BTreeMap::new();

    for location in search_locations {
        let location_root = PathBuf::from(&location.root_path);
        let canonical_textures = match verified_textures_root(&location_root) {
            Some(t) => t,
            None => continue,
        };
        let search_dir = if parent_dir.is_empty() {
            canonical_textures.clone()
        } else {
            canonical_textures.join(parent_dir.replace('/', std::path::MAIN_SEPARATOR_STR))
        };
        if !search_dir.is_dir() {
            continue;
        }
        let entries = match fs::read_dir(&search_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !TEXTURE_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !stem.to_lowercase().starts_with(&base_name_lower) {
                continue;
            }
            if is_mask_stem(stem) {
                continue;
            }
            let canonical_path = match path.canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };
            if canonical_path.strip_prefix(&canonical_textures).is_err() {
                continue;
            }
            let relative = canonical_path
                .strip_prefix(&canonical_textures)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            found.entry(relative).or_insert_with(|| {
                (
                    location.id.clone(),
                    location.display_name.clone(),
                    canonical_path,
                )
            });
        }
    }

    if found.is_empty() {
        warnings.push(format!(
            "No appearance textures found for prefix '{}'.",
            tex_path
        ));
        return vec![GraphicPreviewVariant {
            id: format!("appearance:missing:{}", tex_path),
            label: "Appearance 1".to_string(),
            role: "appearance".to_string(),
            source_location_id: String::new(),
            source_location_name: String::new(),
            relative_texture_path: format!("Textures/{}", tex_path),
            asset_url: String::new(),
            asset_token: None,
            missing: Some(true),
        }];
    }

    let truncated = found.len() > FOLDER_VARIANT_CAP;
    let results: Vec<_> = found.into_iter().take(FOLDER_VARIANT_CAP).collect();
    if truncated {
        warnings.push(format!(
            "Appearance variant results truncated to {} entries.",
            FOLDER_VARIANT_CAP
        ));
    }

    let mut variants = Vec::new();
    for (i, (relative, (loc_id, loc_name, canonical_path))) in results.into_iter().enumerate() {
        let ext = canonical_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "dds" {
            warnings.push(format!(
                "Resolved DDS texture 'Textures/{}'; browser preview may be unsupported.",
                relative
            ));
        }
        let stem = Path::new(&relative)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let label = appearance_label(stem, &base_name_lower, i);
        let relative_texture_path = format!("Textures/{}", relative);
        let token = asset_cache.register(canonical_path);
        variants.push(GraphicPreviewVariant {
            id: format!("appearance:{}:{}", loc_id, relative_texture_path),
            label,
            role: "appearance".to_string(),
            source_location_id: loc_id,
            source_location_name: loc_name,
            relative_texture_path,
            asset_url: preview_asset_url(&token),
            asset_token: Some(token),
            missing: None,
        });
    }
    variants
}
