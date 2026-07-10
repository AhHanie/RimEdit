use std::collections::BTreeMap;

use crate::patches::{
    build_patch_index_with_files, OperationTraceStatus, PatchIndexBuildOptions, PatchOperationKey,
    XPathTarget,
};
use crate::project_model::LocationKind;
use crate::services::patch_preview::{compute_def_preview, PatchPreviewRequest, PreviewInputs};

use super::support::*;

#[test]
fn combines_defs_applies_patches_and_resolves_inheritance() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<?xml version="1.0" encoding="utf-8"?>
<Defs>
    <ThingDef Name="BaseThing" Abstract="True">
        <statBases><MoveSpeed>1</MoveSpeed></statBases>
    </ThingDef>
    <ThingDef ParentName="BaseThing">
        <defName>Wall</defName>
    </ThingDef>
</Defs>
"#,
    );
    write(
        &root,
        "Patches/AddLabel.xml",
        r#"<?xml version="1.0" encoding="utf-8"?>
<Patch>
    <Operation Class="PatchOperationAdd">
        <xpath>Defs/ThingDef[defName="Wall"]</xpath>
        <value>
            <label>a wall</label>
        </value>
    </Operation>
</Patch>
"#,
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

    assert!(result.def_found);
    let xml = result.xml.unwrap();
    assert!(xml.contains("<label>a wall</label>"), "{}", xml);
    assert!(xml.contains("<MoveSpeed>1</MoveSpeed>"), "{}", xml);
    assert!(!result.visible_operations.is_empty());
    assert_eq!(
        result.visible_operations[0].status,
        Some(OperationTraceStatus::Applied)
    );
    assert_eq!(
        result.visible_operations[0].xpath.as_deref(),
        Some(r#"Defs/ThingDef[defName="Wall"]"#)
    );
    assert!(matches!(
        result.visible_operations[0].target,
        XPathTarget::Def { .. }
    ));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn disabling_a_visible_operation_removes_its_effect() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/AddLabel.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>a wall</label></value></Operation></Patch>"#,
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
    let key = baseline.visible_operations[0].key.clone();

    let request = PatchPreviewRequest {
        disabled: vec![key],
        order: vec![],
    };
    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &request);
    let xml = result.xml.unwrap();
    assert!(!xml.contains("label"), "{}", xml);
    assert_eq!(
        result.visible_operations[0].status,
        Some(OperationTraceStatus::Skipped)
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn reordering_visible_operations_changes_final_result() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><value>0</value></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/SetOne.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall"]/value</xpath><value><value>1</value></value></Operation></Patch>"#,
    );
    write(
        &root,
        "Patches/SetTwo.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall"]/value</xpath><value><value>2</value></value></Operation></Patch>"#,
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
    // Default file order: SetOne then SetTwo -> final value is 2.
    assert!(baseline.xml.unwrap().contains("<value>2</value>"));

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
    let result = compute_def_preview(&inputs, "ThingDef", "Wall", &request);
    assert!(result.xml.unwrap().contains("<value>1</value>"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn preview_lists_only_operations_affecting_the_selected_def() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef><ThingDef><defName>Door</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch>
            <Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>wall</label></value></Operation>
            <Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Door"]</xpath><value><label>door</label></value></Operation>
        </Patch>"#,
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
    assert_eq!(result.visible_operations.len(), 1);
    assert!(result.xml.unwrap().contains("wall"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn deftype_wide_xpath_targeting_a_field_is_excluded_when_the_field_does_not_exist_on_this_def() {
    // Regression: `infer_xpath_target` conservatively classifies `Defs/ThingDef/label`
    // (a field-level path, no `defName` predicate) as `XPathTarget::DefType` -- the same
    // coarse "affects every ThingDef" bucket as a bare `Defs/ThingDef`. Trusting that bucket
    // unconditionally would show this Replace operation as affecting Wall even though Wall has
    // no `<label>` child for it to ever match, violating "patches that do not affect the
    // selected Def are not shown in the normal preview control list". Door genuinely has a
    // `<label>`, so the same operation must still show up in Door's preview.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs>
            <ThingDef><defName>Wall</defName></ThingDef>
            <ThingDef><defName>Door</defName><label>door</label></ThingDef>
        </Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef/label</xpath><value><label>a door</label></value></Operation></Patch>"#,
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

    let wall_result =
        compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    assert!(
        wall_result.visible_operations.is_empty(),
        "Wall has no <label> for this xpath to ever match: {:?}",
        wall_result.visible_operations
    );

    let door_result =
        compute_def_preview(&inputs, "ThingDef", "Door", &PatchPreviewRequest::default());
    assert_eq!(door_result.visible_operations.len(), 1);
    assert!(door_result.xml.unwrap().contains("a door"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn deftype_wide_xpath_with_no_field_segment_still_affects_every_instance() {
    // The bare (no child segment, no predicate) `Defs/ThingDef` case must remain intentionally
    // over-inclusive -- it genuinely affects every ThingDef instance, so the `xpath_touches_target`
    // re-verification added above must not exclude it for a Def with no other distinguishing
    // fields.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationAttributeSet"><xpath>Defs/ThingDef</xpath><attribute>Abstract</attribute><value>False</value></Operation></Patch>"#,
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
    assert_eq!(result.visible_operations.len(), 1);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn def_exact_xpath_targeting_a_field_is_excluded_when_the_field_does_not_exist_on_this_def() {
    // Same over-inclusion class as the `DefType` regressions above, but for an exact
    // `XPathTarget::Def` match: `infer_xpath_target` ignores segments after the
    // `defName="..."` predicate too, so `Defs/ThingDef[defName="Wall"]/label` classifies
    // identically to `Defs/ThingDef[defName="Wall"]` even though Wall has no `<label>` for the
    // Replace to ever match. The re-verification must apply to `Def`-exact matches, not just
    // `DefType`-wide ones.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall"]/label</xpath><value><label>a wall</label></value></Operation></Patch>"#,
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
        result.visible_operations.is_empty(),
        "Wall has no <label> for this xpath to ever match: {:?}",
        result.visible_operations
    );
    assert!(
        !result.xml.as_ref().unwrap().contains("label"),
        "the Replace has no node to match, so it must be a no-op: {:?}",
        result.xml
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn unsupported_custom_operation_reports_partial_and_diagnostic() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Custom.xml",
        r#"<Patch><Operation Class="Some.Unknown.PatchOperationFoo"><xpath>Defs/ThingDef[defName="Wall"]</xpath></Operation></Patch>"#,
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
    assert!(result.is_partial);
    assert!(result
        .apply_diagnostics
        .iter()
        .any(|d| d.code == "patch_apply_unsupported_operation"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn missing_def_reports_not_found_without_panicking() {
    let root = temp_project_dir();
    write(&root, "Defs/Things.xml", r#"<Defs></Defs>"#);
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
    let result = compute_def_preview(
        &inputs,
        "ThingDef",
        "DoesNotExist",
        &PatchPreviewRequest::default(),
    );
    assert!(!result.def_found);
    assert!(result.xml.is_none());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn abstract_def_with_no_defname_can_be_previewed_by_its_name_attribute() {
    // Abstract parent templates are never deserialized as real Defs and so have no
    // `<defName>` -- only a `Name` attribute. The frontend has no `nodeId`-based lookup
    // available in this independently-parsed preview document, so `matches_selected_def`'s
    // `Name`-attribute fallback is the only way issue 08's "preview by ... node identity"
    // requirement can be satisfied for such a Def.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs>
            <ThingDef Name="BaseThing" Abstract="True"><value>1</value></ThingDef>
            <ThingDef ParentName="BaseThing"><defName>Wall</defName></ThingDef>
        </Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[@Name="BaseThing"]/value</xpath><value><value>99</value></value></Operation></Patch>"#,
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

    let result = compute_def_preview(
        &inputs,
        "ThingDef",
        "BaseThing",
        &PatchPreviewRequest::default(),
    );
    assert!(result.def_found);
    assert!(
        result.xml.as_ref().unwrap().contains("<value>99</value>"),
        "{:?}",
        result.xml
    );
    assert_eq!(
        result.visible_operations.len(),
        1,
        "the patch directly targets BaseThing by @Name, so it must be visible when BaseThing \
         itself is the selected (Name-identified) Def"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn abstract_def_selected_by_name_does_not_pick_up_operations_targeting_an_unrelated_defname() {
    // Regression: an earlier version of the `Name`-attribute fallback passed the abstract
    // Def's `Name` value straight into `PatchImpactGraph::operations_affecting_def`, whose
    // `by_def` half is keyed by literal `defName="..."` predicate strings from patch XPaths --
    // unrelated to a `Name` attribute. An operation that targets a *different*, concrete Def
    // via `defName="BaseThing"` would then be wrongly treated as affecting the abstract
    // `Name="BaseThing"` template, even though it cannot match that node at all.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs>
            <ThingDef Name="BaseThing" Abstract="True"><value>1</value></ThingDef>
            <ThingDef><defName>BaseThing</defName><value>5</value></ThingDef>
        </Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="BaseThing"]/value</xpath><value><value>77</value></value></Operation></Patch>"#,
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

    // "BaseThing" resolves to the abstract Def first (it's declared first, and it has no
    // `defName` so only the `Name`-attribute fallback can match it) -- the patch targets the
    // *other*, concrete Def via a `defName="BaseThing"` predicate and must not leak in here.
    let result = compute_def_preview(
        &inputs,
        "ThingDef",
        "BaseThing",
        &PatchPreviewRequest::default(),
    );
    assert!(result.def_found);
    assert!(
        result.xml.as_ref().unwrap().contains("<value>1</value>"),
        "the abstract Def's own value must be untouched by a patch that targets the unrelated \
         concrete Def: {:?}",
        result.xml
    );
    assert!(
        result.visible_operations.is_empty(),
        "a defName-predicate patch targeting an unrelated concrete Def must not appear in the \
         abstract Def's preview just because its defName string matches the abstract Def's \
         Name attribute: {:?}",
        result.visible_operations
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn patch_on_abstract_parent_by_name_is_visible_and_disableable_for_the_child() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs>
            <ThingDef Name="BaseThing" Abstract="True"><value>1</value></ThingDef>
            <ThingDef ParentName="BaseThing"><defName>Wall</defName></ThingDef>
        </Defs>"#,
    );
    // Targets the abstract parent by `[@Name=...]`, not the child's own defName -- the
    // static impact graph classifies this as `XPathTarget::Unsupported` (it cannot resolve
    // `@Name`/`@ParentName` predicates to a Def/DefType), so only the runtime pre-patch
    // ancestor-chain correlation can surface it for `Wall`'s preview.
    write(
        &root,
        "Patches/PatchParent.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[@Name="BaseThing"]/value</xpath><value><value>99</value></value></Operation></Patch>"#,
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
    assert!(
        baseline.xml.as_ref().unwrap().contains("<value>99</value>"),
        "{:?}",
        baseline.xml
    );
    assert_eq!(
        baseline.visible_operations.len(),
        1,
        "expected the parent-targeting patch to be runtime-correlated to Wall's preview"
    );
    let key = baseline.visible_operations[0].key.clone();
    assert!(!baseline.visible_operations[0].can_reorder);
    assert!(
        matches!(
            baseline.visible_operations[0].target,
            XPathTarget::Unsupported
        ),
        "a runtime-correlated operation's static target must be Unsupported, so the UI can \
         show it as unknown-impact separately from statically-vouched-for operations"
    );

    // Disabling it (via the key surfaced above) must actually remove its effect.
    let request = PatchPreviewRequest {
        disabled: vec![key],
        order: vec![],
    };
    let disabled_result = compute_def_preview(&inputs, "ThingDef", "Wall", &request);
    assert!(disabled_result.xml.unwrap().contains("<value>1</value>"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn find_mod_dependency_not_registered_surfaces_status_message_on_the_row() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationFindMod"><mods><li>Power++</li></mods><match Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>hi</label></value></match></Operation></Patch>"#,
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
    assert!(!result.xml.as_ref().unwrap().contains("<label>"));
    assert_eq!(result.visible_operations.len(), 1);
    assert_eq!(
        result.visible_operations[0].status,
        Some(OperationTraceStatus::Skipped)
    );
    assert_eq!(
        result.visible_operations[0].status_message.as_deref(),
        Some("Requires mod \"Power++\" to be active")
    );
    assert!(result
        .apply_diagnostics
        .iter()
        .any(|d| d.code == "patch_find_mod_dependency_not_active"));

    // Once "Power++" is registered as a location (any kind -- `active_mod_names` matches by
    // display name regardless of Project/Source), the operation actually applies and both
    // the status and its explanatory message clear.
    let mut settings_with_mod = settings.clone();
    settings_with_mod
        .locations
        .push(location(&root, "Power++", LocationKind::Source));
    let options2 = PatchIndexBuildOptions {
        project_id: Some("proj"),
        include_sources: true,
        force_rebuild: false,
    };
    let (index2, patch_files2) =
        build_patch_index_with_files(&settings_with_mod, options2, &custom_ops);
    let inputs2 = PreviewInputs {
        settings: &settings_with_mod,
        project_id: "proj",
        patch_index: &index2,
        patch_files: &patch_files2,
        custom_operations: &custom_ops,
    };
    let result2 = compute_def_preview(
        &inputs2,
        "ThingDef",
        "Wall",
        &PatchPreviewRequest::default(),
    );
    assert!(result2.xml.as_ref().unwrap().contains("<label>hi</label>"));
    assert_eq!(
        result2.visible_operations[0].status,
        Some(OperationTraceStatus::Applied)
    );
    assert_eq!(result2.visible_operations[0].status_message, None);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn or_chained_def_name_patch_is_a_normal_reorderable_operation_for_each_named_def() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs>
            <ThingDef><defName>A</defName><value>0</value></ThingDef>
            <ThingDef><defName>B</defName><value>0</value></ThingDef>
            <ThingDef><defName>C</defName><value>0</value></ThingDef>
        </Defs>"#,
    );
    // An OR-chain of defName equalities is statically resolvable (see
    // `patches::impact_graph::parse_def_name_or_chain`), so this must show up as a normal,
    // reorderable operation for A and B, not as runtime-correlated "unknown impact" -- and
    // must not affect C at all.
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="A" or defName="B"]/value</xpath><value><value>1</value></value></Operation></Patch>"#,
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

    for def_name in ["A", "B"] {
        let result = compute_def_preview(
            &inputs,
            "ThingDef",
            def_name,
            &PatchPreviewRequest::default(),
        );
        assert!(result.xml.as_ref().unwrap().contains("<value>1</value>"));
        assert_eq!(
            result.visible_operations.len(),
            1,
            "expected the OR-chained patch to statically resolve for {def_name}"
        );
        assert!(
            result.visible_operations[0].can_reorder,
            "a statically-vouched-for operation must be reorder-eligible, unlike a \
             runtime-correlated one"
        );
        assert!(
            matches!(
                result.visible_operations[0].target,
                XPathTarget::Defs { .. }
            ),
            "expected an OR-chained defName match to classify as XPathTarget::Defs, got {:?}",
            result.visible_operations[0].target
        );
    }

    let unaffected = compute_def_preview(&inputs, "ThingDef", "C", &PatchPreviewRequest::default());
    assert!(unaffected
        .xml
        .as_ref()
        .unwrap()
        .contains("<value>0</value>"));
    assert!(unaffected.visible_operations.is_empty());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn complex_xpath_directly_matching_an_unnamed_def_is_still_runtime_correlated() {
    let root = temp_project_dir();
    // `Wall` has neither `Name` nor `ParentName` -- `pre_patch_ancestor_names` returns an
    // empty set for it, which must not skip the direct-identity check in
    // `xpath_touches_target` (a compound predicate like this is beyond
    // `infer_xpath_target`'s conservative subset, so it's `XPathTarget::Unsupported`, not
    // `Def`/`DefType`).
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName><label>wall</label><value>0</value></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall" and label="wall"]/value</xpath><value><value>1</value></value></Operation></Patch>"#,
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
    assert!(baseline.xml.as_ref().unwrap().contains("<value>1</value>"));
    assert_eq!(
        baseline.visible_operations.len(),
        1,
        "compound predicate targeting Wall directly must still be runtime-correlated"
    );

    let key = baseline.visible_operations[0].key.clone();
    let request = PatchPreviewRequest {
        disabled: vec![key],
        order: vec![],
    };
    let disabled_result = compute_def_preview(&inputs, "ThingDef", "Wall", &request);
    assert!(disabled_result.xml.unwrap().contains("<value>0</value>"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn disable_request_cannot_affect_operations_outside_the_selected_def() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs>
            <ThingDef><defName>Wall</defName></ThingDef>
            <ThingDef><defName>Door</defName><value>0</value></ThingDef>
        </Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Door"]/value</xpath><value><value>1</value></value></Operation></Patch>"#,
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

    // The only patch operation in the project affects Door, not Wall -- so it must not be in
    // Wall's visible list, and a (stale/crafted) request naming it while previewing Wall must
    // not be honored.
    let door_result =
        compute_def_preview(&inputs, "ThingDef", "Door", &PatchPreviewRequest::default());
    let door_key = door_result.visible_operations[0].key.clone();
    assert_eq!(
        door_result
            .operation_trace
            .iter()
            .find(|t| t.key == door_key)
            .unwrap()
            .status,
        OperationTraceStatus::Applied
    );

    let wall_baseline =
        compute_def_preview(&inputs, "ThingDef", "Wall", &PatchPreviewRequest::default());
    assert!(wall_baseline.visible_operations.is_empty());

    let request = PatchPreviewRequest {
        disabled: vec![door_key.clone()],
        order: vec![],
    };
    let wall_with_bogus_disable = compute_def_preview(&inputs, "ThingDef", "Wall", &request);
    // Door's operation is not visible for Wall's preview, so the request naming it must be
    // ignored: the full document still applies it (Applied, not Skipped) rather than
    // silently disabling an operation outside the previewed Def's scope.
    assert_eq!(
        wall_with_bogus_disable
            .operation_trace
            .iter()
            .find(|t| t.key == door_key)
            .unwrap()
            .status,
        OperationTraceStatus::Applied
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn result_includes_operation_trace_and_impact_summary() {
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Wall"]</xpath><value><label>a wall</label></value></Operation></Patch>"#,
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
    assert!(!result.operation_trace.is_empty());
    assert_eq!(result.impact_summary.visible_operation_count, 1);
    assert_eq!(result.impact_summary.reorderable_operation_count, 1);
    assert_eq!(result.impact_summary.conflict_count, 0);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn xpath_valid_but_unsupported_for_autocomplete_is_still_evaluated_by_preview() {
    // `patches::xpath`'s autocomplete/target-inference boundary reports a multi-predicate
    // xpath like this one as `xpath_autocomplete_unsupported_pattern` (see
    // `patches::tests::xpath::valid_xpath_outside_conservative_subset_is_unsupported_with_diagnostic`),
    // and `patches::impact_graph::infer_xpath_target` classifies it as
    // `XPathTarget::Unsupported` for the same reason -- but the Plan's "XPath evaluation
    // support and XPath autocomplete support are separate" boundary promises preview can
    // still evaluate it via the real XML library regardless. This proves that promise
    // end-to-end: the operation still applies successfully even though it is unsupported for
    // static target inference.
    let root = temp_project_dir();
    write(
        &root,
        "Defs/Things.xml",
        r#"<Defs><ThingDef Name="Foo"><defName>Wall</defName><value>0</value></ThingDef></Defs>"#,
    );
    write(
        &root,
        "Patches/Patch.xml",
        r#"<Patch><Operation Class="PatchOperationReplace"><xpath>Defs/ThingDef[defName="Wall"][@Name="Foo"]/value</xpath><value><value>1</value></value></Operation></Patch>"#,
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
        result.xml.as_ref().unwrap().contains("<value>1</value>"),
        "a multi-predicate xpath is unsupported for static target inference, but must still \
         be evaluated for real by the preview's XML library: {:?}",
        result.xml
    );

    std::fs::remove_dir_all(&root).ok();
}
