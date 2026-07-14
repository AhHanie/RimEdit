//! Shared fixtures and helpers for `patches::apply`'s test modules: a combined `<Defs>` document
//! builder, key/options builders, a single-operation `apply_patch_operations` runner, and an
//! `element_text` re-export so test assertions don't need to reach into `patches::dom` directly.

use std::collections::{BTreeMap, HashSet};

use sxd_document::dom::{Document, Element};

use crate::patches::apply::{
    apply_patch_operations, PatchApplyOptions, PatchApplyResult, PatchOperationKey,
    TopLevelOperation,
};
use crate::patches::dom::parse_fragment;
use crate::patches::model::{PatchOperationId, PatchOperationNode};
use crate::patches::CustomOperationMetadataMap;

pub(super) fn combined_doc<'d>(document: Document<'d>, defs_xml: &str) -> Element<'d> {
    let defs = document.create_element("Defs");
    document.root().append_child(defs);
    let result = parse_fragment(document, defs_xml);
    assert!(!result.had_fatal_error, "{:?}", result.diagnostics);
    defs.append_children(result.nodes);
    defs
}

pub(super) fn key(id: PatchOperationId) -> PatchOperationKey {
    PatchOperationKey {
        location_id: "loc".to_string(),
        relative_path: "Patches/Test.xml".to_string(),
        operation_id: id,
    }
}

pub(super) fn no_op_options() -> (
    Vec<String>,
    CustomOperationMetadataMap,
    HashSet<PatchOperationKey>,
) {
    (Vec::new(), BTreeMap::new(), HashSet::new())
}

pub(super) fn apply_one<'d>(document: Document<'d>, node: &PatchOperationNode) -> PatchApplyResult {
    let (active_mods, custom_ops, disabled) = no_op_options();
    let options = PatchApplyOptions {
        active_mod_names: &active_mods,
        custom_operations: &custom_ops,
        disabled: &disabled,
    };
    let ops = vec![TopLevelOperation {
        location_id: "loc".to_string(),
        relative_path: "Patches/Test.xml".to_string(),
        node,
    }];
    apply_patch_operations(document, &ops, &options)
}

pub(super) fn element_text(el: Element<'_>) -> Option<String> {
    crate::patches::dom::element_text(el)
}
