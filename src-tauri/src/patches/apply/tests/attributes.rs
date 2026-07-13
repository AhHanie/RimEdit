//! AttributeAdd/Set/Remove coverage: `AttributeAdd`'s existing-value skip, `AttributeSet`'s
//! unconditional overwrite/create, and `AttributeRemove`'s presence-dependent success.

use sxd_document::Package;

use crate::patches::apply::OperationTraceStatus;
use crate::patches::dom::child_elements_named;
use crate::patches::model::{
    AttributeOperation, AttributeValueOperation, PatchOperationKind, PatchOperationNode,
    PatchSuccessMode,
};

use super::support::{apply_one, combined_doc};

#[test]
fn attribute_add_skips_existing_attribute() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef Foo=\"1\"></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationAttributeAdd".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::AttributeAdd(AttributeValueOperation {
            xpath: Some("Defs/ThingDef".to_string()),
            attribute: Some("Foo".to_string()),
            value: Some("2".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert_eq!(thing_def.attribute_value("Foo"), Some("1"));
}

#[test]
fn attribute_set_overwrites_or_creates() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef Foo=\"1\"></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationAttributeSet".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::AttributeSet(AttributeValueOperation {
            xpath: Some("Defs/ThingDef".to_string()),
            attribute: Some("Foo".to_string()),
            value: Some("2".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert_eq!(thing_def.attribute_value("Foo"), Some("2"));
}

#[test]
fn attribute_remove_only_succeeds_if_present() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationAttributeRemove".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::AttributeRemove(AttributeOperation {
            xpath: Some("Defs/ThingDef".to_string()),
            attribute: Some("Foo".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
    let _ = defs;
}
