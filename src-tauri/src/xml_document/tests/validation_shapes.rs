use super::*;

#[test]
fn validation_shape_typed_reference_list_rejects_li_children() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <descriptionHyperlinks>
      <li>SomeDef</li>
    </descriptionHyperlinks>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let mismatch = diagnostics
        .iter()
        .find(|d| d.code == "validation_field_shape_mismatch");
    assert!(
        mismatch.is_some(),
        "<li> in typedReferenceList must produce validation_field_shape_mismatch: {diagnostics:?}"
    );
    assert!(
        !mismatch.unwrap().blocking,
        "shape mismatch must not be blocking"
    );
}

#[test]
fn validation_shape_flagstext_rejects_element_children() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <apparel>
      <developmentalStageFilter>
        <Child/>
      </developmentalStageFilter>
    </apparel>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let mismatch = diagnostics
        .iter()
        .find(|d| d.code == "validation_field_shape_mismatch");
    assert!(
        mismatch.is_some(),
        "element child in flagsText must produce validation_field_shape_mismatch: {diagnostics:?}"
    );
    assert!(
        !mismatch.unwrap().blocking,
        "flagsText shape mismatch must not be blocking"
    );
}

#[test]
fn validation_flags_text_emits_warning_for_unknown_token() {
    // ThoughtDef.developmentalStageFilter allows: None, Newborn, Baby, Child, Adult.
    // "Toddler" is not in that list and must produce a warning.
    let src = r#"<Defs>
  <ThoughtDef>
    <defName>X</defName>
    <developmentalStageFilter>Child, Toddler</developmentalStageFilter>
  </ThoughtDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let warn = diagnostics
        .iter()
        .find(|d| d.code == "validation_flags_text_unknown_value");
    assert!(
        warn.is_some(),
        "unknown flagsText token must produce validation_flags_text_unknown_value: {diagnostics:?}"
    );
    assert!(
        !warn.unwrap().blocking,
        "flags_text_unknown_value must not be blocking"
    );
}

#[test]
fn validation_flags_text_accepts_valid_tokens() {
    let src = r#"<Defs>
  <ThoughtDef>
    <defName>X</defName>
    <developmentalStageFilter>Child, Adult</developmentalStageFilter>
  </ThoughtDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let warn = diagnostics
        .iter()
        .find(|d| d.code == "validation_flags_text_unknown_value");
    assert!(
        warn.is_none(),
        "valid tokens must not produce validation_flags_text_unknown_value: {diagnostics:?}"
    );
}

#[test]
fn validation_keyed_value_list_rejects_li_children() {
    let src = r#"<Defs>
  <RecipeDef>
    <defName>MakeThing</defName>
    <skillRequirements>
      <li>8</li>
    </skillRequirements>
  </RecipeDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_field_shape_mismatch"),
        "<li> in keyedValueList must produce validation_field_shape_mismatch: {diagnostics:?}"
    );
}

#[test]
fn validation_object_list_unknown_class_with_allow_unknown_falls_back_to_base_schema() {
    // knownDiscriminated uses DiscriminatedBase which has allowUnknown=true. An unrecognised
    // Class must not produce validation_unknown_object_class, but any field absent from
    // DiscriminatedBase must still produce validation_unknown_object_field.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <knownDiscriminated>
      <li Class="UnrecognisedVariant">
        <unknownExtraField>some-value</unknownExtraField>
      </li>
    </knownDiscriminated>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "generic_validation", &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unknown_object_class"),
        "allowUnknown=true must suppress comp class warning: {diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|d| {
            d.code == "validation_unknown_object_field"
                && d.field_path
                    .as_deref()
                    .map(|p| p.contains("unknownExtraField"))
                    .unwrap_or(false)
        }),
        "field absent from base schema must produce validation_unknown_object_field: {diagnostics:?}"
    );
}

#[test]
fn validation_missing_required_class_warns_and_validates_base_fields() {
    // StrictCompProperties has allowMissing=false. A <li> with no Class attribute must emit
    // validation_missing_required_class (non-blocking) and still validate base fields.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <strictComps>
      <li>
        <baseField>not-an-integer</baseField>
      </li>
    </strictComps>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "validation_regression", &empty_def_index());
    let missing_warn = diagnostics
        .iter()
        .find(|d| d.code == "validation_missing_required_class");
    assert!(
        missing_warn.is_some(),
        "missing Class with allowMissing=false must produce validation_missing_required_class: {diagnostics:?}"
    );
    assert!(
        !missing_warn.unwrap().blocking,
        "missing required class warning must not be blocking: {diagnostics:?}"
    );
    // Base-field type check must still run.
    assert!(
        diagnostics.iter().any(|d| {
            d.code == "validation_field_type_mismatch"
                && d.field_path
                    .as_deref()
                    .map(|p| p.contains("baseField"))
                    .unwrap_or(false)
        }),
        "base field type check must still run when Class is missing: {diagnostics:?}"
    );
}

#[test]
fn validation_unknown_object_class_warns_and_validates_base_fields() {
    // StrictCompProperties has allowUnknown=false; UnknownClass is not a known variant.
    // The warning must be emitted (non-blocking), AND base field type validation must still run.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <strictComps>
      <li Class="UnknownClass">
        <baseField>not-an-integer</baseField>
      </li>
    </strictComps>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "validation_regression", &empty_def_index());
    let comp_warn = diagnostics
        .iter()
        .find(|d| d.code == "validation_unknown_object_class");
    assert!(
        comp_warn.is_some(),
        "unknown class with allowUnknown=false must produce validation_unknown_object_class: {diagnostics:?}"
    );
    assert!(
        !comp_warn.unwrap().blocking,
        "unknown comp class warning must not be blocking: {diagnostics:?}"
    );
    // Base-field type check must still run; baseField is integer, "not-an-integer" is invalid.
    assert!(
        diagnostics.iter().any(|d| {
            d.code == "validation_field_type_mismatch"
                && d.field_path
                    .as_deref()
                    .map(|p| p == "strictComps[li].baseField")
                    .unwrap_or(false)
        }),
        "base field type check must run after unknown class, with path strictComps[li].baseField: {diagnostics:?}"
    );
}

#[test]
fn validation_missing_required_field_emits_diagnostic() {
    // TestDef.requiredTarget is required with xml="element". A TestDef without
    // <requiredTarget> must produce validation_missing_required_field.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "validation_regression", &empty_def_index());
    let missing = diagnostics
        .iter()
        .find(|d| d.code == "validation_missing_required_field");
    assert!(
        missing.is_some(),
        "absent required field must produce validation_missing_required_field: {diagnostics:?}"
    );
    assert!(
        !missing.unwrap().blocking,
        "validation_missing_required_field must not be blocking: {diagnostics:?}"
    );
    assert!(
        missing
            .unwrap()
            .field_path
            .as_deref()
            .map(|p| p == "requiredTarget")
            .unwrap_or(false),
        "diagnostic must identify the missing field: {diagnostics:?}"
    );
}

#[test]
fn validation_single_object_missing_class_warns() {
    // singleObject uses StrictCompProperties which has allowMissing=false. A bare
    // <singleObject /> with no Class attribute must emit validation_missing_required_class.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <requiredTarget>SomeThing</requiredTarget>
    <singleObject />
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "validation_regression", &empty_def_index());
    let warn = diagnostics
        .iter()
        .find(|d| d.code == "validation_missing_required_class");
    assert!(
        warn.is_some(),
        "single object without Class must emit validation_missing_required_class: {diagnostics:?}"
    );
    assert!(
        !warn.unwrap().blocking,
        "validation_missing_required_class on single object must not be blocking: {diagnostics:?}"
    );
}
