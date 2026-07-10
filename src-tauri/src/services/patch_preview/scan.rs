use std::collections::HashSet;
use std::path::PathBuf;

use sxd_document::dom::{ChildOfElement, Document, Element};
use walkdir::WalkDir;

use crate::patches::dom::parse_fragment;
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation};
use crate::rimworld_load_folders::resolve_load_folders;

pub(super) fn included_locations<'a>(
    settings: &'a ProjectSettings,
    project_id: &str,
) -> Vec<&'a RegisteredLocation> {
    settings
        .locations
        .iter()
        .filter(|l| match l.kind {
            LocationKind::Project => l.id == project_id,
            LocationKind::Source => true,
        })
        .collect()
}

/// Def XML files for one location, in folder-major/file-minor RimWorld load order (mirrors
/// `patches::scan::scan_indexable_patch_xml_files`'s ordering guarantee, applied to `Defs/`
/// instead of `Patches/` -- Def scanning elsewhere in this crate re-sorts globally by relative
/// path for indexing purposes, which is fine for Def lookup but wrong for building a combined
/// document in load order, since later-loaded files must patch/override earlier ones the same
/// way RimWorld's own shadowing does).
pub(super) fn scan_def_files_in_load_order(
    settings: &ProjectSettings,
    location: &RegisteredLocation,
) -> Vec<PathBuf> {
    let resolution = resolve_load_folders(location, settings);
    let mut seen: HashSet<String> = HashSet::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for folder in &resolution.selected_folders {
        let defs_dir = folder.absolute_path.join("Defs");
        if !defs_dir.exists() || !defs_dir.is_dir() {
            continue;
        }
        let mut folder_files: Vec<PathBuf> = Vec::new();
        for entry in WalkDir::new(&defs_dir).follow_links(false) {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            let is_xml = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("xml"))
                .unwrap_or(false);
            if !is_xml {
                continue;
            }
            if resolution.shadow_by_relative_path {
                let key = entry
                    .path()
                    .strip_prefix(&folder.absolute_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());
                if !seen.insert(key) {
                    continue;
                }
            }
            folder_files.push(entry.path().to_path_buf());
        }
        folder_files.sort();
        files.extend(folder_files);
    }

    files
}

/// Parses one Def XML file's content and appends its top-level Defs into `defs_root` (unwrapping
/// its own `<Defs>` root, if present -- a malformed file with a different or missing root just
/// has its top-level content appended directly, best-effort).
pub(super) fn append_def_file_contents<'d>(
    document: Document<'d>,
    defs_root: Element<'d>,
    raw: &str,
) {
    let result = parse_fragment(document, raw);
    for node in result.nodes {
        if let ChildOfElement::Element(el) = node {
            if el.name().local_part() == "Defs" {
                defs_root.append_children(el.children());
                continue;
            }
            defs_root.append_child(el);
            continue;
        }
        defs_root.append_child(node);
    }
}
