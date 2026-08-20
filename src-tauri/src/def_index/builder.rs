use crate::project_files::scan_indexable_def_xml_files;
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};
use crate::rimworld_load_folders::resolve_load_folders;
use crate::xml_document::model::{XmlDocument, XmlNodeId, XmlNodeKind};
use crate::xml_document::parse_to_document;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

use super::model::{
    DefIdentityKey, DefIndex, DefIndexError, DefIndexReplacement, IndexedDef, IndexedDefField,
    IndexedDefSource, IndexedSourceKind,
};

pub struct DefIndexBuildOptions<'a> {
    pub project_id: Option<&'a str>,
    pub include_sources: bool,
    pub replacement: Option<DefIndexReplacement<'a>>,
    pub force_rebuild: bool,
}

impl<'a> DefIndexBuildOptions<'a> {
    #[cfg(test)]
    pub fn for_project(project_id: &'a str) -> Self {
        Self {
            project_id: Some(project_id),
            include_sources: true,
            replacement: None,
            force_rebuild: false,
        }
    }
}

/// Plain (no-progress) entry point. Every production call site now goes through
/// `rebuild_and_store_def_index_with_progress`'s `None` path instead (see
/// `rebuild_and_store_def_index`), so this is only reachable from tests today -- kept as the
/// simplest possible public API for them rather than forcing every test call site to pass
/// `None` explicitly to `build_def_index_with_progress`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn build_def_index(settings: &ProjectSettings, options: DefIndexBuildOptions<'_>) -> DefIndex {
    build_def_index_with_progress(settings, options, None)
}

/// Same as `build_def_index`, but invokes `on_file_indexed` once for every discovered Def XML
/// file it attempts (successfully parsed or not), for live progress reporting during a full
/// rebuild's `Indexing` stage (see `def_index::IndexingStage` and
/// `services::indexing::jobs::execute_full_rebuild`). `on_file_indexed: None` behaves identically
/// to `build_def_index`; this is a separate function (rather than a new `DefIndexBuildOptions`
/// field) so the ~15 other call sites that construct `DefIndexBuildOptions` by struct literal
/// don't need to change.
pub fn build_def_index_with_progress(
    settings: &ProjectSettings,
    options: DefIndexBuildOptions<'_>,
    on_file_indexed: Option<&dyn Fn()>,
) -> DefIndex {
    let mut index = DefIndex {
        defs: Vec::new(),
        errors: Vec::new(),
        built_at_unix_ms: OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000,
        by_type: Default::default(),
    };

    for location in included_locations(settings, &options) {
        add_location_to_index(
            &mut index,
            settings,
            location,
            options.replacement.as_ref(),
            on_file_indexed,
        );
    }

    index.rebuild_computed_fields();
    index
}

/// Cheap, parse-free scan statistics for a full rebuild's `Discovering` stage (see
/// `def_index::IndexingStage`) -- computed by directory listing only, never reading or parsing
/// XML content. No absolute paths or file/mod names: aggregate counts only.
#[derive(Debug, Clone, Default)]
pub struct ScanDiscoveryStats {
    pub included_locations: usize,
    pub resolved_workshop_item_count: usize,
    pub selected_load_folder_count: usize,
    pub discovered_files: usize,
}

/// Discovers scan statistics for every location that would be included in a full rebuild with
/// `options`, without parsing any XML. Reuses the same `resolve_load_folders`/
/// `scan_indexable_def_xml_files` calls the real build performs (see `add_location_to_index`) --
/// so this walks each location's directories twice per rebuild. Intentionally not deduplicated;
/// that optimization is only worth making once timing shows it is a material bottleneck. This
/// function exists purely to make discovery observable before parsing starts, per
/// `IndexingStage::Discovering`.
pub fn discover_scan_stats(
    settings: &ProjectSettings,
    options: &DefIndexBuildOptions<'_>,
) -> ScanDiscoveryStats {
    let locations = included_locations(settings, options);
    let mut stats = ScanDiscoveryStats {
        included_locations: locations.len(),
        ..Default::default()
    };

    for location in locations {
        let resolution = resolve_load_folders(location, settings);
        stats.selected_load_folder_count += resolution.selected_folders.len();

        if location.source_type == SourceType::SteamWorkshop {
            let item_scopes: HashSet<&str> = resolution
                .selected_folders
                .iter()
                .map(|f| f.scope.as_str())
                .filter(|s| !s.is_empty())
                .collect();
            stats.resolved_workshop_item_count += item_scopes.len();
        }

        if let Ok(scan) = scan_indexable_def_xml_files(settings, location) {
            stats.discovered_files += scan.files.len();
        }
    }

    stats
}

pub(super) fn included_locations<'a>(
    settings: &'a ProjectSettings,
    options: &DefIndexBuildOptions<'_>,
) -> Vec<&'a RegisteredLocation> {
    settings
        .locations
        .iter()
        .filter(|location| match location.kind {
            LocationKind::Project => options.project_id == Some(location.id.as_str()),
            LocationKind::Source => options.include_sources,
        })
        .collect()
}

fn add_location_to_index(
    index: &mut DefIndex,
    settings: &ProjectSettings,
    location: &RegisteredLocation,
    replacement: Option<&DefIndexReplacement<'_>>,
    on_file_indexed: Option<&dyn Fn()>,
) {
    let scan = match scan_indexable_def_xml_files(settings, location) {
        Ok(scan) => scan,
        Err(error) => {
            index.errors.push(index_error_for_location(
                location,
                None,
                "def_index_location_scan_failed",
                error.to_string(),
                None,
                None,
                crate::diagnostics::DiagnosticArgs::new(),
            ));
            return;
        }
    };

    // Non-fatal load-folder resolution and directory-walk diagnostics (malformed
    // `LoadFolders.xml`, missing referenced folder, an unreadable file/directory, ...), scoped
    // per content pack. One malformed Workshop item's `LoadFolders.xml` -- or one unwalkable
    // entry anywhere in the collection -- never aborts indexing for the rest of the collection:
    // the other items' folders were still resolved and scanned normally above/below, this only
    // records what went wrong for the affected item. `relative_path` is already fully resolved
    // relative to the location root (see `LoadFolderDiagnostic::relative_path`'s doc comment).
    for diagnostic in &scan.diagnostics {
        index.errors.push(index_error_for_location(
            location,
            diagnostic.relative_path.clone(),
            &diagnostic.code,
            diagnostic.message.clone(),
            None,
            None,
            diagnostic.args.clone(),
        ));
    }

    let source = indexed_source_for_location(location);
    let root = PathBuf::from(scan.root_path);
    let replacement_relative_path = replacement.map(|r| normalize_relative_path(r.relative_path));

    for file in scan.files {
        if let Some(cb) = on_file_indexed {
            cb();
        }
        if replacement.map(|r| r.location_id) == Some(location.id.as_str())
            && replacement_relative_path.as_deref() == Some(file.relative_path.as_str())
        {
            continue;
        }

        let path = root.join(Path::new(&file.relative_path));
        match std::fs::read_to_string(&path) {
            Ok(raw) => add_document_defs(index, &file.relative_path, &raw, &source),
            Err(error) => index.errors.push(index_error_for_location(
                location,
                Some(file.relative_path),
                "def_index_file_read_failed",
                error.to_string(),
                None,
                None,
                crate::diagnostics::DiagnosticArgs::new(),
            )),
        }
    }

    if let Some(replacement) = replacement {
        if replacement.location_id == location.id {
            add_document_defs(
                index,
                &normalize_relative_path(replacement.relative_path),
                replacement.source,
                &source,
            );
        }
    }
}

pub(crate) fn add_document_defs(
    index: &mut DefIndex,
    relative_path: &str,
    raw_xml: &str,
    source: &IndexedDefSource,
) {
    let doc = parse_to_document(relative_path, raw_xml);
    for diagnostic in &doc.parse_diagnostics {
        index.errors.push(DefIndexError {
            location_id: source.location_id.clone(),
            location_name: source.location_name.clone(),
            source_kind: source.source_kind.clone(),
            relative_path: Some(relative_path.to_string()),
            code: "def_index_parse_error".to_string(),
            message: diagnostic.message.clone(),
            line: diagnostic.line,
            column: diagnostic.column,
            args: diagnostic.args.clone(),
        });
    }
    if doc.had_fatal_parse_error {
        return;
    }

    for summary in &doc.def_summaries {
        let Some(def_name) = summary.def_name.as_deref().map(str::trim) else {
            continue;
        };
        if def_name.is_empty() {
            continue;
        }

        index.defs.push(IndexedDef {
            key: DefIdentityKey {
                def_type: summary.def_type.clone(),
                def_name: def_name.to_string(),
            },
            def_type: summary.def_type.clone(),
            def_name: def_name.to_string(),
            label: summary.label.clone(),
            parent_name: summary.parent_name.clone(),
            relative_path: relative_path.to_string(),
            node_id: Some(summary.node_id),
            line: summary.line,
            column: summary.column,
            source: source.clone(),
            fields: direct_child_fields(&doc, summary.node_id),
            def_name_lower: String::new(),
            label_lower: String::new(),
        });
    }
}

pub(crate) fn indexed_source_for_location(location: &RegisteredLocation) -> IndexedDefSource {
    IndexedDefSource {
        location_id: location.id.clone(),
        location_name: location.display_name.clone(),
        source_kind: IndexedSourceKind::from(&location.kind),
        source_type: location.source_type.clone(),
        read_only: location.read_only,
        mod_id: location.mod_id.clone(),
        game_version: location.game_version.clone(),
        expansion_name: location.expansion_name.clone(),
    }
}

fn direct_child_fields(doc: &XmlDocument, def_node_id: XmlNodeId) -> Vec<IndexedDefField> {
    let Some(def_node) = doc.nodes.get(def_node_id) else {
        return Vec::new();
    };

    def_node
        .children
        .iter()
        .filter_map(|&child_id| {
            let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
                return None;
            };
            Some(IndexedDefField {
                name: child_el.name.clone(),
                text_value: scalar_text(doc, child_id),
                line: Some(doc.nodes[child_id].span.line),
                column: Some(doc.nodes[child_id].span.column),
            })
        })
        .collect()
}

fn scalar_text(doc: &XmlDocument, node_id: XmlNodeId) -> Option<String> {
    let node = doc.nodes.get(node_id)?;
    let mut parts = Vec::new();
    for &child_id in &node.children {
        match &doc.nodes[child_id].kind {
            XmlNodeKind::Text(t) | XmlNodeKind::CData(t) => parts.push(t.value.as_str()),
            _ => {}
        }
    }
    let value = parts.join("").trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn index_error_for_location(
    location: &RegisteredLocation,
    relative_path: Option<String>,
    code: &str,
    message: String,
    line: Option<usize>,
    column: Option<usize>,
    args: crate::diagnostics::DiagnosticArgs,
) -> DefIndexError {
    DefIndexError {
        location_id: location.id.clone(),
        location_name: location.display_name.clone(),
        source_kind: IndexedSourceKind::from(&location.kind),
        relative_path,
        code: code.to_string(),
        message,
        line,
        column,
        args,
    }
}

pub(crate) fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub(crate) fn apply_file_change(
    index: &mut DefIndex,
    location: &RegisteredLocation,
    relative_path: &str,
    raw_xml: &str,
) {
    index.remove_file(&location.id, relative_path);
    let source = indexed_source_for_location(location);
    let normalized = normalize_relative_path(relative_path);
    add_document_defs(index, &normalized, raw_xml, &source);
    index.mark_rebuilt_now();
}
