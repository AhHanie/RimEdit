use super::*;
use crate::schema_pack::XmlFieldShape;

fn validate_keyed_map(src: &str, def_index: &DefIndex) -> Vec<ValidationDiagnostic> {
    validate_test_xml_with_fixture(src, "keyed_object_map_validation", def_index)
}

fn tag_def_index(tag_name: &str) -> DefIndex {
    let mut def = indexed_test_def("tags.xml", IndexedSourceKind::Source);
    def.key.def_type = "TagDef".to_string();
    def.key.def_name = tag_name.to_string();
    def.def_type = "TagDef".to_string();
    def.def_name = tag_name.to_string();
    DefIndex {
        defs: vec![def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    }
}

#[test]
fn schema_pack_loads_keyed_object_map_shape() {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack/keyed_object_map_validation");
    let result = build_schema_catalog(&[fixture_path], None);
    assert!(
        result.diagnostics.is_empty(),
        "keyed_object_map_validation fixture must load without diagnostics: {:?}",
        result.diagnostics
    );
    let def_type = result
        .catalog
        .def_types
        .get("TestDef")
        .expect("TestDef must be present");
    let field = def_type
        .fields
        .get("keyedMap")
        .expect("keyedMap field must be present");
    assert_eq!(
        field.xml,
        XmlFieldShape::KeyedObjectMap,
        "keyedMap must have xml=keyedObjectMap"
    );
    assert_eq!(
        field.key_reference.as_ref().map(|r| r.def_type.as_str()),
        Some("TagDef"),
        "keyedMap must have keyReference.defType=TagDef"
    );
    assert_eq!(
        field.items.as_ref().and_then(|i| i.schema_ref.as_deref()),
        Some("PartValue"),
        "keyedMap items must reference PartValue"
    );
}

#[test]
fn validation_reports_missing_keyed_object_map_key() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <value><knownField>hello</knownField></value>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_keyed_object_map_missing_key"),
        "missing <key> must produce validation_keyed_object_map_missing_key: {diagnostics:?}"
    );
}

#[test]
fn validation_reports_missing_keyed_object_map_value() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <key>Root</key>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_keyed_object_map_missing_value"),
        "missing <value> must produce validation_keyed_object_map_missing_value: {diagnostics:?}"
    );
}

#[test]
fn validation_reports_unresolved_keyed_object_map_reference() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <key>UnknownTag</key>
        <value><knownField>hello</knownField></value>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_map_key"),
        "unknown <key> must produce validation_unresolved_map_key: {diagnostics:?}"
    );
}

#[test]
fn validation_does_not_warn_when_keyed_object_map_key_resolves() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <key>Root</key>
        <value><knownField>hello</knownField></value>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &tag_def_index("Root"));
    let ref_warn = diagnostics
        .iter()
        .any(|d| d.code == "validation_unresolved_map_key");
    assert!(
        !ref_warn,
        "resolved <key> must not produce validation_unresolved_map_key: {diagnostics:?}"
    );
}

#[test]
fn validation_reports_duplicate_keyed_object_map_key() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <key>Root</key>
        <value><knownField>first</knownField></value>
      </li>
      <li>
        <key>Root</key>
        <value><knownField>second</knownField></value>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &tag_def_index("Root"));
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_keyed_object_map_duplicate_key"),
        "duplicate <key> must produce validation_keyed_object_map_duplicate_key: {diagnostics:?}"
    );
}

#[test]
fn validation_reports_unknown_value_child_in_keyed_object_map() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <key>Root</key>
        <value>
          <knownField>hello</knownField>
          <unknownExtraField>bad</unknownExtraField>
        </value>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &tag_def_index("Root"));
    assert!(
        diagnostics.iter().any(|d| {
            d.code == "validation_unknown_object_field"
                && d.field_path
                    .as_deref()
                    .map(|p| p.contains("unknownExtraField"))
                    .unwrap_or(false)
        }),
        "unknown field inside <value> must produce validation_unknown_object_field: {diagnostics:?}"
    );
}

#[test]
fn validation_validates_scalar_type_in_keyed_object_map_value() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <key>Root</key>
        <value>
          <count>not-an-integer</count>
        </value>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &tag_def_index("Root"));
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "bad integer in <value> must produce validation_field_type_mismatch: {diagnostics:?}"
    );
}

#[test]
fn validation_accepts_valid_keyed_object_map_entry() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <key>Root</key>
        <value>
          <knownField>hello</knownField>
          <count>5</count>
        </value>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &tag_def_index("Root"));
    let blocking: Vec<_> = diagnostics.iter().filter(|d| d.blocking).collect();
    assert!(
        blocking.is_empty(),
        "valid keyed object map entry must not produce blocking diagnostics: {diagnostics:?}"
    );
}

#[test]
fn validation_reports_both_missing_key_and_value_for_empty_li() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li/>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_keyed_map(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_keyed_object_map_missing_key"),
        "empty <li/> must produce validation_keyed_object_map_missing_key: {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_keyed_object_map_missing_value"),
        "empty <li/> must also produce validation_keyed_object_map_missing_value: {diagnostics:?}"
    );
}

#[test]
fn xml_edit_updates_nested_keyed_object_map_value_field() {
    let src = r#"<Defs>
  <TestDef>
    <defName>D</defName>
    <keyedMap>
      <li>
        <key>Root</key>
        <value>
          <knownField>old</knownField>
        </value>
      </li>
    </keyedMap>
  </TestDef>
</Defs>"#;
    let mut doc = parse_to_document("test.xml", src);
    assert!(
        doc.parse_diagnostics.is_empty(),
        "parse must succeed: {:?}",
        doc.parse_diagnostics
    );

    // Retrieve the <li> node id via the editor view.
    let li_node_id = {
        let view = build_editor_view(&doc);
        let def = &view.defs[0];
        let keyed_map = def.children.iter().find(|c| c.name == "keyedMap").unwrap();
        keyed_map.li_items[0].node_id
    };

    // SetNestedElementText anchored at <li>, navigating into <value> to reach <knownField>.
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetNestedElementText {
            parent_node_id: li_node_id,
            object_path: vec!["value".to_string()],
            field_name: "knownField".to_string(),
            value: "updated".to_string(),
            field_order: vec![],
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    assert!(
        out.contains("<knownField>updated</knownField>"),
        "knownField must be updated: {out}"
    );
    assert!(
        out.contains("<key>Root</key>"),
        "<key> must be preserved: {out}"
    );
}
