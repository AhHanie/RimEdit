use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

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

fn relative_path_to_forward_slash(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str().map(|s| s.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// One Def XML file discovered for a location, in RimWorld load order (see
/// `scan_def_files_in_load_order`). `relative_path` is normalized to forward slashes and relative
/// to the location root -- the same convention as `XmlEditorFileRef.relativePath` on the frontend
/// and `patches::scan::scan_indexable_patch_xml_files`'s `relative_path` -- so a
/// [`super::model::PatchPreviewTarget`] built from an open editor tab can be matched against it
/// directly, without any path-format translation.
pub(super) struct ScannedDefFile {
    pub absolute_path: PathBuf,
    pub relative_path: String,
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
) -> Vec<ScannedDefFile> {
    let resolution = resolve_load_folders(location, settings);
    let location_root = &resolution.root_path;
    let mut seen: HashSet<String> = HashSet::new();
    let mut files: Vec<ScannedDefFile> = Vec::new();

    for folder in &resolution.selected_folders {
        let defs_dir = folder.absolute_path.join("Defs");
        if !defs_dir.exists() || !defs_dir.is_dir() {
            continue;
        }
        let mut folder_files: Vec<ScannedDefFile> = Vec::new();
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
            let relative_path = entry
                .path()
                .strip_prefix(location_root)
                .map(relative_path_to_forward_slash)
                .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());
            folder_files.push(ScannedDefFile {
                absolute_path: entry.path().to_path_buf(),
                relative_path,
            });
        }
        folder_files.sort_by(|a, b| a.absolute_path.cmp(&b.absolute_path));
        files.extend(folder_files);
    }

    files
}

/// Parses one Def XML file's content and appends its top-level Defs into `defs_root` (unwrapping
/// its own `<Defs>` root, if present -- a malformed file with a different or missing root just
/// has its top-level content appended directly, best-effort). Returns the top-level Def elements
/// this call appended, in document order -- the caller uses each element's position in this
/// returned list as its provenance ordinal within this file, matching
/// `xml_document::def_summary::extract_def_summaries`'s per-file ordinal for the same file.
pub(super) fn append_def_file_contents<'d>(
    document: Document<'d>,
    defs_root: Element<'d>,
    raw: &str,
) -> Vec<Element<'d>> {
    let before = defs_root.children().len();
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
    defs_root
        .children()
        .into_iter()
        .skip(before)
        .filter_map(|c| match c {
            ChildOfElement::Element(el) => Some(el),
            _ => None,
        })
        .collect()
}
