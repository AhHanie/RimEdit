use std::collections::BTreeMap;

use crate::patches::{build_patch_index_with_files, PatchIndexBuildOptions, PatchOperationKey};
use crate::services::patch_preview::{compute_def_preview, PatchPreviewRequest, PreviewInputs};

use super::support::*;

#[test]
fn multiple_operations_targeting_same_def_report_distinctly_named_diagnostics() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><value>0</value></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>a wall</label></value></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationAttributeSet"><xpath>Defs/ThingDef[defName="Wall"]</xpath><attribute>Foo</attribute><value>1</value></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    let matches: Vec<_> = result
        .conflict_diagnostics
        .iter()
        .filter(|d| d.code == "patch_conflict_multiple_operations")
        .collect();
    assert_eq!(matches.len(), 2, "{:?}", result.conflict_diagnostics);
    // Each entry names its own operation's class, so the two diagnostics read as distinct
    // rather than identical, unexplained duplicates.
    assert!(
        matches[0].message != matches[1].message,
        "expected each conflicting operation's diagnostic to be individually identifiable: {:?}",
        matches
    );
    assert!(matches
        .iter()
        .any(|d| d.message.contains("PatchOperationAdd")));
    assert!(matches
        .iter()
        .any(|d| d.message.contains("PatchOperationAttributeSet")));
    assert!(matches.iter().all(|d| d.message.contains('2')));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn two_replace_operations_on_the_same_node_report_duplicate_conflict() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><value>0</value></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall"]/value</xpath><value><value>1</value></value></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationRemove"><xpath>Defs/ThingDef[defName="Wall"]/value</xpath></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    let matches: Vec<_> = result
        .conflict_diagnostics
        .iter()
        .filter(|d| d.code == "patch_conflict_duplicate_replace_or_remove")
        .collect();
    assert_eq!(matches.len(), 2, "{:?}", result.conflict_diagnostics);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn two_add_operations_adding_the_same_child_tag_report_duplicate_conflict() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>a wall</label></value></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>another wall</label></value></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    let matches: Vec<_> = result
        .conflict_diagnostics
        .iter()
        .filter(|d| d.code == "patch_conflict_duplicate_add_child")
        .collect();
    assert_eq!(matches.len(), 2, "{:?}", result.conflict_diagnostics);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn add_operations_at_different_xpaths_do_not_report_duplicate_conflict() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><stuffCategories/></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>a wall</label></value></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]/stuffCategories</xpath><value><li>Metallic</li></value></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    assert!(!result
        .conflict_diagnostics
        .iter()
        .any(|d| d.code == "patch_conflict_duplicate_add_child"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn later_operation_targeting_a_node_removed_earlier_reports_conflict() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><statBases><MoveSpeed>1</MoveSpeed></statBases></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationRemove"><xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall"]/statBases/MoveSpeed</xpath><value><MoveSpeed>2</MoveSpeed></value></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    assert!(
        result
            .conflict_diagnostics
            .iter()
            .any(|d| d.code == "patch_conflict_targets_removed_node"),
        "{:?}",
        result.conflict_diagnostics
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn custom_operation_affecting_selected_def_reports_unpreviewable_conflict() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Custom.xml",
        r#"<Patch><Operation Class="MyMod.PatchOperationFoo"><xpath>Defs/ThingDef[defName="Wall"]</xpath></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    assert!(
        result
            .conflict_diagnostics
            .iter()
            .any(|d| d.code == "patch_conflict_custom_operation_unpreviewable"),
        "{:?}",
        result.conflict_diagnostics
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn disabling_one_of_two_duplicate_replace_operations_clears_the_conflict() {
    // Regression (codex review, issue 10): `detect_visible_conflicts` originally computed
    // conflicts from the static visible-operations list, ignoring the request's own
    // disable/reorder state -- so disabling one side of a duplicate pair left the conflict
    // diagnostic reported even though only one operation is actually still running.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><value>0</value></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall"]/value</xpath><value><value>1</value></value></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationRemove"><xpath>Defs/ThingDef[defName="Wall"]/value</xpath></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let baseline =
        compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    assert!(baseline
        .conflict_diagnostics
        .iter()
        .any(|d| d.code == "patch_conflict_duplicate_replace_or_remove"));

    let disable_one = baseline.visible_operations[0].key.clone();
    let request = PatchPreviewRequest {
        disabled: vec![disable_one],
        order: vec![],
    };
    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &request);
    assert!(
        !result
            .conflict_diagnostics
            .iter()
            .any(|d| d.code == "patch_conflict_duplicate_replace_or_remove"),
        "disabling one side of the duplicate pair must clear the conflict: {:?}",
        result.conflict_diagnostics
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn two_add_operations_adding_different_li_items_to_the_same_list_are_not_a_conflict() {
    // Regression (codex review, issue 10): grouping solely by raw tag name treated two
    // ordinary `<li>` list-item additions to the same container as a "duplicate scalar child"
    // conflict, even though appending distinct items to a list across multiple patches is
    // completely normal.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><stuffCategories/></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]/stuffCategories</xpath><value><li>Metallic</li></value></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]/stuffCategories</xpath><value><li>Wooden</li></value></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    assert!(
        !result
            .conflict_diagnostics
            .iter()
            .any(|d| d.code == "patch_conflict_duplicate_add_child"),
        "{:?}",
        result.conflict_diagnostics
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn duplicate_add_detection_checks_every_value_child_not_only_the_first() {
    // Regression (codex review, issue 10): only reading the first top-level element of
    // `<value>` missed a duplicate on a later sibling field, e.g. one operation adds
    // `<description/><label/>` and another adds `<label/>` alone at the same xpath.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><description>a wall</description><label>wall</label></value></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>another wall</label></value></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    let matches: Vec<_> = result
        .conflict_diagnostics
        .iter()
        .filter(|d| d.code == "patch_conflict_duplicate_add_child")
        .collect();
    assert_eq!(matches.len(), 2, "{:?}", result.conflict_diagnostics);
    assert!(matches.iter().any(|d| d.message.contains("<label>")));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn reordering_a_remove_after_a_dependent_operation_removes_the_targets_removed_node_conflict() {
    // Regression (codex review, issue 10): the ordering check originally used static
    // file/operation order instead of the actual (possibly reordered) apply order, so
    // reordering the Remove to run *after* the operation that depends on its target could
    // still leave a stale "targets removed node" warning that no longer reflects reality.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><statBases><MoveSpeed>1</MoveSpeed></statBases></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/One.xml",
        r#"<Patch><Operation Class="PatchOperationRemove"><xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/Two.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall"]/statBases/MoveSpeed</xpath><value><MoveSpeed>2</MoveSpeed></value></Operation></Patch>"#,
    );

    let settings = settings_for(&root);
    let options = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_ops = BTreeMap::new();
    let (index, patch_files) = build_patch_index_with_files(&settings, options, &custom_ops);
    let inputs = PreviewInputs {
        settings: &settings,
        project_id: "proj",
        patch_index: &index,
        patch_files: &patch_files,
        custom_operations: &custom_ops,
    };

    let baseline =
        compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    assert!(baseline
        .conflict_diagnostics
        .iter()
        .any(|d| d.code == "patch_conflict_targets_removed_node"));

    // Reorder so the Replace runs *before* the Remove -- the Replace now succeeds against the
    // still-present node, and the Remove genuinely runs last, so nothing downstream targets
    // an already-removed node.
    let mut keys: Vec<PatchOperationKey> = baseline
        .visible_operations
        .iter()
        .map(|s| s.key.clone())
        .collect();
    keys.reverse();
    let request = PatchPreviewRequest {
        disabled: vec![],
        order: keys,
    };
    let reordered = compute_def_preview(&inputs, "ThingDef", "Wall", &request);
    assert!(
        !reordered
            .conflict_diagnostics
            .iter()
            .any(|d| d.code == "patch_conflict_targets_removed_node"),
        "{:?}",
        reordered.conflict_diagnostics
    );
    // The Replace ran first (against the still-present node) and the Remove now genuinely
    // runs last, taking the whole `statBases` (including the Replace's own effect) with it --
    // confirming the reorder actually changed real apply order, not just the diagnostic.
    assert!(!reordered.xml.as_ref().unwrap().contains("statBases"));

    std::fs::remove_dir_all(&root).ok();
}
