//! Whitespace-formatted xpath equivalence: RimEdit does not preprocess xpath text before handing
//! it to `sxd_xpath` (see `patches::dom::select_nodes`), so a readable, indented xpath -- the
//! shape `PatchPathInput`'s multiline editor is expected to produce -- must select exactly the
//! same node(s) as its compact equivalent.

use sxd_document::Package;

use crate::patches::apply::OperationTraceStatus;
use crate::patches::dom::child_elements_named;
use crate::patches::model::{
    PatchOperationKind, PatchOperationNode, PatchSuccessMode, PathedValueOrderOperation,
};

use super::support::{apply_one, combined_doc};

fn add_op(xpath: &str) -> PatchOperationNode {
    PatchOperationNode {
        id: 0,
        class_name: "PatchOperationAdd".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Add(PathedValueOrderOperation {
            xpath: Some(xpath.to_string()),
            value_xml: Some("<label>wall</label>".to_string()),
            order: None,
        }),
        span: None,
    }
}

#[test]
fn whitespace_after_slash_and_in_predicate_matches_compact_equivalent() {
    let compact = r#"Defs/ThingDef[defName="Wall"]/statBases"#;
    let multiline = "Defs/\n  ThingDef[\n    defName = \"Wall\"\n  ]/\n  statBases";

    for xpath in [compact, multiline] {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(
            doc,
            "<ThingDef><defName>Wall</defName><statBases><MoveSpeed>1</MoveSpeed></statBases></ThingDef>",
        );
        let node = add_op(xpath);
        let result = apply_one(doc, &node);
        assert!(!result.is_partial, "{xpath:?}: {:?}", result.trace);
        assert_eq!(
            result.trace[0].status,
            OperationTraceStatus::Applied,
            "{xpath:?}: {:?}",
            result.trace
        );
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        let stat_bases = child_elements_named(thing_def, "statBases")[0];
        let labels = child_elements_named(stat_bases, "label");
        assert_eq!(labels.len(), 1, "{xpath:?}: expected <label> to be added");
    }
}
