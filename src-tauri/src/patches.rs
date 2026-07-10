// Re-export facade: all public items from patches submodules live here. Some re-exports are
// only referenced by name in test code, by Tauri commands not yet written for later
// patches-editor issues, or appear only in function-signature types, so they look unused to
// `cargo check` outside of `--tests` builds. Allow the lints for the entire facade rather than
// annotating every line (mirrors `def_index::mod`'s facade).
#![allow(dead_code, unused_imports)]

pub(crate) mod apply;
pub(crate) mod cache;
pub(crate) mod custom_metadata;
pub(crate) mod dom;
pub(crate) mod fingerprint;
pub(crate) mod impact_graph;
pub(crate) mod index;
pub(crate) mod inheritance;
pub(crate) mod model;
mod parser;
mod scan;
mod serializer;
pub(crate) mod state;
pub(crate) mod value_xml;
pub(crate) mod xpath;

#[cfg(test)]
mod tests;

pub use apply::{
    apply_patch_operations, ApplyDiagnostic, ApplyDiagnosticSeverity, OperationTraceEntry,
    OperationTraceStatus, PatchApplyOptions, PatchApplyResult, PatchOperationKey,
    TopLevelOperation,
};
pub use cache::{
    cache_state_inputs, load_or_rebuild_patch_index, rebuild_and_store_patch_index,
    PatchIndexCacheError,
};
pub use custom_metadata::{
    format_custom_operation_attributes, lookup_custom_operation_metadata,
    serialize_custom_operation_fields, CustomFieldValue, SerializedCustomOperationFields,
};
pub(crate) use fingerprint::settings_fingerprint;
pub use fingerprint::IndexedPatchFileFingerprint;
pub use impact_graph::{
    infer_xpath_target, target_for_operation, PatchImpactGraph, PatchImpactRef, XPathTarget,
};
pub use index::{
    build_patch_index, build_patch_index_with_files, summarize_patch_index,
    CustomOperationMetadataMap, IndexedPatchOperation, PatchIndex, PatchIndexBuildOptions,
    PatchIndexError, PatchIndexFile, PatchIndexSummary, PatchOperationClassification,
    PatchPreviewSupport,
};
pub use inheritance::{
    resolve_inheritance, InheritanceDiagnostic, InheritanceDiagnosticSeverity,
    InheritanceResolution,
};
pub use model::{
    AttributeOperation, AttributeValueOperation, PatchDiagnostic, PatchFile, PatchOperationId,
    PatchOperationKind, PatchOperationNode, PatchOrderMode, PatchSpan, PatchSuccessMode,
    PathedOperation, PathedValueOperation, PathedValueOrderOperation, SetNameOperation,
    UnknownPatchOperation, XmlAttributeModel, BUILT_IN_OPERATION_CLASSES,
};
pub use parser::parse_patch_file;
pub(crate) use scan::scan_indexable_patch_xml_files;
pub use serializer::serialize_patch_file;
pub use state::{PatchFilesState, PatchIndexState};
pub use value_xml::{parse_value_fragment, serialize_initial_elements};
pub use xpath::{
    complete_patch_xpath, XPathCompletionItem, XPathCompletionItemKind, XPathCompletionResult,
    XPathDiagnostic, XPathDiagnosticSeverity, XPathResolvedField,
};
