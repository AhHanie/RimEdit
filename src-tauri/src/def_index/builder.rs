use crate::project_files::scan_indexable_def_xml_files;
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation};
use crate::xml_document::model::{XmlDocument, XmlNodeId, XmlNodeKind};
use crate::xml_document::parse_to_document;
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

pub fn build_def_index(settings: &ProjectSettings, options: DefIndexBuildOptions<'_>) -> DefIndex {
    let mut index = DefIndex {
        defs: Vec::new(),
        errors: Vec::new(),
        built_at_unix_ms: OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000,
        by_type: Default::default(),
    };

    for location in included_locations(settings, &options) {
        add_location_to_index(&mut index, settings, location, options.replacement.as_ref());
    }

    index.rebuild_computed_fields();
    index
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
            ));
            return;
        }
    };

    let source = indexed_source_for_location(location);
    let root = PathBuf::from(scan.root_path);
    let replacement_relative_path = replacement.map(|r| normalize_relative_path(r.relative_path));

    for file in scan.files {
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
