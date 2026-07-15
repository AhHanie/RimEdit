use super::super::loader::{
    assemble_schema_pack, load_built_in_packs, load_pack_from_directory, parse_def_type_schema,
    parse_schema_pack_manifest, LoadedPack,
};
use super::super::lookup::{collect_effective_top_level_def_fields, lookup_field};
use super::super::merge::merge_packs;
use super::super::model::{
    DefTypeSchema, DefTypeSchemaFile, FormViewSource, SchemaFormView, SchemaLoadDiagnostic,
    SchemaLoadSeverity, SchemaPackManifest,
};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

// Issue 01 scope only: these tests prove the new parse-time `FormViewDef` shape deserializes
// through the existing (unmodified) def-type file parser, and that the resolved `SchemaFormView`
// / `FormViewSource` catalog types round-trip through serde. Loader-side validation (rejecting
// the reserved "default" id, requiring formatVersion 3, blank-label/duplicate-id diagnostics,
// etc.) and inheritance/pack-precedence merge resolution are explicitly out of scope until
// issues 02 and 03.

#[test]
fn form_views_deserialize_from_def_type_file() {
    let json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": {
            "weapon": {
                "label": "Weapon",
                "description": "Fields commonly needed for equippable weapons.",
                "icon": "sword",
                "order": 20,
                "recommended": true,
                "hiddenFields": ["apparel", "plant", "race", "building"]
            },
            "minimal": {
                "label": "Minimal item",
                "hiddenFields": ["graphicData", "comps"]
            }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:ThingDef.json", "test.pack", json, 3);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "parse errors: {:?}", errors);
    let def_file = def_opt.expect("def file must parse");

    assert_eq!(def_file.schema.form_views.len(), 2);

    let weapon = def_file
        .schema
        .form_views
        .get("weapon")
        .expect("weapon view missing");
    assert_eq!(weapon.label.as_deref(), Some("Weapon"));
    assert_eq!(
        weapon.description.as_deref(),
        Some("Fields commonly needed for equippable weapons.")
    );
    assert_eq!(weapon.icon.as_deref(), Some("sword"));
    assert_eq!(weapon.order, Some(20));
    assert_eq!(weapon.recommended, Some(true));
    assert_eq!(
        weapon.hidden_fields,
        Some(vec![
            "apparel".to_string(),
            "plant".to_string(),
            "race".to_string(),
            "building".to_string(),
        ])
    );
    assert_eq!(weapon.unhide_fields, None);
    assert_eq!(weapon.replace, None);
    assert_eq!(weapon.disabled, None);

    let minimal = def_file
        .schema
        .form_views
        .get("minimal")
        .expect("minimal view missing");
    assert_eq!(minimal.label.as_deref(), Some("Minimal item"));
    assert_eq!(minimal.icon, None);
    assert_eq!(minimal.order, None);
    assert_eq!(minimal.recommended, None);
}

#[test]
fn form_view_delta_fields_deserialize_without_a_label() {
    // Mirrors the exact child-schema "delta" shape documented in Plan.md section 4
    // (~line 117-127): a delta amendment to an inherited view can be valid with only
    // hiddenFields/unhideFields, or just `{ "disabled": true }` -- no `label` key at all. Serde
    // must not require `label` here; issue 02/03's validation layer (not Serde) is responsible
    // for rejecting a *new* (non-delta) view that omits a label.
    let json = r#"{
        "defType": "GunDef",
        "inherits": ["ThingDef"],
        "fields": {},
        "formViews": {
            "weapon": {
                "hiddenFields": ["canDeteriorateUnspawned"],
                "unhideFields": ["graphicData"]
            },
            "legacy": {
                "disabled": true
            }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:GunDef.json", "test.pack", json, 3);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "parse errors: {:?}", errors);
    let def_file = def_opt.expect("def file must parse");

    let weapon = &def_file.schema.form_views["weapon"];
    assert_eq!(
        weapon.label, None,
        "a delta amendment need not repeat a label"
    );
    assert_eq!(
        weapon.hidden_fields,
        Some(vec!["canDeteriorateUnspawned".to_string()])
    );
    assert_eq!(weapon.unhide_fields, Some(vec!["graphicData".to_string()]));

    let legacy = &def_file.schema.form_views["legacy"];
    assert_eq!(
        legacy.label, None,
        "a disable-only delta need not repeat a label"
    );
    assert_eq!(legacy.disabled, Some(true));
    assert_eq!(legacy.hidden_fields, None);
}

#[test]
fn absent_form_views_resolves_to_empty_map() {
    let json = r#"{ "defType": "ThingDef", "fields": {} }"#;
    let (def_opt, diags) = parse_def_type_schema("test:NoViews.json", "test.pack", json, 3);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "parse errors: {:?}", errors);
    let def_file = def_opt.expect("def file must parse");
    assert!(
        def_file.schema.form_views.is_empty(),
        "no formViews key should resolve to an empty map"
    );
}

#[test]
fn schema_form_view_serializes_camel_case_with_source() {
    let view = SchemaFormView {
        id: "weapon".to_string(),
        label: "Weapon".to_string(),
        description: Some("Fields for equippable weapons.".to_string()),
        icon: Some("sword".to_string()),
        order: 20,
        recommended: true,
        hidden_field_ids: vec!["apparel".to_string(), "plant".to_string()],
        declared_on_def_type: "ThingDef".to_string(),
        source: Some(FormViewSource {
            pack_id: "rimedit.rimworld.core".to_string(),
            pack_version: "1.6.0".to_string(),
        }),
    };

    let json = serde_json::to_value(&view).expect("serialize");
    assert_eq!(json["id"], "weapon");
    assert_eq!(json["label"], "Weapon");
    assert_eq!(json["order"], 20);
    assert_eq!(json["recommended"], true);
    assert_eq!(
        json["hiddenFieldIds"],
        serde_json::json!(["apparel", "plant"])
    );
    assert_eq!(json["declaredOnDefType"], "ThingDef");
    assert_eq!(json["source"]["packId"], "rimedit.rimworld.core");
    assert_eq!(json["source"]["packVersion"], "1.6.0");
}

#[test]
fn schema_form_view_omits_absent_source() {
    let view = SchemaFormView {
        id: "minimal".to_string(),
        label: "Minimal item".to_string(),
        description: None,
        icon: None,
        order: 0,
        recommended: false,
        hidden_field_ids: vec!["graphicData".to_string()],
        declared_on_def_type: "ThingDef".to_string(),
        source: None,
    };

    let json = serde_json::to_value(&view).expect("serialize");
    assert!(
        json.get("source").is_none(),
        "absent source should be omitted, not serialized as null"
    );
}

fn empty_def_type_schema() -> DefTypeSchema {
    // Mirrors how `DefTypeSchema` is built at every construction site today (merge.rs,
    // patches::tests::xpath helper): `form_views` starts as an empty `BTreeMap` until issue 03
    // implements resolution.
    DefTypeSchema {
        label: None,
        description: None,
        inherits: Vec::new(),
        abstract_type: false,
        field_order: Vec::new(),
        fields: BTreeMap::new(),
        templates: BTreeMap::new(),
        validation_rules: BTreeMap::new(),
        form_views: BTreeMap::new(),
    }
}

#[test]
fn def_type_schema_always_serializes_form_views_key_even_when_empty() {
    // Issue 03's "empty/no-view Def types serialize an empty map" requirement: the frontend must
    // be able to rely on `formViews` always being present (as `{}` at minimum) to synthesize the
    // Default View, rather than special-casing an absent key. This supersedes issue 01's
    // placeholder `skip_serializing_if`, which predated real resolution.
    let schema = empty_def_type_schema();
    let json = serde_json::to_value(&schema).expect("serialize");
    assert_eq!(
        json.get("formViews"),
        Some(&serde_json::json!({})),
        "an empty form_views map must still serialize as an explicit empty object, not be omitted"
    );
}

#[test]
fn def_type_schema_serializes_populated_form_views() {
    let mut schema = empty_def_type_schema();
    schema.form_views.insert(
        "weapon".to_string(),
        SchemaFormView {
            id: "weapon".to_string(),
            label: "Weapon".to_string(),
            description: None,
            icon: None,
            order: 0,
            recommended: false,
            hidden_field_ids: Vec::new(),
            declared_on_def_type: "ThingDef".to_string(),
            source: None,
        },
    );

    let json = serde_json::to_value(&schema).expect("serialize");
    assert_eq!(json["formViews"]["weapon"]["label"], "Weapon");
    assert_eq!(json["formViews"]["weapon"]["declaredOnDefType"], "ThingDef");
}

// ---------------------------------------------------------------------------
// Issue 02: manifest formatVersion gating + per-declaration validation.
//
// These tests validate one Def-type file's own `formViews` declarations in isolation: manifest
// version 1/2 rejection, blank/reserved id, blank/missing label, impossible `disabled`
// combinations, and contradictory hiddenFields/unhideFields. They deliberately do NOT test
// anything about inheritance/merge resolution or validating field ids against a real known
// field universe -- that is issue 03's job.
// ---------------------------------------------------------------------------

fn v3_manifest(pack_id: &str) -> String {
    format!(
        r#"{{ "formatVersion": 3, "packId": "{}", "name": "Test", "version": "1.0.0", "defTypeDirectories": ["def-types"] }}"#,
        pack_id
    )
}

/// Parse+assemble a single-Def-type pack inline without asserting success, so tests can inspect
/// diagnostics for declarations that are expected to be rejected.
fn assemble_form_views_inline(
    manifest_json: &str,
    def_json: &str,
) -> (Option<SchemaPackManifest>, Vec<SchemaLoadDiagnostic>) {
    let (manifest_opt, mut diags) = parse_schema_pack_manifest("test:manifest", manifest_json);
    let manifest_file = match manifest_opt {
        Some(m) => m,
        None => return (None, diags),
    };
    let pack_id = manifest_file.pack_id.clone();
    let (def_opt, ddiags) = parse_def_type_schema(
        "test:def.json",
        &pack_id,
        def_json,
        manifest_file.format_version,
    );
    diags.extend(ddiags);
    let def_file = match def_opt {
        Some(d) => d,
        None => return (None, diags),
    };
    let def_refs = vec![("test:def.json", &def_file)];
    let (pack_opt, adiags) = assemble_schema_pack("test:pack", manifest_file, &def_refs, &[], &[]);
    diags.extend(adiags);
    (pack_opt, diags)
}

#[test]
fn malformed_form_views_shape_rejects_whole_def_file() {
    // formViews must be a keyed object (like templates/validationRules); an array is a shape
    // error caught by serde deserialization, same granularity as any other malformed def file.
    let json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": ["not", "an", "object"]
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:bad_shape.json", "test.pack", json, 3);
    assert!(
        def_opt.is_none(),
        "malformed formViews shape must reject the whole def file"
    );
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_def_type_json_invalid"));
}

#[test]
fn v1_manifest_with_form_views_in_def_file_is_rejected() {
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.formviews.v1", "name": "V1", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": { "defName": { "type": { "kind": "string" }, "required": true } },
        "formViews": { "weapon": { "label": "Weapon", "hiddenFields": ["apparel"] } }
    }"#;
    let (pack_opt, diags) = assemble_form_views_inline(manifest_json, def_json);
    assert!(
        pack_opt.is_some(),
        "pack should still assemble with formViews dropped, not rejected wholesale"
    );
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_views_requires_v3"));
    let pack = pack_opt.unwrap();
    let thing_def = &pack.def_types["ThingDef"];
    assert!(
        thing_def.form_views.is_empty(),
        "v1 pack's formViews must be dropped entirely"
    );
    assert!(
        thing_def.fields.contains_key("defName"),
        "unrelated fields must still load"
    );
}

#[test]
fn v2_manifest_with_form_views_in_def_file_is_rejected() {
    let manifest_json = r#"{ "formatVersion": 2, "packId": "test.formviews.v2", "name": "V2", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "weapon": { "label": "Weapon", "hiddenFields": ["apparel"] } }
    }"#;
    let (pack_opt, diags) = assemble_form_views_inline(manifest_json, def_json);
    assert!(pack_opt.is_some());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_views_requires_v3"));
    assert!(pack_opt.unwrap().def_types["ThingDef"]
        .form_views
        .is_empty());
}

#[test]
fn v1_manifest_with_reserved_id_form_view_still_loads_the_rest_of_the_file() {
    // Ordering-fix regression: the manifest-version gate must run BEFORE the v3-only structural
    // validation (`validate_form_view_declarations`), not after. A v1/v2 pack's formViews is
    // unconditionally unsupported regardless of whether the content inside it would also be
    // structurally invalid on a v3 pack -- so a reserved "default" id here must NOT trigger the
    // whole-file-fatal `schema_pack_form_view_reserved_id` path (that check must never even run
    // for a v1/v2 pack). Only the version-gate diagnostic fires, formViews is dropped, and the
    // rest of the Def-type file (fields, templates) loads completely normally.
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.formviews.v1badview", "name": "V1", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": { "defName": { "type": { "kind": "string" }, "required": true } },
        "templates": { "basic": { "label": "Basic" } },
        "formViews": { "default": { "label": "Reserved id, also malformed" } }
    }"#;
    let (pack_opt, diags) = assemble_form_views_inline(manifest_json, def_json);
    assert!(
        pack_opt.is_some(),
        "the whole Def-type file must still load on a v1/v2 pack, even with a formViews \
         declaration that would also be structurally invalid on a v3 pack"
    );
    let pack = pack_opt.unwrap();
    let thing_def = &pack.def_types["ThingDef"];
    assert!(
        thing_def.form_views.is_empty(),
        "formViews must be dropped entirely on a v1/v2 pack"
    );
    assert!(
        thing_def.fields.contains_key("defName"),
        "fields must still load"
    );
    assert!(
        thing_def.templates.contains_key("basic"),
        "templates must still load"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_form_views_requires_v3"),
        "expected schema_pack_form_views_requires_v3, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "schema_pack_form_view_reserved_id"),
        "the v3-only structural validation must never run for a v1/v2 pack: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn v1_manifest_with_non_object_form_views_still_loads_the_rest_of_the_file() {
    // Same ordering-fix regression, but for a shape-level violation (formViews given as an array
    // instead of an object) rather than a semantic one. On a v1/v2 pack this must be stripped
    // with the version-gate diagnostic before any struct deserialization is attempted against it
    // -- not fall through to a generic `schema_pack_def_type_json_invalid` whole-file failure.
    let manifest_json = r#"{ "formatVersion": 2, "packId": "test.formviews.v2array", "name": "V2", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": { "defName": { "type": { "kind": "string" }, "required": true } },
        "formViews": ["not", "an", "object"]
    }"#;
    let (pack_opt, diags) = assemble_form_views_inline(manifest_json, def_json);
    assert!(
        pack_opt.is_some(),
        "a non-object formViews on a v1/v2 pack must not sink the whole file"
    );
    let pack = pack_opt.unwrap();
    let thing_def = &pack.def_types["ThingDef"];
    assert!(thing_def.form_views.is_empty());
    assert!(thing_def.fields.contains_key("defName"));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_form_views_requires_v3"),
        "expected schema_pack_form_views_requires_v3, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "schema_pack_def_type_json_invalid"),
        "a non-object formViews on a v1/v2 pack must not be treated as a generic malformed-JSON \
         whole-file failure: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn v1_manifest_with_explicit_null_form_views_still_loads_the_rest_of_the_file() {
    // Same ordering-fix regression, but for `formViews: null` specifically. `null` is NOT
    // equivalent to "key absent": `#[serde(default)]` only substitutes a default when the key is
    // missing entirely, so an unstripped `formViews: null` would otherwise reach
    // `serde_json::from_value` and fail to deserialize into `BTreeMap<String, FormViewDef>` --
    // the same whole-file-loss bug a non-object shape like an array causes if left unstripped.
    // `value_has_nonempty_form_views` must treat explicit `null` as "present" for the v1/v2 gate.
    let manifest_json = r#"{ "formatVersion": 1, "packId": "test.formviews.v1null", "name": "V1", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": { "defName": { "type": { "kind": "string" }, "required": true } },
        "templates": { "basic": { "label": "Basic" } },
        "formViews": null
    }"#;
    let (pack_opt, diags) = assemble_form_views_inline(manifest_json, def_json);
    assert!(
        pack_opt.is_some(),
        "an explicit null formViews on a v1/v2 pack must not sink the whole file"
    );
    let pack = pack_opt.unwrap();
    let thing_def = &pack.def_types["ThingDef"];
    assert!(thing_def.form_views.is_empty());
    assert!(
        thing_def.fields.contains_key("defName"),
        "fields must still load"
    );
    assert!(
        thing_def.templates.contains_key("basic"),
        "templates must still load"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_form_views_requires_v3"),
        "expected schema_pack_form_views_requires_v3, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "schema_pack_def_type_json_invalid"),
        "an explicit null formViews on a v1/v2 pack must not be treated as a generic \
         malformed-JSON whole-file failure: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn manifest_format_version_four_is_unsupported() {
    let json = r#"{ "formatVersion": 4, "packId": "test.formviews.v4", "name": "V4", "version": "1.0.0", "defTypeDirectories": ["def-types"] }"#;
    let (manifest_opt, diags) = parse_schema_pack_manifest("test:v4", json);
    assert!(manifest_opt.is_none());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_manifest_format_unsupported"));
}

#[test]
fn v3_manifest_with_well_formed_view_has_no_errors() {
    let manifest_json = v3_manifest("test.formviews.valid");
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "weapon": { "label": "Weapon", "hiddenFields": ["apparel"] } }
    }"#;
    let (pack_opt, diags) = assemble_form_views_inline(&manifest_json, def_json);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    let pack = pack_opt.expect("pack must assemble");
    assert_eq!(pack.def_types["ThingDef"].form_views.len(), 1);
}

#[test]
fn blank_form_view_id_rejects_the_whole_def_file() {
    // Plan.md section 5: blank/invalid id is fatal for the whole v3 Def schema file, not a
    // recoverable per-declaration skip -- so `parse_def_type_schema` itself must return `None`
    // (the same outcome as any other malformed def file), never reaching pack assembly at all.
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "": { "label": "Blank id view" } }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(
        def_opt.is_none(),
        "a blank formViews id must reject the whole def file"
    );
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_blank_id"));
}

#[test]
fn reserved_default_id_rejects_the_whole_def_file() {
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "default": { "label": "Should not be allowed" } }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(
        def_opt.is_none(),
        "a reserved 'default' formViews id must reject the whole def file"
    );
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_reserved_id"));
}

#[test]
fn entirely_empty_declaration_rejects_the_whole_def_file() {
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "nothing": {} }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_empty_declaration"));
}

#[test]
fn blank_label_rejects_the_whole_def_file_even_on_a_delta_shaped_declaration() {
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "weapon": { "label": "   ", "hiddenFields": ["apparel"] } }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_blank_label"));
}

#[test]
fn missing_label_rejects_the_whole_def_file_when_declaration_carries_new_view_metadata() {
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": {
            "weapon": { "description": "No label here", "hiddenFields": ["apparel"] }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_missing_label"));
}

#[test]
fn label_less_pure_delta_declarations_are_accepted() {
    // Mirrors the Plan.md section 4 delta shape: a declaration with only hiddenFields/
    // unhideFields, or only `disabled: true`, is a legitimate amendment candidate and must not
    // be rejected for missing a label (issue 03 resolves it against an inherited base).
    let manifest_json = v3_manifest("test.formviews.delta");
    let def_json = r#"{
        "defType": "GunDef",
        "fields": {},
        "formViews": {
            "weapon": {
                "hiddenFields": ["canDeteriorateUnspawned"],
                "unhideFields": ["graphicData"]
            },
            "legacy": { "disabled": true }
        }
    }"#;
    let (pack_opt, diags) = assemble_form_views_inline(&manifest_json, def_json);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "pure delta declarations must not error: {:?}",
        errors
    );
    let pack = pack_opt.expect("pack must assemble");
    assert_eq!(pack.def_types["GunDef"].form_views.len(), 2);
}

#[test]
fn disabled_true_combined_with_other_content_rejects_the_whole_def_file() {
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "legacy": { "disabled": true, "label": "Legacy" } }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_disabled_with_content"));
}

#[test]
fn disabled_true_combined_with_description_only_is_diagnosed_as_disabled_with_content_not_missing_label(
) {
    // Regression for a priority-ordering bug: `disabled: true` plus `description` (with no
    // label) must be diagnosed as the impossible disabled-with-content declaration, not
    // misdiagnosed as a missing-label new-view declaration. The disabled+content check must run
    // before the label-related checks.
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "legacy": { "disabled": true, "description": "Why is this here" } }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_form_view_disabled_with_content"),
        "expected schema_pack_form_view_disabled_with_content, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "schema_pack_form_view_missing_label"),
        "must not be misdiagnosed as missing_label: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn disabled_true_alone_is_accepted() {
    let manifest_json = v3_manifest("test.formviews.disabledalone");
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "legacy": { "disabled": true } }
    }"#;
    let (pack_opt, diags) = assemble_form_views_inline(&manifest_json, def_json);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    assert_eq!(pack_opt.unwrap().def_types["ThingDef"].form_views.len(), 1);
}

#[test]
fn contradictory_hidden_and_unhide_fields_rejects_the_whole_def_file() {
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": {
            "weapon": { "hiddenFields": ["apparel", "plant"], "unhideFields": ["plant"] }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_conflicting_hidden_unhide"));
}

#[test]
fn duplicate_entry_within_hidden_fields_rejects_the_whole_def_file() {
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["apparel", "apparel"] }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_duplicate_hidden_field"));
}

#[test]
fn duplicate_entry_within_unhide_fields_rejects_the_whole_def_file() {
    let def_json = r#"{
        "defType": "GunDef",
        "fields": {},
        "formViews": {
            "weapon": { "unhideFields": ["graphicData", "graphicData"] }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_duplicate_unhide_field"));
}

#[test]
fn a_single_bad_declaration_rejects_the_whole_def_file_even_with_other_valid_views_present() {
    // Regression for the fatal/recoverable granularity fix: Plan.md section 5 lists blank/
    // reserved id (among other structural issues) as fatal for the whole v3 Def schema file, not
    // a recoverable per-declaration skip. A file with one perfectly valid view ("weapon") and one
    // reserved-id violation ("default") must fail to load *all* of its formViews -- the valid
    // "weapon" view must NOT survive.
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["apparel"] },
            "default": { "label": "Bad reserved id" }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(
        def_opt.is_none(),
        "the whole def file must be rejected, not just the 'default' declaration"
    );
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_reserved_id"));
}

#[test]
fn duplicate_view_id_in_raw_json_text_rejects_the_whole_def_file() {
    // The raw JSON text contains two literal "weapon" keys in the same formViews object. A
    // generic `serde_json::Value` parse (or a plain derived `BTreeMap`) would silently keep only
    // the last one with no diagnostic; `parse_def_type_schema` must instead surface this as an
    // ordinary deserialize error (schema_pack_def_type_json_invalid), the same whole-file
    // rejection path used for any other malformed def file.
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": {
            "weapon": { "label": "First", "hiddenFields": ["apparel"] },
            "weapon": { "label": "Second", "hiddenFields": ["plant"] }
        }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(
        def_opt.is_none(),
        "a duplicate formViews id in the raw JSON must reject the whole def file"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_def_type_json_invalid"),
        "expected schema_pack_def_type_json_invalid, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn diagnostics_carry_pack_id_path_and_field_path() {
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "default": { "label": "Bad reserved id" } }
    }"#;
    let (_def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    let diag = diags
        .iter()
        .find(|d| d.code == "schema_pack_form_view_reserved_id")
        .expect("expected reserved id diagnostic");
    assert_eq!(diag.pack_id.as_deref(), Some("test.pack"));
    assert_eq!(diag.path.as_deref(), Some("test:def.json"));
    assert_eq!(
        diag.field_path.as_deref(),
        Some("ThingDef.formViews.default")
    );
}

#[test]
fn deserialize_level_error_under_form_views_has_no_structured_field_path_but_message_has_location()
{
    // A deserialize-level failure under formViews (a wrong scalar type, not something our own
    // semantic validate_form_view_declarations would catch since the file never even reaches
    // that stage) does NOT get a structured field path today: `serde_path_to_error` was
    // investigated and found unable to see through `DefTypeSchemaFile`'s `#[serde(flatten)]`
    // field (flatten's derive-generated buffering uses its own untracked deserializer to
    // redistribute fields, so path tracking is lost for any field behind the flatten boundary --
    // confirmed empirically, not specific to formViews). Reworking the model away from `flatten`
    // to fix this is disproportionate for one diagnostic's precision.
    //
    // The accepted fallback: `serde_json::Error`'s `Display` already includes a source location
    // ("at line N column M"), which lands in the diagnostic message. This test documents and
    // locks in that fallback rather than silently regressing to "no location information at all".
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "weapon": { "label": 1 } }
    }"#;
    let (def_opt, diags) = parse_def_type_schema("test:def.json", "test.pack", def_json, 3);
    assert!(def_opt.is_none());
    let diag = diags
        .iter()
        .find(|d| d.code == "schema_pack_def_type_json_invalid")
        .expect("expected schema_pack_def_type_json_invalid");
    assert!(
        diag.field_path.is_none(),
        "no structured field path is available for a deserialize-level error today (see comment \
         above); update this test if that ever becomes feasible"
    );
    assert!(
        diag.message.contains("line") && diag.message.contains("column"),
        "expected a source line/column in the message as a fallback location hint, got: {}",
        diag.message
    );
}

// --- Fixture-based coverage (directory layout matching other loader tests) ---

#[test]
fn v1_pack_with_form_views_in_def_file_is_rejected_via_fixture() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack/form_views_v1_rejected/schema-pack.json");
    let (pack_opt, diags) = load_pack_from_directory(&path);
    assert!(
        pack_opt.is_some(),
        "pack should still load with formViews dropped"
    );
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_views_requires_v3"));
    let pack = pack_opt.unwrap();
    let thing_def = pack.manifest.def_types.get("ThingDef").expect("ThingDef");
    assert!(thing_def.form_views.is_empty());
    assert!(
        thing_def.fields.contains_key("defName"),
        "fields must still load despite dropped formViews"
    );
}

#[test]
fn v3_pack_with_well_formed_view_loads_via_fixture() {
    let pack = super::load_fixture("form_views_v3_valid");
    assert_eq!(pack.manifest.format_version, 3);
    let thing_def = pack.manifest.def_types.get("ThingDef").expect("ThingDef");
    assert_eq!(thing_def.form_views.len(), 2);
    let weapon = thing_def.form_views.get("weapon").expect("weapon view");
    assert_eq!(weapon.label.as_deref(), Some("Weapon"));
    let minimal = thing_def.form_views.get("minimal").expect("minimal view");
    assert_eq!(minimal.label.as_deref(), Some("Minimal item"));
}

// ---------------------------------------------------------------------------
// Issue 03: inheritance + pack-precedence resolution into `DefTypeSchema.form_views`.
//
// These tests exercise `merge_packs` end to end (unlike the issue 01/02 tests above, which stop
// at the raw per-file `FormViewDef` declarations) to prove the *resolved* `SchemaFormView` map is
// correctly materialized per concrete def type.
// ---------------------------------------------------------------------------

fn v3_manifest_with_priority(pack_id: &str, priority: i32) -> String {
    format!(
        r#"{{ "formatVersion": 3, "packId": "{}", "name": "Test", "version": "1.0.0", "priority": {}, "defTypeDirectories": ["def-types"] }}"#,
        pack_id, priority
    )
}

/// Build a `LoadedPack` from an inline manifest and several inline Def-type JSON bodies, without
/// touching disk. Mirrors `super::inline_pack`, but supports more than one def type per pack --
/// needed for inheritance-chain (parent/child/grandchild) Form View tests.
fn inline_multi_def_pack(manifest_json: &str, def_jsons: &[&str]) -> LoadedPack {
    let (manifest_opt, mdiags) = parse_schema_pack_manifest("test:manifest", manifest_json);
    assert!(
        manifest_opt.is_some(),
        "inline manifest failed: {:?}",
        mdiags
    );
    let manifest_file = manifest_opt.unwrap();
    let pack_id = manifest_file.pack_id.clone();
    let mut def_files: Vec<DefTypeSchemaFile> = Vec::new();
    for raw in def_jsons {
        let (def_opt, ddiags) =
            parse_def_type_schema("test:def", &pack_id, raw, manifest_file.format_version);
        assert!(def_opt.is_some(), "inline def failed: {:?}", ddiags);
        def_files.push(def_opt.unwrap());
    }
    let def_refs: Vec<(&str, &DefTypeSchemaFile)> =
        def_files.iter().map(|f| ("test:def", f)).collect();
    let (pack_opt, adiags) = assemble_schema_pack("test:pack", manifest_file, &def_refs, &[], &[]);
    let errors: Vec<_> = adiags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "inline pack errors: {:?}", errors);
    LoadedPack {
        manifest: pack_opt.expect("assemble must succeed"),
        is_builtin: false,
        source_path: None,
        locales: Default::default(),
    }
}

#[test]
fn child_inherits_parent_view_unchanged_with_same_provenance() {
    let manifest_json = v3_manifest("test.formviews.inherit1");
    let parent_json = r#"{
        "defType": "BuildableDef",
        "abstractType": true,
        "fields": {
            "apparel": { "type": { "kind": "string" } },
            "plant": { "type": { "kind": "string" } }
        },
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["apparel", "plant"] }
        }
    }"#;
    let child_json = r#"{
        "defType": "ThingDef",
        "inherits": ["BuildableDef"],
        "fields": {}
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[parent_json, child_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let parent_view = catalog.def_types["BuildableDef"].form_views["weapon"].clone();
    let child_view = catalog.def_types["ThingDef"].form_views["weapon"].clone();

    assert_eq!(child_view.label, parent_view.label);
    assert_eq!(child_view.hidden_field_ids, parent_view.hidden_field_ids);
    assert_eq!(child_view.declared_on_def_type, "BuildableDef");
    assert_eq!(
        child_view.source.as_ref().map(|s| s.pack_id.as_str()),
        Some("test.formviews.inherit1")
    );
}

#[test]
fn child_delta_adds_additional_hidden_fields_on_inherited_view() {
    let manifest_json = v3_manifest("test.formviews.inherit2");
    let parent_json = r#"{
        "defType": "BuildableDef",
        "abstractType": true,
        "fields": {
            "apparel": { "type": { "kind": "string" } },
            "canDeteriorateUnspawned": { "type": { "kind": "boolean" } }
        },
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["apparel"] }
        }
    }"#;
    let child_json = r#"{
        "defType": "GunDef",
        "inherits": ["BuildableDef"],
        "fields": {},
        "formViews": {
            "weapon": { "hiddenFields": ["canDeteriorateUnspawned"] }
        }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[parent_json, child_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let child_view = &catalog.def_types["GunDef"].form_views["weapon"];
    assert_eq!(
        child_view.hidden_field_ids,
        vec!["apparel".to_string(), "canDeteriorateUnspawned".to_string()]
    );
    assert_eq!(child_view.label, "Weapon");
    assert_eq!(child_view.declared_on_def_type, "GunDef");
}

#[test]
fn child_unhide_removes_a_parent_hidden_field() {
    let manifest_json = v3_manifest("test.formviews.inherit3");
    let parent_json = r#"{
        "defType": "BuildableDef",
        "abstractType": true,
        "fields": {
            "apparel": { "type": { "kind": "string" } },
            "graphicData": { "type": { "kind": "string" } }
        },
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["apparel", "graphicData"] }
        }
    }"#;
    let child_json = r#"{
        "defType": "GunDef",
        "inherits": ["BuildableDef"],
        "fields": {},
        "formViews": {
            "weapon": { "unhideFields": ["graphicData"] }
        }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[parent_json, child_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let child_view = &catalog.def_types["GunDef"].form_views["weapon"];
    assert_eq!(child_view.hidden_field_ids, vec!["apparel".to_string()]);
}

#[test]
fn child_replace_resets_inherited_hidden_set_before_applying_own_hidden_fields() {
    let manifest_json = v3_manifest("test.formviews.inherit4");
    let parent_json = r#"{
        "defType": "BuildableDef",
        "abstractType": true,
        "fields": {
            "apparel": { "type": { "kind": "string" } },
            "plant": { "type": { "kind": "string" } },
            "race": { "type": { "kind": "string" } }
        },
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["apparel", "plant"] }
        }
    }"#;
    let child_json = r#"{
        "defType": "GunDef",
        "inherits": ["BuildableDef"],
        "fields": {},
        "formViews": {
            "weapon": { "replace": true, "hiddenFields": ["race"] }
        }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[parent_json, child_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let child_view = &catalog.def_types["GunDef"].form_views["weapon"];
    assert_eq!(child_view.hidden_field_ids, vec!["race".to_string()]);
}

#[test]
fn child_disabled_removes_inherited_view_and_grandchild_can_redeclare_it_fresh() {
    let manifest_json = v3_manifest("test.formviews.inherit5");
    let parent_json = r#"{
        "defType": "BuildableDef",
        "abstractType": true,
        "fields": { "apparel": { "type": { "kind": "string" } } },
        "formViews": {
            "legacy": { "label": "Legacy", "hiddenFields": ["apparel"] }
        }
    }"#;
    let child_json = r#"{
        "defType": "GunDef",
        "inherits": ["BuildableDef"],
        "fields": {},
        "formViews": { "legacy": { "disabled": true } }
    }"#;
    let grandchild_json = r#"{
        "defType": "PistolDef",
        "inherits": ["GunDef"],
        "fields": {},
        "formViews": {
            "legacy": { "label": "Legacy Reborn", "hiddenFields": ["apparel"] }
        }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[parent_json, child_json, grandchild_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    assert!(
        !catalog.def_types["GunDef"]
            .form_views
            .contains_key("legacy"),
        "disabled view must not exist on the child"
    );
    let grandchild_view = &catalog.def_types["PistolDef"].form_views["legacy"];
    assert_eq!(grandchild_view.label, "Legacy Reborn");
    assert_eq!(grandchild_view.declared_on_def_type, "PistolDef");
}

#[test]
fn same_view_id_overlay_across_two_packs_uses_winning_pack_provenance() {
    let low_manifest = v3_manifest_with_priority("test.formviews.low", 0);
    let low_def = r#"{
        "defType": "ThingDef",
        "fields": {
            "apparel": { "type": { "kind": "string" } },
            "plant": { "type": { "kind": "string" } }
        },
        "formViews": { "weapon": { "label": "Weapon", "hiddenFields": ["apparel"] } }
    }"#;
    let low_pack = inline_multi_def_pack(&low_manifest, &[low_def]);

    let high_manifest = v3_manifest_with_priority("test.formviews.high", 10);
    let high_def = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "weapon": { "hiddenFields": ["plant"] } }
    }"#;
    let high_pack = inline_multi_def_pack(&high_manifest, &[high_def]);

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![low_pack, high_pack], &mut diags);

    let view = &catalog.def_types["ThingDef"].form_views["weapon"];
    assert_eq!(
        view.label, "Weapon",
        "label survives from the lower-priority pack's base declaration"
    );
    assert_eq!(
        view.hidden_field_ids,
        vec!["apparel".to_string(), "plant".to_string()]
    );
    assert_eq!(
        view.source.as_ref().map(|s| s.pack_id.as_str()),
        Some("test.formviews.high"),
        "provenance must reflect the last (highest-precedence) declaration applied"
    );
}

#[test]
fn unknown_hidden_field_reference_is_dropped_with_warning_not_fatal() {
    let manifest_json = v3_manifest("test.formviews.unknownfield");
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": { "apparel": { "type": { "kind": "string" } } },
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["apparel", "totallyMadeUpField"] }
        }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[def_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let view = &catalog.def_types["ThingDef"].form_views["weapon"];
    assert_eq!(view.hidden_field_ids, vec!["apparel".to_string()]);
    let warning = diags
        .iter()
        .find(|d| d.code == "schema_pack_form_view_unknown_field_reference")
        .unwrap_or_else(|| {
            panic!(
                "expected an unknown-field-reference warning, got: {:?}",
                diags.iter().map(|d| &d.code).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        warning.path.as_deref(),
        Some("test:def"),
        "the diagnostic must expose the source file path so an author can locate the offending \
         Def-type file, not just the pack id and field path"
    );
    assert_eq!(
        warning.pack_id.as_deref(),
        Some("test.formviews.unknownfield")
    );
    assert_eq!(
        warning.field_path.as_deref(),
        Some("ThingDef.formViews.weapon")
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "unknown field reference must be a warning, not an error: {:?}",
        errors
    );
}

#[test]
fn amendment_with_no_inherited_base_warns_and_is_skipped() {
    let manifest_json = v3_manifest("test.formviews.nobase");
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": { "apparel": { "type": { "kind": "string" } } },
        "formViews": {
            "weapon": { "hiddenFields": ["apparel"] }
        }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[def_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    assert!(
        !catalog.def_types["ThingDef"]
            .form_views
            .contains_key("weapon"),
        "a delta-only declaration with no base must not produce a view"
    );
    let warning = diags
        .iter()
        .find(|d| d.code == "schema_pack_form_view_amendment_without_base")
        .unwrap_or_else(|| {
            panic!(
                "expected amendment-without-base warning, got: {:?}",
                diags.iter().map(|d| &d.code).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        warning.path.as_deref(),
        Some("test:def"),
        "the diagnostic must expose the source file path so an author can locate the offending \
         Def-type file, not just the pack id and field path"
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "must be recoverable, not fatal: {:?}",
        errors
    );
}

#[test]
fn disabled_with_no_prior_view_also_warns_as_amendment_without_base() {
    let manifest_json = v3_manifest("test.formviews.nobasedisabled");
    let def_json = r#"{
        "defType": "ThingDef",
        "fields": {},
        "formViews": { "legacy": { "disabled": true } }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[def_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    assert!(!catalog.def_types["ThingDef"]
        .form_views
        .contains_key("legacy"));
    assert!(diags
        .iter()
        .any(|d| d.code == "schema_pack_form_view_amendment_without_base"));
}

#[test]
fn form_view_resolution_is_deterministic_across_repeated_merges() {
    let manifest_json = v3_manifest("test.formviews.deterministic");
    let parent_json = r#"{
        "defType": "BuildableDef",
        "abstractType": true,
        "fields": {
            "apparel": { "type": { "kind": "string" } },
            "plant": { "type": { "kind": "string" } }
        },
        "formViews": { "weapon": { "label": "Weapon", "hiddenFields": ["apparel", "plant"] } }
    }"#;
    let child_json = r#"{
        "defType": "GunDef",
        "inherits": ["BuildableDef"],
        "fields": {},
        "formViews": { "weapon": { "unhideFields": ["plant"] } }
    }"#;

    let pack1 = inline_multi_def_pack(&manifest_json, &[parent_json, child_json]);
    let mut diags1 = Vec::new();
    let catalog1 = merge_packs(vec![pack1], &mut diags1);

    let pack2 = inline_multi_def_pack(&manifest_json, &[parent_json, child_json]);
    let mut diags2 = Vec::new();
    let catalog2 = merge_packs(vec![pack2], &mut diags2);

    let view1 = &catalog1.def_types["GunDef"].form_views["weapon"];
    let view2 = &catalog2.def_types["GunDef"].form_views["weapon"];
    assert_eq!(view1.hidden_field_ids, view2.hidden_field_ids);
    assert_eq!(view1.label, view2.label);
    assert_eq!(view1.declared_on_def_type, view2.declared_on_def_type);
    assert_eq!(
        view1.source.as_ref().map(|s| s.pack_id.clone()),
        view2.source.as_ref().map(|s| s.pack_id.clone())
    );
}

#[test]
fn inherits_cycle_does_not_infinite_loop_during_form_view_resolution() {
    let manifest_json = v3_manifest("test.formviews.cycle");
    let a_json = r#"{
        "defType": "ADef",
        "inherits": ["BDef"],
        "fields": {},
        "formViews": { "weapon": { "label": "A Weapon" } }
    }"#;
    let b_json = r#"{
        "defType": "BDef",
        "inherits": ["ADef"],
        "fields": {},
        "formViews": { "weapon": { "label": "B Weapon" } }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[a_json, b_json]);
    let mut diags = Vec::new();
    // The test itself would hang/stack-overflow if resolution didn't terminate on a cycle; simply
    // completing is the assertion. The exact resolved content of a malformed cyclic schema is not
    // load-bearing.
    let catalog = merge_packs(vec![pack], &mut diags);
    assert!(catalog.def_types.contains_key("ADef"));
    assert!(catalog.def_types.contains_key("BDef"));
}

#[test]
fn diamond_inheritance_merges_both_parents_deltas_into_the_child() {
    // Regression for multi-parent `inherits` (diamond inheritance): shared grandparent `D`
    // defines "weapon" hiding "d"; parent `B` (extends D) adds a hiddenFields delta ("b"); sibling
    // parent `C` (extends D) adds an unhideFields delta that unhides "d"; child `A` extends
    // [B, C]. Both parents' deltas must survive in A's resolved view: "b" ends up hidden (B's
    // delta) AND "d" ends up NOT hidden (C's delta) -- neither sibling's contribution may
    // wholesale-replace the other's the way a naive "last parent wins" fold would (B and C would
    // otherwise silently clobber each other since they both amend the same inherited view id).
    let manifest_json = v3_manifest("test.formviews.diamond");
    let d_json = r#"{
        "defType": "DDef",
        "abstractType": true,
        "fields": {
            "d": { "type": { "kind": "string" } },
            "other": { "type": { "kind": "string" } }
        },
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["d"] }
        }
    }"#;
    let b_json = r#"{
        "defType": "BDef",
        "inherits": ["DDef"],
        "fields": { "b": { "type": { "kind": "boolean" } } },
        "formViews": {
            "weapon": { "hiddenFields": ["b"] }
        }
    }"#;
    let c_json = r#"{
        "defType": "CDef",
        "inherits": ["DDef"],
        "fields": {},
        "formViews": {
            "weapon": { "unhideFields": ["d"] }
        }
    }"#;
    let a_json = r#"{
        "defType": "ADef",
        "inherits": ["BDef", "CDef"],
        "fields": {}
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[d_json, b_json, c_json, a_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    // B's own standalone resolution: inherits D's {d}, adds "b" -> {b, d}.
    let b_view = &catalog.def_types["BDef"].form_views["weapon"];
    assert_eq!(
        b_view.hidden_field_ids,
        vec!["b".to_string(), "d".to_string()]
    );

    // C's own standalone resolution: inherits D's {d}, unhides "d" -> {}.
    let c_view = &catalog.def_types["CDef"].form_views["weapon"];
    assert!(c_view.hidden_field_ids.is_empty());

    // A (extends [B, C]) must reflect BOTH deltas: "b" hidden (from B), "d" NOT hidden (from C's
    // unhide) -- not just whichever of B/C happened to be folded in last.
    let a_view = &catalog.def_types["ADef"].form_views["weapon"];
    assert_eq!(
        a_view.hidden_field_ids,
        vec!["b".to_string()],
        "both parents' deltas must survive: b hidden (B's delta), d not hidden (C's delta)"
    );
}

#[test]
fn collect_effective_top_level_def_fields_is_ancestor_first_keep_first_for_duplicate_names() {
    // Mirrors the frontend's ancestor-first-keep-first `getAllSchemaFields`
    // (src/features/xml-editor/lib/formDescriptors.ts), NOT Rust's own-fields-first
    // `lookup_field`. Plan.md section 5 flags this exact discrepancy; Form View field-reference
    // validation must use the ancestor-first-keep-first resolver so a hidden-field reference is
    // judged against the same field universe/identity the form actually renders.
    let manifest_json = v3_manifest("test.formviews.canonicalorder");
    let parent_json = r#"{
        "defType": "BuildableDef",
        "abstractType": true,
        "fields": {
            "range": {
                "label": "Ancestor range",
                "required": true,
                "type": { "kind": "float" }
            }
        }
    }"#;
    let child_json = r#"{
        "defType": "GunDef",
        "inherits": ["BuildableDef"],
        "fields": {
            "range": {
                "label": "Child range override",
                "required": false,
                "type": { "kind": "float" }
            }
        }
    }"#;
    let pack = inline_multi_def_pack(&manifest_json, &[parent_json, child_json]);
    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let effective = collect_effective_top_level_def_fields(&catalog, "GunDef");
    let (name, field) = effective
        .iter()
        .find(|(n, _)| n == "range")
        .expect("range field must be present");
    assert_eq!(name, "range");
    assert_eq!(
        field.label.as_deref(),
        Some("Ancestor range"),
        "ancestor-first-keep-first must report the ANCESTOR's field definition as canonical, \
         matching the frontend's getAllSchemaFields -- not the child's own-fields-first override \
         that Rust's existing lookup_field would return"
    );

    // Contrast with lookup_field's own-first policy on the SAME catalog: it deliberately picks
    // the opposite (child-first) definition, which is exactly the discrepancy Plan.md section 5
    // calls out. Form View validation must use the ancestor-first collector above, not this.
    let own_first = lookup_field(&catalog, "GunDef", "range").expect("range must resolve");
    assert_eq!(own_first.label.as_deref(), Some("Child range override"));
}

// ---------------------------------------------------------------------------
// Generic built-in-pack Form View contract checks. These deliberately assert nothing about
// which Def types declare views, which ids they use, or which fields any given view hides --
// that's schema-pack *data*, which changes independently of this code and shouldn't be pinned by
// a Rust test. Instead they validate that whatever the embedded Core pack currently declares
// satisfies the Form View mechanism's own general contract (Plan.md's acceptance criteria),
// against the real built-in pack rather than an inline fixture.
// ---------------------------------------------------------------------------

fn load_built_in_catalog() -> super::super::model::SchemaCatalog {
    let (packs, diags) = load_built_in_packs();
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "built-in pack load errors: {:?}", errors);
    let mut merge_diags = Vec::new();
    let catalog = merge_packs(packs, &mut merge_diags);
    let merge_errors: Vec<_> = merge_diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(
        merge_errors.is_empty(),
        "built-in pack merge errors: {:?}",
        merge_errors
    );
    catalog
}

#[test]
fn built_in_packs_never_trigger_form_views_version_gating() {
    // Whichever built-in packs declare formViews must do so on a formatVersion 3 manifest; this
    // doesn't hardcode which pack or how many views it has, just that the version gate never
    // fires against shipped data.
    let (_packs, diags) = load_built_in_packs();
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "schema_pack_form_views_requires_v3"),
        "a built-in pack declared formViews on a pre-v3 manifest: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn every_built_in_form_view_satisfies_the_general_form_view_contract() {
    let catalog = load_built_in_catalog();
    for (def_type, schema) in &catalog.def_types {
        if schema.form_views.is_empty() {
            continue;
        }
        let effective: HashSet<String> = collect_effective_top_level_def_fields(&catalog, def_type)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        for (id, view) in &schema.form_views {
            assert_ne!(
                id, "default",
                "'default' is reserved for the synthetic Default View"
            );
            assert!(
                !view.label.trim().is_empty(),
                "{}.{} has a blank label",
                def_type,
                id
            );
            assert!(
                view.description
                    .as_deref()
                    .map(|d| !d.trim().is_empty())
                    .unwrap_or(false),
                "{}.{} must have a nonblank description",
                def_type,
                id
            );
            assert!(
                !view.recommended,
                "{}.{} must not be recommended: no schema predicate can safely infer the \
                 archetype from category/class alone (Plan.md)",
                def_type, id
            );

            let mut seen = HashSet::new();
            for field_id in &view.hidden_field_ids {
                assert!(
                    seen.insert(field_id.clone()),
                    "{}.{} has a duplicate hidden field id '{}'",
                    def_type,
                    id,
                    field_id
                );
                assert!(
                    effective.contains(field_id),
                    "{}.{} hides unknown field id '{}'",
                    def_type,
                    id,
                    field_id
                );
            }
        }
    }
}
