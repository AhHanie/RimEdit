use super::loader::{
    assemble_schema_pack, load_built_in_packs, load_external_packs, load_pack_from_directory,
    parse_def_type_schema, parse_object_type_schema, parse_patch_operation_metadata,
    parse_schema_pack_manifest, LoadedPack,
};
use super::lookup::{
    collect_object_fields_ordered, lookup_def_type, lookup_field, lookup_object_field,
    lookup_object_field_with_alias, lookup_object_type,
};
use super::merge::merge_packs;
use super::model::{
    DefTypeSchemaFile, FieldTypeKind, ObjectTypeSchemaFile, PatchOperationMetadataFile,
    SchemaLoadSeverity,
};
use crate::def_index::DefIndex;
use crate::xml_document::{parse_to_document, validate_document, ValidationContext};
use std::path::Path;

mod build_file_collection;
mod build_path_safety;
mod form_views;
mod locale;
mod locale_catalog_sync;
mod patch_operations;
mod schema_mechanics;

/// Build a `LoadedPack` from an inline manifest and a set of inline patch operation metadata
/// files, without touching disk. Mirrors `inline_pack_with_objects` but for patch operations.
fn inline_pack_with_patch_operations(manifest_json: &str, patch_op_jsons: &[&str]) -> LoadedPack {
    let (manifest_opt, mdiags) = parse_schema_pack_manifest("test:manifest", manifest_json);
    assert!(
        manifest_opt.is_some(),
        "inline manifest failed: {:?}",
        mdiags
    );
    let manifest_file = manifest_opt.unwrap();
    let pack_id = manifest_file.pack_id.clone();
    let mut patch_op_files: Vec<(&str, PatchOperationMetadataFile)> = Vec::new();
    for (i, raw) in patch_op_jsons.iter().enumerate() {
        let label = Box::leak(format!("test:patch_op_{i}.json").into_boxed_str()) as &str;
        let (op_opt, pdiags) = parse_patch_operation_metadata(label, &pack_id, raw);
        assert!(
            op_opt.is_some(),
            "inline patch operation failed: {:?}",
            pdiags
        );
        patch_op_files.push((label, op_opt.unwrap()));
    }
    let patch_op_refs: Vec<(&str, &PatchOperationMetadataFile)> =
        patch_op_files.iter().map(|(l, o)| (*l, o)).collect();
    let (pack_opt, adiags) =
        assemble_schema_pack("test:pack", manifest_file, &[], &[], &patch_op_refs);
    let errors: Vec<_> = adiags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "inline pack errors: {:?}", errors);
    LoadedPack {
        manifest: pack_opt.expect("assemble must succeed"),
        is_builtin: false,
        source_path: None,
        locales: Default::default(),
    }
}

fn load_fixture(name: &str) -> LoadedPack {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack")
        .join(name)
        .join("schema-pack.json");
    let (pack_opt, diags) = load_pack_from_directory(&path);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "fixture '{}' had errors: {:?}",
        name,
        errors
    );
    pack_opt.expect("fixture must load")
}

fn inline_pack(manifest_json: &str, def_json: &str) -> LoadedPack {
    let (manifest_opt, mdiags) = parse_schema_pack_manifest("test:manifest", manifest_json);
    assert!(
        manifest_opt.is_some(),
        "inline manifest failed: {:?}",
        mdiags
    );
    let manifest_file = manifest_opt.unwrap();
    let pack_id = manifest_file.pack_id.clone();
    let (def_opt, ddiags) =
        parse_def_type_schema("test:def", &pack_id, def_json, manifest_file.format_version);
    assert!(def_opt.is_some(), "inline def failed: {:?}", ddiags);
    let def_file = def_opt.unwrap();
    let def_refs = vec![("test:def", &def_file)];
    let (pack_opt, adiags) = assemble_schema_pack("test:pack", manifest_file, &def_refs, &[], &[]);
    let errors: Vec<_> = adiags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "inline pack errors: {:?}", errors);
    LoadedPack {
        manifest: pack_opt.expect("assemble must succeed"),
        is_builtin: false,
        source_path: None,
        locales: Default::default(),
    }
}

fn inline_pack_with_objects(manifest_json: &str, def_json: &str, obj_jsons: &[&str]) -> LoadedPack {
    let (manifest_opt, mdiags) = parse_schema_pack_manifest("test:manifest", manifest_json);
    assert!(
        manifest_opt.is_some(),
        "inline manifest failed: {:?}",
        mdiags
    );
    let manifest_file = manifest_opt.unwrap();
    let pack_id = manifest_file.pack_id.clone();
    let (def_opt, ddiags) =
        parse_def_type_schema("test:def", &pack_id, def_json, manifest_file.format_version);
    assert!(def_opt.is_some(), "inline def failed: {:?}", ddiags);
    let def_file = def_opt.unwrap();
    let mut obj_files: Vec<(&str, ObjectTypeSchemaFile)> = Vec::new();
    for (i, raw) in obj_jsons.iter().enumerate() {
        let label = Box::leak(format!("test:obj_{i}.json").into_boxed_str()) as &str;
        let (obj_opt, odiags) = parse_object_type_schema(label, &pack_id, raw);
        assert!(obj_opt.is_some(), "inline obj failed: {:?}", odiags);
        obj_files.push((label, obj_opt.unwrap()));
    }
    let def_refs = vec![("test:def", &def_file)];
    let obj_refs: Vec<(&str, &ObjectTypeSchemaFile)> =
        obj_files.iter().map(|(l, o)| (*l, o)).collect();
    let (pack_opt, adiags) =
        assemble_schema_pack("test:pack", manifest_file, &def_refs, &obj_refs, &[]);
    let errors: Vec<_> = adiags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "inline pack errors: {:?}", errors);
    LoadedPack {
        manifest: pack_opt.expect("assemble must succeed"),
        is_builtin: false,
        source_path: None,
        locales: Default::default(),
    }
}

// --- 1. Valid pack deserializes ---

#[test]
fn valid_minimal_pack_deserializes() {
    let pack = load_fixture("valid_minimal");
    assert_eq!(pack.manifest.format_version, 1);
    assert_eq!(pack.manifest.pack_id, "test.minimal");

    let thing_def = pack
        .manifest
        .def_types
        .get("ThingDef")
        .expect("ThingDef missing");
    assert_eq!(thing_def.fields.len(), 2);

    let def_name_field = thing_def.fields.get("defName").expect("defName missing");
    assert_eq!(def_name_field.field_type.kind, FieldTypeKind::String);
    assert_eq!(def_name_field.required, Some(true));

    let stack_field = thing_def
        .fields
        .get("stackLimit")
        .expect("stackLimit missing");
    assert_eq!(stack_field.field_type.kind, FieldTypeKind::Integer);
}

// --- 2. Merge precedence ---

#[test]
fn higher_priority_pack_wins_scalars_and_examples_deduplicate() {
    let base_pack = load_fixture("valid_minimal");
    let override_pack = load_fixture("override_pack");

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![base_pack, override_pack], &mut diags);

    let thing_def = catalog
        .def_types
        .get("ThingDef")
        .expect("ThingDef missing after merge");

    assert_eq!(
        thing_def.label.as_deref(),
        Some("Thing (overridden)"),
        "override label should win"
    );
    assert!(
        thing_def.description.is_some(),
        "override description should be set"
    );

    let def_name = thing_def.fields.get("defName").expect("defName missing");
    assert!(
        def_name.examples.contains(&"Steel".to_string()),
        "Steel should be present"
    );
    assert!(
        def_name.examples.contains(&"ExtraExample".to_string()),
        "ExtraExample should be present"
    );
    assert_eq!(
        def_name.examples.iter().filter(|e| *e == "Steel").count(),
        1,
        "Steel should not be duplicated"
    );
}

// --- 3. Invalid manifests return structured diagnostics ---

#[test]
fn malformed_manifest_json_returns_diagnostic() {
    let (manifest_opt, diags) = parse_schema_pack_manifest("test:bad", "{ not valid json }");
    assert!(manifest_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_manifest_json_invalid"),
        "expected schema_pack_manifest_json_invalid"
    );
}

#[test]
fn unsupported_format_version_returns_diagnostic() {
    let json = r#"{ "formatVersion": 99, "packId": "test.v99", "name": "V99", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let (manifest_opt, diags) = parse_schema_pack_manifest("test:v99", json);
    assert!(manifest_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_manifest_format_unsupported"),
        "expected schema_pack_manifest_format_unsupported"
    );
}

#[test]
fn empty_pack_id_returns_diagnostic() {
    let json = r#"{ "formatVersion": 1, "packId": "  ", "name": "No ID", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let (manifest_opt, diags) = parse_schema_pack_manifest("test:noid", json);
    assert!(manifest_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_missing_pack_id"),
        "expected schema_pack_missing_pack_id"
    );
}

#[test]
fn missing_def_type_directories_returns_diagnostic() {
    let json = r#"{ "formatVersion": 1, "packId": "test.nodirs", "name": "No Dirs", "version": "1.0.0", "defTypeDirectories": [] }"#;
    let (manifest_opt, diags) = parse_schema_pack_manifest("test:nodirs", json);
    assert!(manifest_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_def_type_directory_missing"),
        "expected schema_pack_def_type_directory_missing"
    );
}

#[test]
fn absent_def_type_directories_field_returns_directory_diagnostic_not_json_error() {
    // When defTypeDirectories is completely absent (old monolithic format), the loader
    // should emit schema_pack_def_type_directory_missing, not a generic JSON error.
    let json =
        r#"{ "formatVersion": 1, "packId": "test.monolithic", "name": "Old", "version": "1.0.0" }"#;
    let (manifest_opt, diags) = parse_schema_pack_manifest("test:old", json);
    assert!(manifest_opt.is_none());
    assert!(
        diags.iter().any(|d| d.code == "schema_pack_def_type_directory_missing"),
        "absent defTypeDirectories should produce schema_pack_def_type_directory_missing, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "schema_pack_manifest_json_invalid"),
        "should not produce a JSON error when field is simply absent"
    );
}

#[test]
fn absolute_def_type_directory_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    // Absolute path in defTypeDirectories - should be rejected silently (dir treated as
    // missing/escaped), not read from the absolute filesystem location.
    #[cfg(windows)]
    let abs_entry = "C:\\Windows\\System32";
    #[cfg(not(windows))]
    let abs_entry = "/etc";

    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.absdir",
        "name": "Abs Dir",
        "version": "1.0.0",
        "defTypeDirectories": [abs_entry]
    });
    let manifest_path = tmp.path().join("schema-pack.json");
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&manifest_path);
    // Pack still assembles (zero def types loaded).
    assert!(
        pack_opt.is_some(),
        "pack should assemble even when dir is rejected"
    );
    // Must not silently read from the absolute path - expect an escape/path error.
    assert!(
        diags.iter().any(|d| d.code == "schema_pack_def_type_directory_escape"),
        "absolute defTypeDirectories entry should produce schema_pack_def_type_directory_escape, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 4. Invalid def files return structured diagnostics ---

#[test]
fn malformed_def_file_returns_diagnostic() {
    let (def_opt, diags) = parse_def_type_schema("test:bad.json", "test.pack", "{ not json }", 1);
    assert!(def_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_def_type_json_invalid"),
        "expected schema_pack_def_type_json_invalid"
    );
}

#[test]
fn missing_def_type_field_returns_diagnostic() {
    let json = r#"{ "fields": {} }"#;
    let (def_opt, diags) = parse_def_type_schema("test:nodeftype.json", "test.pack", json, 1);
    assert!(def_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_missing_def_type"),
        "expected schema_pack_missing_def_type"
    );
}

#[test]
fn unknown_field_type_kind_in_def_file_returns_warning() {
    let json = r#"{
        "defType": "ThingDef",
        "fields": {
            "weirdField": {
                "type": { "kind": "superMadeUpType" },
                "required": false
            }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:badtype.json", "test.pack", json, 1);
    assert!(
        def_opt.is_some(),
        "def file should load despite unknown type"
    );

    let warning = diags
        .iter()
        .find(|d| d.code == "schema_pack_invalid_field_type");
    assert!(
        warning.is_some(),
        "expected schema_pack_invalid_field_type warning"
    );

    let def_file = def_opt.unwrap();
    let field = def_file
        .schema
        .fields
        .get("weirdField")
        .expect("weirdField missing");
    assert_eq!(field.field_type.kind, FieldTypeKind::Unknown);
}

// --- 5. Duplicate def type in one pack ---

#[test]
fn duplicate_def_type_in_pack_produces_diagnostic() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.dup", "name": "Dup", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let (manifest_opt, _) = parse_schema_pack_manifest("test:manifest", manifest_json);
    let manifest_file = manifest_opt.unwrap();

    let def_json = r#"{ "defType": "ThingDef", "fields": {} }"#;
    let (def1_opt, _) = parse_def_type_schema("test:a.json", "test.dup", def_json, 1);
    let (def2_opt, _) = parse_def_type_schema("test:b.json", "test.dup", def_json, 1);
    let def1 = def1_opt.unwrap();
    let def2 = def2_opt.unwrap();

    let def_refs: Vec<(&str, &DefTypeSchemaFile)> =
        vec![("test:a.json", &def1), ("test:b.json", &def2)];
    let (pack_opt, diags) = assemble_schema_pack("test:pack", manifest_file, &def_refs, &[], &[]);

    assert!(
        pack_opt.is_some(),
        "pack should assemble even with duplicate (first wins)"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_duplicate_def_type"),
        "expected schema_pack_duplicate_def_type diagnostic"
    );
}

// --- 6. Field lookup traverses inherits ---

#[test]
fn lookup_field_traverses_inherits_chain() {
    let pack = load_fixture("valid_minimal");
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let field = lookup_field(&catalog, "ThingDef", "defName");
    assert!(field.is_some(), "defName should be found on ThingDef");

    let missing = lookup_field(&catalog, "ThingDef", "nonExistentField");
    assert!(missing.is_none());
}

#[test]
fn lookup_field_walks_inherits_to_parent() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.inherit", "name": "Inherit", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let (manifest_opt, _) = parse_schema_pack_manifest("test:manifest", manifest_json);
    let manifest_file = manifest_opt.unwrap();

    let def_json = r#"{ "defType": "Def", "abstractType": true, "fields": { "defName": { "type": { "kind": "string" }, "required": true } } }"#;
    let thing_json = r#"{ "defType": "ThingDef", "inherits": ["Def"], "fields": {} }"#;
    let (def_opt, _) = parse_def_type_schema("test:Def.json", "test.inherit", def_json, 1);
    let (thing_opt, _) = parse_def_type_schema("test:ThingDef.json", "test.inherit", thing_json, 1);
    let def_file = def_opt.unwrap();
    let thing_file = thing_opt.unwrap();

    let def_refs = vec![
        ("test:Def.json", &def_file),
        ("test:ThingDef.json", &thing_file),
    ];
    let (pack_opt, _) = assemble_schema_pack("test:pack", manifest_file, &def_refs, &[], &[]);
    let mut diags = Vec::new();
    let catalog = merge_packs(
        vec![LoadedPack {
            manifest: pack_opt.unwrap(),
            is_builtin: false,
            source_path: None,
            locales: Default::default(),
        }],
        &mut diags,
    );

    let field = lookup_field(&catalog, "ThingDef", "defName");
    assert!(
        field.is_some(),
        "defName should be found on ThingDef via Def inheritance"
    );
}

#[test]
fn lookup_def_type_returns_correct_schema() {
    let pack = load_fixture("valid_minimal");
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let dt = lookup_def_type(&catalog, "ThingDef");
    assert!(dt.is_some());

    let missing = lookup_def_type(&catalog, "NonExistent");
    assert!(missing.is_none());
}

// --- 7. Duplicate packId ---

#[test]
fn duplicate_pack_id_produces_diagnostic_and_first_wins() {
    let p1 = load_fixture("valid_minimal");
    let p2 = load_fixture("valid_minimal");

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![p1, p2], &mut diags);

    let dup_diag = diags
        .iter()
        .find(|d| d.code == "schema_pack_duplicate_pack_id");
    assert!(dup_diag.is_some(), "expected duplicate_pack_id diagnostic");
    assert_eq!(catalog.packs.len(), 1);
}

// --- 8. Built-in pack loads ---

#[test]
fn built_in_pack_loads_without_errors() {
    let (packs, diags) = load_built_in_packs();
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "built-in pack has errors: {:?}", errors);
    assert!(!packs.is_empty(), "expected at least one built-in pack");

    let mut merge_diags = Vec::new();
    let _catalog = merge_packs(packs, &mut merge_diags);
    let merge_errors: Vec<_> = merge_diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(
        merge_errors.is_empty(),
        "built-in pack merge has errors: {:?}",
        merge_errors
    );
}

// --- 9. fieldOrder deserialization and merge ---

#[test]
fn field_order_deserializes_from_pack() {
    let pack = load_fixture("valid_minimal");
    let thing_def = pack.manifest.def_types.get("ThingDef").expect("ThingDef");
    assert_eq!(
        thing_def.field_order,
        vec!["defName".to_string(), "stackLimit".to_string()],
        "fieldOrder should deserialize in the declared order"
    );
}

#[test]
fn merge_append_dedup_field_order() {
    let base_pack = load_fixture("valid_minimal");
    let override_pack = load_fixture("override_pack");
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![base_pack, override_pack], &mut diags);
    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");

    assert!(
        thing_def.field_order.contains(&"defName".to_string()),
        "defName must be in field_order"
    );
    assert!(
        thing_def.field_order.contains(&"stackLimit".to_string()),
        "stackLimit must be in field_order"
    );

    let dn_pos = thing_def
        .field_order
        .iter()
        .position(|n| n == "defName")
        .unwrap();
    let sl_pos = thing_def
        .field_order
        .iter()
        .position(|n| n == "stackLimit")
        .unwrap();
    assert!(
        sl_pos < dn_pos,
        "stackLimit should precede defName after override reorder"
    );
}

#[test]
fn merge_appends_unlisted_fields_as_fallback() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.partial", "name": "Partial", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{
        "defType": "ThingDef",
        "fieldOrder": ["defName"],
        "fields": {
            "defName": { "type": { "kind": "string" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let pack = inline_pack(manifest_json, def_json);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);
    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");
    assert!(thing_def.field_order.contains(&"defName".to_string()));
    assert!(thing_def.field_order.contains(&"label".to_string()));
}

#[test]
fn catalog_output_preserves_and_appends_field_order() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.order", "name": "Order", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{
        "defType": "TestDef",
        "fieldOrder": ["firstName", "secondField"],
        "fields": {
            "firstName": { "type": { "kind": "string" }, "required": false },
            "secondField": { "type": { "kind": "integer" }, "required": false },
            "thirdField": { "type": { "kind": "boolean" }, "required": false }
        }
    }"#;
    let pack = inline_pack(manifest_json, def_json);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);
    let def = catalog.def_types.get("TestDef").expect("TestDef");

    let first_pos = def
        .field_order
        .iter()
        .position(|n| n == "firstName")
        .expect("firstName must be in field_order");
    let second_pos = def
        .field_order
        .iter()
        .position(|n| n == "secondField")
        .expect("secondField must be in field_order");
    assert!(
        first_pos < second_pos,
        "firstName must precede secondField in field_order"
    );

    let third_pos = def
        .field_order
        .iter()
        .position(|n| n == "thirdField")
        .expect("unlisted thirdField must be appended to field_order");
    assert!(
        third_pos > second_pos,
        "unlisted thirdField must come after listed fields"
    );
}

// --- 10. fieldOrder references unknown fields produce diagnostic ---

#[test]
fn field_order_with_unknown_field_emits_warning() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.badorder", "name": "Bad Order", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{
        "defType": "ThingDef",
        "fieldOrder": ["defName", "typoField"],
        "fields": {
            "defName": { "type": { "kind": "string" }, "required": true }
        }
    }"#;
    let pack = inline_pack(manifest_json, def_json);
    let mut diags = Vec::new();
    let _catalog = merge_packs(vec![pack], &mut diags);
    let warning = diags
        .iter()
        .find(|d| d.code == "schema_pack_field_order_unknown");
    assert!(
        warning.is_some(),
        "expected schema_pack_field_order_unknown for typoField"
    );

    let unknown_warnings: Vec<_> = diags
        .iter()
        .filter(|d| d.code == "schema_pack_field_order_unknown")
        .collect();
    assert_eq!(
        unknown_warnings.len(),
        1,
        "only typoField should produce a warning"
    );
}

// --- 11. External discovery: missing root produces warning ---

#[test]
fn missing_external_root_produces_warning() {
    let fake_root = std::path::PathBuf::from("C:/nonexistent/schema/root");
    let (_packs, diags) = load_external_packs(&[fake_root]);
    let warning = diags.iter().find(|d| d.code == "schema_pack_root_missing");
    assert!(
        warning.is_some(),
        "expected schema_pack_root_missing warning"
    );
}

// --- 12. Missing def type directory produces error ---

#[test]
fn missing_def_type_directory_in_pack_produces_diagnostic() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack/valid_minimal/schema-pack.json");

    // Write a temporary manifest that points to a nonexistent directory.
    // We test this via load_pack_from_directory on a temp dir.
    let tmp = tempfile::tempdir().expect("temp dir");
    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.missingdir",
        "name": "Missing Dir",
        "version": "1.0.0",
        "defTypeDirectories": ["nonexistent-dir"]
    });
    let manifest_path = tmp.path().join("schema-pack.json");
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&manifest_path);
    assert!(
        pack_opt.is_some(),
        "pack assembles even when dir is missing (zero def types)"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_def_type_directory_missing"),
        "expected schema_pack_def_type_directory_missing diagnostic, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );

    // suppress unused variable warning
    let _ = path;
}

// --- 13. Object type file parsing ---

#[test]
fn object_type_file_deserializes() {
    let json = r#"{
        "objectType": "TestObject",
        "label": "Test object",
        "fieldOrder": ["testField"],
        "fields": {
            "testField": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let (obj_opt, diags) = parse_object_type_schema("test:TestObject.json", "test.pack", json);
    assert!(
        obj_opt.is_some(),
        "expected successful parse, got diags: {:?}",
        diags
    );
    let obj = obj_opt.unwrap();
    assert_eq!(obj.object_type, "TestObject");
    assert!(obj.schema.fields.contains_key("testField"));
}

#[test]
fn malformed_object_json_returns_diagnostic() {
    let (obj_opt, diags) = parse_object_type_schema("test:bad.json", "test.pack", "{ not json }");
    assert!(obj_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_object_type_json_invalid"),
        "expected schema_pack_object_type_json_invalid"
    );
}

#[test]
fn missing_object_type_field_returns_diagnostic() {
    let json = r#"{ "fields": {} }"#;
    let (obj_opt, diags) = parse_object_type_schema("test:noobj.json", "test.pack", json);
    assert!(obj_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_missing_object_type"),
        "expected schema_pack_missing_object_type"
    );
}

#[test]
fn unknown_field_type_kind_in_object_file_returns_warning() {
    let json = r#"{
        "objectType": "TestObject",
        "fields": {
            "weirdField": { "type": { "kind": "superMadeUpType" }, "required": false }
        }
    }"#;
    let (obj_opt, diags) = parse_object_type_schema("test:badtype.json", "test.pack", json);
    assert!(
        obj_opt.is_some(),
        "object file should load despite unknown type"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_invalid_field_type"),
        "expected schema_pack_invalid_field_type warning"
    );
    let obj = obj_opt.unwrap();
    assert_eq!(
        obj.schema.fields["weirdField"].field_type.kind,
        FieldTypeKind::Unknown
    );
}

// --- 14. Duplicate object type in one pack ---

#[test]
fn duplicate_object_type_in_pack_produces_diagnostic() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.dup", "name": "Dup", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let (manifest_opt, _) = parse_schema_pack_manifest("test:manifest", manifest_json);
    let manifest_file = manifest_opt.unwrap();

    let obj_json = r#"{ "objectType": "TestObject", "fields": {} }"#;
    let (obj1_opt, _) = parse_object_type_schema("test:a.json", "test.dup", obj_json);
    let (obj2_opt, _) = parse_object_type_schema("test:b.json", "test.dup", obj_json);
    let obj1 = obj1_opt.unwrap();
    let obj2 = obj2_opt.unwrap();

    let def_json = r#"{ "defType": "ThingDef", "fields": {} }"#;
    let (def_opt, _) = parse_def_type_schema(
        "test:def.json",
        "test.dup",
        def_json,
        manifest_file.format_version,
    );
    let def_file = def_opt.unwrap();

    let def_refs = vec![("test:def.json", &def_file)];
    let obj_refs: Vec<(&str, &ObjectTypeSchemaFile)> =
        vec![("test:a.json", &obj1), ("test:b.json", &obj2)];
    let (pack_opt, diags) =
        assemble_schema_pack("test:pack", manifest_file, &def_refs, &obj_refs, &[]);

    assert!(
        pack_opt.is_some(),
        "pack should assemble even with duplicate (first wins)"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_duplicate_object_type"),
        "expected schema_pack_duplicate_object_type diagnostic"
    );
}

// --- 15. Object types appear in catalog ---

#[test]
fn merge_packs_exposes_object_types() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.obj", "name": "Obj", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    let obj_json = r#"{
        "objectType": "TestObject",
        "label": "Test object",
        "fieldOrder": ["testField"],
        "fields": {
            "testField": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let pack = inline_pack_with_objects(manifest_json, def_json, &[obj_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);
    assert!(
        catalog.object_types.contains_key("TestObject"),
        "object types should be in catalog"
    );
    assert!(
        catalog.object_types["TestObject"]
            .fields
            .contains_key("testField"),
        "testField should be in TestObject"
    );
}

// --- 16. Object fieldOrder validation ---

#[test]
fn object_field_order_unknown_entry_emits_warning() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.ofou", "name": "OFOU", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    let obj_json = r#"{
        "objectType": "TestObject",
        "fieldOrder": ["testField", "typoField"],
        "fields": {
            "testField": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let pack = inline_pack_with_objects(manifest_json, def_json, &[obj_json]);
    let mut diags = Vec::new();
    let _catalog = merge_packs(vec![pack], &mut diags);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_object_field_order_unknown"),
        "expected schema_pack_object_field_order_unknown"
    );
}

// --- 17. schemaRef validation ---

#[test]
fn unknown_schema_ref_emits_warning() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.ref", "name": "Ref", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{
        "defType": "TestDef",
        "fields": {
            "nestedField": { "type": { "kind": "object", "schemaRef": "NonExistentType" }, "required": false, "xml": "object" }
        }
    }"#;
    let pack = inline_pack(manifest_json, def_json);
    let mut diags = Vec::new();
    let _catalog = merge_packs(vec![pack], &mut diags);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_unknown_object_schema_ref"),
        "expected schema_pack_unknown_object_schema_ref warning"
    );
}

// --- 18. Object type lookup helpers ---

#[test]
fn lookup_object_type_and_field_work() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.lookup", "name": "Lookup", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    let obj_json = r#"{
        "objectType": "TestObject",
        "fields": {
            "testField": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let pack = inline_pack_with_objects(manifest_json, def_json, &[obj_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let obj_type = lookup_object_type(&catalog, "TestObject");
    assert!(obj_type.is_some(), "TestObject should be found");

    let field = lookup_object_field(&catalog, "TestObject", "testField");
    assert!(field.is_some(), "testField should be found on TestObject");

    let missing_type = lookup_object_type(&catalog, "NonExistent");
    assert!(missing_type.is_none());

    let missing_field = lookup_object_field(&catalog, "TestObject", "nonExistentField");
    assert!(missing_field.is_none());
}

#[test]
fn lookup_object_field_with_alias_resolves_inherited_aliases_to_their_canonical_name() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.aliaslookup", "name": "AliasLookup", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    let base_json = r#"{
        "objectType": "BaseObject",
        "fields": {
            "verbClass": { "type": { "kind": "string" }, "required": false, "xmlAliases": ["Verb"] }
        }
    }"#;
    let child_json = r#"{
        "objectType": "ChildObject",
        "inherits": ["BaseObject"],
        "fields": {
            "range": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let pack = inline_pack_with_objects(manifest_json, def_json, &[base_json, child_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    // A field declared on the *parent* resolves directly on the child -- object-type inheritance
    // has no "direct fields only" restriction the way Def fields do.
    let (canonical, field) = lookup_object_field_with_alias(&catalog, "ChildObject", "verbClass")
        .expect("verbClass should resolve");
    assert_eq!(canonical, "verbClass");
    assert_eq!(field.field_type.kind, FieldTypeKind::String);

    // An XML alias declared on the inherited parent field resolves to the *canonical* name, not
    // the alias itself.
    let (canonical, _) = lookup_object_field_with_alias(&catalog, "ChildObject", "Verb")
        .expect("alias should resolve");
    assert_eq!(canonical, "verbClass");

    // A field declared directly on the child itself also resolves.
    let (canonical, _) = lookup_object_field_with_alias(&catalog, "ChildObject", "range")
        .expect("range should resolve");
    assert_eq!(canonical, "range");

    assert!(lookup_object_field_with_alias(&catalog, "ChildObject", "nonExistentField").is_none());
    assert!(lookup_object_field_with_alias(&catalog, "NonExistentType", "verbClass").is_none());
}

#[test]
fn lookup_object_field_with_alias_and_collect_object_fields_ordered_guard_inheritance_cycles() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.cyclelookup", "name": "CycleLookup", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    // A mutually-recursive `inherits` pair -- malformed, but the lookup/collection helpers must
    // not hang (this test would time out the suite rather than fail an assertion if the cycle
    // guard were missing or broken).
    let cycle_a_json = r#"{
        "objectType": "CycleA",
        "inherits": ["CycleB"],
        "fields": {
            "a": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let cycle_b_json = r#"{
        "objectType": "CycleB",
        "inherits": ["CycleA"],
        "fields": {
            "b": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let pack = inline_pack_with_objects(manifest_json, def_json, &[cycle_a_json, cycle_b_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let (canonical, _) = lookup_object_field_with_alias(&catalog, "CycleA", "b")
        .expect("b should resolve through the cycle");
    assert_eq!(canonical, "b");
    assert!(lookup_object_field_with_alias(&catalog, "CycleA", "nonExistentField").is_none());

    let fields: Vec<&str> = collect_object_fields_ordered(&catalog, "CycleA")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(fields.contains(&"a"), "{fields:?}");
    assert!(fields.contains(&"b"), "{fields:?}");
    assert_eq!(
        fields.len(),
        2,
        "each field name should appear exactly once: {fields:?}"
    );
}

#[test]
fn collect_object_fields_ordered_prefers_own_field_over_a_same_named_inherited_one() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.overridelookup", "name": "OverrideLookup", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    let base_json = r#"{
        "objectType": "BaseObject",
        "fields": {
            "shared": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let child_json = r#"{
        "objectType": "ChildObject",
        "inherits": ["BaseObject"],
        "fields": {
            "shared": { "type": { "kind": "integer" }, "required": false }
        }
    }"#;
    let pack = inline_pack_with_objects(manifest_json, def_json, &[base_json, child_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let fields = collect_object_fields_ordered(&catalog, "ChildObject");
    let shared_entries: Vec<_> = fields
        .iter()
        .filter(|(name, _)| *name == "shared")
        .collect();
    assert_eq!(
        shared_entries.len(),
        1,
        "a same-named inherited field must not appear twice: {fields:?}"
    );
    // The child's own redeclaration wins, matching `lookup_object_field_with_alias`'s
    // own-fields-first search order.
    assert_eq!(shared_entries[0].1.field_type.kind, FieldTypeKind::Integer);
}

// --- 19. Object type directory diagnostics ---

#[test]
fn escaped_object_type_directory_produces_diagnostic() {
    let tmp = tempfile::tempdir().expect("temp dir");
    #[cfg(windows)]
    let abs_entry = "C:\\Windows\\System32";
    #[cfg(not(windows))]
    let abs_entry = "/etc";

    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.objesc",
        "name": "Obj Esc",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "objectTypeDirectories": [abs_entry]
    });
    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    let manifest_path = tmp.path().join("schema-pack.json");
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&manifest_path);
    assert!(
        pack_opt.is_some(),
        "pack should assemble even when object dir is escaped"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_object_type_directory_escape"),
        "expected schema_pack_object_type_directory_escape, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn missing_object_type_directory_produces_diagnostic() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.objmiss",
        "name": "Obj Miss",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "objectTypeDirectories": ["nonexistent-object-types"]
    });
    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    let manifest_path = tmp.path().join("schema-pack.json");
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&manifest_path);
    assert!(
        pack_opt.is_some(),
        "pack should assemble even when object dir is missing"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_object_type_directory_missing"),
        "expected schema_pack_object_type_directory_missing, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 20. Packs without objectTypeDirectories still load ---

#[test]
fn pack_without_object_type_directories_still_loads() {
    let pack = load_fixture("valid_minimal");
    assert!(
        pack.manifest.object_types.is_empty(),
        "pack with no objectTypeDirectories should have no object types"
    );
}

// --- 21. Recursive object-type directory loading ---

#[test]
fn recursive_object_type_directory_loads_nested_files() {
    let tmp = tempfile::tempdir().expect("temp dir");

    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    std::fs::write(
        def_types_dir.join("TestDef.json"),
        r#"{ "defType": "TestDef", "fields": {} }"#,
    )
    .unwrap();

    let nested_dir = tmp
        .path()
        .join("object-types")
        .join("TestDef")
        .join("Nested");
    std::fs::create_dir_all(&nested_dir).unwrap();
    std::fs::write(
        nested_dir.join("OuterObject.json"),
        r#"{ "objectType": "OuterObject", "fields": {} }"#,
    )
    .unwrap();
    std::fs::write(
        nested_dir.join("InnerObject.json"),
        r#"{ "objectType": "InnerObject", "fields": {} }"#,
    )
    .unwrap();

    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.recursive",
        "name": "Recursive",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "objectTypeDirectories": ["object-types"]
    });
    let manifest_path = tmp.path().join("schema-pack.json");
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&manifest_path);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "recursive pack had errors: {:?}", errors);
    let pack = pack_opt.expect("recursive pack must load");
    assert!(
        pack.manifest.object_types.contains_key("OuterObject"),
        "OuterObject should be loaded from nested directory"
    );
    assert!(
        pack.manifest.object_types.contains_key("InnerObject"),
        "InnerObject should be loaded from nested directory"
    );
}

// --- 22. Built-in pack contains known nested object types ---

#[test]
fn duty_def_schema_refs_resolve() {
    // Verifies that all schemaRef values in the built-in pack resolve to known
    // object types and that recursive self-referencing schemas (e.g. a think-node
    // tree whose subNodes items reference the same base schema) do not cause a
    // loader stack overflow or leave unresolved refs.
    let (packs, load_diags) = load_built_in_packs();
    let load_errors: Vec<_> = load_diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(
        load_errors.is_empty(),
        "built-in pack load errors: {:?}",
        load_errors
    );

    let mut merge_diags = Vec::new();
    let _catalog = merge_packs(packs, &mut merge_diags);
    let merge_errors: Vec<_> = merge_diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(
        merge_errors.is_empty(),
        "built-in pack merge errors: {:?}",
        merge_errors
    );

    // No schemaRef should be unresolved; every unknown_object_schema_ref warning
    // indicates a broken cross-reference in the schema pack.
    let unresolved_ref_warnings: Vec<_> = merge_diags
        .iter()
        .filter(|d| d.code == "schema_pack_unknown_object_schema_ref")
        .collect();
    assert!(
        unresolved_ref_warnings.is_empty(),
        "all schemaRefs must resolve - unresolved refs found: {:?}",
        unresolved_ref_warnings
    );
}

#[test]
fn built_in_pack_nested_objects_are_present() {
    let (packs, _) = load_built_in_packs();
    let mut diags = Vec::new();
    let catalog = merge_packs(packs, &mut diags);

    for expected in &[
        "GraphicData",
        "SubSoundDef",
        "AbilityCompProperties",
        "SoundParamSource_Random",
    ] {
        assert!(
            catalog.object_types.contains_key(*expected),
            "expected '{}' to be present in built-in catalog after nested move",
            expected
        );
    }
}

// --- 23. Unresolvable game version falls back to the full catalog ---
//
// The version filter always keeps "universal" (no-`gameVersion`) packs
// regardless of the selected version. Before this fix, an unresolvable selected version (one no
// installed pack actually declares) would silently narrow the catalog down to ONLY those
// universal packs instead of behaving like "no filter" -- if a universal pack happens to define
// a field differently than a (now-dropped) versioned pack, that silent narrowing can produce a
// genuinely NEW diagnostic, including a blocking one, that never existed in the true unfiltered
// catalog. These tests reproduce that exact conflicting-pack scenario directly against
// `filter_packs_by_game_version` (the extracted helper `build_schema_catalog` now delegates to).

fn conflict_pack(
    pack_id: &str,
    game_version: Option<&str>,
    priority: u32,
    numfield_kind: &str,
) -> LoadedPack {
    let gv_field = game_version
        .map(|v| format!(r#""gameVersion": "{v}","#))
        .unwrap_or_default();
    let manifest_json = format!(
        r#"{{ "formatVersion": 1, "packId": "{pack_id}", "name": "{pack_id}", "version": "1.0.0", {gv_field} "priority": {priority}, "defTypeDirectories": ["x"] }}"#
    );
    let def_json = format!(
        r#"{{ "defType": "ConflictDef", "fields": {{
            "defName": {{ "type": {{ "kind": "string" }}, "required": true }},
            "numField": {{ "type": {{ "kind": "{numfield_kind}" }}, "required": false }}
        }} }}"#
    );
    inline_pack(&manifest_json, &def_json)
}

#[test]
fn unresolvable_game_version_falls_back_to_full_catalog_not_universal_only() {
    // A versioned pack (declares "1.6", the ONLY declared version among these two packs, and has
    // higher priority) and a "universal" pack (no declared gameVersion, so it always passed the
    // old unconditional filter) define the SAME field with CONFLICTING types. In a true
    // unfiltered merge, the higher-priority versioned pack wins: `numField` resolves to integer.
    let versioned = conflict_pack("test.gv.versioned", Some("1.6"), 10, "integer");
    let universal = conflict_pack("test.gv.universal", None, 0, "string");

    let mut diags = Vec::new();
    // Select a version that matches NEITHER pack's declaration -- unresolvable.
    let filtered = crate::schema_pack::filter_packs_by_game_version(
        vec![universal, versioned],
        Some("9.9"),
        &mut diags,
    );

    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_game_version_unresolvable"),
        "expected schema_pack_game_version_unresolvable diagnostic, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
    assert_eq!(
        filtered.len(),
        2,
        "an unresolvable version must fall back to keeping every pack, not just the universal one"
    );

    let mut merge_diags = Vec::new();
    let catalog = merge_packs(filtered, &mut merge_diags);
    let num_field = catalog.def_types["ConflictDef"]
        .fields
        .get("numField")
        .expect("numField");
    assert_eq!(
        num_field.field_type.kind,
        FieldTypeKind::Integer,
        "fallback must match the true unfiltered merge (versioned pack's higher priority wins), \
         not silently narrow down to the universal-only pack's definition"
    );

    // Prove the practical consequence directly: a value that is invalid under the CORRECT
    // (fallback) resolution must actually be flagged -- if the old bug were still present
    // (silently narrowing to the universal pack's `string` type), this same value would be
    // accepted as valid and no diagnostic would appear at all.
    let xml =
        r#"<Defs><ConflictDef><defName>X</defName><numField>abc</numField></ConflictDef></Defs>"#;
    let doc = parse_to_document("test.xml", xml);
    let def_index = DefIndex::default();
    let diagnostics = validate_document(
        &doc,
        &ValidationContext {
            catalog: &catalog,
            def_index: &def_index,
        },
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch" && d.blocking),
        "expected a blocking type-mismatch diagnostic matching the true unfiltered/priority-correct \
         resolution, got: {:?}",
        diagnostics.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn resolvable_game_version_still_filters_out_mismatched_versioned_packs() {
    let versioned = conflict_pack("test.gv.versioned2", Some("1.6"), 10, "integer");
    let universal = conflict_pack("test.gv.universal2", None, 0, "string");

    let mut diags = Vec::new();
    // "1.6" IS declared by the versioned pack, so filtering proceeds normally (not the fallback).
    let filtered = crate::schema_pack::filter_packs_by_game_version(
        vec![universal, versioned],
        Some("1.6"),
        &mut diags,
    );

    assert!(
        !diags
            .iter()
            .any(|d| d.code == "schema_pack_game_version_unresolvable"),
        "a resolvable version must not trigger the unresolvable fallback"
    );
    assert_eq!(
        filtered.len(),
        2,
        "both a matching versioned pack and a universal pack should remain when the version resolves"
    );

    // A pack declaring "1.6" must be present for "1.6" to count as resolvable at all -- otherwise
    // this second call would itself hit the unresolvable-fallback path (returning `mismatched`
    // unfiltered) rather than exercising genuine mismatch filtering.
    let matching = conflict_pack("test.gv.matching", Some("1.6"), 0, "string");
    let mismatched = conflict_pack("test.gv.mismatched", Some("1.5"), 20, "boolean");
    let mut diags2 = Vec::new();
    let filtered2 = crate::schema_pack::filter_packs_by_game_version(
        vec![matching, mismatched],
        Some("1.6"),
        &mut diags2,
    );
    assert_eq!(
        filtered2.len(),
        1,
        "a pack declaring a DIFFERENT, but otherwise installed/resolvable, game version must still \
         be filtered out"
    );
    assert_eq!(filtered2[0].manifest.pack_id, "test.gv.matching");
    assert!(diags2
        .iter()
        .any(|d| d.code == "schema_pack_game_version_mismatch"));
}
