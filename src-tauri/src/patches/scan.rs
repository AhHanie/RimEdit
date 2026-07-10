//! Discovers `Patches/**/*.xml` files for a registered location, reusing the same
//! load-folder resolution and first-load-folder-wins shadowing rules as Def scanning
//! (see `project_files::scan::scan_indexable_def_xml_files`).
//!
//! Unlike Def scanning, file order here is *not* re-sorted across load folders: the final
//! list is folder-major (in `resolve_load_folders`'s precedence order), file-minor (sorted
//! alphabetically within each folder). Def order doesn't matter for indexing, but patch
//! application order does, so the stable default preview order documented in
//! `docs/patches-editor/02-patch-file-scanning-and-indexing.md` must be preserved:
//! location order, then load folder order, then file order, then operation order.

use std::collections::HashSet;
use std::path::{Component, Path};

use walkdir::WalkDir;

use crate::project_files::{
    LocationXmlFileScan, ProjectFileEntry, ProjectFileError, ProjectFileKind,
};
use crate::project_model::{ProjectSettings, RegisteredLocation};
use crate::rimworld_load_folders::resolve_load_folders;

fn is_xml_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("xml"))
        .unwrap_or(false)
}

fn relative_path_to_forward_slash(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str().map(|s| s.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn folder_path_of(normalized: &str) -> String {
    match normalized.rfind('/') {
        Some(i) => normalized[..i].to_string(),
        None => String::new(),
    }
}

/// Scan only the Patch XML files RimWorld would load for the given location and project
/// game version.
///
/// For each resolved load folder (in precedence order), this scans `<folder>/Patches/**/*.xml`.
/// The first file with a given relative-to-load-folder identity key wins (shadowing), mirroring
/// `scan_indexable_def_xml_files`. Returned `relative_path` values are relative to the location
/// root. Files are grouped by load folder (highest precedence first) and sorted alphabetically
/// within each folder; the overall list is not re-sorted across folders so callers can rely on
/// folder-major, file-minor ordering.
pub(crate) fn scan_indexable_patch_xml_files(
    settings: &ProjectSettings,
    location: &RegisteredLocation,
) -> Result<LocationXmlFileScan, ProjectFileError> {
    let resolution = resolve_load_folders(location, settings);
    let location_root = &resolution.root_path;

    let mut seen_keys: HashSet<String> = HashSet::new();
    let should_shadow = resolution.shadow_by_relative_path;
    let mut files: Vec<ProjectFileEntry> = Vec::new();

    for folder in &resolution.selected_folders {
        let patches_dir = folder.absolute_path.join("Patches");
        if !patches_dir.exists() || !patches_dir.is_dir() {
            continue;
        }

        let mut folder_files: Vec<ProjectFileEntry> = Vec::new();
        for result in WalkDir::new(&patches_dir).follow_links(false) {
            let entry = result.map_err(|e| ProjectFileError::ScanFailed(e.to_string()))?;
            if !entry.file_type().is_file() || !is_xml_extension(entry.path()) {
                continue;
            }

            if should_shadow {
                // Identity key: path relative to the load folder (e.g. `Patches/Foo.xml`).
                let identity_key = entry
                    .path()
                    .strip_prefix(&folder.absolute_path)
                    .map(relative_path_to_forward_slash)
                    .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());

                if !seen_keys.insert(identity_key) {
                    // Shadowed by an earlier (higher-precedence) load folder.
                    continue;
                }
            }

            // Relative path from the location root (used for file opening and indexing).
            let rel_from_root = entry
                .path()
                .strip_prefix(location_root)
                .map(relative_path_to_forward_slash)
                .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());

            let file_name = entry.file_name().to_string_lossy().to_string();
            let folder_path = folder_path_of(&rel_from_root);
            let size_bytes = entry.path().metadata().map(|m| m.len()).unwrap_or(0);

            folder_files.push(ProjectFileEntry {
                relative_path: rel_from_root,
                folder_path,
                file_name,
                extension: "xml".to_string(),
                size_bytes,
                file_kind: ProjectFileKind::Xml,
                active_for_game_version: None,
            });
        }

        // RimWorld's own Patches folder enumeration order is filesystem-dependent (no sort is
        // applied by DirectXmlLoader for the Patches path, unlike some other asset scans). Sort
        // alphabetically within each folder for a stable, reproducible default preview order.
        folder_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        files.extend(folder_files);
    }

    Ok(LocationXmlFileScan {
        location_id: location.id.clone(),
        location_name: location.display_name.clone(),
        root_path: location_root.to_string_lossy().to_string(),
        source_kind: location.kind.clone(),
        source_type: location.source_type.clone(),
        read_only: location.read_only,
        mod_id: location.mod_id.clone(),
        files,
    })
}
