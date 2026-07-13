//! Cross-cutting outcome coverage that isn't specific to one mutation or container: the
//! `success` attribute's Always/Never/Invert modes, disabled-operation and unsupported-operation
//! handling, and the XPath no-match vs. XPath-error diagnostic distinction.

use std::collections::{BTreeMap, HashSet};

use sxd_document::Package;

use crate::patches::apply::{
    apply_patch_operations, OperationTraceStatus, PatchApplyOptions, TopLevelOperation,
};
use crate::patches::dom::{child_elements_named, first_child_element_named};
use crate::patches::model::{
    PatchOperationKind, PatchOperationNode, PatchSuccessMode, PathedOperation,
    PathedValueOrderOperation,
};
use crate::patches::parser::parse_patch_file;

use super::support::{apply_one, combined_doc, key};

#[test]
fn success_mode_always_forces_success() {
    let package = Package::new();
    let doc = package.as_document();
    combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationRemove".to_string(),
        success: PatchSuccessMode::Always,
        attributes: vec![],
        kind: PatchOperationKind::Remove(PathedOperation {
            xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
}

#[test]
fn success_mode_never_forces_failure() {
    let package = Package::new();
    let doc = package.as_document();
    combined_doc(doc, "<ThingDef><target/></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationRemove".to_string(),
        success: PatchSuccessMode::Never,
        attributes: vec![],
        kind: PatchOperationKind::Remove(PathedOperation {
            xpath: Some("Defs/ThingDef/target".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
}

#[test]
fn success_mode_invert_flips_result() {
    let package = Package::new();
    let doc = package.as_document();
    combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationRemove".to_string(),
        success: PatchSuccessMode::Invert,
        attributes: vec![],
        kind: PatchOperationKind::Remove(PathedOperation {
            xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    // Raw result is false (no match); Invert flips it to true.
    assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
}

#[test]
fn disabled_operation_is_skipped_and_treated_as_success() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef><a/></ThingDef>");
    let node = PatchOperationNode {
        id: 5,
        class_name: "PatchOperationRemove".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Remove(PathedOperation {
            xpath: Some("Defs/ThingDef/a".to_string()),
        }),
        span: None,
    };
    let mut disabled = HashSet::new();
    disabled.insert(key(5));
    let options = PatchApplyOptions {
        active_mod_names: &[],
        custom_operations: &BTreeMap::new(),
        disabled: &disabled,
    };
    let ops = vec![TopLevelOperation {
        location_id: "loc".to_string(),
        relative_path: "Patches/Test.xml".to_string(),
        node: &node,
    }];
    let result = apply_patch_operations(doc, &ops, &options);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Skipped);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert!(first_child_element_named(thing_def, "a").is_some());
}

#[test]
fn unsupported_custom_operation_marks_partial() {
    let package = Package::new();
    let doc = package.as_document();
    combined_doc(doc, "<ThingDef></ThingDef>");
    let raw = "<Operation Class=\"MyMod.PatchOperationFoo\"></Operation>";
    let file = parse_patch_file("Patches/Custom.xml", &format!("<Patch>{}</Patch>", raw));
    let node = &file.operations[0];
    let result = apply_one(doc, node);
    assert!(result.is_partial);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Unsupported);
}

#[test]
fn xpath_no_match_reports_diagnostic_distinct_from_xpath_error() {
    let package = Package::new();
    let doc = package.as_document();
    combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationAdd".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Add(PathedValueOrderOperation {
            xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
            value_xml: Some("<label>hi</label>".to_string()),
            order: None,
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "patch_apply_xpath_no_match"));
    assert!(!result
        .diagnostics
        .iter()
        .any(|d| d.code == "patch_apply_xpath_error"));
}

#[test]
fn success_always_masking_a_real_failure_reports_diagnostic() {
    let package = Package::new();
    let doc = package.as_document();
    combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationRemove".to_string(),
        success: PatchSuccessMode::Always,
        attributes: vec![],
        kind: PatchOperationKind::Remove(PathedOperation {
            xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "patch_apply_success_always_masks_failure"));
}

#[test]
fn success_always_does_not_report_masking_diagnostic_when_operation_really_succeeds() {
    let package = Package::new();
    let doc = package.as_document();
    combined_doc(doc, "<ThingDef><target/></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationRemove".to_string(),
        success: PatchSuccessMode::Always,
        attributes: vec![],
        kind: PatchOperationKind::Remove(PathedOperation {
            xpath: Some("Defs/ThingDef/target".to_string()),
        }),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
    assert!(!result
        .diagnostics
        .iter()
        .any(|d| d.code == "patch_apply_success_always_masks_failure"));
}
