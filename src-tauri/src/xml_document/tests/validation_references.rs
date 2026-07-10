use super::*;

#[test]
fn validation_unresolved_optional_scalar_reference_is_warning() {
    let src = r#"<Defs><RecipeDef><defName>Cook</defName><workSpeedStat>NoSuchStat</workSpeedStat></RecipeDef></Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_unresolved_reference")
        .expect("expected unresolved reference diagnostic");
    assert!(!diagnostic.blocking);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.field_path.as_deref(), Some("workSpeedStat"));
}

#[test]
fn validation_resolved_scalar_reference_has_no_diagnostic() {
    let src = r#"<Defs><RecipeDef><defName>Cook</defName><workSpeedStat>GeneralLaborSpeed</workSpeedStat></RecipeDef></Defs>"#;
    let mut stat_def = indexed_test_def("core_stats.xml", IndexedSourceKind::Source);
    stat_def.key.def_type = "StatDef".to_string();
    stat_def.key.def_name = "GeneralLaborSpeed".to_string();
    stat_def.def_type = "StatDef".to_string();
    stat_def.def_name = "GeneralLaborSpeed".to_string();
    let def_index = DefIndex {
        defs: vec![stat_def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    let diagnostics = validate_test_xml(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "resolved reference should produce no diagnostic"
    );
}

#[test]
fn validation_unresolved_list_item_reference_is_warning() {
    let src = r#"<Defs><ThingDef><defName>Workbench</defName><recipes><li>NoSuchRecipe</li></recipes></ThingDef></Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_unresolved_reference")
        .expect("expected unresolved list reference diagnostic");
    assert!(!diagnostic.blocking);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.field_path.as_deref(), Some("recipes"));
}

#[test]
fn validation_resolved_list_item_reference_has_no_diagnostic() {
    let src = r#"<Defs><ThingDef><defName>Workbench</defName><recipes><li>MakeSomething</li></recipes></ThingDef></Defs>"#;
    let mut recipe_def = indexed_test_def("recipes.xml", IndexedSourceKind::Source);
    recipe_def.key.def_type = "RecipeDef".to_string();
    recipe_def.key.def_name = "MakeSomething".to_string();
    recipe_def.def_type = "RecipeDef".to_string();
    recipe_def.def_name = "MakeSomething".to_string();
    let def_index = DefIndex {
        defs: vec![recipe_def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    let diagnostics = validate_test_xml(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "resolved list item should produce no diagnostic"
    );
}

#[test]
fn validation_named_map_key_reference_unresolved_emits_warning() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <equippedStatOffsets>
      <NoSuchStat>1.5</NoSuchStat>
    </equippedStatOffsets>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let diag = diagnostics
        .iter()
        .find(|d| d.code == "validation_unresolved_map_key");
    assert!(
        diag.is_some(),
        "unresolved equippedStatOffsets key must produce validation_unresolved_map_key, got: {diagnostics:?}"
    );
    assert!(
        !diag.unwrap().blocking,
        "unresolved map key must be a warning, not blocking"
    );
}

#[test]
fn validation_named_map_key_reference_resolved_has_no_diagnostic() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>X</defName>
    <equippedStatOffsets>
      <MeleeDPS>1.5</MeleeDPS>
    </equippedStatOffsets>
  </ThingDef>
</Defs>"#;
    let mut stat_def = indexed_test_def("stats.xml", IndexedSourceKind::Source);
    stat_def.key.def_type = "StatDef".to_string();
    stat_def.key.def_name = "MeleeDPS".to_string();
    stat_def.def_type = "StatDef".to_string();
    stat_def.def_name = "MeleeDPS".to_string();
    let def_index = DefIndex {
        defs: vec![stat_def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    let diagnostics = validate_test_xml(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_map_key"),
        "resolved map key must produce no diagnostic"
    );
}

#[test]
fn validation_typed_reference_resolved_produces_no_diagnostic() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>Prosthetic</defName>
    <descriptionHyperlinks>
      <HediffDef>SimpleProstheticLeg</HediffDef>
    </descriptionHyperlinks>
  </ThingDef>
</Defs>"#;
    let def_index = make_typed_ref_index("HediffDef", "SimpleProstheticLeg");
    let diagnostics = validate_test_xml(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_typed_reference"),
        "resolved typed reference should produce no diagnostic: {diagnostics:?}"
    );
}

#[test]
fn validation_typed_reference_unresolved_produces_warning() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>Prosthetic</defName>
    <descriptionHyperlinks>
      <HediffDef>NoSuchHediff</HediffDef>
    </descriptionHyperlinks>
  </ThingDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    let diag = diagnostics
        .iter()
        .find(|d| d.code == "validation_unresolved_typed_reference")
        .expect("expected validation_unresolved_typed_reference diagnostic");
    assert!(!diag.blocking);
    assert_eq!(
        diag.field_path.as_deref(),
        Some("descriptionHyperlinks.HediffDef.NoSuchHediff")
    );
}

#[test]
fn validation_typed_reference_duplicate_produces_warning() {
    let src = r#"<Defs>
  <ThingDef>
    <defName>Prosthetic</defName>
    <descriptionHyperlinks>
      <HediffDef>SimpleProstheticLeg</HediffDef>
      <HediffDef>SimpleProstheticLeg</HediffDef>
    </descriptionHyperlinks>
  </ThingDef>
</Defs>"#;
    let def_index = make_typed_ref_index("HediffDef", "SimpleProstheticLeg");
    let diagnostics = validate_test_xml(src, &def_index);
    let diag = diagnostics
        .iter()
        .find(|d| d.code == "validation_duplicate_typed_reference")
        .expect("expected validation_duplicate_typed_reference diagnostic");
    assert!(!diag.blocking);
    assert_eq!(
        diag.field_path.as_deref(),
        Some("descriptionHyperlinks.HediffDef.SimpleProstheticLeg")
    );
}

#[test]
fn validation_subtype_reference_resolves_without_diagnostic() {
    // structureLayoutDef is typed as "LayoutDef" in the schema, but all real shipped defs
    // are StructureLayoutDef (a subtype).  is_reference_resolved must expand subtypes so
    // the reference does not produce a spurious validation_unresolved_reference warning.
    let src = r#"<Defs>
  <TileMutatorDef>
    <defName>TestMutator</defName>
    <structureGenParms>
      <structureLayoutDef>AncientGarrison</structureLayoutDef>
    </structureGenParms>
  </TileMutatorDef>
</Defs>"#;
    let mut layout_def = indexed_test_def("layouts.xml", IndexedSourceKind::Source);
    layout_def.key.def_type = "StructureLayoutDef".to_string();
    layout_def.key.def_name = "AncientGarrison".to_string();
    layout_def.def_type = "StructureLayoutDef".to_string();
    layout_def.def_name = "AncientGarrison".to_string();
    let def_index = DefIndex {
        defs: vec![layout_def],
        errors: vec![],
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    let diagnostics = validate_test_xml(src, &def_index);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "StructureLayoutDef subtype should resolve a LayoutDef reference field: {diagnostics:?}"
    );
}

#[test]
fn validation_subtype_reference_still_warns_when_missing() {
    // Regression: inheritance-aware resolution must still produce a warning when the
    // referenced def truly does not exist in the index under any subtype.
    let src = r#"<Defs>
  <TileMutatorDef>
    <defName>TestMutator</defName>
    <structureGenParms>
      <structureLayoutDef>NoSuchLayout</structureLayoutDef>
    </structureGenParms>
  </TileMutatorDef>
</Defs>"#;
    let diagnostics = validate_test_xml(src, &empty_def_index());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "validation_unresolved_reference"),
        "missing def should still produce validation_unresolved_reference: {diagnostics:?}"
    );
}

#[test]
fn validation_required_reference_is_blocking() {
    // requiredTarget has required=true in the TestDef fixture schema.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <requiredTarget>NonExistent</requiredTarget>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "validation_regression", &empty_def_index());
    let unresolved = diagnostics
        .iter()
        .find(|d| d.code == "validation_unresolved_reference");
    assert!(
        unresolved.is_some(),
        "required unresolved reference must produce validation_unresolved_reference: {diagnostics:?}"
    );
    assert!(
        unresolved.unwrap().blocking,
        "unresolved required reference must be blocking (error severity): {diagnostics:?}"
    );
}
