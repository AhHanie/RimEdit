use std::collections::BTreeMap;

use crate::def_index::{DefIdentityKey, DefIndex, IndexedDef, IndexedDefSource, IndexedSourceKind};
use crate::patches::{
    complete_patch_xpath, XPathCompletionItemKind, XPathDiagnosticSeverity, XPathTarget,
};
use crate::project_model::SourceType;
use crate::schema_pack::{
    DefTypeSchema, FieldSchema, FieldType, FieldTypeKind, ObjectTypeSchema, SchemaCatalog,
    XmlFieldShape,
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
        label_source_pack_id: None,
        description_source_pack_id: None,
        items: None,
        flags: false,
        default_collapsed: None,
        xml_aliases: xml_aliases.iter().map(|s| s.to_string()).collect(),
        role: None,
    }
}

/// An `object`-shaped field whose children live on `schema_ref`'s object-type schema (e.g.
/// `ThingDef.graphicData`).
fn object_field(schema_ref: &str, xml_aliases: &[&str]) -> FieldSchema {
    let mut f = field(xml_aliases);
    f.field_type = FieldType {
        kind: FieldTypeKind::Object,
        schema_ref: Some(schema_ref.to_string()),
        reference: None,
    };
    f.xml = XmlFieldShape::Object;
    f
}

/// A `listOfLi` field whose items are objects of `schema_ref` (e.g. `ThingDef.comps`).
fn object_list_field(schema_ref: &str) -> FieldSchema {
    let mut f = field(&[]);
    f.field_type = FieldType {
        kind: FieldTypeKind::List,
        schema_ref: None,
        reference: None,
    };
    f.xml = XmlFieldShape::ListOfLi;
    f.items = Some(FieldType {
        kind: FieldTypeKind::Object,
        schema_ref: Some(schema_ref.to_string()),
        reference: None,
    });
    f
}

/// A `keyedObjectMap` field (`<li><key>..</key><value>..</value></li>` entries) whose values are
/// objects of `schema_ref`.
fn keyed_object_map_field(schema_ref: &str) -> FieldSchema {
    let mut f = field(&[]);
    f.xml = XmlFieldShape::KeyedObjectMap;
    f.items = Some(FieldType {
        kind: FieldTypeKind::Object,
        schema_ref: Some(schema_ref.to_string()),
        reference: None,
    });
    f
}

/// A `keyedObjectList` field (`<actualKey>...</actualKey>` entries keyed by element name) whose
/// items are objects of `schema_ref`.
fn keyed_object_list_field(schema_ref: &str) -> FieldSchema {
    let mut f = field(&[]);
    f.xml = XmlFieldShape::KeyedObjectList;
    f.items = Some(FieldType {
        kind: FieldTypeKind::Object,
        schema_ref: Some(schema_ref.to_string()),
        reference: None,
    });
    f
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

fn object_type_schema(inherits: &[&str], fields: &[(&str, FieldSchema)]) -> ObjectTypeSchema {
    ObjectTypeSchema {
        label: None,
        description: None,
        inherits: inherits.iter().map(|s| s.to_string()).collect(),
        field_order: Vec::new(),
        fields: fields
            .iter()
            .cloned()
            .map(|(name, field)| (name.to_string(), field))
            .collect(),
        discriminator: None,
    }
}

/// A small synthetic schema exercising unlimited-depth completion:
///
/// - `Def` (defName, label) <- `BuildableDef` (statBases, costList) <- `ThingDef`, plus an
///   unrelated `ThingDefStyleUnlockDef` sharing the `Def`/`ThingDef` name prefix (to exercise
///   prefix-only, not substring, matching).
/// - `ThingDef.graphicData` -> object `GraphicData` (texPath[alias: graphicPath], graphicClass,
///   shaderParameters -> object `ShaderParameters` (colorOne)), a 3-level object chain via
///   `shaderParameters`.
/// - `ThingDef.comps` -> `listOfLi` of object `CompProperties` (compClass).
/// - `ThingDef.verbs` -> `listOfLi` of object `VerbPropertiesAI`, which `inherits`
///   `VerbProperties` (verbClass[alias: Verb]) and adds its own `range` -- object-type
///   inheritance, not Def inheritance, so both levels' fields are directly completable.
/// - `ThingDef.keyframeParts` -> `keyedObjectMap` of object `KeyframePart` (partName).
/// - `ThingDef.things` -> `keyedObjectList` of object `PrefabThing` (pos) -- data-dependent keys.
/// - `ThingDef.weird` -> object field whose `schemaRef` ("NoSuchObjectType") is absent from the
///   catalog, to exercise the unknown-ref termination boundary.
/// - `ThingDef.cyclic` -> object `CycleA`, which mutually `inherits` `CycleB` (and vice versa), to
///   exercise cycle protection in the object-inheritance walk.
/// - `ThingDef.thingClass` -> a plain scalar direct field, to exercise scalar termination.
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
            &[
                ("comps", object_list_field("CompProperties")),
                ("graphicData", object_field("GraphicData", &["graphic"])),
                ("verbs", object_list_field("VerbPropertiesAI")),
                ("keyframeParts", keyed_object_map_field("KeyframePart")),
                ("things", keyed_object_list_field("PrefabThing")),
                ("weird", object_field("NoSuchObjectType", &[])),
                ("cyclic", object_field("CycleA", &[])),
                ("thingClass", field(&[])),
            ],
        ),
    );
    def_types.insert(
        "ThingDefStyleUnlockDef".to_string(),
        def_type_schema(&[], &[]),
    );

    let mut object_types = BTreeMap::new();
    object_types.insert(
        "GraphicData".to_string(),
        object_type_schema(
            &[],
            &[
                ("texPath", field(&["graphicPath"])),
                ("graphicClass", field(&[])),
                ("shaderParameters", object_field("ShaderParameters", &[])),
            ],
        ),
    );
    object_types.insert(
        "ShaderParameters".to_string(),
        object_type_schema(&[], &[("colorOne", field(&[]))]),
    );
    object_types.insert(
        "CompProperties".to_string(),
        object_type_schema(&[], &[("compClass", field(&[]))]),
    );
    object_types.insert(
        "VerbProperties".to_string(),
        object_type_schema(&[], &[("verbClass", field(&["Verb"]))]),
    );
    object_types.insert(
        "VerbPropertiesAI".to_string(),
        object_type_schema(&["VerbProperties"], &[("range", field(&[]))]),
    );
    object_types.insert(
        "KeyframePart".to_string(),
        object_type_schema(&[], &[("partName", field(&[]))]),
    );
    object_types.insert(
        "PrefabThing".to_string(),
        object_type_schema(&[], &[("pos", field(&[]))]),
    );
    object_types.insert(
        "CycleA".to_string(),
        object_type_schema(&["CycleB"], &[("a", field(&[]))]),
    );
    object_types.insert(
        "CycleB".to_string(),
        object_type_schema(&["CycleA"], &[("b", field(&[]))]),
    );

    SchemaCatalog {
        format_version: 1,
        packs: Vec::new(),
        def_types,
        object_types,
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

// ---------------------------------------------------------------------------
// Unlimited-depth field/structural descent
// ---------------------------------------------------------------------------

#[test]
fn nested_object_field_completes_at_every_level() {
    let catalog = test_catalog();
    let index = test_def_index();

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/graphicData/");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"texPath"), "{labels:?}");
    assert!(labels.contains(&"graphicClass"), "{labels:?}");
    assert!(labels.contains(&"shaderParameters"), "{labels:?}");
    assert!(result
        .items
        .iter()
        .filter(|i| i.label == "texPath")
        .all(|i| i.kind == XPathCompletionItemKind::Field));
    assert!(result.diagnostics.is_empty());

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/graphicData/texP");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"texPath"), "{labels:?}");
    // "texP" is a prefix, not an exact field name -- no *deeper* terminal-field resolution yet,
    // but the walk's last known-good field ("graphicData" itself) is kept rather than discarded,
    // matching every other cursor kind's "in-progress typing still resolves to the last
    // known-good field" behavior.
    let resolved = result
        .resolved_field
        .expect("graphicData should still resolve as a fallback");
    assert_eq!(resolved.field_name, "graphicData");
}

#[test]
fn resolves_deeply_nested_object_field_for_value_subform() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/graphicData/texPath");
    let resolved = result.resolved_field.expect("texPath should resolve");
    // The wire contract's `defType` stays the *root* Def type, not the nested object schema.
    assert_eq!(resolved.def_type, "ThingDef");
    assert_eq!(resolved.field_name, "texPath");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn resolves_xml_alias_on_a_nested_object_field_to_its_canonical_name() {
    let catalog = test_catalog();
    let index = test_def_index();

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/graphicData/graphicP");
    let alias = result
        .items
        .iter()
        .find(|i| i.label == "graphicPath")
        .expect("alias suggested");
    assert_eq!(alias.kind, XPathCompletionItemKind::FieldAlias);

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/graphicData/graphicPath");
    let resolved = result.resolved_field.expect("alias should resolve");
    assert_eq!(resolved.field_name, "texPath");
}

#[test]
fn three_level_object_chain_completes_and_resolves() {
    let catalog = test_catalog();
    let index = test_def_index();

    let result = complete_patch_xpath(
        &catalog,
        &index,
        "Defs/ThingDef/graphicData/shaderParameters/",
    );
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"colorOne"), "{labels:?}");

    let result = complete_patch_xpath(
        &catalog,
        &index,
        "Defs/ThingDef/graphicData/shaderParameters/colorOne",
    );
    let resolved = result.resolved_field.expect("colorOne should resolve");
    assert_eq!(resolved.def_type, "ThingDef");
    assert_eq!(resolved.field_name, "colorOne");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn object_list_of_li_suggests_li_then_descends_into_item_schema() {
    let catalog = test_catalog();
    let index = test_def_index();

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/comps/");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].insert_text, "li");
    assert_eq!(result.items[0].kind, XPathCompletionItemKind::ListItem);
    assert!(result.diagnostics.is_empty());

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/comps/li/");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"compClass"), "{labels:?}");

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/comps/li/compClass");
    let resolved = result.resolved_field.expect("compClass should resolve");
    assert_eq!(resolved.def_type, "ThingDef");
    assert_eq!(resolved.field_name, "compClass");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn object_list_item_resolves_fields_through_object_type_inheritance_and_aliases() {
    let catalog = test_catalog();
    let index = test_def_index();

    // `verbClass` is declared on `VerbProperties`, the *parent* of `verbs`' own item schema
    // `VerbPropertiesAI` -- unlike Def fields, object-type-inherited fields resolve directly (no
    // "inherited-only" diagnostic), since object inheritance is ordinary schema inheritance
    // already resolved on one XML element, not RimWorld's before-patches Def inheritance.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/verbs/li/verbClass");
    let resolved = result.resolved_field.expect("verbClass should resolve");
    assert_eq!(resolved.field_name, "verbClass");
    assert!(result.diagnostics.is_empty());

    // `range` is declared directly on `VerbPropertiesAI` itself.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/verbs/li/range");
    let resolved = result.resolved_field.expect("range should resolve");
    assert_eq!(resolved.field_name, "range");

    // An alias declared on the inherited parent's field also resolves to its canonical name.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/verbs/li/Verb");
    let resolved = result.resolved_field.expect("alias should resolve");
    assert_eq!(resolved.field_name, "verbClass");

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/verbs/li/");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"verbClass"), "{labels:?}");
    assert!(labels.contains(&"range"), "{labels:?}");
}

#[test]
fn keyed_object_map_suggests_li_then_key_or_value_and_only_value_descends() {
    let catalog = test_catalog();
    let index = test_def_index();

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/keyframeParts/");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].insert_text, "li");
    assert_eq!(result.items[0].kind, XPathCompletionItemKind::ListItem);

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/keyframeParts/li/");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels.len(), 2);
    assert!(labels.contains(&"key"), "{labels:?}");
    assert!(labels.contains(&"value"), "{labels:?}");
    assert!(result
        .items
        .iter()
        .all(|i| i.kind == XPathCompletionItemKind::MapEntry));

    let result = complete_patch_xpath(
        &catalog,
        &index,
        "Defs/ThingDef/keyframeParts/li/value/partName",
    );
    let resolved = result.resolved_field.expect("partName should resolve");
    assert_eq!(resolved.field_name, "partName");
    assert!(result.diagnostics.is_empty());

    // "key" is a scalar terminal -- it never enters the item object schema.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/keyframeParts/li/key/");
    assert!(result.items.is_empty());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn keyed_object_list_offers_no_invented_key_but_accepts_a_typed_key() {
    let catalog = test_catalog();
    let index = test_def_index();

    // Keys are data-dependent (e.g. a defName RimEdit has no index of here) -- no suggestions at
    // an empty segment, and no diagnostic either.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/things/");
    assert!(result.items.is_empty());
    assert!(result.diagnostics.is_empty());

    // Any typed, well-formed key is accepted and descends into the item object schema.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/things/SomeThingKey/");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"pos"), "{labels:?}");

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/things/SomeThingKey/pos");
    let resolved = result.resolved_field.expect("pos should resolve");
    assert_eq!(resolved.field_name, "pos");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn unknown_schema_ref_terminates_traversal_without_a_diagnostic() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/weird/");
    assert!(result.items.is_empty());
    assert!(result.diagnostics.is_empty());
    // The field itself is real and still resolves -- only its (unknown) children can't.
    let resolved = result.resolved_field.expect("weird should still resolve");
    assert_eq!(resolved.field_name, "weird");
}

#[test]
fn object_type_inheritance_cycle_does_not_hang_and_still_completes_reachable_fields() {
    let catalog = test_catalog();
    let index = test_def_index();
    // CycleA <-> CycleB mutually `inherits` each other -- if the object-inheritance walk lacked a
    // cycle guard this would hang the test suite rather than fail an assertion.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/cyclic/");
    let labels: Vec<&str> = result.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"a"), "{labels:?}");
    assert!(labels.contains(&"b"), "{labels:?}");

    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/cyclic/b");
    let resolved = result
        .resolved_field
        .expect("b should resolve through the cycle");
    assert_eq!(resolved.field_name, "b");
}

#[test]
fn scalar_field_terminates_traversal_without_the_old_depth_warning() {
    let catalog = test_catalog();
    let index = test_def_index();
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/thingClass/anything");
    assert!(result.items.is_empty());
    // No `xpath_autocomplete_unsupported_pattern` (or any other) diagnostic -- typing past a
    // scalar field is simply not completed, not an error.
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    // Best-effort: the last real field on the path ("thingClass") still resolves for the value
    // subform even though nothing deeper does.
    let resolved = result
        .resolved_field
        .expect("thingClass should still resolve");
    assert_eq!(resolved.field_name, "thingClass");
}

#[test]
fn structural_segment_mismatch_stops_with_unsupported_pattern_diagnostic() {
    let catalog = test_catalog();
    let index = test_def_index();
    // "list" is not the literal "li" a `listOfLi` object item expects.
    let result = complete_patch_xpath(&catalog, &index, "Defs/ThingDef/comps/list/compClass");
    assert!(result.items.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "xpath_autocomplete_unsupported_pattern"));
    // Best-effort: "comps" itself still resolved before the mismatch.
    let resolved = result.resolved_field.expect("comps should still resolve");
    assert_eq!(resolved.field_name, "comps");
}

#[test]
fn discriminator_variant_class_predicate_on_a_list_item_stays_unsupported() {
    let catalog = test_catalog();
    let index = test_def_index();
    // Narrowing to one `Class="..."` variant's own members needs predicate parsing, which stays
    // outside this conservative grammar -- only the declared base `items.schemaRef` is traversed
    // (see `SchemaCursor::ListItem`'s docs). The bracketed segment isn't the literal `li`, so this
    // stops with the same diagnostic as any other unrecognized structural segment.
    let result = complete_patch_xpath(
        &catalog,
        &index,
        r#"Defs/ThingDef/comps/li[@Class="CompProperties_Foo"]/compClass"#,
    );
    assert!(result.items.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "xpath_autocomplete_unsupported_pattern"));
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
