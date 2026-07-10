//! Builds an index of patch files and operations from project and source locations, reusing
//! `patches::scan` for file discovery and `patches::parser` for parsing each file.
//!
//! The index preserves the stable default preview order documented in
//! `docs/patches-editor/02-patch-file-scanning-and-indexing.md`: registered location order,
//! then resolved load folder order, then patch file order, then operation order within a file.
//! Location order and folder/file order are already correct by construction (`build_patch_index`
//! iterates `settings.locations` in order and appends each location's already-ordered file scan),
//! so no additional sorting is applied here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::def_index::{indexed_source_for_location, IndexedDefSource, IndexedSourceKind};
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation};
use crate::schema_pack::{PatchOperationMetadata, PatchOperationPreviewKind};

use super::impact_graph::{infer_xpath_target, XPathTarget};
use super::model::{
    PatchDiagnostic, PatchFile, PatchOperationId, PatchOperationKind, PatchOperationNode,
    PatchSuccessMode,
};
use super::parser::parse_patch_file;
use super::scan::scan_indexable_patch_xml_files;

/// Patch operation metadata keyed by `className`, used to classify non-built-in operation
/// classes as `Custom` (known via metadata) vs. `Unknown` (not recognized at all). Empty means
/// "no custom operation metadata available" -- every non-built-in class is then `Unknown`.
pub type CustomOperationMetadataMap = BTreeMap<String, PatchOperationMetadata>;

pub struct PatchIndexBuildOptions<'a> {
    pub project_id: Option<&'a str>,
    pub include_sources: bool,
    pub force_rebuild: bool,
}

impl<'a> PatchIndexBuildOptions<'a> {
    #[cfg(test)]
    pub fn for_project(project_id: &'a str) -> Self {
        Self {
            project_id: Some(project_id),
            include_sources: true,
            force_rebuild: false,
        }
    }
}

/// Whether a `class_name` is one of RimEdit's understood built-in operation classes, a class
/// recognized via schema-pack-defined patch operation metadata (see `patches::custom_metadata`
/// and `docs/patches-editor/03-custom-operation-metadata.md`), or an unrecognized class. A
/// non-built-in class is `Custom` when a `PatchOperationMetadata` entry exists for its
/// `class_name` in the schema catalog passed to `build_patch_index`, and `Unknown` otherwise,
/// matching the AST's own `PatchOperationKind::Unknown` (see `patches::model`'s implementation
/// notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatchOperationClassification {
    BuiltIn,
    Custom,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PatchPreviewSupport {
    Supported,
    Unsupported { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedPatchOperation {
    pub id: PatchOperationId,
    /// Structural position within the file's operation tree, e.g. `"0"` for the first
    /// top-level operation, `"0.sequence[1]"` for the second child of a
    /// `PatchOperationSequence`, or `"2.match"` for a `PatchOperationConditional`'s `match`.
    pub tree_path: String,
    pub class_name: String,
    pub classification: PatchOperationClassification,
    pub xpath: Option<String>,
    pub target: XPathTarget,
    pub preview_support: PatchPreviewSupport,
    pub success: PatchSuccessMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchIndexFile {
    pub source: IndexedDefSource,
    pub relative_path: String,
    /// Position of this file within the overall stable preview order (location order, then
    /// load folder order, then file order).
    pub file_order: usize,
    pub xml_declaration: Option<String>,
    pub diagnostics: Vec<PatchDiagnostic>,
    pub had_fatal_parse_error: bool,
    pub operations: Vec<IndexedPatchOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchIndexError {
    pub location_id: String,
    pub location_name: String,
    pub source_kind: IndexedSourceKind,
    pub relative_path: Option<String>,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchIndex {
    pub files: Vec<PatchIndexFile>,
    pub errors: Vec<PatchIndexError>,
    pub built_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchIndexSummary {
    pub indexed_files: usize,
    pub indexed_operations: usize,
    pub project_files: usize,
    pub source_files: usize,
    pub errors: usize,
    pub built_at_unix_ms: i64,
}

pub fn build_patch_index(
    settings: &ProjectSettings,
    options: PatchIndexBuildOptions<'_>,
    custom_operations: &CustomOperationMetadataMap,
) -> PatchIndex {
    build_patch_index_with_files(settings, options, custom_operations).0
}

/// Like [`build_patch_index`], but also returns the full parsed [`PatchFile`] AST for each
/// indexed file (same order, 1:1 by position with `PatchIndex.files`). The index alone only
/// retains enough per-operation data for classification/target-inference (see
/// `IndexedPatchOperation`); applying operations for real (preview) needs the full AST -- value
/// XML, attribute name/value, order mode, nested operation trees -- which this returns alongside
/// without re-deriving the classification logic in a second place.
pub fn build_patch_index_with_files(
    settings: &ProjectSettings,
    options: PatchIndexBuildOptions<'_>,
    custom_operations: &CustomOperationMetadataMap,
) -> (PatchIndex, Vec<PatchFile>) {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let mut raw_files = Vec::new();

    for location in included_locations(settings, &options) {
        add_location_to_index(
            &mut files,
            &mut errors,
            &mut raw_files,
            settings,
            location,
            custom_operations,
        );
    }

    (
        PatchIndex {
            files,
            errors,
            built_at_unix_ms: now_unix_ms(),
        },
        raw_files,
    )
}

pub fn summarize_patch_index(index: &PatchIndex) -> PatchIndexSummary {
    PatchIndexSummary {
        indexed_files: index.files.len(),
        indexed_operations: index.files.iter().map(|f| f.operations.len()).sum(),
        project_files: index
            .files
            .iter()
            .filter(|f| f.source.source_kind == IndexedSourceKind::Project)
            .count(),
        source_files: index
            .files
            .iter()
            .filter(|f| f.source.source_kind == IndexedSourceKind::Source)
            .count(),
        errors: index.errors.len(),
        built_at_unix_ms: index.built_at_unix_ms,
    }
}

pub(super) fn included_locations<'a>(
    settings: &'a ProjectSettings,
    options: &PatchIndexBuildOptions<'_>,
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
    files: &mut Vec<PatchIndexFile>,
    errors: &mut Vec<PatchIndexError>,
    raw_files: &mut Vec<PatchFile>,
    settings: &ProjectSettings,
    location: &RegisteredLocation,
    custom_operations: &CustomOperationMetadataMap,
) {
    let scan = match scan_indexable_patch_xml_files(settings, location) {
        Ok(scan) => scan,
        Err(error) => {
            errors.push(PatchIndexError {
                location_id: location.id.clone(),
                location_name: location.display_name.clone(),
                source_kind: IndexedSourceKind::from(&location.kind),
                relative_path: None,
                code: "patch_index_location_scan_failed".to_string(),
                message: error.to_string(),
            });
            return;
        }
    };

    let source = indexed_source_for_location(location);
    let root = PathBuf::from(scan.root_path);

    for file_entry in scan.files {
        let path = root.join(Path::new(&file_entry.relative_path));
        match std::fs::read_to_string(&path) {
            Ok(raw) => {
                let file_order = files.len();
                let patch_file = parse_patch_file(&file_entry.relative_path, &raw);
                raw_files.push(patch_file.clone());
                files.push(build_indexed_patch_file(
                    patch_file,
                    source.clone(),
                    file_order,
                    custom_operations,
                ));
            }
            Err(error) => errors.push(PatchIndexError {
                location_id: location.id.clone(),
                location_name: location.display_name.clone(),
                source_kind: IndexedSourceKind::from(&location.kind),
                relative_path: Some(file_entry.relative_path),
                code: "patch_index_file_read_failed".to_string(),
                message: error.to_string(),
            }),
        }
    }
}

fn build_indexed_patch_file(
    patch_file: PatchFile,
    source: IndexedDefSource,
    file_order: usize,
    custom_operations: &CustomOperationMetadataMap,
) -> PatchIndexFile {
    let mut operations = Vec::new();
    for (i, op) in patch_file.operations.iter().enumerate() {
        index_operation(op, i.to_string(), custom_operations, &mut operations);
    }

    PatchIndexFile {
        source,
        relative_path: patch_file.relative_path,
        file_order,
        xml_declaration: patch_file.xml_declaration,
        diagnostics: patch_file.diagnostics,
        had_fatal_parse_error: patch_file.had_fatal_parse_error,
        operations,
    }
}

fn index_operation(
    node: &PatchOperationNode,
    tree_path: String,
    custom_operations: &CustomOperationMetadataMap,
    out: &mut Vec<IndexedPatchOperation>,
) {
    let xpath = extract_xpath(&node.kind);
    let target = xpath
        .as_deref()
        .map(infer_xpath_target)
        .unwrap_or(XPathTarget::NoXPath);
    let custom_metadata = if node.is_known_class() {
        None
    } else {
        custom_operations.get(&node.class_name)
    };
    let classification = if node.is_known_class() {
        PatchOperationClassification::BuiltIn
    } else if custom_metadata.is_some() {
        PatchOperationClassification::Custom
    } else {
        PatchOperationClassification::Unknown
    };
    let preview_support = match classification {
        PatchOperationClassification::BuiltIn => PatchPreviewSupport::Supported,
        PatchOperationClassification::Custom => {
            let metadata = custom_metadata.expect("classification is Custom only when Some");
            match metadata.preview.kind {
                PatchOperationPreviewKind::Unsupported => PatchPreviewSupport::Unsupported {
                    reason: metadata.preview.message.clone().unwrap_or_else(|| {
                        format!(
                            "'{}' is a custom operation without declared preview support",
                            node.class_name
                        )
                    }),
                },
            }
        }
        PatchOperationClassification::Unknown => PatchPreviewSupport::Unsupported {
            reason: format!(
                "'{}' is not a recognized built-in patch operation class",
                node.class_name
            ),
        },
    };

    out.push(IndexedPatchOperation {
        id: node.id,
        tree_path: tree_path.clone(),
        class_name: node.class_name.clone(),
        classification,
        xpath,
        target,
        preview_support,
        success: node.success,
    });

    match &node.kind {
        PatchOperationKind::Sequence(children) => {
            for (i, child) in children.iter().enumerate() {
                index_operation(
                    child,
                    format!("{}.sequence[{}]", tree_path, i),
                    custom_operations,
                    out,
                );
            }
        }
        PatchOperationKind::FindMod {
            match_op,
            nomatch_op,
            ..
        }
        | PatchOperationKind::Conditional {
            match_op,
            nomatch_op,
            ..
        } => {
            if let Some(m) = match_op {
                index_operation(m, format!("{}.match", tree_path), custom_operations, out);
            }
            if let Some(nm) = nomatch_op {
                index_operation(nm, format!("{}.nomatch", tree_path), custom_operations, out);
            }
        }
        _ => {}
    }
}

fn extract_xpath(kind: &PatchOperationKind) -> Option<String> {
    match kind {
        PatchOperationKind::Add(op) | PatchOperationKind::Insert(op) => op.xpath.clone(),
        PatchOperationKind::Remove(op) | PatchOperationKind::Test(op) => op.xpath.clone(),
        PatchOperationKind::Replace(op) | PatchOperationKind::AddModExtension(op) => {
            op.xpath.clone()
        }
        PatchOperationKind::AttributeAdd(op) | PatchOperationKind::AttributeSet(op) => {
            op.xpath.clone()
        }
        PatchOperationKind::AttributeRemove(op) => op.xpath.clone(),
        PatchOperationKind::SetName(op) => op.xpath.clone(),
        PatchOperationKind::Conditional { xpath, .. } => xpath.clone(),
        PatchOperationKind::Unknown(op) => extract_xpath_from_unknown_raw_xml(&op.raw_xml),
        PatchOperationKind::Sequence(_) | PatchOperationKind::FindMod { .. } => None,
    }
}

/// Unknown/custom operations only preserve their raw XML span (see `patches::model`'s
/// implementation notes), not parsed fields. Many custom operations still extend
/// `PatchOperationPathed` and declare a top-level `<xpath>` child in the same shape as built-in
/// pathed operations, so extract it when present rather than always reporting `NoXPath` --
/// otherwise a custom operation's target could never be indexed, even as `Unsupported`.
fn extract_xpath_from_unknown_raw_xml(raw_xml: &str) -> Option<String> {
    let doc = crate::xml_document::parse_to_document("_unknown_patch_operation_.xml", raw_xml);
    if doc.had_fatal_parse_error {
        return None;
    }
    let root_id = doc.top_level_nodes.iter().copied().find(|&id| {
        matches!(
            doc.nodes[id].kind,
            crate::xml_document::model::XmlNodeKind::Element(_)
        )
    })?;
    let xpath_id = super::parser::child_element_named(&doc, root_id, "xpath")?;
    Some(super::parser::element_text(&doc, xpath_id))
}

fn now_unix_ms() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000
}
