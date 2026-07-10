use std::collections::BTreeMap;
use std::fs;

use crate::patches::{
    build_patch_index, PatchIndexBuildOptions, PatchOperationClassification, PatchPreviewSupport,
};
use crate::project_model::LocationKind;
use crate::schema_pack::{
    PatchOperationMetadata, PatchOperationPreview, PatchOperationPreviewKind,
};

use super::{location, settings_with_locations, temp_dir};

const SIMPLE_ADD: &str = r#"<Patch>
  <Operation Class="PatchOperationAdd">
    <xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath>
    <value>
      <MaxHitPoints>300</MaxHitPoints>
    </value>
  </Operation>
</Patch>"#;

#[test]
fn indexes_operations_from_a_single_file() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(root.join("Patches").join("a.xml"), SIMPLE_ADD).unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");

    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    assert!(index.errors.is_empty(), "errors: {:?}", index.errors);
    assert_eq!(index.files.len(), 1);
    let file = &index.files[0];
    assert_eq!(file.relative_path, "Patches/a.xml");
    assert_eq!(file.operations.len(), 1);
    let op = &file.operations[0];
    assert_eq!(op.tree_path, "0");
    assert_eq!(op.class_name, "PatchOperationAdd");
    assert_eq!(op.classification, PatchOperationClassification::BuiltIn);
    assert_eq!(op.preview_support, PatchPreviewSupport::Supported);
    assert_eq!(
        op.xpath.as_deref(),
        Some(r#"Defs/ThingDef[defName="Wall"]/statBases"#)
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn parse_error_in_one_file_does_not_abort_the_full_index() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(root.join("Patches").join("good.xml"), SIMPLE_ADD).unwrap();
    fs::write(
        root.join("Patches").join("bad.xml"),
        "<Patch><Operation Class=\"PatchOperationAdd\">",
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");

    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    assert!(
        index.errors.is_empty(),
        "unexpected scan errors: {:?}",
        index.errors
    );
    assert_eq!(index.files.len(), 2);
    let good = index
        .files
        .iter()
        .find(|f| f.relative_path == "Patches/good.xml")
        .unwrap();
    assert_eq!(good.operations.len(), 1);
    assert!(!good.had_fatal_parse_error);
    let bad = index
        .files
        .iter()
        .find(|f| f.relative_path == "Patches/bad.xml")
        .unwrap();
    assert!(bad.had_fatal_parse_error);
    assert!(!bad.diagnostics.is_empty());
    assert!(bad.operations.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn indexes_nested_sequence_and_conditional_operations_with_tree_paths() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="PatchOperationSequence">
            <operations>
              <li Class="PatchOperationAdd">
                <xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath>
                <value><MaxHitPoints>300</MaxHitPoints></value>
              </li>
            </operations>
          </Operation>
          <Operation Class="PatchOperationConditional">
            <xpath>Defs/ThingDef[defName="Wall"]</xpath>
            <match Class="PatchOperationRemove">
              <xpath>Defs/ThingDef[defName="Wall"]/statBases</xpath>
            </match>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");

    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    assert!(index.errors.is_empty(), "errors: {:?}", index.errors);
    let file = &index.files[0];
    let tree_paths: Vec<&str> = file
        .operations
        .iter()
        .map(|op| op.tree_path.as_str())
        .collect();
    assert_eq!(tree_paths, vec!["0", "0.sequence[0]", "1", "1.match"]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn unknown_operation_class_is_marked_unsupported() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="MyMod.PatchOperationCustom">
            <xpath>Defs/ThingDef[defName="Wall"]</xpath>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");

    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    let op = &index.files[0].operations[0];
    assert_eq!(op.classification, PatchOperationClassification::Unknown);
    assert!(matches!(
        op.preview_support,
        PatchPreviewSupport::Unsupported { .. }
    ));
    // Even though the operation's *class* is unknown, its `<xpath>` child is still the common
    // pathed-operation shape and should be extracted, not silently dropped as `NoXPath`.
    assert_eq!(
        op.xpath.as_deref(),
        Some(r#"Defs/ThingDef[defName="Wall"]"#)
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn operation_class_known_via_metadata_is_marked_custom() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(
        root.join("Patches").join("a.xml"),
        r#"<Patch>
          <Operation Class="MyMod.PatchOperationCustom">
            <xpath>Defs/ThingDef[defName="Wall"]</xpath>
          </Operation>
        </Patch>"#,
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc], "project");

    let mut custom_operations = BTreeMap::new();
    custom_operations.insert(
        "MyMod.PatchOperationCustom".to_string(),
        PatchOperationMetadata {
            class_name: "MyMod.PatchOperationCustom".to_string(),
            label: Some("Custom".to_string()),
            description: None,
            field_order: vec!["xpath".to_string()],
            fields: BTreeMap::new(),
            preview: PatchOperationPreview {
                kind: PatchOperationPreviewKind::Unsupported,
                message: Some("no preview for this custom operation".to_string()),
            },
            source_pack_id: Some("test.pack".to_string()),
        },
    );

    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &custom_operations,
    );

    let op = &index.files[0].operations[0];
    assert_eq!(op.classification, PatchOperationClassification::Custom);
    assert_eq!(
        op.preview_support,
        PatchPreviewSupport::Unsupported {
            reason: "no preview for this custom operation".to_string()
        }
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn preserves_location_folder_and_file_order_across_locations() {
    let project_root = temp_dir();
    let source_root = temp_dir();
    fs::create_dir(project_root.join("Patches")).unwrap();
    fs::create_dir(source_root.join("Patches")).unwrap();
    fs::write(
        project_root.join("Patches").join("p.xml"),
        "<Patch></Patch>",
    )
    .unwrap();
    fs::write(source_root.join("Patches").join("s.xml"), "<Patch></Patch>").unwrap();
    let project_loc = location(&project_root, "project", LocationKind::Project);
    let source_loc = location(&source_root, "source", LocationKind::Source);
    let settings = settings_with_locations(vec![project_loc, source_loc], "project");

    let index = build_patch_index(
        &settings,
        PatchIndexBuildOptions::for_project("project"),
        &BTreeMap::new(),
    );

    // Registered location order (project before source) must be preserved in `file_order`.
    assert_eq!(index.files.len(), 2);
    assert_eq!(index.files[0].relative_path, "Patches/p.xml");
    assert_eq!(index.files[0].file_order, 0);
    assert_eq!(index.files[1].relative_path, "Patches/s.xml");
    assert_eq!(index.files[1].file_order, 1);
    fs::remove_dir_all(&project_root).ok();
    fs::remove_dir_all(&source_root).ok();
}
