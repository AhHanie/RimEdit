use super::*;

// Color validation tests exercise the built-in ColorDef schema (color field, type "color").

#[test]
fn color_field_kind_parses_integer_rgb() {
    let src = r#"<Defs>
  <ColorDef>
    <defName>TestColor</defName>
    <color>(118, 49, 57)</color>
  </ColorDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "integer RGB color must not produce a type mismatch: {diagnostics:?}"
    );
}

#[test]
fn color_field_kind_parses_float_rgb() {
    let src = r#"<Defs>
  <ColorDef>
    <defName>TestColor</defName>
    <color>(0.1, 0.1, 0.1)</color>
  </ColorDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "float RGB color must not produce a type mismatch: {diagnostics:?}"
    );
}

#[test]
fn color_field_kind_parses_float_rgba() {
    let src = r#"<Defs>
  <ColorDef>
    <defName>TestColor</defName>
    <color>(0.68, 0.68, 0.68, 0.4)</color>
  </ColorDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "float RGBA color must not produce a type mismatch: {diagnostics:?}"
    );
}

#[test]
fn color_field_kind_parses_integer_rgba() {
    let src = r#"<Defs>
  <ColorDef>
    <defName>TestColor</defName>
    <color>(118, 49, 57, 200)</color>
  </ColorDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "integer RGBA color must not produce a type mismatch: {diagnostics:?}"
    );
}

#[test]
fn color_field_kind_rejects_malformed_string() {
    let src = r#"<Defs>
  <ColorDef>
    <defName>TestColor</defName>
    <color>not a color</color>
  </ColorDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "malformed color string must produce a type mismatch: {diagnostics:?}"
    );
}
