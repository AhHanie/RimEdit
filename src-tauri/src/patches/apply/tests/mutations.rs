//! Add/Insert/Remove/Replace/AddModExtension/SetName coverage: append vs. prepend ordering, the
//! `PatchOperationInsert` reversal quirk, sibling-splice ordering for Replace, and SetName's
//! attribute-dropping rename.

use sxd_document::dom::ChildOfElement;
use sxd_document::Package;

use crate::patches::apply::OperationTraceStatus;
use crate::patches::dom::{child_elements_named, first_child_element_named};
use crate::patches::model::{
    PatchOperationKind, PatchOperationNode, PatchOrderMode, PatchSuccessMode, PathedOperation,
    PathedValueOperation, PathedValueOrderOperation, SetNameOperation,
};

use super::support::{apply_one, combined_doc, element_text};

#[test]
fn add_appends_by_default() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef><defName>Wall</defName></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationAdd".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Add(PathedValueOrderOperation {
            xpath: Some("Defs/ThingDef".to_string()),
            value_xml: Some("<label>wall</label>".to_string()),
            order: None,
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert!(!result.is_partial);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert_eq!(
        first_child_element_named(thing_def, "label").map(|e| element_text(e)),
        Some(Some("wall".to_string()))
    );
}

#[test]
fn add_prepend_preserves_value_order() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef><a>existing</a></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationAdd".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Add(PathedValueOrderOperation {
            xpath: Some("Defs/ThingDef".to_string()),
            value_xml: Some("<b>1</b><c>2</c>".to_string()),
            order: Some(PatchOrderMode::Prepend),
        }),
        span: None,
    };
    apply_one(doc, &node);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    let names: Vec<String> = thing_def
        .children()
        .into_iter()
        .filter_map(|c| match c {
            ChildOfElement::Element(e) => Some(e.name().local_part().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["b", "c", "a"]);
}

#[test]
fn insert_reverses_multiple_value_children() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef><target /></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationInsert".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Insert(PathedValueOrderOperation {
            xpath: Some("Defs/ThingDef/target".to_string()),
            value_xml: Some("<a/><b/>".to_string()),
            order: None,
        }),
        span: None,
    };
    apply_one(doc, &node);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    let names: Vec<String> = thing_def
        .children()
        .into_iter()
        .filter_map(|c| match c {
            ChildOfElement::Element(e) => Some(e.name().local_part().to_string()),
            _ => None,
        })
        .collect();
    // Default order is Prepend (insert before target); RimWorld's real reversal quirk means
    // the two value children land as b, a (not a, b) immediately before `target`.
    assert_eq!(names, vec!["b", "a", "target"]);
}

#[test]
fn replace_preserves_value_order() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef><target /></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationReplace".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Replace(PathedValueOperation {
            xpath: Some("Defs/ThingDef/target".to_string()),
            value_xml: Some("<a/><b/>".to_string()),
        }),
        span: None,
    };
    apply_one(doc, &node);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    let names: Vec<String> = thing_def
        .children()
        .into_iter()
        .filter_map(|c| match c {
            ChildOfElement::Element(e) => Some(e.name().local_part().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn remove_deletes_matched_node() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef><target /></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationRemove".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Remove(PathedOperation {
            xpath: Some("Defs/ThingDef/target".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert!(first_child_element_named(thing_def, "target").is_none());
}

#[test]
fn add_mod_extension_creates_container_once() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationAddModExtension".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::AddModExtension(PathedValueOperation {
            xpath: Some("Defs/ThingDef".to_string()),
            value_xml: Some("<li Class=\"Foo\"/>".to_string()),
        }),
        span: None,
    };
    apply_one(doc, &node);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    let mod_ext = first_child_element_named(thing_def, "modExtensions").unwrap();
    assert_eq!(child_elements_named(mod_ext, "li").len(), 1);
}

#[test]
fn set_name_renames_and_drops_attributes() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef Name=\"Old\"><a>1</a></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationSetName".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::SetName(SetNameOperation {
            xpath: Some("Defs/ThingDef".to_string()),
            name: Some("Renamed".to_string()),
        }),
        span: None,
    };
    apply_one(doc, &node);
    assert!(child_elements_named(defs, "ThingDef").is_empty());
    let renamed = child_elements_named(defs, "Renamed");
    assert_eq!(renamed.len(), 1);
    assert_eq!(renamed[0].attribute_value("Name"), None);
    assert_eq!(
        first_child_element_named(renamed[0], "a").map(element_text),
        Some(Some("1".to_string()))
    );
}
