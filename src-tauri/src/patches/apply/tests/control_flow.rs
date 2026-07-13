//! `Sequence`, `Conditional`, and `FindMod` coverage: short-circuit + its diagnostic, both
//! `Conditional` branches, and every `FindMod` path (matched, `nomatch` fallback, and the
//! no-`nomatch`-branch dependency-skip case).

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

use super::support::{apply_one, combined_doc, element_text, key};

#[test]
fn sequence_stops_at_first_failed_child() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef><a/></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationSequence".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Sequence(vec![
            PatchOperationNode {
                id: 1,
                class_name: "PatchOperationRemove".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Remove(PathedOperation {
                    xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
                }),
                span: None,
            },
            PatchOperationNode {
                id: 2,
                class_name: "PatchOperationRemove".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Remove(PathedOperation {
                    xpath: Some("Defs/ThingDef/a".to_string()),
                }),
                span: None,
            },
        ]),
        span: None,
    };
    let result = apply_one(doc, &node);
    assert_eq!(result.trace.len(), 2); // sequence itself + first (failed) child only
    assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert!(
        first_child_element_named(thing_def, "a").is_some(),
        "second child must not run"
    );
}

#[test]
fn conditional_runs_match_when_xpath_matches() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef><a/></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationConditional".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Conditional {
            xpath: Some("Defs/ThingDef/a".to_string()),
            match_op: Some(Box::new(PatchOperationNode {
                id: 1,
                class_name: "PatchOperationRemove".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Remove(PathedOperation {
                    xpath: Some("Defs/ThingDef/a".to_string()),
                }),
                span: None,
            })),
            nomatch_op: None,
        },
        span: None,
    };
    apply_one(doc, &node);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert!(first_child_element_named(thing_def, "a").is_none());
}

#[test]
fn conditional_runs_nomatch_when_xpath_does_not_match() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationConditional".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Conditional {
            xpath: Some("Defs/ThingDef/missing".to_string()),
            match_op: None,
            nomatch_op: Some(Box::new(PatchOperationNode {
                id: 1,
                class_name: "PatchOperationAdd".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Add(PathedValueOrderOperation {
                    xpath: Some("Defs/ThingDef".to_string()),
                    value_xml: Some("<label>hi</label>".to_string()),
                    order: None,
                }),
                span: None,
            })),
        },
        span: None,
    };
    apply_one(doc, &node);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert!(first_child_element_named(thing_def, "label").is_some());
}

#[test]
fn find_mod_matches_by_display_name() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationFindMod".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::FindMod {
            mods: vec!["Harmony".to_string()],
            match_op: Some(Box::new(PatchOperationNode {
                id: 1,
                class_name: "PatchOperationAdd".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Add(PathedValueOrderOperation {
                    xpath: Some("Defs/ThingDef".to_string()),
                    value_xml: Some("<label>hi</label>".to_string()),
                    order: None,
                }),
                span: None,
            })),
            nomatch_op: None,
        },
        span: None,
    };
    let options = PatchApplyOptions {
        active_mod_names: &["Harmony".to_string()],
        custom_operations: &BTreeMap::new(),
        disabled: &HashSet::new(),
    };
    let ops = vec![TopLevelOperation {
        location_id: "loc".to_string(),
        relative_path: "Patches/Test.xml".to_string(),
        node: &node,
    }];
    apply_patch_operations(doc, &ops, &options);
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert!(first_child_element_named(thing_def, "label").is_some());
}

#[test]
fn find_mod_with_no_nomatch_branch_and_inactive_mod_reports_skip_reason() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationFindMod".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::FindMod {
            mods: vec!["Harmony".to_string()],
            match_op: Some(Box::new(PatchOperationNode {
                id: 1,
                class_name: "PatchOperationAdd".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Add(PathedValueOrderOperation {
                    xpath: Some("Defs/ThingDef".to_string()),
                    value_xml: Some("<label>hi</label>".to_string()),
                    order: None,
                }),
                span: None,
            })),
            nomatch_op: None,
        },
        span: None,
    };
    let options = PatchApplyOptions {
        active_mod_names: &[],
        custom_operations: &BTreeMap::new(),
        disabled: &HashSet::new(),
    };
    let ops = vec![TopLevelOperation {
        location_id: "loc".to_string(),
        relative_path: "Patches/Test.xml".to_string(),
        node: &node,
    }];
    let result = apply_patch_operations(doc, &ops, &options);

    // The wrapped Add never ran, so nothing changed in the document.
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert!(first_child_element_named(thing_def, "label").is_none());

    // Its own trace entry (keyed to the inner Add, not the FindMod wrapper) reports Skipped
    // with an explanatory message, rather than being entirely absent from the trace.
    let inner_key = key(1);
    let inner_trace = result
        .trace
        .iter()
        .find(|t| t.key == inner_key)
        .expect("expected a trace entry for the skipped inner operation");
    assert_eq!(inner_trace.status, OperationTraceStatus::Skipped);
    assert_eq!(
        inner_trace.message.as_deref(),
        Some("Requires mod \"Harmony\" to be active")
    );

    let diagnostic = result
        .diagnostics
        .iter()
        .find(|d| d.code == "patch_find_mod_dependency_not_active")
        .expect("expected a patch_find_mod_dependency_not_active diagnostic");
    assert_eq!(diagnostic.key, Some(inner_key));
    assert!(
        diagnostic.message.contains("\"Harmony\""),
        "{}",
        diagnostic.message
    );
}

#[test]
fn find_mod_with_nomatch_branch_is_normal_branching_not_a_skip() {
    let package = Package::new();
    let doc = package.as_document();
    let defs = combined_doc(doc, "<ThingDef></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationFindMod".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::FindMod {
            mods: vec!["Harmony".to_string()],
            match_op: Some(Box::new(PatchOperationNode {
                id: 1,
                class_name: "PatchOperationAdd".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Add(PathedValueOrderOperation {
                    xpath: Some("Defs/ThingDef".to_string()),
                    value_xml: Some("<label>hi</label>".to_string()),
                    order: None,
                }),
                span: None,
            })),
            nomatch_op: Some(Box::new(PatchOperationNode {
                id: 2,
                class_name: "PatchOperationAdd".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Add(PathedValueOrderOperation {
                    xpath: Some("Defs/ThingDef".to_string()),
                    value_xml: Some("<label>fallback</label>".to_string()),
                    order: None,
                }),
                span: None,
            })),
        },
        span: None,
    };
    let options = PatchApplyOptions {
        active_mod_names: &[],
        custom_operations: &BTreeMap::new(),
        disabled: &HashSet::new(),
    };
    let ops = vec![TopLevelOperation {
        location_id: "loc".to_string(),
        relative_path: "Patches/Test.xml".to_string(),
        node: &node,
    }];
    let result = apply_patch_operations(doc, &ops, &options);

    // A `nomatch` branch is normal, expected behavior -- it ran, so no "why didn't this
    // apply" diagnostic should fire.
    assert!(!result
        .diagnostics
        .iter()
        .any(|d| d.code == "patch_find_mod_dependency_not_active"));
    let thing_def = child_elements_named(defs, "ThingDef")[0];
    assert_eq!(
        first_child_element_named(thing_def, "label").map(element_text),
        Some(Some("fallback".to_string()))
    );
}

#[test]
fn sequence_short_circuit_reports_diagnostic_with_skipped_count() {
    let package = Package::new();
    let doc = package.as_document();
    combined_doc(doc, "<ThingDef><a/><b/></ThingDef>");
    let node = PatchOperationNode {
        id: 0,
        class_name: "PatchOperationSequence".to_string(),
        success: PatchSuccessMode::Normal,
        attributes: vec![],
        kind: PatchOperationKind::Sequence(vec![
            PatchOperationNode {
                id: 1,
                class_name: "PatchOperationRemove".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Remove(PathedOperation {
                    xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
                }),
                span: None,
            },
            PatchOperationNode {
                id: 2,
                class_name: "PatchOperationRemove".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Remove(PathedOperation {
                    xpath: Some("Defs/ThingDef/a".to_string()),
                }),
                span: None,
            },
            PatchOperationNode {
                id: 3,
                class_name: "PatchOperationRemove".to_string(),
                success: PatchSuccessMode::Normal,
                attributes: vec![],
                kind: PatchOperationKind::Remove(PathedOperation {
                    xpath: Some("Defs/ThingDef/b".to_string()),
                }),
                span: None,
            },
        ]),
        span: None,
    };
    let result = apply_one(doc, &node);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|d| d.code == "patch_apply_sequence_short_circuited")
        .expect("expected a sequence short-circuit diagnostic");
    assert!(
        diagnostic
            .message
            .contains("2 subsequent operations skipped"),
        "{}",
        diagnostic.message
    );
    assert_eq!(diagnostic.key, Some(key(0)));
}
