//! Contract tests for `PatchFile`/`PatchOperationNode`'s JSON wire shape, exercised by the
//! `parse_patch_operations`/`serialize_patch_operations` Tauri commands the patches editor UI
//! calls across the IPC boundary.

use super::*;

#[test]
fn patch_file_json_round_trips_for_every_built_in_fixture() {
    for (name, src) in ALL_BUILT_IN_FIXTURES {
        let file = parse_patch_file(name, src);
        let json = serde_json::to_string(&file).expect("PatchFile should serialize to JSON");
        let restored: PatchFile =
            serde_json::from_str(&json).expect("PatchFile should deserialize from its own JSON");
        assert_eq!(
            restored.operations, file.operations,
            "{name}: operations changed across a JSON round trip"
        );

        // Deserialize -> serialize (XML) must reproduce the same XML as the original AST, proving
        // a frontend-edited-then-sent-back PatchFile serializes identically to the original.
        assert_eq!(serialize_patch_file(&restored), serialize_patch_file(&file));
    }
}

#[test]
fn patch_operation_kind_uses_adjacently_tagged_camel_case_wire_shape() {
    let file = parse_patch_file("add.xml", ADD_XML);
    let op = &file.operations[0];
    assert_eq!(op.class_name, "PatchOperationAdd");

    let json = serde_json::to_value(&op.kind).expect("kind should serialize");
    assert_eq!(json["type"], "add");
    assert!(
        json.get("data").is_some(),
        "adjacently tagged enum must nest payload under `data`"
    );
    assert!(
        json["data"].get("valueXml").is_some(),
        "payload fields must be camelCase (value_xml -> valueXml)"
    );
}

#[test]
fn nested_sequence_kind_serializes_operations_array_under_data() {
    let file = parse_patch_file("sequence.xml", SEQUENCE_XML);
    let op = &file.operations[0];
    let json = serde_json::to_value(&op.kind).expect("kind should serialize");
    assert_eq!(json["type"], "sequence");
    assert!(
        json["data"].is_array(),
        "Sequence's newtype Vec payload should serialize as a JSON array under `data`"
    );
}

#[test]
fn find_mod_kind_serializes_camel_case_struct_fields_under_data() {
    let file = parse_patch_file("find_mod.xml", FIND_MOD_XML);
    let op = &file.operations[0];
    let json = serde_json::to_value(&op.kind).expect("kind should serialize");
    assert_eq!(json["type"], "findMod");
    assert!(json["data"]["mods"].is_array());
    assert!(
        json["data"].get("matchOp").is_some() || json["data"].get("nomatchOp").is_some(),
        "FindMod struct-variant fields must be camelCase (match_op -> matchOp)"
    );
}

#[test]
fn unknown_custom_operation_round_trips_raw_xml_through_json() {
    let file = parse_patch_file("custom_operation.xml", CUSTOM_OPERATION_XML);
    let op = &file.operations[0];
    assert!(matches!(op.kind, PatchOperationKind::Unknown(_)));

    let json = serde_json::to_string(op).expect("node should serialize");
    let restored: PatchOperationNode =
        serde_json::from_str(&json).expect("node should deserialize");
    assert_eq!(restored, *op);
    match &restored.kind {
        PatchOperationKind::Unknown(unknown) => {
            assert!(unknown.raw_xml.contains(&op.class_name));
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}
