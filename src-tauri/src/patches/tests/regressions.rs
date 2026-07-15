use super::*;

// Regression tests verified against RimWorld's decompiled
// `Verse.DirectXmlToObject`/`ModContentPack.LoadPatches`.

#[test]
fn multiple_root_elements_report_diagnostic_instead_of_using_the_first() {
    let file = parse_patch_file("test.xml", MULTIPLE_ROOTS_XML);
    assert!(!file.had_fatal_parse_error);
    assert!(file.operations.is_empty());
    assert!(file
        .diagnostics
        .iter()
        .any(|d| d.message.contains("2 root elements")));
}

#[test]
fn duplicate_field_reports_diagnostic_and_last_value_wins() {
    let file = parse_patch_file("test.xml", DUPLICATE_XPATH_XML);
    assert!(file
        .diagnostics
        .iter()
        .any(|d| d.message.contains("defines the field <xpath> twice")));
    assert_eq!(file.operations.len(), 1);
    match &file.operations[0].kind {
        PatchOperationKind::Remove(inner) => {
            assert_eq!(
                inner.xpath.as_deref(),
                Some(r#"Defs/ThingDef[defName="Wall"]"#)
            );
        }
        other => panic!("expected Remove, got {:?}", other),
    }
}

#[test]
fn known_class_with_unrecognized_field_falls_back_to_raw_xml_instead_of_dropping_it() {
    // A mod-authored (or not-yet-modeled) extra child on an otherwise-recognized class: the typed
    // `PathedValueOrderOperation` model has no field for `<foo>`, so if this parsed as `Add` the
    // reader would silently discard it and any edit-then-reserialize round trip would lose it.
    let src = r#"<Patch>
  <Operation Class="PatchOperationAdd">
    <xpath>Defs/ThingDef[defName="Wall"]</xpath>
    <value>
      <statBases><MoveSpeed>1</MoveSpeed></statBases>
    </value>
    <foo>keep me</foo>
  </Operation>
</Patch>
"#;
    let file = parse_patch_file("test.xml", src);
    assert_eq!(file.operations.len(), 1);
    match &file.operations[0].kind {
        PatchOperationKind::Unknown(unknown) => {
            assert!(unknown.raw_xml.contains("<foo>keep me</foo>"));
            assert!(unknown.raw_xml.contains("<xpath>"));
        }
        other => panic!("expected Unknown fallback, got {:?}", other),
    }
    assert!(file
        .diagnostics
        .iter()
        .any(|d| d.message.contains("not recognized for PatchOperationAdd")));

    // Round trip preserves it byte-for-byte, unlike a typed Add would.
    let serialized = serialize_patch_file(&file);
    assert!(serialized.contains("<foo>keep me</foo>"));
}

#[test]
fn unrecognized_field_fallback_only_affects_the_offending_node_not_its_siblings() {
    let src = r#"<Patch>
  <Operation Class="PatchOperationSequence">
    <operations>
      <li Class="PatchOperationAdd">
        <xpath>Defs/ThingDef</xpath>
        <value><a/></value>
        <foo>extra</foo>
      </li>
      <li Class="PatchOperationRemove">
        <xpath>Defs/ThingDef2</xpath>
      </li>
    </operations>
  </Operation>
</Patch>
"#;
    let file = parse_patch_file("test.xml", src);
    assert_eq!(file.operations.len(), 1);
    match &file.operations[0].kind {
        PatchOperationKind::Sequence(ops) => {
            assert_eq!(ops.len(), 2);
            assert!(matches!(ops[0].kind, PatchOperationKind::Unknown(_)));
            assert!(matches!(ops[1].kind, PatchOperationKind::Remove(_)));
        }
        other => panic!("expected Sequence, got {:?}", other),
    }
}

#[test]
fn empty_value_element_is_preserved_as_empty_string_not_missing() {
    let file = parse_patch_file("test.xml", EMPTY_ATTRIBUTE_VALUE_XML);
    assert!(
        file.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        file.diagnostics
    );
    assert_eq!(file.operations.len(), 1);
    match &file.operations[0].kind {
        PatchOperationKind::AttributeSet(inner) => {
            assert_eq!(inner.value.as_deref(), Some(""));
        }
        other => panic!("expected AttributeSet, got {:?}", other),
    }
}
