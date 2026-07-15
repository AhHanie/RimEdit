//! Issue 06 -- locale-aware catalog command contract and caching. These tests exercise the
//! explicit `locale` threading added on top of issue 05's sidecar mechanism: picking the
//! requested locale's overlay at the `merge_packs_with_locale` layer, and the application
//! locale-policy fallback applied by `build_schema_catalog_with_locale`/`resolve_catalog_locale`.

use super::super::loader::LoadedPack;
use super::super::locale::SchemaLocaleOverlay;
use super::super::merge::{merge_packs, merge_packs_with_locale};
use super::super::{
    build_schema_catalog, build_schema_catalog_with_locale, resolve_catalog_locale,
};
use super::inline_pack;

fn pack_with_two_locale_overrides() -> LoadedPack {
    let manifest = r#"{ "formatVersion": 1, "packId": "test.locale.sync", "name": "Sync", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "ThingDef", "label": "Thing", "fields": {} }"#;
    let mut pack = inline_pack(manifest, def_json);

    let mut en_overlay = SchemaLocaleOverlay::new();
    en_overlay.insert(
        "defTypes.ThingDef.label".to_string(),
        "Thing (en)".to_string(),
    );
    pack.locales.insert("en".to_string(), en_overlay);

    let mut fr_overlay = SchemaLocaleOverlay::new();
    fr_overlay.insert(
        "defTypes.ThingDef.label".to_string(),
        "Chose (fr)".to_string(),
    );
    pack.locales.insert("fr".to_string(), fr_overlay);

    pack
}

#[test]
fn merge_packs_with_locale_applies_only_the_requested_locale_bundle() {
    let mut en_diags = Vec::new();
    let en_catalog =
        merge_packs_with_locale(vec![pack_with_two_locale_overrides()], &mut en_diags, "en");
    assert_eq!(
        en_catalog
            .def_types
            .get("ThingDef")
            .unwrap()
            .label
            .as_deref(),
        Some("Thing (en)")
    );

    let mut fr_diags = Vec::new();
    let fr_catalog =
        merge_packs_with_locale(vec![pack_with_two_locale_overrides()], &mut fr_diags, "fr");
    assert_eq!(
        fr_catalog
            .def_types
            .get("ThingDef")
            .unwrap()
            .label
            .as_deref(),
        Some("Chose (fr)")
    );
}

#[test]
fn merge_packs_default_wrapper_matches_explicit_fallback_locale() {
    let mut default_diags = Vec::new();
    let default_catalog = merge_packs(vec![pack_with_two_locale_overrides()], &mut default_diags);
    let mut explicit_diags = Vec::new();
    let explicit_catalog = merge_packs_with_locale(
        vec![pack_with_two_locale_overrides()],
        &mut explicit_diags,
        crate::locale::FALLBACK_LOCALE,
    );
    assert_eq!(
        default_catalog.def_types.get("ThingDef").unwrap().label,
        explicit_catalog.def_types.get("ThingDef").unwrap().label,
    );
}

#[test]
fn resolve_catalog_locale_falls_back_deterministically() {
    assert_eq!(resolve_catalog_locale(None), "en");
    assert_eq!(resolve_catalog_locale(Some("en")), "en");
    // "fr" is not in `crate::locale::SUPPORTED_LOCALES` (the *application* locale registry --
    // distinct from a schema pack's own locale sidecar tags), so it must fall back to "en" too.
    assert_eq!(resolve_catalog_locale(Some("fr")), "en");
    assert_eq!(resolve_catalog_locale(Some("")), "en");
}

#[test]
fn build_schema_catalog_with_locale_falls_back_for_unsupported_app_locale() {
    let none_result = build_schema_catalog_with_locale(&[], None, None);
    let unsupported_result = build_schema_catalog_with_locale(&[], None, Some("fr"));
    let explicit_en_result = build_schema_catalog_with_locale(&[], None, Some("en"));

    let none_label = none_result
        .catalog
        .def_types
        .get("ThingDef")
        .unwrap()
        .label
        .clone();
    let unsupported_label = unsupported_result
        .catalog
        .def_types
        .get("ThingDef")
        .unwrap()
        .label
        .clone();
    let explicit_label = explicit_en_result
        .catalog
        .def_types
        .get("ThingDef")
        .unwrap()
        .label
        .clone();

    assert_eq!(none_label, unsupported_label);
    assert_eq!(none_label, explicit_label);
}

#[test]
fn build_schema_catalog_locale_neutral_wrapper_matches_explicit_fallback() {
    let default_result = build_schema_catalog(&[], None);
    let explicit_result = build_schema_catalog_with_locale(&[], None, None);
    assert_eq!(
        default_result
            .catalog
            .def_types
            .get("ThingDef")
            .unwrap()
            .label,
        explicit_result
            .catalog
            .def_types
            .get("ThingDef")
            .unwrap()
            .label,
    );
}
