use super::super::loader::{load_built_in_packs, load_pack_from_directory, parse_locale_bundle};
use super::super::locale::SchemaLocaleOverlay;
use super::super::merge::merge_packs;
use super::super::model::{FieldTypeKind, SchemaLoadSeverity};
use super::{inline_pack, inline_pack_with_patch_operations, load_fixture};
use std::path::Path;

// Mirrors `project_files::tests::create_symlink_file`'s established cross-platform pattern: a
// symlink-to-a-directory helper, tried and gracefully skipped (not failed) by callers when
// creation errors, since Windows can require elevated privileges or Developer Mode to create
// symlinks and CI/dev environments vary.
#[cfg(unix)]
fn create_dir_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn create_dir_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(src, dst)
}

// --- 1. Valid sidecar overrides only its own pack's own content ---
//
// Uses `load_pack_from_directory` directly (not the `load_fixture` test helper) so the load-time
// `schema_pack_locale_unknown_key` warning -- which `load_fixture` would otherwise discard after
// asserting there are no *errors* -- is still observable here.

#[test]
fn valid_locale_sidecar_overrides_own_pack_content_and_ignores_unknown_keys() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema_pack/valid_with_locales/schema-pack.json");
    let (pack_opt, load_diags) = load_pack_from_directory(&manifest_path);
    let errors: Vec<_> = load_diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "fixture had errors: {:?}", errors);
    let pack = pack_opt.expect("fixture must load");
    assert!(
        pack.locales.contains_key("en"),
        "expected an 'en' locale bundle to be loaded"
    );
    assert!(
        load_diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_unknown_key"),
        "expected schema_pack_locale_unknown_key for the malformed sidecar key, got: {:?}",
        load_diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);

    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");
    assert_eq!(
        thing_def.label.as_deref(),
        Some("Thing (localized)"),
        "def-type label should be overridden by the sidecar"
    );

    let def_name = thing_def.fields.get("defName").expect("defName");
    assert_eq!(
        def_name.description.as_deref(),
        Some("The unique identifier (localized)."),
        "field description should be overridden by the sidecar"
    );
    assert_eq!(
        def_name.label.as_deref(),
        Some("Def name"),
        "field label was not targeted by the sidecar and must stay canonical"
    );
}

// --- 2. Pack without a sidecar behaves exactly as before ---

#[test]
fn pack_without_locale_sidecar_behaves_exactly_as_today() {
    let pack = load_fixture("valid_minimal");
    assert!(
        pack.locales.is_empty(),
        "a pack with no localesDirectory must load zero locale bundles"
    );

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack], &mut diags);
    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");
    assert_eq!(thing_def.label.as_deref(), Some("Thing"));
    assert!(
        !diags
            .iter()
            .any(|d| d.code.starts_with("schema_pack_locale_")),
        "no locale diagnostics should appear for a pack with no sidecars, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 3. A sidecar cannot override a resource owned by a different pack ---

#[test]
fn sidecar_cannot_override_a_resource_owned_by_a_different_pack() {
    let owner_manifest = r#"{ "formatVersion": 1, "packId": "test.locale.owner", "name": "Owner", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["x"] }"#;
    let owner_def = r#"{ "defType": "ThingDef", "label": "Thing A", "fields": {} }"#;
    let owner_pack = inline_pack(owner_manifest, owner_def);

    let intruder_manifest = r#"{ "formatVersion": 1, "packId": "test.locale.intruder", "name": "Intruder", "version": "1.0.0", "priority": 10, "defTypeDirectories": ["x"] }"#;
    // The intruder pack never declares ThingDef itself -- it only ships a locale sidecar that
    // tries to override a def type it doesn't own.
    let intruder_def = r#"{ "defType": "UnrelatedDef", "fields": {} }"#;
    let mut intruder_pack = inline_pack(intruder_manifest, intruder_def);
    let mut overlay = SchemaLocaleOverlay::new();
    overlay.insert(
        "defTypes.ThingDef.label".to_string(),
        "Hijacked".to_string(),
    );
    intruder_pack.locales.insert("en".to_string(), overlay);

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![owner_pack, intruder_pack], &mut diags);

    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");
    assert_eq!(
        thing_def.label.as_deref(),
        Some("Thing A"),
        "a different pack's sidecar must never override a resource it doesn't own"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_wrong_owner"),
        "expected schema_pack_locale_wrong_owner, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 3b. A partial-amendment pack cannot sidecar-override a scalar it never supplied ---
//
// Pack A originally declares ThingDef with a label. Pack B amends the SAME ThingDef, changing only
// a field pack A never declared (not the label) -- this is the "amendment", not "never declared at
// all" case test 3 above covers. Ownership of `label` must stay with pack A: pack B's sidecar
// attempting to override the label must be rejected as wrong-owner, even though pack B did touch
// this def type, while pack B's sidecar overriding the field it *did* supply must be accepted.

#[test]
fn partial_amendment_pack_cannot_override_a_scalar_it_never_supplied() {
    let pack_a_manifest = r#"{ "formatVersion": 1, "packId": "test.locale.amend.a", "name": "A", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["x"] }"#;
    let pack_a_def = r#"{ "defType": "ThingDef", "label": "Thing A", "fields": {} }"#;
    let pack_a = inline_pack(pack_a_manifest, pack_a_def);

    // Pack B amends the same ThingDef, adding a field of its own but never touching `label`.
    let pack_b_manifest = r#"{ "formatVersion": 1, "packId": "test.locale.amend.b", "name": "B", "version": "1.0.0", "priority": 10, "defTypeDirectories": ["x"] }"#;
    let pack_b_def = r#"{ "defType": "ThingDef", "fields": { "bStat": { "type": { "kind": "integer" }, "label": "B Stat" } } }"#;
    let mut pack_b = inline_pack(pack_b_manifest, pack_b_def);

    let mut overlay = SchemaLocaleOverlay::new();
    // Wrong-owner attempt: pack B never supplied the label -- pack A still owns it.
    overlay.insert(
        "defTypes.ThingDef.label".to_string(),
        "Hijacked".to_string(),
    );
    // Legitimate override: pack B does own the field it declared.
    overlay.insert(
        "defTypes.ThingDef.fields.bStat.label".to_string(),
        "B Stat (localized)".to_string(),
    );
    pack_b.locales.insert("en".to_string(), overlay);

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack_a, pack_b], &mut diags);

    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");
    assert_eq!(
        thing_def.label.as_deref(),
        Some("Thing A"),
        "an amending pack must never override a label it didn't itself supply"
    );
    assert_eq!(
        thing_def
            .fields
            .get("bStat")
            .expect("bStat")
            .label
            .as_deref(),
        Some("B Stat (localized)"),
        "the amending pack's sidecar must still be able to override the field it did supply"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_wrong_owner"),
        "expected schema_pack_locale_wrong_owner for the label override, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 4. A sidecar key that doesn't resolve at all produces a recoverable diagnostic ---

#[test]
fn sidecar_key_that_does_not_resolve_produces_unresolved_diagnostic() {
    let manifest = r#"{ "formatVersion": 1, "packId": "test.locale.unresolved", "name": "U", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["x"] }"#;
    let def_json = r#"{ "defType": "ThingDef", "label": "Thing", "fields": {} }"#;
    let mut pack = inline_pack(manifest, def_json);
    let mut overlay = SchemaLocaleOverlay::new();
    overlay.insert("defTypes.NoSuchDef.label".to_string(), "Ghost".to_string());
    pack.locales.insert("en".to_string(), overlay);

    let mut diags = Vec::new();
    let _catalog = merge_packs(vec![pack], &mut diags);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_unresolved_key"),
        "expected schema_pack_locale_unresolved_key, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 5. Declared-but-missing localesDirectory is silently a no-op ---

#[test]
fn missing_locales_directory_produces_no_diagnostic_and_empty_bundle() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    std::fs::write(
        def_types_dir.join("ThingDef.json"),
        r#"{ "defType": "ThingDef", "fields": {} }"#,
    )
    .unwrap();
    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.locale.missingdir",
        "name": "Missing Locale Dir",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "localesDirectory": "locales"
    });
    std::fs::write(tmp.path().join("schema-pack.json"), manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&tmp.path().join("schema-pack.json"));
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    let pack = pack_opt.expect("pack must still load");
    assert!(
        pack.locales.is_empty(),
        "a declared-but-absent localesDirectory must yield zero locale bundles"
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.code.starts_with("schema_pack_locale_")),
        "a declared-but-absent localesDirectory must not itself produce a diagnostic, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 6. localesDirectory escaping the pack root is rejected ---

#[test]
fn locales_directory_escaping_pack_root_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    std::fs::write(
        def_types_dir.join("ThingDef.json"),
        r#"{ "defType": "ThingDef", "fields": {} }"#,
    )
    .unwrap();
    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.locale.escape",
        "name": "Locale Escape",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "localesDirectory": "../evil"
    });
    std::fs::write(tmp.path().join("schema-pack.json"), manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&tmp.path().join("schema-pack.json"));
    assert!(pack_opt.is_some(), "pack should still assemble");
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_directory_escape"),
        "expected schema_pack_locale_directory_escape, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 6b. localesDirectory escaping the pack root via an *intermediate* symlink
// component is rejected the same way a lexical '..' escape is ---------------------------------
//
// `link` is a symlink to a directory *outside* the pack root (`tmp`); `locales` is a real,
// non-symlink subdirectory reached only by first following that symlink. The pack declares
// `"localesDirectory": "link/locales"`. A check that only inspects whether the *final resolved
// directory itself* is a symlink (as `read_locale_directory_files`'s `is_symlink(&resolved)` call
// does) would miss this, since the final component (`locales`) is not itself a symlink -- only an
// earlier component is. `resolve_manifest_relative_dir` must canonicalize the full path (which
// follows every component, including intermediate symlinks) to catch it.

#[test]
fn locales_directory_escaping_pack_root_via_intermediate_symlink_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("outside temp dir");

    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    std::fs::write(
        def_types_dir.join("ThingDef.json"),
        r#"{ "defType": "ThingDef", "fields": {} }"#,
    )
    .unwrap();

    // A real, non-symlink "locales" directory living outside the pack root, containing a locale
    // file that must never be read.
    let outside_locales = outside.path().join("locales");
    std::fs::create_dir(&outside_locales).unwrap();
    std::fs::write(
        outside_locales.join("en.json"),
        r#"{ "defTypes.ThingDef.label": "Hijacked" }"#,
    )
    .unwrap();

    let link = tmp.path().join("link");
    if create_dir_symlink(outside.path(), &link).is_err() {
        // No symlink privilege in this environment (e.g. non-admin, non-Developer-Mode Windows)
        // -- nothing meaningful to assert; skip rather than fail the suite.
        return;
    }

    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.locale.symlinkescape",
        "name": "Locale Symlink Escape",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "localesDirectory": "link/locales"
    });
    std::fs::write(tmp.path().join("schema-pack.json"), manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&tmp.path().join("schema-pack.json"));
    let pack = pack_opt.expect("pack should still assemble");
    assert!(
        pack.locales.is_empty(),
        "the locale file reached only through the outside-root symlink must never be loaded"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_directory_escape"),
        "expected schema_pack_locale_directory_escape, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 7. Oversized locale file is skipped ---

#[test]
fn oversized_locale_file_is_skipped() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    std::fs::write(
        def_types_dir.join("ThingDef.json"),
        r#"{ "defType": "ThingDef", "fields": {} }"#,
    )
    .unwrap();
    let locales_dir = tmp.path().join("locales");
    std::fs::create_dir(&locales_dir).unwrap();
    // Build an oversized (> 256 KiB) but otherwise valid JSON object.
    let padding = "x".repeat(300 * 1024);
    let oversized = format!(r#"{{ "defTypes.ThingDef.label": "{}" }}"#, padding);
    std::fs::write(locales_dir.join("en.json"), oversized).unwrap();

    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.locale.oversized",
        "name": "Oversized Locale",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "localesDirectory": "locales"
    });
    std::fs::write(tmp.path().join("schema-pack.json"), manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&tmp.path().join("schema-pack.json"));
    let pack = pack_opt.expect("pack must still load");
    assert!(
        pack.locales.is_empty(),
        "oversized locale file must be skipped entirely"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_file_too_large"),
        "expected schema_pack_locale_file_too_large, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 8. Invalid locale tag filename is rejected ---

#[test]
fn invalid_locale_tag_filename_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    std::fs::write(
        def_types_dir.join("ThingDef.json"),
        r#"{ "defType": "ThingDef", "fields": {} }"#,
    )
    .unwrap();
    let locales_dir = tmp.path().join("locales");
    std::fs::create_dir(&locales_dir).unwrap();
    std::fs::write(locales_dir.join("123.json"), "{}").unwrap();

    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.locale.badtag",
        "name": "Bad Tag",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "localesDirectory": "locales"
    });
    std::fs::write(tmp.path().join("schema-pack.json"), manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&tmp.path().join("schema-pack.json"));
    let pack = pack_opt.expect("pack must still load");
    assert!(pack.locales.is_empty());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_invalid_tag"),
        "expected schema_pack_locale_invalid_tag, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 9. Duplicate (case-insensitive) locale tag is rejected, first occurrence wins ---
//
// Exercises `parse_locale_bundle` directly with two distinct (label, content) pairs rather than
// two real files on disk: on a case-insensitive filesystem (default on Windows and macOS),
// `EN.json` and `en.json` name the very same file, so a real two-file fixture can't actually
// reproduce this collision on every platform CI/dev runs on.

#[test]
fn duplicate_locale_tag_case_insensitive_is_rejected() {
    let mut diags = Vec::new();
    let bundle = parse_locale_bundle(
        "test.locale.duptag",
        &[("locales/EN.json", "{}"), ("locales/en.json", "{}")],
        &mut diags,
    );
    assert_eq!(
        bundle.len(),
        1,
        "only one 'en' bundle should survive, got tags: {:?}",
        bundle.keys().collect::<Vec<_>>()
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_duplicate_tag"),
        "expected schema_pack_locale_duplicate_tag, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 10. Malformed locale JSON is fatal only for that file ---

#[test]
fn malformed_locale_json_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let def_types_dir = tmp.path().join("def-types");
    std::fs::create_dir(&def_types_dir).unwrap();
    std::fs::write(
        def_types_dir.join("ThingDef.json"),
        r#"{ "defType": "ThingDef", "fields": {} }"#,
    )
    .unwrap();
    let locales_dir = tmp.path().join("locales");
    std::fs::create_dir(&locales_dir).unwrap();
    std::fs::write(locales_dir.join("en.json"), "{ not valid json }").unwrap();

    let manifest = serde_json::json!({
        "formatVersion": 1,
        "packId": "test.locale.malformed",
        "name": "Malformed",
        "version": "1.0.0",
        "defTypeDirectories": ["def-types"],
        "localesDirectory": "locales"
    });
    std::fs::write(tmp.path().join("schema-pack.json"), manifest.to_string()).unwrap();

    let (pack_opt, diags) = load_pack_from_directory(&tmp.path().join("schema-pack.json"));
    let pack = pack_opt.expect("pack must still load despite the malformed locale file");
    assert!(pack.locales.is_empty());
    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_json_invalid"),
        "expected schema_pack_locale_json_invalid, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 11. Built-in pack loads cleanly with a declared-but-absent localesDirectory ---

#[test]
fn built_in_pack_label_stays_canonical_english_with_no_shipped_sidecar() {
    let (packs, diags) = load_built_in_packs();
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, SchemaLoadSeverity::Error))
        .collect();
    assert!(errors.is_empty(), "built-in pack has errors: {:?}", errors);
    assert!(
        !diags.iter().any(|d| d.code.starts_with("schema_pack_locale_")),
        "built-in pack ships no locale sidecars yet and must produce zero locale diagnostics, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );

    let mut merge_diags = Vec::new();
    let catalog = merge_packs(packs, &mut merge_diags);
    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");
    assert!(
        thing_def.label.is_some(),
        "built-in ThingDef must keep its canonical English label"
    );
}

// --- 12. A field-level amendment that touches only a non-display property must
// not transfer sidecar-override rights over that field's label/description -----------------------
//
// Pack A originally declares ThingDef's "aStat" field with a label, a description, and an integer
// type. Pack B amends the SAME field, changing only its `type` (never touching label or
// description) -- and separately declares a brand-new field "bStat" of its own, with its own
// label. Ownership of aStat's label/description must stay with pack A even though pack B did
// amend some other property of the very same field; pack B's sidecar must still be able to
// override the field it actually did originate (bStat).

#[test]
fn field_type_only_amendment_does_not_transfer_label_description_ownership() {
    let pack_a_manifest = r#"{ "formatVersion": 1, "packId": "test.locale.fieldamend.a", "name": "A", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["x"] }"#;
    let pack_a_def = r#"{
        "defType": "ThingDef",
        "fields": {
            "aStat": {
                "type": { "kind": "integer" },
                "label": "A Stat",
                "description": "A stat description."
            }
        }
    }"#;
    let pack_a = inline_pack(pack_a_manifest, pack_a_def);

    // Pack B amends aStat's type only (never label/description), and separately declares a new
    // field of its own (bStat).
    let pack_b_manifest = r#"{ "formatVersion": 1, "packId": "test.locale.fieldamend.b", "name": "B", "version": "1.0.0", "priority": 10, "defTypeDirectories": ["x"] }"#;
    let pack_b_def = r#"{
        "defType": "ThingDef",
        "fields": {
            "aStat": { "type": { "kind": "float" } },
            "bStat": { "type": { "kind": "string" }, "label": "B Stat" }
        }
    }"#;
    let mut pack_b = inline_pack(pack_b_manifest, pack_b_def);

    let mut overlay = SchemaLocaleOverlay::new();
    // Wrong-owner attempts: pack B amended aStat's type, but never its label/description -- pack A
    // still owns both.
    overlay.insert(
        "defTypes.ThingDef.fields.aStat.label".to_string(),
        "Hijacked".to_string(),
    );
    overlay.insert(
        "defTypes.ThingDef.fields.aStat.description".to_string(),
        "Hijacked description.".to_string(),
    );
    // Legitimate override: pack B does own the field it declared from scratch.
    overlay.insert(
        "defTypes.ThingDef.fields.bStat.label".to_string(),
        "B Stat (localized)".to_string(),
    );
    pack_b.locales.insert("en".to_string(), overlay);

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack_a, pack_b], &mut diags);

    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");
    let a_stat = thing_def.fields.get("aStat").expect("aStat");
    assert_eq!(
        a_stat.label.as_deref(),
        Some("A Stat"),
        "a type-only amendment must never grant sidecar-override rights over a label the \
         amending pack didn't itself supply"
    );
    assert_eq!(
        a_stat.description.as_deref(),
        Some("A stat description."),
        "a type-only amendment must never grant sidecar-override rights over a description the \
         amending pack didn't itself supply"
    );
    assert_eq!(
        a_stat.field_type.kind,
        FieldTypeKind::Float,
        "pack B's type amendment must still take effect"
    );
    assert_eq!(
        thing_def
            .fields
            .get("bStat")
            .expect("bStat")
            .label
            .as_deref(),
        Some("B Stat (localized)"),
        "the amending pack's sidecar must still be able to override a field it did originate"
    );

    let wrong_owner_count = diags
        .iter()
        .filter(|d| d.code == "schema_pack_locale_wrong_owner")
        .count();
    assert_eq!(
        wrong_owner_count,
        2,
        "expected exactly two wrong-owner diagnostics (aStat label + description), got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 13. A patch-operation amendment that only adds a field must not transfer
// sidecar-override rights over the operation's own label/description -----------------------------

#[test]
fn patch_operation_field_only_amendment_does_not_transfer_label_description_ownership() {
    let op_a_json = r#"{
        "formatVersion": 1,
        "className": "PatchOperationFoo",
        "label": "Foo",
        "description": "Does foo things.",
        "fields": {}
    }"#;
    let manifest_a = r#"{ "formatVersion": 1, "packId": "test.locale.opamend.a", "name": "A", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["x"] }"#;
    let pack_a = inline_pack_with_patch_operations(manifest_a, &[op_a_json]);

    // Pack B amends the SAME operation, adding only a new field -- it never touches label or
    // description.
    let op_b_json = r#"{
        "formatVersion": 1,
        "className": "PatchOperationFoo",
        "fields": {
            "extra": { "type": { "kind": "string" }, "label": "Extra" }
        }
    }"#;
    let manifest_b = r#"{ "formatVersion": 1, "packId": "test.locale.opamend.b", "name": "B", "version": "1.0.0", "priority": 10, "defTypeDirectories": ["x"] }"#;
    let mut pack_b = inline_pack_with_patch_operations(manifest_b, &[op_b_json]);

    let mut overlay = SchemaLocaleOverlay::new();
    overlay.insert(
        "patchOperations.PatchOperationFoo.label".to_string(),
        "Hijacked".to_string(),
    );
    overlay.insert(
        "patchOperations.PatchOperationFoo.description".to_string(),
        "Hijacked description.".to_string(),
    );
    // Legitimate: pack B does own the field it declared.
    overlay.insert(
        "patchOperations.PatchOperationFoo.fields.extra.label".to_string(),
        "Extra (localized)".to_string(),
    );
    pack_b.locales.insert("en".to_string(), overlay);

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack_a, pack_b], &mut diags);

    let op = catalog.patch_operations.get("PatchOperationFoo").unwrap();
    assert_eq!(
        op.label.as_deref(),
        Some("Foo"),
        "a field-only amendment must never grant sidecar-override rights over a label the \
         amending pack didn't itself supply"
    );
    assert_eq!(
        op.description.as_deref(),
        Some("Does foo things."),
        "a field-only amendment must never grant sidecar-override rights over a description the \
         amending pack didn't itself supply"
    );
    assert!(
        op.fields.contains_key("extra"),
        "pack B's new field must still be merged in"
    );
    assert_eq!(
        op.fields.get("extra").unwrap().label.as_deref(),
        Some("Extra (localized)"),
        "the amending pack's sidecar must still be able to override a field it did originate"
    );

    let wrong_owner_count = diags
        .iter()
        .filter(|d| d.code == "schema_pack_locale_wrong_owner")
        .count();
    assert_eq!(
        wrong_owner_count,
        2,
        "expected exactly two wrong-owner diagnostics (label + description), got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// --- 14. A Form View delta amendment (hiddenFields/unhideFields only) must not
// transfer sidecar-override rights over that view's inherited label/description -------------------

#[test]
fn form_view_delta_amendment_does_not_transfer_label_description_ownership() {
    let manifest_a = r#"{ "formatVersion": 3, "packId": "test.locale.fvamend.a", "name": "A", "version": "1.0.0", "priority": 0, "defTypeDirectories": ["x"] }"#;
    let def_a = r#"{
        "defType": "ThingDef",
        "fields": {
            "fieldX": { "type": { "kind": "string" }, "required": false },
            "fieldY": { "type": { "kind": "string" }, "required": false }
        },
        "formViews": {
            "weapon": { "label": "Weapon", "hiddenFields": ["fieldY"] }
        }
    }"#;
    let pack_a = inline_pack(manifest_a, def_a);

    // Pack B amends the SAME view with a pure hiddenFields delta (no label), and separately
    // declares a brand-new view of its own.
    let manifest_b = r#"{ "formatVersion": 3, "packId": "test.locale.fvamend.b", "name": "B", "version": "1.0.0", "priority": 10, "defTypeDirectories": ["x"] }"#;
    let def_b = r#"{
        "defType": "ThingDef",
        "fields": {
            "fieldX": { "type": { "kind": "string" }, "required": false },
            "fieldY": { "type": { "kind": "string" }, "required": false }
        },
        "formViews": {
            "weapon": { "unhideFields": ["fieldY"] },
            "armor": { "label": "Armor" }
        }
    }"#;
    let mut pack_b = inline_pack(manifest_b, def_b);

    let mut overlay = SchemaLocaleOverlay::new();
    overlay.insert(
        "defTypes.ThingDef.formViews.weapon.label".to_string(),
        "Hijacked".to_string(),
    );
    // Legitimate: pack B does own the view it declared from scratch.
    overlay.insert(
        "defTypes.ThingDef.formViews.armor.label".to_string(),
        "Armor (localized)".to_string(),
    );
    pack_b.locales.insert("en".to_string(), overlay);

    let mut diags = Vec::new();
    let catalog = merge_packs(vec![pack_a, pack_b], &mut diags);

    let thing_def = catalog.def_types.get("ThingDef").expect("ThingDef");
    let weapon = thing_def.form_views.get("weapon").expect("weapon view");
    assert_eq!(
        weapon.label, "Weapon",
        "a hiddenFields-only delta amendment must never grant sidecar-override rights over a \
         label the amending pack didn't itself supply"
    );
    assert!(
        !weapon.hidden_field_ids.contains(&"fieldY".to_string()),
        "pack B's unhideFields delta must still take effect"
    );
    let armor = thing_def.form_views.get("armor").expect("armor view");
    assert_eq!(
        armor.label, "Armor (localized)",
        "the amending pack's sidecar must still be able to override a view it did originate"
    );

    assert!(
        diags
            .iter()
            .any(|d| d.code == "schema_pack_locale_wrong_owner"),
        "expected schema_pack_locale_wrong_owner for the weapon label override, got: {:?}",
        diags.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}
