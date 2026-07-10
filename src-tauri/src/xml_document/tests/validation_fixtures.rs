use super::*;

#[test]
fn known_top_level_field_produces_no_unknown_field_diagnostic() {
    let src = r#"<Defs>
  <TestDef>
    <defName>SomeDef</defName>
    <knownScalar>hello</knownScalar>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "generic_validation", &empty_def_index());
    let unknown = diagnostics.iter().find(|d| {
        d.code == "validation_unknown_field" && d.field_path.as_deref() == Some("knownScalar")
    });
    assert!(
        unknown.is_none(),
        "knownScalar must not produce validation_unknown_field: {diagnostics:?}"
    );
}

#[test]
fn unknown_top_level_field_produces_unknown_field_diagnostic() {
    let src = r#"<Defs>
  <TestDef>
    <defName>SomeDef</defName>
    <unknownField>value</unknownField>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "generic_validation", &empty_def_index());
    assert!(
        diagnostics.iter().any(|d| {
            d.code == "validation_unknown_field" && d.field_path.as_deref() == Some("unknownField")
        }),
        "unknownField must produce validation_unknown_field: {diagnostics:?}"
    );
}

#[test]
fn known_nested_object_field_produces_no_unknown_object_field_diagnostic() {
    let src = r#"<Defs>
  <TestDef>
    <defName>SomeDef</defName>
    <knownNested>
      <nestedKnown>value</nestedKnown>
    </knownNested>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "generic_validation", &empty_def_index());
    let unknown = diagnostics.iter().find(|d| {
        d.code == "validation_unknown_object_field"
            && d.field_path
                .as_deref()
                .map(|p| p.contains("nestedKnown"))
                .unwrap_or(false)
    });
    assert!(
        unknown.is_none(),
        "nestedKnown must not produce validation_unknown_object_field: {diagnostics:?}"
    );
}

#[test]
fn unknown_nested_object_field_produces_unknown_object_field_diagnostic() {
    let src = r#"<Defs>
  <TestDef>
    <defName>SomeDef</defName>
    <knownNested>
      <unknownNestedField>value</unknownNestedField>
    </knownNested>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "generic_validation", &empty_def_index());
    assert!(
        diagnostics.iter().any(|d| {
            d.code == "validation_unknown_object_field"
                && d.field_path
                    .as_deref()
                    .map(|p| p.contains("unknownNestedField"))
                    .unwrap_or(false)
        }),
        "unknownNestedField must produce validation_unknown_object_field: {diagnostics:?}"
    );
}

#[test]
fn object_list_known_item_field_is_schema_validated() {
    let src = r#"<Defs>
  <TestDef>
    <defName>SomeDef</defName>
    <knownList>
      <li>
        <itemKnown>value</itemKnown>
      </li>
    </knownList>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "generic_validation", &empty_def_index());
    let unknown = diagnostics.iter().find(|d| {
        d.code == "validation_unknown_object_field"
            && d.field_path
                .as_deref()
                .map(|p| p.contains("itemKnown"))
                .unwrap_or(false)
    });
    assert!(
        unknown.is_none(),
        "itemKnown inside list item must not produce validation_unknown_object_field: {diagnostics:?}"
    );
}

#[test]
fn known_discriminator_class_suppresses_unknown_variant_warning() {
    let src = r#"<Defs>
  <TestDef>
    <defName>SomeDef</defName>
    <knownDiscriminated>
      <li Class="VariantA">
        <variantAField>hello</variantAField>
      </li>
    </knownDiscriminated>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "generic_validation", &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unknown_object_class"),
        "known discriminator class VariantA must not produce validation_unknown_object_class: {diagnostics:?}"
    );
}
