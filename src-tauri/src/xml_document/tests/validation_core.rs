use super::*;

#[test]
fn validation_missing_def_name_produces_no_diagnostic() {
    let src = r#"<Defs><ThingDef><label>steel</label></ThingDef></Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_missing_def_name"),
        "missing defName should not produce a validation_missing_def_name diagnostic"
    );
    assert!(
        !diagnostics.iter().any(|d| d.blocking),
        "missing defName should not produce any blocking diagnostic"
    );
}

#[test]
fn validation_unknown_field_is_non_blocking_warning() {
    let src = r#"<Defs><ThingDef><defName>Steel</defName><madeUp>1</madeUp></ThingDef></Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_unknown_field")
        .expect("unknown field diagnostic");
    assert!(!diagnostic.blocking);
    assert_eq!(diagnostic.field_path.as_deref(), Some("madeUp"));
}

#[test]
fn validation_field_type_mismatch_for_primitives_and_vectors() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>BadTypes</defName>
    <stackLimit>many</stackLimit>
    <uiOrder>first</uiOrder>
    <destroyable>sometimes</destroyable>
    <size>(1, nope)</size>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let mismatches = diagnostics
        .iter()
        .filter(|d| d.code == "validation_field_type_mismatch")
        .count();
    assert_eq!(mismatches, 4);
    assert!(diagnostics.iter().any(|d| d.blocking));
}

#[test]
fn validation_unknown_def_type_warns_and_skips_field_checks() {
    let src = r#"<Defs><CustomDef><defName>X</defName><madeUp>1</madeUp></CustomDef></Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "validation_unknown_def_type" && !d.blocking));
    assert!(!diagnostics
        .iter()
        .any(|d| d.code == "validation_unknown_field"));
}

#[test]
fn validation_patch_root_produces_no_unknown_def_type_warning() {
    let src = r#"<Patch>
  <Operation Class="PatchOperationAdd">
    <xpath>/Defs</xpath>
    <value><ThingDef><defName>X</defName></ThingDef></value>
  </Operation>
</Patch>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        diagnostics.is_empty(),
        "Patch root files should not be validated as Defs: {:?}",
        diagnostics
    );
}

#[test]
fn validation_list_non_li_children_emit_shape_and_type_diagnostics() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>BadList</defName>
    <recipes><recipe>MakeThing</recipe></recipes>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "validation_field_shape_mismatch"));
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "validation_field_type_mismatch" && d.blocking));
}

#[test]
fn validation_list_scalar_text_is_blocking_type_mismatch() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>ScalarList</defName>
    <recipes>MakeThing</recipes>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "validation_field_shape_mismatch"));
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "validation_field_type_mismatch" && d.blocking));
}

#[test]
fn validation_vector_requires_parenthesized_tuple() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>BadVector</defName>
    <size>1, 2</size>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_field_type_mismatch")
        .expect("vector type mismatch");
    assert!(diagnostic.blocking);
    assert_eq!(diagnostic.field_path.as_deref(), Some("size"));
}

#[test]
fn validation_duplicate_def_name_is_blocking() {
    let src = r#"<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>"#;
    let def_index = DefIndex {
        defs: vec![
            indexed_test_def("test.xml", IndexedSourceKind::Project),
            indexed_test_def("other.xml", IndexedSourceKind::Project),
        ],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    let diagnostics = validate_test_xml(src, &def_index);
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_duplicate_def_name")
        .expect("duplicate diagnostic");
    assert!(diagnostic.blocking);
}

#[test]
fn validation_source_duplicate_def_name_is_warning() {
    let src = r#"<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>"#;
    let def_index = DefIndex {
        defs: vec![indexed_test_def("source.xml", IndexedSourceKind::Source)],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    let diagnostics = validate_test_xml(src, &def_index);
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_duplicate_source_def_name")
        .expect("source duplicate warning diagnostic");
    assert!(!diagnostic.blocking);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert!(
        diagnostic.message.contains("Source"),
        "message should include location name: {}",
        diagnostic.message
    );
}

#[test]
fn validation_source_duplicate_different_def_type_is_allowed() {
    let src = r#"<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>"#;
    let recipe_source = {
        let mut def = indexed_test_def("source.xml", IndexedSourceKind::Source);
        def.key.def_type = "RecipeDef".to_string();
        def.def_type = "RecipeDef".to_string();
        def
    };
    let def_index = DefIndex {
        defs: vec![recipe_source],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    let diagnostics = validate_test_xml(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_duplicate_source_def_name"),
        "different def type should not trigger source duplicate warning"
    );
}

#[test]
fn validation_source_duplicate_without_matching_project_def_is_not_reported() {
    let src = r#"<Defs><ThingDef><defName>WoodLog</defName></ThingDef></Defs>"#;
    let def_index = DefIndex {
        defs: vec![indexed_test_def("source.xml", IndexedSourceKind::Source)],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    let diagnostics = validate_test_xml(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_duplicate_source_def_name"),
        "source Steel should not warn when project uses WoodLog"
    );
}

#[test]
fn validation_recipe_maker_skill_requirements_accept_keyed_value_list() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>AdvancedComponent</defName>
    <recipeMaker>
      <skillRequirements>
        <Crafting>8</Crafting>
      </skillRequirements>
    </recipeMaker>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &skill_def_index("Crafting"));
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "keyed skill requirement should not produce a type mismatch: {diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_map_key"),
        "indexed skill key should resolve: {diagnostics:?}"
    );
}

#[test]
fn validation_recipe_maker_skill_requirements_unknown_key_warns() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>AdvancedComponent</defName>
    <recipeMaker>
      <skillRequirements>
        <NoSuchSkill>8</NoSuchSkill>
      </skillRequirements>
    </recipeMaker>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_unresolved_map_key")
        .expect("unknown keyed-list key should warn");
    assert!(!diagnostic.blocking);
    assert_eq!(
        diagnostic.field_path.as_deref(),
        Some("recipeMaker.skillRequirements.NoSuchSkill")
    );
}

#[test]
fn validation_recipe_maker_skill_requirements_value_must_be_integer() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>AdvancedComponent</defName>
    <recipeMaker>
      <skillRequirements>
        <Crafting>expert</Crafting>
      </skillRequirements>
    </recipeMaker>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &skill_def_index("Crafting"));
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_field_type_mismatch")
        .expect("non-integer keyed-list value should fail");
    assert!(diagnostic.blocking);
    assert_eq!(
        diagnostic.field_path.as_deref(),
        Some("recipeMaker.skillRequirements.Crafting")
    );
}

#[test]
fn validation_recipe_def_skill_requirements_accept_keyed_value_list() {
    let src = r#"<Defs>
  <RecipeDef>
    <defName>MakeAdvancedComponent</defName>
    <skillRequirements>
      <Crafting>8</Crafting>
    </skillRequirements>
  </RecipeDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &skill_def_index("Crafting"));
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unknown_field"),
        "RecipeDef.skillRequirements should be in schema: {diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_field_type_mismatch"),
        "RecipeDef keyed skill requirement should not produce a type mismatch: {diagnostics:?}"
    );
}

#[test]
fn validation_nested_object_field_unknown_child_emits_warning() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <building>
      <totallyMadeUpField>1</totallyMadeUpField>
    </building>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let unknown = diagnostics
        .iter()
        .find(|d| d.code == "validation_unknown_object_field");
    assert!(
        unknown.is_some(),
        "unknown field inside building must produce validation_unknown_object_field, got: {diagnostics:?}"
    );
}

#[test]
fn validation_nested_object_boolean_type_mismatch_emits_error() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <building>
      <isNaturalRock>maybe</isNaturalRock>
    </building>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let mismatch = diagnostics
        .iter()
        .find(|d| d.code == "validation_field_type_mismatch");
    assert!(
        mismatch.is_some(),
        "boolean mismatch inside building must produce validation_field_type_mismatch, got: {diagnostics:?}"
    );
}

#[test]
fn validation_nested_object_known_fields_have_no_unknown_diagnostic() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <building>
      <isNaturalRock>false</isNaturalRock>
      <isPowerConduit>false</isPowerConduit>
    </building>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    // Known fields must not produce validation_unknown_object_field.
    for field in &["isNaturalRock", "isPowerConduit"] {
        let has_unknown = diagnostics.iter().any(|d| {
            d.code == "validation_unknown_object_field"
                && d.field_path
                    .as_deref()
                    .map(|p| p.contains(field))
                    .unwrap_or(false)
        });
        assert!(
            !has_unknown,
            "known field '{field}' inside building must not produce validation_unknown_object_field"
        );
    }
}
