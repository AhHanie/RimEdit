use std::collections::BTreeMap;

use crate::def_index::{DefIdentityKey, DefIndex, IndexedDef, IndexedDefSource, IndexedSourceKind};
use crate::patches::{
    complete_patch_xpath, XPathCompletionItemKind, XPathDiagnosticSeverity, XPathTarget,
};
use crate::project_model::SourceType;
use crate::schema_pack::{
    DefTypeSchema, FieldSchema, FieldType, FieldTypeKind, SchemaCatalog, XmlFieldShape,
};

fn field(xml_aliases: &[&str]) -> FieldSchema {
    FieldSchema {
        label: None,
        description: None,
        field_type: FieldType {
            kind: FieldTypeKind::String,
            schema_ref: None,
            reference: None,
        },
        required: false,
        default_value: None,
        examples: Vec::new(),
        validation_hints: None,
        reference: None,
        key_reference: None,
        typed_reference: None,
        key_field: None,
        value_field: None,
        default_value_field: None,
        value_type: None,
        repeatable: false,
        xml: XmlFieldShape::Element,
        source_pack_id: None,
        items: None,
        flags: false,
        default_collapsed: None,
        xml_aliases: xml_aliases.iter().map(|s| s.to_string()).collect(),
        role: None,
    }
}

fn def_type_schema(inherits: &[&str], fields: &[(&str, FieldSchema)]) -> DefTypeSchema {
    DefTypeSchema {
        label: None,
        description: None,
        inherits: inherits.iter().map(|s| s.to_string()).collect(),
        abstract_type: false,
        field_order: Vec::new(),
        fields: fields
            .iter()
            .cloned()
            .map(|(name, field)| (name.to_string(), field))
            .collect(),
        templates: BTreeMap::new(),
        validation_rules: BTreeMap::new(),
        form_views: BTreeMap::new(),
    }
}

/// A small synthetic schema: `Def` (defName, label) <- `BuildableDef` (statBases, costList) <-
/// `ThingDef` (comps, graphicData[alias: graphic]), plus an unrelated `ThingDefStyleUnlockDef`
/// sharing the `Def`/`ThingDef` name prefix (to exercise prefix-only, not substring, matching).
fn test_catalog() -> SchemaCatalog {
    let mut def_types = BTreeMap::new();
    def_types.insert(
        "Def".to_string(),
        def_type_schema(
            &[],
            &[("defName", field(&[])), ("label", field(&["Label"]))],
        ),
    );
    def_types.insert(
        "BuildableDef".to_string(),
        def_type_schema(
            &["Def"],
            &[("statBases", field(&[])), ("costList", field(&[]))],
        ),
    );
    def_types.insert(
        "ThingDef".to_string(),
        def_type_schema(
            &["BuildableDef"],
            &[("comps", field(&[])), ("graphicData", field(&["graphic"]))],
        ),
    );
    def_types.insert(
        "ThingDefStyleUnlockDef".to_string(),
        def_type_schema(&[], &[]),
    );

    SchemaCatalog {
        format_version: 1,
        packs: Vec::new(),
        def_types,
        object_types: BTreeMap::new(),
        patch_operations: BTreeMap::new(),
    }
}

fn indexed_def_source() -> IndexedDefSource {
    IndexedDefSource {
        location_id: "project".to_string(),
        location_name: "Project".to_string(),
        source_kind: IndexedSourceKind::Project,
        source_type: SourceType::Folder,
        read_only: false,
        mod_id: None,
        game_version: None,
        expansion_name: None,
    }
}

fn indexed_def(def_type: &str, def_name: &str) -> IndexedDef {
    IndexedDef {
        key: DefIdentityKey {
            def_type: def_type.to_string(),
            def_name: def_name.to_string(),
        },
        def_type: def_type.to_string(),
        def_name: def_name.to_string(),
        label: None,
        parent_name: None,
        relative_path: "Defs/Things.xml".to_string(),
        node_id: None,
        line: None,
        column: None,
        source: indexed_def_source(),
        fields: Vec::new(),
        def_name_lower: String::new(),
        label_lower: String::new(),
    }
}

fn test_def_index() -> DefIndex {
    let mut index = DefIndex {
        defs: vec![
            indexed_def("ThingDef", "Wall"),
            indexed_def("ThingDef", "WallStone"),
            indexed_def("ThingDef", "Door"),
        ],
        errors: Vec::new(),
        built_at_unix_ms: 0,
        by_type: Default::default(),
    };
    index.rebuild_computed_fields();
    index
}

#[test]
fn suggests_defs_root_when_empty_or_slash() {
    let catalog = test_catalog();
    let index = test_def_index();
    for input in ["", "/"] {
        let result = complete_patch_xpath(&catalog, &index, input);
        assert_eq!(result.items.len(), 1, "input {input:?}");
        assert_eq!(result.items[0].insert_text, "Defs");
        assert_eq!(result.items[0].kind, XPathCompletionItemKind::Root);
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.replace_from, input.len());
    }
}

#[test]
fn completes_def_type_names_from_schema_catalog() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/Thing");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"ThingDef"), "{labels:?}");
    assert!(labels.contains(&"ThingDefStyleUnlockDef"), "{labels:?}");
    assert!(!labels.contains(&"BuildableDef"), "{labels:?}");
    assert!(!labels.contains(&"Def"), "{labels:?}");
    assert_eq!(result.replace_from, "Defs/".len());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn completes_def_names_from_def_index_in_defname_predicate() {
    let catalog = test_catalog();
    let index = test_def_index();
    let input = r#"Defs/ThingDef[defName="Wa"#;
    let result = complete_patch_xpath(&catalog, &index, input);

    let names: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(names.contains(&"Wall"), "{names:?}");
    assert!(names.contains(&"WallStone"), "{names:?}");
    assert!(!names.contains(&"Door"), "{names:?}");

    // Selecting a suggestion should close out the predicate (quote + bracket).
    let wall = result
        .items
        .iter()
        .find(|i| i.label == "Wall")
        .expect("Wall suggested");
    assert_eq!(wall.insert_text, "Wall\"]");
    assert_eq!(wall.kind, XPathCompletionItemKind::DefName);

    // replace_from should point right after the opening quote.
    assert_eq!(&input[..result.replace_from], r#"Defs/ThingDef[defName=""#);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn offers_predicate_key_templates_right_after_open_bracket() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef[");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"defName=\"...\""), "{labels:?}");
    assert!(labels.contains(&"@Name=\"...\""), "{labels:?}");
    assert!(labels.contains(&"@ParentName=\"...\""), "{labels:?}");
    assert!(result
        .items
        .iter()
        .all(|i| i.kind == XPathCompletionItemKind::PredicateKey));
}

#[test]
fn completes_direct_fields_after_def_type_and_predicate() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, r#"Defs/ThingDef[defName="Wall"]/"#);

    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"comps"), "{labels:?}");
    assert!(labels.contains(&"graphicData"), "{labels:?}");
    // Inherited (from BuildableDef/Def), not declared directly on ThingDef -- must be excluded.
    assert!(!labels.contains(&"statBases"), "{labels:?}");
    assert!(!labels.contains(&"costList"), "{labels:?}");
    assert!(!labels.contains(&"defName"), "{labels:?}");

    assert_eq!(
        result.target,
        XPathTarget::Def {
            def_type: "ThingDef".to_string(),
            def_name: "Wall".to_string(),
        }
    );
}

#[test]
fn completes_xml_aliases_for_direct_fields() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/gra");
    let alias = result
        .items
        .iter()
        .find(|i| i.label == "graphic")
        .expect("alias suggested");
    assert_eq!(alias.kind, XPathCompletionItemKind::FieldAlias);
    assert_eq!(alias.insert_text, "graphic");
}

#[test]
fn excludes_inherited_fields_and_diagnoses_exact_match() {
    let catalog = test_catalog();
    let index = test_def_index();
    // "statBases" is only declared on BuildableDef, an ancestor of ThingDef, not on ThingDef
    // itself -- typed out in full, it should not resolve to a value-subform field and should
    // surface the inherited-field diagnostic instead.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/statBases");
    assert!(result.resolved_field.is_none());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "xpath_autocomplete_inherited_field"),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn resolves_direct_field_for_value_subform() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/comps");
    let resolved = result.resolved_field.expect("comps should resolve");
    assert_eq!(resolved.def_type, "ThingDef");
    assert_eq!(resolved.field_name, "comps");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn resolves_field_via_alias_to_its_canonical_name() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/graphic");
    let resolved = result.resolved_field.expect("alias should resolve");
    assert_eq!(resolved.field_name, "graphicData");
}

#[test]
fn bracket_characters_inside_a_quoted_predicate_value_are_not_invalid_syntax() {
    let catalog = test_catalog();
    let index = test_def_index();
    // `contains(...)` is a real XPath function, outside our conservative subset, but the literal
    // `]` inside its quoted string argument must not be mistaken for an unmatched closing bracket.
    let result = complete_patch_xpath(&catalog, &index, r#"Defs/ThingDef[contains(defName, "]")]"#);
    assert!(result.items.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].severity,
        XPathDiagnosticSeverity::Warning,
        "a quoted bracket must not be reported as invalid syntax: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.diagnostics[0].code,
        "xpath_autocomplete_unsupported_pattern"
    );
}

#[test]
fn xpath_not_rooted_at_defs_is_unsupported_not_invalid() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "SomeOther/ThingDef");
    assert!(result.items.is_empty());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].severity,
        XPathDiagnosticSeverity::Warning
    );
    assert_eq!(
        result.diagnostics[0].code,
        "xpath_autocomplete_unsupported_root"
    );
}

#[test]
fn valid_xpath_outside_conservative_subset_is_unsupported_with_diagnostic() {
    let catalog = test_catalog();
    let index = test_def_index();
    for input in [
        "Defs/*",
        r#"Defs/ThingDef[defName="Wall"][@Name="Foo"]"#,
        "Defs/ThingDef[0]",
        "//Defs/ThingDef",
        "Defs/ThingDef/text()",
        "Defs/ThingDef/@someAttr",
        r#"Defs/ThingDef[defName=Wall]"#,
    ] {
        let result = complete_patch_xpath(&catalog, &index, input);
        assert!(
            result.items.is_empty(),
            "expected no completions for {input}"
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "expected exactly one diagnostic for {input}, got {:?}",
            result.diagnostics
        );
        assert_eq!(
            result.diagnostics[0].severity,
            XPathDiagnosticSeverity::Warning,
            "for {input}"
        );
        assert_eq!(
            result.diagnostics[0].code, "xpath_autocomplete_unsupported_pattern",
            "for {input}"
        );
    }
}

#[test]
fn only_one_field_segment_deep_is_supported() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(
        &catalog,
        &index,
        "Defs/ThingDef[defName=\"Wall\"]/comps/foo",
    );
    assert!(result.items.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "xpath_autocomplete_unsupported_pattern"));
    // Best-effort: the first field segment ("comps") still resolves for the value subform even
    // though we can't offer completions one level deeper.
    let resolved = result.resolved_field.expect("comps should still resolve");
    assert_eq!(resolved.field_name, "comps");
}

#[test]
fn invalid_xpath_stray_closing_bracket_is_an_error_not_a_warning() {
    let catalog = test_catalog();
    let index = test_def_index();
    for input in ["Defs/ThingDef]", r#"Defs/ThingDef[defName="Wall"]]"#] {
        let result = complete_patch_xpath(&catalog, &index, input);
        assert!(result.items.is_empty(), "for {input}");
        assert_eq!(result.diagnostics.len(), 1, "for {input}");
        assert_eq!(
            result.diagnostics[0].severity,
            XPathDiagnosticSeverity::Error,
            "for {input}"
        );
        assert_eq!(
            result.diagnostics[0].code, "xpath_invalid_syntax",
            "for {input}"
        );
    }
}

#[test]
fn unknown_xpath_stays_editable_never_panics() {
    let catalog = test_catalog();
    let index = test_def_index();
    // A grab-bag of odd inputs a live text field can produce mid-edit; none of these should
    // panic, and every one should still come back with *some* result (possibly with
    // diagnostics, possibly not) rather than an error the caller has to unwrap.
    for input in [
        "D",
        "Defs",
        "Defs/",
        "Defs/ThingDef[",
        "Defs/ThingDef[def",
        "Defs/ThingDef[@Na",
        "Defs/ThingDef[defName",
        "Defs/ThingDef[defName=",
        "Defs/ThingDef[defName=\"",
        "[[[",
        "Defs/ThingDef[@Name=\"Foo\"]/",
        "totally not xpath at all !!",
    ] {
        let _ = complete_patch_xpath(&catalog, &index, input);
    }
}

#[test]
fn name_and_parentname_predicates_resolve_def_type_but_no_specific_def_name() {
    let catalog = test_catalog();
    let index = test_def_index();
    for input in [
        r#"Defs/ThingDef[@Name="BaseThing"]"#,
        r#"Defs/ThingDef[@ParentName="BaseThing"]"#,
    ] {
        let result = complete_patch_xpath(&catalog, &index, input);
        assert_eq!(
            result.target,
            XPathTarget::DefType {
                def_type: "ThingDef".to_string(),
            },
            "for {input}"
        );
        assert!(
            result.diagnostics.is_empty(),
            "for {input}: {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn field_completion_offers_nothing_extra_for_unknown_def_type() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/NotARealDefType/");
    assert!(result.items.is_empty());
    assert!(result.diagnostics.is_empty());
    assert_eq!(
        result.target,
        XPathTarget::DefType {
            def_type: "NotARealDefType".to_string(),
        }
    );
}
