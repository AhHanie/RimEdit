use super::*;

fn validate_with_fixture(src: &str, def_index: &DefIndex) -> Vec<ValidationDiagnostic> {
    validate_test_xml_with_fixture(src, "accepted_def_types", def_index)
}

fn make_choice_a_index(def_name: &str) -> DefIndex {
    let mut def = indexed_test_def("choices.xml", IndexedSourceKind::Source);
    def.key.def_type = "ChoiceDefA".to_string();
    def.key.def_name = def_name.to_string();
    def.def_type = "ChoiceDefA".to_string();
    def.def_name = def_name.to_string();
    DefIndex {
        defs: vec![def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    }
}

fn make_choice_b_index(def_name: &str) -> DefIndex {
    let mut def = indexed_test_def("choices.xml", IndexedSourceKind::Source);
    def.key.def_type = "ChoiceDefB".to_string();
    def.key.def_name = def_name.to_string();
    def.def_type = "ChoiceDefB".to_string();
    def.def_name = def_name.to_string();
    DefIndex {
        defs: vec![def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    }
}

// --- Schema model round-trip ---

#[test]
fn accepted_def_types_preserved_through_catalog() {
    // Loading the fixture and checking that the TestDef.listRef field
    // serializes acceptedDefTypes from the JSON schema into the catalog.
    use crate::schema_pack::build_schema_catalog;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack/accepted_def_types");
    let result = build_schema_catalog(&[fixture_path], None);
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "schema_pack_load_error"),
        "pack should load without errors: {:?}",
        result.diagnostics
    );
    let test_def = result
        .catalog
        .def_types
        .get("TestDef")
        .expect("TestDef must be in catalog");
    let list_ref = test_def
        .fields
        .get("listRef")
        .expect("listRef field must exist");
    let reference = list_ref
        .reference
        .as_ref()
        .expect("listRef must have reference metadata");
    assert_eq!(reference.def_type, "BaseChoiceDef");
    let accepted = reference
        .accepted_def_types
        .as_ref()
        .expect("listRef must have acceptedDefTypes");
    assert!(accepted.contains(&"ChoiceDefA".to_string()));
    assert!(accepted.contains(&"ChoiceDefB".to_string()));
    assert!(
        accepted.contains(&"UnknownFutureDef".to_string()),
        "unknown type must be preserved"
    );
}

#[test]
fn old_reference_metadata_without_accepted_def_types_works() {
    // The directRef field has no acceptedDefTypes - it must still serialize and validate normally.
    use crate::schema_pack::build_schema_catalog;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack/accepted_def_types");
    let result = build_schema_catalog(&[fixture_path], None);
    let test_def = result.catalog.def_types.get("TestDef").unwrap();
    let direct_ref = test_def.fields.get("directRef").unwrap();
    let reference = direct_ref.reference.as_ref().unwrap();
    assert!(
        reference.accepted_def_types.is_none(),
        "directRef should have no acceptedDefTypes"
    );
}

// --- Scalar (element) reference validation with acceptedDefTypes ---

#[test]
fn scalar_ref_resolves_under_accepted_type_a() {
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <scalarRef>Alpha</scalarRef>
  </TestDef>
</Defs>"#;
    let def_index = make_choice_a_index("Alpha");
    let diagnostics = validate_with_fixture(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "ChoiceDefA scalar ref should resolve under acceptedDefTypes: {diagnostics:?}"
    );
}

#[test]
fn scalar_ref_warns_when_not_found_in_any_accepted_type() {
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <scalarRef>NoSuchDef</scalarRef>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "missing scalar reference must produce validation_unresolved_reference: {diagnostics:?}"
    );
}

// --- listOfLi reference validation with acceptedDefTypes ---

#[test]
fn list_ref_resolves_under_accepted_type_a() {
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <listRef>
      <li>Alpha</li>
    </listRef>
  </TestDef>
</Defs>"#;
    let def_index = make_choice_a_index("Alpha");
    let diagnostics = validate_with_fixture(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "ChoiceDefA reference should resolve under acceptedDefTypes: {diagnostics:?}"
    );
}

#[test]
fn list_ref_resolves_under_accepted_type_b() {
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <listRef>
      <li>Beta</li>
    </listRef>
  </TestDef>
</Defs>"#;
    let def_index = make_choice_b_index("Beta");
    let diagnostics = validate_with_fixture(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "ChoiceDefB reference should resolve under acceptedDefTypes: {diagnostics:?}"
    );
}

#[test]
fn list_ref_warns_when_not_found_in_any_accepted_type() {
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <listRef>
      <li>NoSuchDef</li>
    </listRef>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "missing reference should produce validation_unresolved_reference: {diagnostics:?}"
    );
}

#[test]
fn unknown_accepted_def_type_does_not_prevent_pack_load() {
    // listRef includes "UnknownFutureDef" in acceptedDefTypes.
    // The pack must still load, and a known-type reference must still resolve.
    use crate::schema_pack::build_schema_catalog;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack/accepted_def_types");
    let result = build_schema_catalog(&[fixture_path], None);
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "schema_pack_load_error"),
        "pack with unknown acceptedDefTypes must load without error: {:?}",
        result.diagnostics
    );
    // A ChoiceDefA ref to listRef must still resolve despite the unknown third type.
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <listRef>
      <li>Alpha</li>
    </listRef>
  </TestDef>
</Defs>"#;
    let def_index = make_choice_a_index("Alpha");
    let diagnostics = validate_with_fixture(src, &def_index);
    assert!(
        !diagnostics.iter().any(|d| d.code == "validation_unresolved_reference"),
        "known accepted type ref must still resolve when other accepted types are unknown: {diagnostics:?}"
    );
}

// --- Keyed-value-list key reference validation with acceptedDefTypes ---

#[test]
fn keyed_ref_key_resolves_under_accepted_type_a() {
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <keyedRef>
      <Alpha>5</Alpha>
    </keyedRef>
  </TestDef>
</Defs>"#;
    let def_index = make_choice_a_index("Alpha");
    let diagnostics = validate_with_fixture(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_map_key"),
        "ChoiceDefA key should resolve under keyReference.acceptedDefTypes: {diagnostics:?}"
    );
}

#[test]
fn keyed_ref_key_warns_when_not_found_in_any_accepted_type() {
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <keyedRef>
      <NoSuchDef>5</NoSuchDef>
    </keyedRef>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_map_key"),
        "missing key should produce validation_unresolved_map_key: {diagnostics:?}"
    );
}
