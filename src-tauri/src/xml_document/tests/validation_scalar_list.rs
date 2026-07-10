use super::*;

// --- integer ---

#[test]
fn scalar_integer_list_accepts_valid_items() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <intList>
      <li>100</li>
      <li>200</li>
    </intList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "valid integer list items must not produce type mismatch: {mismatches:?}"
    );
}

#[test]
fn scalar_integer_list_rejects_non_integer_item() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <intList>
      <li>100</li>
      <li>not-a-number</li>
    </intList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert_eq!(
        mismatches.len(),
        1,
        "non-integer list item must produce exactly one type mismatch: {diagnostics:?}"
    );
    assert!(
        mismatches[0].blocking,
        "item type mismatch must be blocking"
    );
    assert_eq!(
        mismatches[0].field_path.as_deref(),
        Some("intList[1]"),
        "field path must be fieldName[index]"
    );
}

// --- float ---

#[test]
fn float_list_accepts_valid_floats() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <floatList>
      <li>0.5</li>
      <li>1.2</li>
    </floatList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "valid float list items must not produce type mismatch: {mismatches:?}"
    );
}

// --- vector2 ---

#[test]
fn scalar_vector2_list_accepts_valid_tuples() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <vec2List>
      <li>(0, 0)</li>
      <li>(1800, 1.2)</li>
    </vec2List>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "valid vector2 list items must not produce type mismatch: {mismatches:?}"
    );
}

#[test]
fn scalar_vector2_list_rejects_malformed_tuple() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <vec2List>
      <li>(0, 0)</li>
      <li>not-a-vector</li>
    </vec2List>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch" && d.blocking),
        "malformed vector2 list item must produce type mismatch: {diagnostics:?}"
    );
}

// --- empty container ---

#[test]
fn scalar_list_accepts_empty_container() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <intList></intList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "empty list container must not produce type mismatch: {mismatches:?}"
    );
}

// --- boolean ---

#[test]
fn boolean_list_accepts_valid_booleans() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <boolList>
      <li>true</li>
      <li>false</li>
      <li>True</li>
      <li>False</li>
    </boolList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "valid boolean list items must not produce type mismatch: {mismatches:?}"
    );
}

#[test]
fn boolean_list_rejects_non_boolean_item() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <boolList>
      <li>true</li>
      <li>yes</li>
    </boolList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch" && d.blocking),
        "non-boolean list item must produce type mismatch: {diagnostics:?}"
    );
}

// --- enum (always valid string; test for no false positives) ---

#[test]
fn enum_list_accepts_any_string_value() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <enumList>
      <li>Normal</li>
      <li>Manhunters</li>
      <li>UnknownModValue</li>
    </enumList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "enum list items must not produce type mismatch (enum values are not restricted by the generic validator): {mismatches:?}"
    );
}

// --- defReference (always valid string; test for no false positives) ---

#[test]
fn def_reference_list_accepts_any_string_value() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <refList>
      <li>SomeDef</li>
      <li>AnotherDef</li>
    </refList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "defReference list items must not produce type mismatch from the scalar validator: {mismatches:?}"
    );
}

// --- intRange ---

#[test]
fn int_range_list_accepts_valid_ranges() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <intRangeList>
      <li>5~10</li>
      <li>0~100</li>
      <li>42</li>
    </intRangeList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "valid intRange list items must not produce type mismatch: {mismatches:?}"
    );
}

#[test]
fn int_range_list_rejects_malformed_range() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <intRangeList>
      <li>5~10</li>
      <li>not~valid</li>
    </intRangeList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch" && d.blocking),
        "malformed intRange list item must produce type mismatch: {diagnostics:?}"
    );
}

// --- floatRange ---

#[test]
fn float_range_list_accepts_valid_ranges() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <floatRangeList>
      <li>0.5~1.5</li>
      <li>0.0~1.0</li>
    </floatRangeList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "valid floatRange list items must not produce type mismatch: {mismatches:?}"
    );
}

#[test]
fn float_range_list_rejects_malformed_range() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <floatRangeList>
      <li>0.5~1.5</li>
      <li>not~valid</li>
    </floatRangeList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch" && d.blocking),
        "malformed floatRange list item must produce type mismatch: {diagnostics:?}"
    );
}

// --- color ---

#[test]
fn color_list_accepts_valid_rgb_tuples() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <colorList>
      <li>(255, 128, 0)</li>
      <li>(0.1, 0.5, 0.9)</li>
      <li>(10, 20, 30, 255)</li>
    </colorList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    let mismatches: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .collect();
    assert!(
        mismatches.is_empty(),
        "valid color list items must not produce type mismatch: {mismatches:?}"
    );
}

#[test]
fn color_list_rejects_malformed_color() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <colorList>
      <li>(255, 128, 0)</li>
      <li>red</li>
    </colorList>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "scalar_list_validation", &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch" && d.blocking),
        "non-color string list item must produce type mismatch: {diagnostics:?}"
    );
}
