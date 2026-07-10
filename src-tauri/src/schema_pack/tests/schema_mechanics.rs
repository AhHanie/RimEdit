use super::*;

// --- items and flags deserialization ---

#[test]
fn field_schema_items_and_flags_deserialize() {
    let obj_json = r#"{
      "objectType": "MyThing",
      "fields": {
        "myFlags": {
          "label": "My flags",
          "type": { "kind": "list" },
          "items": { "kind": "enum" },
          "flags": true,
          "xml": "listOfLi",
          "validationHints": { "allowedValues": ["A", "B"] }
        }
      }
    }"#;
    let (file_opt, diags) = parse_object_type_schema("test:MyThing", "test", obj_json);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "parse errors: {:?}", errors);
    let file = file_opt.expect("must parse");
    let field = file.schema.fields.get("myFlags").expect("myFlags");
    assert!(field.items.is_some(), "items must be Some");
    assert_eq!(
        field.items.as_ref().unwrap().kind,
        FieldTypeKind::Enum,
        "items.kind must be enum"
    );
    assert_eq!(field.flags, Some(true), "flags must be Some(true)");
}

#[test]
fn items_and_flags_merge_into_catalog() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.fc", "name": "FC", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    let obj_json = r#"{
        "objectType": "FlagContainer",
        "fields": {
            "myFlags": {
                "type": { "kind": "list" },
                "items": { "kind": "enum" },
                "flags": true,
                "xml": "listOfLi",
                "validationHints": { "allowedValues": ["A", "B", "C"] }
            },
            "myItems": {
                "type": { "kind": "list" },
                "items": { "kind": "string" },
                "xml": "listOfLi"
            }
        }
    }"#;
    let pack = inline_pack_with_objects(manifest_json, def_json, &[obj_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let fc = catalog
        .object_types
        .get("FlagContainer")
        .expect("FlagContainer missing");

    let flags = fc.fields.get("myFlags").expect("myFlags missing");
    assert!(flags.items.is_some(), "myFlags must have items");
    assert_eq!(
        flags.items.as_ref().unwrap().kind,
        FieldTypeKind::Enum,
        "myFlags items.kind must be enum"
    );
    assert!(flags.flags, "myFlags must have flags = true");

    let items = fc.fields.get("myItems").expect("myItems missing");
    assert!(items.items.is_some(), "myItems must have items");
    assert_eq!(
        items.items.as_ref().unwrap().kind,
        FieldTypeKind::String,
        "myItems items.kind must be string"
    );
    assert!(!items.flags, "myItems must not be flags");
}

// --- unknown list item schemaRef warning ---

#[test]
fn unknown_list_item_schema_ref_emits_warning() {
    let manifest = r#"{"packId":"test-unknown-ref","formatVersion":1,"name":"Test","version":"1.0","defTypeDirectories":["def-types"],"objectTypeDirectories":["object-types"]}"#;
    let def_json = r#"{"defType":"TestDef","fields":{}}"#;
    let obj_json = r#"{
      "objectType": "MyObjectWithBadRef",
      "fields": {
        "myList": {
          "label": "My list",
          "type": { "kind": "list" },
          "items": { "kind": "object", "schemaRef": "DoesNotExist" },
          "xml": "listOfLi"
        }
      }
    }"#;
    let pack = inline_pack_with_objects(manifest, def_json, &[obj_json]);

    let mut merge_diags = Vec::new();
    merge_packs(vec![pack], &mut merge_diags);

    let warning = merge_diags
        .iter()
        .find(|d| d.code == "schema_pack_unknown_list_item_schema_ref");
    assert!(
        warning.is_some(),
        "expected schema_pack_unknown_list_item_schema_ref warning, got: {:?}",
        merge_diags
    );
}

// --- defaultCollapsed flag ---

#[test]
fn default_collapsed_deserializes_from_object_field() {
    let obj_json = r#"{
        "objectType": "ContainerObject",
        "fields": {
            "collapsedSection": {
                "type": { "kind": "object", "schemaRef": "NestedSection" },
                "xml": "object",
                "defaultCollapsed": true
            },
            "expandedSection": {
                "type": { "kind": "object", "schemaRef": "AnotherSection" },
                "xml": "object",
                "defaultCollapsed": false
            },
            "scalarField": {
                "type": { "kind": "string" },
                "xml": "element"
            }
        }
    }"#;
    let (obj_opt, diags) = parse_object_type_schema("test:container.json", "test.pack", obj_json);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "parse errors: {:?}", errors);
    let obj = obj_opt.expect("must parse");
    let collapsed = obj
        .schema
        .fields
        .get("collapsedSection")
        .expect("collapsedSection");
    assert_eq!(collapsed.default_collapsed, Some(true));
    let expanded = obj
        .schema
        .fields
        .get("expandedSection")
        .expect("expandedSection");
    assert_eq!(expanded.default_collapsed, Some(false));
    let scalar = obj.schema.fields.get("scalarField").expect("scalarField");
    assert_eq!(
        scalar.default_collapsed, None,
        "scalar field should have no defaultCollapsed"
    );
}

#[test]
fn default_collapsed_merges_into_catalog_field_schema() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.dc", "name": "DC", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    let obj_json = r#"{
        "objectType": "ContainerObject",
        "fields": {
            "nestedSection": {
                "type": { "kind": "object", "schemaRef": "NestedSection" },
                "xml": "object",
                "defaultCollapsed": true
            }
        }
    }"#;
    let pack = inline_pack_with_objects(manifest_json, def_json, &[obj_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);
    let container = catalog
        .object_types
        .get("ContainerObject")
        .expect("ContainerObject");
    let nested = container
        .fields
        .get("nestedSection")
        .expect("nestedSection");
    assert_eq!(
        nested.default_collapsed,
        Some(true),
        "defaultCollapsed should be preserved in catalog"
    );
}

#[test]
fn default_collapsed_override_semantics() {
    let base_manifest = r#"{ "formatVersion": 1, "packId": "test.base", "name": "Base", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["def-types"] }"#;
    let base_def = r#"{ "defType": "TestDef", "fields": {} }"#;
    let base_obj = r#"{
        "objectType": "ContainerObject",
        "fields": {
            "nestedSection": { "type": { "kind": "object", "schemaRef": "NestedSection" }, "xml": "object", "defaultCollapsed": true }
        }
    }"#;

    // Override omits defaultCollapsed - base value should remain unchanged
    let override_omit_manifest = r#"{ "formatVersion": 1, "packId": "test.over1", "name": "Over1", "version": "1.0.0", "priority": 1, "defTypeDirectories": ["def-types"] }"#;
    let override_omit_def = r#"{ "defType": "TestDef", "fields": {} }"#;
    let override_omit_obj = r#"{
        "objectType": "ContainerObject",
        "fields": {
            "nestedSection": { "type": { "kind": "object", "schemaRef": "NestedSection" }, "xml": "object" }
        }
    }"#;

    let base = inline_pack_with_objects(base_manifest, base_def, &[base_obj]);
    let over_omit = inline_pack_with_objects(
        override_omit_manifest,
        override_omit_def,
        &[override_omit_obj],
    );

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![base, over_omit], &mut diags);
    let container = catalog
        .object_types
        .get("ContainerObject")
        .expect("ContainerObject");
    let nested = container
        .fields
        .get("nestedSection")
        .expect("nestedSection");
    assert_eq!(
        nested.default_collapsed,
        Some(true),
        "omitting defaultCollapsed in override should leave base value true unchanged"
    );

    // Override explicitly sets false - should overwrite base true
    let base2_manifest = r#"{ "formatVersion": 1, "packId": "test.base2", "name": "Base2", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["def-types"] }"#;
    let override_explicit_manifest = r#"{ "formatVersion": 1, "packId": "test.over2", "name": "Over2", "version": "1.0.0", "priority": 1, "defTypeDirectories": ["def-types"] }"#;
    let override_explicit_obj = r#"{
        "objectType": "ContainerObject",
        "fields": {
            "nestedSection": { "type": { "kind": "object", "schemaRef": "NestedSection" }, "xml": "object", "defaultCollapsed": false }
        }
    }"#;

    let base2 = inline_pack_with_objects(base2_manifest, base_def, &[base_obj]);
    let over_explicit = inline_pack_with_objects(
        override_explicit_manifest,
        override_omit_def,
        &[override_explicit_obj],
    );

    let mut diags2 = Vec::new();
    let catalog2 = merge_packs(vec![base2, over_explicit], &mut diags2);
    let container2 = catalog2
        .object_types
        .get("ContainerObject")
        .expect("ContainerObject");
    let nested2 = container2
        .fields
        .get("nestedSection")
        .expect("nestedSection");
    assert_eq!(
        nested2.default_collapsed,
        Some(false),
        "explicit defaultCollapsed: false in override should overwrite base true"
    );
}

// --- discriminator variant resolution ---

#[test]
fn discriminator_variants_resolve_when_registered() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.disc", "name": "Disc", "version": "1.0.0", "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "TestDef", "fields": {} }"#;
    let base_obj = r#"{
        "objectType": "DiscriminatedBase",
        "discriminator": {
            "attribute": "Class",
            "allowMissing": true,
            "allowUnknown": true,
            "fallbackSchemaRef": "DiscriminatedBase",
            "variants": {
                "VariantA": "VariantA",
                "VariantB": "VariantB"
            }
        },
        "fields": {
            "Class": { "type": { "kind": "string" }, "required": false, "xml": "attribute" },
            "baseField": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let variant_a_obj = r#"{
        "objectType": "VariantA",
        "fields": {
            "Class": { "type": { "kind": "string" }, "required": false, "xml": "attribute" },
            "variantAField": { "type": { "kind": "string" }, "required": false }
        }
    }"#;
    let variant_b_obj = r#"{
        "objectType": "VariantB",
        "fields": {
            "Class": { "type": { "kind": "string" }, "required": false, "xml": "attribute" },
            "variantBField": { "type": { "kind": "integer" }, "required": false }
        }
    }"#;

    let pack = inline_pack_with_objects(
        manifest_json,
        def_json,
        &[base_obj, variant_a_obj, variant_b_obj],
    );
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let base = catalog
        .object_types
        .get("DiscriminatedBase")
        .expect("DiscriminatedBase missing");
    let disc = base
        .discriminator
        .as_ref()
        .expect("DiscriminatedBase must have a discriminator");
    assert_eq!(disc.attribute, "Class");
    assert!(disc.allow_missing, "discriminator must allow missing Class");
    assert!(disc.allow_unknown, "discriminator must allow unknown Class");
    assert_eq!(
        disc.fallback_schema_ref.as_deref(),
        Some("DiscriminatedBase")
    );

    for (class_name, target_ref) in &disc.variants {
        assert!(
            catalog.object_types.contains_key(target_ref),
            "variant '{class_name}' maps to '{target_ref}' which is missing from catalog"
        );
    }

    let unresolved: Vec<_> = diags
        .iter()
        .filter(|d| d.code == "schema_pack_unknown_discriminator_variant_target")
        .collect();
    assert!(
        unresolved.is_empty(),
        "unresolved discriminator variant targets: {:#?}",
        unresolved
    );
}
