use super::*;

fn validate_with_fixture(src: &str, def_index: &DefIndex) -> Vec<ValidationDiagnostic> {
    validate_test_xml_with_fixture(src, "keyed_value_defaults", def_index)
}

#[test]
fn empty_integer_value_with_default_is_valid() {
    // <Key/> has no text - intValueList.amount has defaultValue:0 so this must be accepted.
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <intValueList>
      <SomeKey/>
    </intValueList>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "empty keyed value with integer default must not produce type mismatch: {diagnostics:?}"
    );
}

#[test]
fn empty_range_value_with_default_is_valid() {
    // rangeValueList.range has defaultValue:"0~0" so empty text must be accepted.
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <rangeValueList>
      <Shooting/>
    </rangeValueList>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "empty keyed range value with default must not produce type mismatch: {diagnostics:?}"
    );
}

#[test]
fn non_empty_range_value_is_validated_normally() {
    // A valid range "14~18" must produce no type mismatch.
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <rangeValueList>
      <Shooting>14~18</Shooting>
    </rangeValueList>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "valid range value must produce no type mismatch: {diagnostics:?}"
    );
}

#[test]
fn empty_value_without_schema_default_is_invalid() {
    // noDefaultList.count has no defaultValue - empty text must produce type mismatch.
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <noDefaultList>
      <SomeKey/>
    </noDefaultList>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "empty keyed value with no default must produce type mismatch: {diagnostics:?}"
    );
}

#[test]
fn malformed_non_empty_value_reports_type_mismatch() {
    // A non-empty, non-integer value in intValueList must still fail validation.
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <intValueList>
      <SomeKey>notanumber</SomeKey>
    </intValueList>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "malformed non-empty value must produce type mismatch: {diagnostics:?}"
    );
}

#[test]
fn empty_value_with_invalid_schema_default_is_invalid() {
    // badDefaultList.amount has defaultValue:"notanumber" for integer type.
    // An empty element text must be rejected because the default itself is not a valid integer.
    let src = r#"<Defs>
  <TestDef>
    <defName>X</defName>
    <badDefaultList>
      <SomeKey/>
    </badDefaultList>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_with_fixture(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "empty keyed value with invalid schema default must produce type mismatch: {diagnostics:?}"
    );
}
