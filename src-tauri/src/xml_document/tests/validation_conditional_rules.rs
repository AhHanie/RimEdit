use super::*;

// --- greaterThan + schema default fallback ---

#[test]
fn required_when_no_diagnostic_when_condition_false() {
    // threshold absent; schema default is 0; condition "threshold greaterThan 0" is false.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "conditional_rules", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "no conditional-required diagnostic when condition is false: {rule_diags:?}"
    );
}

#[test]
fn required_when_emits_diagnostic_when_condition_true_and_field_absent() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <threshold>0.5</threshold>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "conditional_rules", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "conditional-required diagnostic expected when condition true and field absent: {diagnostics:?}"
    );
}

#[test]
fn required_when_no_diagnostic_when_field_present() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <threshold>0.5</threshold>
    <options>
      <li>100</li>
    </options>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "conditional_rules", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "no diagnostic when required target field is present: {rule_diags:?}"
    );
}

#[test]
fn required_when_condition_uses_schema_default() {
    // Explicitly set threshold to 0; condition must be false via default comparison.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <threshold>0</threshold>
  </TestDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "conditional_rules", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "threshold 0 must not trigger the greaterThan 0 condition: {rule_diags:?}"
    );
}

// --- equals operator ---

#[test]
fn required_when_equals_operator_triggers_correctly() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <mode>special</mode>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_equals", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "equals operator should trigger when mode == 'special': {diagnostics:?}"
    );
}

#[test]
fn required_when_equals_operator_no_trigger_when_not_equal() {
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <mode>normal</mode>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_equals", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "equals operator must not trigger when value does not match: {rule_diags:?}"
    );
}

// --- schema inheritance ---

#[test]
fn required_when_inherited_rules_apply() {
    // ChildDef inherits from TestDef. Rules from TestDef must apply.
    let src = r#"<Defs>
  <ChildDef>
    <defName>T</defName>
    <threshold>0.5</threshold>
  </ChildDef>
</Defs>"#;
    let diagnostics = validate_test_xml_with_fixture(src, "conditional_rules", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "rules inherited from parent def type must apply to child def: {diagnostics:?}"
    );
}

// --- load-time diagnostics ---

#[test]
fn schema_load_warns_for_unknown_rule_field() {
    use crate::schema_pack::build_schema_catalog;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack/conditional_rules_bad_field");
    let result = build_schema_catalog(&[fixture_path], None);
    let warn = result.diagnostics.iter().find(|d| {
        d.code == "schema_pack_validation_rule_unknown_field"
            || d.code == "schema_pack_validation_rule_unknown_condition_field"
    });
    assert!(
        warn.is_some(),
        "loading a rule with unknown field references must emit a schema-load warning: {:?}",
        result.diagnostics
    );
}

// --- greaterThanOrEqual ---

#[test]
fn greater_than_or_equal_triggers_at_exact_boundary() {
    // score = 5 satisfies score >= 5.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <score>5</score>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_numeric_ops", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "greaterThanOrEqual must trigger when value equals boundary: {diagnostics:?}"
    );
}

#[test]
fn greater_than_or_equal_no_trigger_below_boundary() {
    // score = 4 does NOT satisfy score >= 5; default priority=10 and chance=1.0 don't trigger other rules.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <score>4</score>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_numeric_ops", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "greaterThanOrEqual must not trigger when value is below boundary: {rule_diags:?}"
    );
}

// --- lessThan ---

#[test]
fn less_than_triggers_below_boundary() {
    // priority = 4 satisfies priority < 5; default score=0 doesn't trigger gte-rule.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <priority>4</priority>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_numeric_ops", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "lessThan must trigger when value is below boundary: {diagnostics:?}"
    );
}

#[test]
fn less_than_no_trigger_at_boundary() {
    // priority = 5 does NOT satisfy priority < 5.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <priority>5</priority>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_numeric_ops", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "lessThan must not trigger at exact boundary: {rule_diags:?}"
    );
}

// --- lessThanOrEqual ---

#[test]
fn less_than_or_equal_triggers_at_boundary() {
    // chance = 0.0 satisfies chance <= 0.0; defaults don't trigger other rules.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <chance>0.0</chance>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_numeric_ops", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "lessThanOrEqual must trigger when value equals boundary: {diagnostics:?}"
    );
}

#[test]
fn less_than_or_equal_no_trigger_above_boundary() {
    // chance = 0.1 does NOT satisfy chance <= 0.0.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <chance>0.1</chance>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_numeric_ops", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "lessThanOrEqual must not trigger above boundary: {rule_diags:?}"
    );
}

// --- notEquals ---

#[test]
fn not_equals_triggers_when_value_differs() {
    // mode = "special" satisfies mode != "normal"; defaults don't trigger numeric rules.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <mode>special</mode>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_numeric_ops", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "notEquals must trigger when value differs from expected: {diagnostics:?}"
    );
}

#[test]
fn not_equals_no_trigger_when_value_matches() {
    // mode = "normal" does NOT satisfy mode != "normal".
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <mode>normal</mode>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_numeric_ops", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "notEquals must not trigger when value matches expected: {rule_diags:?}"
    );
}

// --- present operator ---

#[test]
fn present_triggers_when_field_exists() {
    // sourceField is present; target is absent -> diagnostic.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <sourceField>value</sourceField>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_present_op", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("target")
    });
    assert!(
        rule_diag.is_some(),
        "present operator must trigger when the condition field exists: {diagnostics:?}"
    );
}

#[test]
fn present_no_trigger_when_field_absent() {
    // sourceField is absent; condition is false; no diagnostic.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_present_op", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("target")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "present operator must not trigger when the condition field is absent: {rule_diags:?}"
    );
}

#[test]
fn present_no_trigger_when_target_also_present() {
    // sourceField present and target present -> no diagnostic.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <sourceField>value</sourceField>
    <target>
      <li>1</li>
    </target>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_present_op", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("target")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "no diagnostic when condition true but target field is also present: {rule_diags:?}"
    );
}

// --- absent operator ---

#[test]
fn absent_triggers_when_field_missing() {
    // controlField absent; target absent -> diagnostic.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_absent_op", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("target")
    });
    assert!(
        rule_diag.is_some(),
        "absent operator must trigger when the condition field is missing: {diagnostics:?}"
    );
}

#[test]
fn absent_no_trigger_when_field_present() {
    // controlField present; condition is false.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <controlField>value</controlField>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_absent_op", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("target")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "absent operator must not trigger when the condition field is present: {rule_diags:?}"
    );
}

// --- boolean condition value ---

#[test]
fn boolean_condition_triggers_when_true() {
    // enabled = true; options absent -> diagnostic.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <enabled>true</enabled>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_bool_op", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "boolean equals true must trigger when field is 'true': {diagnostics:?}"
    );
}

#[test]
fn boolean_condition_no_trigger_when_false() {
    // enabled = false (explicit); condition false.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <enabled>false</enabled>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_bool_op", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "boolean condition must not trigger when field is 'false': {rule_diags:?}"
    );
}

#[test]
fn boolean_condition_uses_schema_default_false() {
    // enabled absent; schema default is false; condition false -> no diagnostic.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_bool_op", &empty_def_index());
    let rule_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_missing_required_field"
                && d.field_path.as_deref() == Some("options")
        })
        .collect();
    assert!(
        rule_diags.is_empty(),
        "boolean default false must not trigger equals-true condition: {rule_diags:?}"
    );
}

// --- rule merge-by-id: higher priority replaces lower priority ---

#[test]
fn rule_merge_by_id_higher_priority_wins() {
    use crate::schema_pack::{build_schema_catalog, ValidationRule};
    let base_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema_pack");
    let low_path = base_path.join("conditional_rules_override_low");
    let high_path = base_path.join("conditional_rules_override_high");
    let result = build_schema_catalog(&[low_path, high_path], None);
    let override_def = result
        .catalog
        .def_types
        .get("OverrideDef")
        .expect("OverrideDef must be in catalog");
    let rule = override_def
        .validation_rules
        .get("shared-rule")
        .expect("shared-rule must be in catalog");
    let message = match rule {
        ValidationRule::RequiredWhen { message, .. } => message.as_str(),
    };
    assert_eq!(
        message, "From high priority pack.",
        "higher-priority pack must replace lower-priority rule with same id"
    );
}

// --- XML-alias handling in condition field lookup ---

#[test]
fn condition_field_evaluated_via_xml_alias() {
    // The XML uses <legacyName> which is an alias for the 'primaryName' condition field.
    // The condition "primaryName > 0" should read the alias element and fire.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <legacyName>1.5</legacyName>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_alias", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "condition field must be resolvable via its XML alias: {diagnostics:?}"
    );
}

#[test]
fn condition_field_evaluated_via_canonical_name_still_works() {
    // Sanity check: canonical name also triggers the rule.
    let src = r#"<Defs>
  <TestDef>
    <defName>T</defName>
    <primaryName>1.5</primaryName>
  </TestDef>
</Defs>"#;
    let diagnostics =
        validate_test_xml_with_fixture(src, "conditional_rules_alias", &empty_def_index());
    let rule_diag = diagnostics.iter().find(|d| {
        d.code == "validation_missing_required_field" && d.field_path.as_deref() == Some("options")
    });
    assert!(
        rule_diag.is_some(),
        "canonical field name must also trigger the condition: {diagnostics:?}"
    );
}
