use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::project_model::RegisteredLocation;
use crate::services::graphic_preview::asset_protocol::{preview_asset_url, AssetTokenCache};
use crate::services::graphic_preview::labels::{detect_direction, is_mask_stem, stack_count_label};
use crate::services::graphic_preview::model::GraphicPreviewVariant;
use crate::services::graphic_preview::paths::{verified_textures_root, TEXTURE_EXTENSIONS};

const FOLDER_VARIANT_CAP: usize = 64;

pub(in crate::services::graphic_preview) fn resolve_folder_collection(
    graphic_class: &str,
    tex_path: &str,
    search_locations: &[&RegisteredLocation],
    asset_cache: &AssetTokenCache,
    warnings: &mut Vec<String>,
) -> Vec<GraphicPreviewVariant> {
    // Map relative-to-textures-root path -> (loc_id, loc_name, canonical_path).
    // or_insert keeps the first entry, so higher-precedence locations (project) win.
    let mut found: BTreeMap<String, (String, String, PathBuf)> = BTreeMap::new();

    for location in search_locations {
        let location_root = PathBuf::from(&location.root_path);
        let canonical_textures = match verified_textures_root(&location_root) {
            Some(t) => t,
            None => continue,
        };
        let search_dir = if tex_path.is_empty() {
            canonical_textures.clone()
        } else {
            canonical_textures.join(tex_path.replace('/', std::path::MAIN_SEPARATOR_STR))
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
            "No loose textures found in folder for '{}'.",
            tex_path
        ));
        return vec![GraphicPreviewVariant {
            id: format!("variant:missing:{}", tex_path),
            label: "Variant 1".to_string(),
            role: "variant".to_string(),
            source_location_id: String::new(),
            source_location_name: String::new(),
            relative_texture_path: format!("Textures/{}", tex_path),
            asset_url: String::new(),
            asset_token: None,
            missing: Some(true),
        }];
    }

    // --- Phase 2: group by base stem, detecting direction suffixes ---
    struct GroupMember {
        relative: String,
        dir_role: Option<&'static str>,
        dir_label: Option<&'static str>,
        loc_id: String,
        loc_name: String,
        canonical_path: PathBuf,
    }

    let mut groups: BTreeMap<String, Vec<GroupMember>> = BTreeMap::new();

    for (relative, (loc_id, loc_name, canonical_path)) in found {
        let stem = Path::new(&relative)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let (base_key, dir_role, dir_label) = match detect_direction(stem) {
            Some((role, label, suffix_len)) => (
                stem[..stem.len() - suffix_len].to_lowercase(),
                Some(role),
                Some(label),
            ),
            None => (stem.to_lowercase(), None, None),
        };
        groups.entry(base_key).or_default().push(GroupMember {
            relative,
            dir_role,
            dir_label,
            loc_id,
            loc_name,
            canonical_path,
        });
    }

    // --- Phase 3: apply FOLDER_VARIANT_CAP after grouping ---
    let truncated = groups.len() > FOLDER_VARIANT_CAP;
    let groups_to_emit: Vec<_> = groups.into_iter().take(FOLDER_VARIANT_CAP).collect();
    if truncated {
        warnings.push(format!(
            "Folder variant results truncated to {} groups.",
            FOLDER_VARIANT_CAP
        ));
    }

    // --- Phase 4: flatten groups into labelled variants ---
    let is_stack_count = graphic_class == "Graphic_StackCount";
    let mut variants = Vec::new();

    for (group_index, (_base_key, members)) in groups_to_emit.into_iter().enumerate() {
        let first_stem = Path::new(&members[0].relative)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let base_stem = match detect_direction(first_stem) {
            Some((_, _, suffix_len)) => &first_stem[..first_stem.len() - suffix_len],
            None => first_stem,
        };
        let group_label_base = if is_stack_count {
            stack_count_label(base_stem, group_index)
        } else {
            format!("Variant {}", group_index + 1)
        };

        for member in members {
            let GroupMember {
                relative,
                dir_role,
                dir_label,
                loc_id,
                loc_name,
                canonical_path,
            } = member;
            let ext = canonical_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "dds" {
                warnings.push(format!(
                    "Resolved DDS texture 'Textures/{}'; browser preview may be unsupported until conversion is added.",
                    relative
                ));
            }
            let label = match dir_label {
                Some(dir) => format!("{} {}", group_label_base, dir),
                None => group_label_base.clone(),
            };
            let role = dir_role.unwrap_or("variant");
            let relative_texture_path = format!("Textures/{}", relative);
            let token = asset_cache.register(canonical_path);
            variants.push(GraphicPreviewVariant {
                id: format!("{}:{}:{}", role, loc_id, relative_texture_path),
                label,
                role: role.to_string(),
                source_location_id: loc_id,
                source_location_name: loc_name,
                relative_texture_path,
                asset_url: preview_asset_url(&token),
                asset_token: Some(token),
                missing: None,
            });
        }
    }

    variants
}
