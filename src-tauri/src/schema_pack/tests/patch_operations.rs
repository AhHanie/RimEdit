//! Tests for issue 03 (custom patch operation metadata): metadata loading, merge-by-priority,
//! diagnostics, and built-in operation metadata parity with the hardcoded AST model.

use super::*;
use crate::patches::BUILT_IN_OPERATION_CLASSES;
use crate::schema_pack::locale::SchemaLocaleOverlay;
use crate::schema_pack::merge::merge_packs_with_locale;
use crate::schema_pack::model::{PatchOperationFieldRole, PatchOperationPreviewKind};

const DUMMY_MANIFEST: &str = r#"{
    "formatVersion": 2,
    "packId": "test.patchops",
    "name": "Patch Ops",
    "version": "1.0.0",
    "defTypeDirectories": ["x"]
}"#;

// --- 1. Valid custom operation metadata loads ---

#[test]
fn valid_custom_operation_metadata_loads() {
    let op_json = r#"{
        "formatVersion": 1,
        "className": "MyMod.PatchOperationAddOrReplace",
        "label": "Add or Replace",
        "description": "Adds a missing node or replaces an existing one.",
        "fieldOrder": ["xpath", "value"],
        "fields": {
            "xpath": { "type": { "kind": "string" }, "xml": "element", "role": "xpath", "required": true },
            "value": { "type": { "kind": "object" }, "xml": "object", "role": "xmlValue", "required": true }
        }
    }"#;

    let pack = inline_pack_with_patch_operations(DUMMY_MANIFEST, &[op_json]);
    let op = pack
        .manifest
        .patch_operations
        .get("MyMod.PatchOperationAddOrReplace")
        .expect("custom operation metadata should be present in the pack");
    assert_eq!(op.label.as_deref(), Some("Add or Replace"));
    assert_eq!(
        op.field_order,
        vec!["xpath".to_string(), "value".to_string()]
    );
    let xpath_field = op.fields.get("xpath").expect("xpath field missing");
    assert_eq!(xpath_field.role, Some(PatchOperationFieldRole::Xpath));
    assert_eq!(xpath_field.required, Some(true));
    let value_field = op.fields.get("value").expect("value field missing");
    assert_eq!(value_field.role, Some(PatchOperationFieldRole::XmlValue));

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    let merged = catalog
        .patch_operations
        .get("MyMod.PatchOperationAddOrReplace")
        .expect("custom operation metadata should survive merge");
    assert_eq!(merged.class_name, "MyMod.PatchOperationAddOrReplace");
    assert_eq!(merged.preview.kind, PatchOperationPreviewKind::Unsupported);
}

// --- 2. Duplicate custom class names merge by priority ---

#[test]
fn duplicate_custom_class_names_merge_by_priority() {
    let low_json = r#"{
        "formatVersion": 1,
        "className": "MyMod.PatchOperationFoo",
        "label": "Foo (base)",
        "fieldOrder": ["xpath"],
        "fields": {
            "xpath": { "type": { "kind": "string" }, "xml": "element", "role": "xpath", "required": true }
        }
    }"#;
    let high_json = r#"{
        "formatVersion": 1,
        "className": "MyMod.PatchOperationFoo",
        "label": "Foo (overridden)",
        "fieldOrder": ["xpath", "extra"],
        "fields": {
            "extra": { "type": { "kind": "string" }, "xml": "element", "required": false }
        }
    }"#;

    let low_manifest = r#"{
        "formatVersion": 2,
        "packId": "test.patchops.low",
        "name": "Low",
        "version": "1.0.0",
        "priority": 0,
        "defTypeDirectories": ["x"]
    }"#;
    let high_manifest = r#"{
        "formatVersion": 2,
        "packId": "test.patchops.high",
        "name": "High",
        "version": "1.0.0",
        "priority": 10,
        "defTypeDirectories": ["x"]
    }"#;

    let low_pack = inline_pack_with_patch_operations(low_manifest, &[low_json]);
    let high_pack = inline_pack_with_patch_operations(high_manifest, &[high_json]);

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![low_pack, high_pack], &mut diags);

    let merged = catalog
        .patch_operations
        .get("MyMod.PatchOperationFoo")
        .expect("class should be present after merge");
    assert_eq!(
        merged.label.as_deref(),
        Some("Foo (overridden)"),
        "higher priority pack's label should win"
    );
    assert!(
        merged.fields.contains_key("xpath"),
        "field from the lower priority pack should still be present"
    );
    assert!(
        merged.fields.contains_key("extra"),
        "field only declared by the higher priority pack should be merged in"
    );
    assert_eq!(
        merged.field_order,
        vec!["xpath".to_string(), "extra".to_string()],
        "field order should follow the higher priority pack's declaration"
    );
}

#[test]
fn duplicate_class_name_within_one_pack_produces_diagnostic() {
    let op_json = r#"{
        "formatVersion": 1,
        "className": "MyMod.PatchOperationFoo",
        "fields": {}
    }"#;
    let (manifest_opt, _) = parse_schema_pack_manifest("test:manifest", DUMMY_MANIFEST);
    let manifest_file = manifest_opt.unwrap();
    let (op1_opt, _) = parse_patch_operation_metadata("test:a.json", "test.patchops", op_json);
    let (op2_opt, _) = parse_patch_operation_metadata("test:b.json", "test.patchops", op_json);
    let op1 = op1_opt.unwrap();
    let op2 = op2_opt.unwrap();

    let refs = vec![("test:a.json", &op1), ("test:b.json", &op2)];
    let (pack_opt, diags) = assemble_schema_pack("test:pack", manifest_file, &[], &[], &refs);
    assert!(
        pack_opt.is_some(),
        "pack should assemble even with duplicate (first wins)"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "patch_operation_metadata_duplicate_class_name_in_pack"),
        "expected patch_operation_metadata_duplicate_class_name_in_pack diagnostic"
    );
}

// --- 3. Invalid metadata reports schema diagnostics ---

#[test]
fn malformed_patch_operation_json_returns_diagnostic() {
    let (op_opt, diags) =
        parse_patch_operation_metadata("test:bad.json", "test.pack", "{ not json }");
    assert!(op_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "patch_operation_metadata_json_invalid"),
        "expected patch_operation_metadata_json_invalid"
    );
}

#[test]
fn missing_class_name_returns_diagnostic() {
    let json = r#"{ "formatVersion": 1, "fields": {} }"#;
    let (op_opt, diags) = parse_patch_operation_metadata("test:noclass.json", "test.pack", json);
    assert!(op_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "patch_operation_metadata_missing_class_name"),
        "expected patch_operation_metadata_missing_class_name"
    );
}

#[test]
fn unknown_field_type_kind_in_patch_operation_returns_warning() {
    let json = r#"{
        "formatVersion": 1,
        "className": "MyMod.PatchOperationWeird",
        "fields": {
            "weirdField": { "type": { "kind": "superMadeUpType" }, "required": false }
        }
    }"#;
    let (op_opt, diags) = parse_patch_operation_metadata("test:weird.json", "test.pack", json);
    assert!(op_opt.is_some(), "should load despite unknown field type");
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_invalid_field_type"),
        "expected schema_pack_invalid_field_type warning"
    );
}

// --- 4. Unsupported formatVersion reports schema diagnostics ---

#[test]
fn unsupported_patch_operation_format_version_returns_diagnostic() {
    let json = r#"{ "formatVersion": 99, "className": "MyMod.PatchOperationFoo", "fields": {} }"#;
    let (op_opt, diags) = parse_patch_operation_metadata("test:v99.json", "test.pack", json);
    assert!(op_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "patch_operation_metadata_format_unsupported"),
        "expected patch_operation_metadata_format_unsupported"
    );
}

#[test]
fn unknown_preview_kind_is_normalized_to_unsupported_with_warning() {
    let op_json = r#"{
        "formatVersion": 1,
        "className": "MyMod.PatchOperationFoo",
        "fields": {},
        "preview": { "kind": "somethingMadeUp" }
    }"#;
    let pack = inline_pack_with_patch_operations(DUMMY_MANIFEST, &[op_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "patch_operation_metadata_unknown_preview_kind"),
        "expected patch_operation_metadata_unknown_preview_kind warning"
    );
    let merged = catalog
        .patch_operations
        .get("MyMod.PatchOperationFoo")
        .unwrap();
    assert_eq!(merged.preview.kind, PatchOperationPreviewKind::Unsupported);
}

// --- 4b. Locale sidecar overrides a patch operation's preview.message (issue 05 grammar --
// the key grammar had no shape for a preview message) ---

#[test]
fn locale_sidecar_overrides_patch_operation_preview_message() {
    // Uses an undotted class name -- `parse_locale_key`'s split-based grammar treats every
    // `.`-separated segment between `patchOperations` and `preview.message` as part of the class
    // name path, so (like the pre-existing `PatchOperationLabel`/`PatchOperationDescription`
    // grammar this mirrors) it only resolves for a class name with no literal `.` in it. A
    // namespaced/dotted custom operation class name is a pre-existing grammar limitation shared by
    // every `patchOperations.*` key, not something this fix introduces or is scoped to address.
    let op_json = r#"{
        "formatVersion": 1,
        "className": "PatchOperationFoo",
        "fields": {},
        "preview": { "kind": "unsupported", "message": "Custom operation, cannot preview." }
    }"#;
    let mut pack = inline_pack_with_patch_operations(DUMMY_MANIFEST, &[op_json]);

    let mut en_overlay = SchemaLocaleOverlay::new();
    en_overlay.insert(
        "patchOperations.PatchOperationFoo.preview.message".to_string(),
        "Custom operation (en) -- cannot preview.".to_string(),
    );
    pack.locales.insert("en".to_string(), en_overlay);

    let mut diags = Vec::new();
    let catalog = merge_packs_with_locale(vec![pack], &mut diags, "en");
    let merged = catalog.patch_operations.get("PatchOperationFoo").unwrap();
    assert_eq!(
        merged.preview.message.as_deref(),
        Some("Custom operation (en) -- cannot preview.")
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_unresolved_key"
                || d.code == "schema_pack_locale_unknown_key"),
        "preview.message override should resolve cleanly, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn locale_sidecar_preview_message_override_from_a_different_pack_is_ignored() {
    let op_json = r#"{
        "formatVersion": 1,
        "className": "PatchOperationFoo",
        "fields": {},
        "preview": { "kind": "unsupported", "message": "Original message." }
    }"#;
    let mut owner_pack = inline_pack_with_patch_operations(DUMMY_MANIFEST, &[op_json]);
    owner_pack
        .locales
        .insert("en".to_string(), SchemaLocaleOverlay::new());

    let intruder_manifest = r#"{
        "formatVersion": 1,
        "packId": "test.patchops.intruder",
        "name": "Intruder",
        "version": "1.0.0",
        "defTypeDirectories": ["x"]
    }"#;
    let mut intruder_pack = inline_pack(intruder_manifest, r#"{ "defType": "Def", "fields": {} }"#);
    let mut intruder_overlay = SchemaLocaleOverlay::new();
    intruder_overlay.insert(
        "patchOperations.PatchOperationFoo.preview.message".to_string(),
        "Hijacked message.".to_string(),
    );
    intruder_pack
        .locales
        .insert("en".to_string(), intruder_overlay);

    let mut diags = Vec::new();
    let catalog = merge_packs_with_locale(vec![owner_pack, intruder_pack], &mut diags, "en");
    let merged = catalog.patch_operations.get("PatchOperationFoo").unwrap();
    assert_eq!(
        merged.preview.message.as_deref(),
        Some("Original message."),
        "a sidecar must never override a preview.message owned by a different pack"
    );
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_locale_wrong_owner"));
}

// --- 5. Built-in operation metadata matches the hardcoded AST model ---

#[test]
fn built_in_pack_ships_metadata_for_every_built_in_operation_class() {
    let (packs, diags) = load_built_in_packs();
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "built-in pack load errors: {:?}", errors);

    let mut merge_diags = Vec::new();
    let catalog = merge_packs(packs, &mut merge_diags);

    for class_name in BUILT_IN_OPERATION_CLASSES {
        assert!(
            catalog.patch_operations.contains_key(*class_name),
            "expected built-in operation metadata for '{}'",
            class_name
        );
    }
}

#[test]
fn built_in_add_metadata_matches_hardcoded_pathed_value_order_shape() {
    let (packs, _) = load_built_in_packs();
    let mut diags = Vec::new();
    let catalog = merge_packs(packs, &mut diags);

    let add = catalog
        .patch_operations
        .get("PatchOperationAdd")
        .expect("PatchOperationAdd metadata missing");

    // Matches `PathedValueOrderOperation { xpath, value_xml, order }` in `patches::model`.
    assert!(add.fields.contains_key("xpath"));
    assert!(add.fields.contains_key("value"));
    assert!(add.fields.contains_key("order"));
    assert_eq!(
        add.fields["xpath"].role,
        Some(PatchOperationFieldRole::Xpath)
    );
    assert_eq!(
        add.fields["value"].role,
        Some(PatchOperationFieldRole::XmlValue)
    );
    assert!(add.fields["xpath"].required);
    assert!(add.fields["value"].required);
    assert!(!add.fields["order"].required);
}

#[test]
fn built_in_sequence_metadata_declares_operation_list_role() {
    let (packs, _) = load_built_in_packs();
    let mut diags = Vec::new();
    let catalog = merge_packs(packs, &mut diags);

    let sequence = catalog
        .patch_operations
        .get("PatchOperationSequence")
        .expect("PatchOperationSequence metadata missing");
    let operations_field = sequence
        .fields
        .get("operations")
        .expect("operations field missing");
    assert_eq!(
        operations_field.role,
        Some(PatchOperationFieldRole::OperationList)
    );
}

#[test]
fn built_in_conditional_metadata_declares_operation_role_for_match_and_nomatch() {
    let (packs, _) = load_built_in_packs();
    let mut diags = Vec::new();
    let catalog = merge_packs(packs, &mut diags);

    let conditional = catalog
        .patch_operations
        .get("PatchOperationConditional")
        .expect("PatchOperationConditional metadata missing");
    assert_eq!(
        conditional.fields["match"].role,
        Some(PatchOperationFieldRole::Operation)
    );
    assert_eq!(
        conditional.fields["nomatch"].role,
        Some(PatchOperationFieldRole::Operation)
    );
    assert_eq!(
        conditional.fields["xpath"].role,
        Some(PatchOperationFieldRole::Xpath)
    );
}
