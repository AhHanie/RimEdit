use crate::def_index::{
    load_cached_index_only, load_cached_index_unverified, load_or_rebuild_def_index,
    rebuild_and_store_def_index, verify_fingerprints, DefIndexBuildOptions, DefIndexReplacement,
    FingerprintVerification,
};
use crate::project_model::{LocationKind, ProjectSettings};
use std::fs;

use super::{location, temp_dir};

fn steel_project_settings(project_dir: &std::path::Path, game_version: &str) -> ProjectSettings {
    ProjectSettings {
        schema_version: 4,
        game_version: game_version.to_string(),
        locale: "en".to_string(),
        locations: vec![location(project_dir, "project", LocationKind::Project)],
        active_project_id: Some("project".to_string()),
        save_backups_enabled: false,
    }
}

fn cache_file_path(app_dir: &std::path::Path) -> std::path::PathBuf {
    app_dir.join("def-index").join("index-cache-v3.bin.zst")
}

#[test]
fn cache_roundtrip_preserves_defs() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = ProjectSettings {
        schema_version: 4,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations: vec![location(&project_dir, "project", LocationKind::Project)],
        active_project_id: Some("project".to_string()),
        save_backups_enabled: false,
    };

    let first = rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();
    let second = load_or_rebuild_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();

    assert_eq!(first.defs.len(), second.defs.len());
    assert_eq!(second.defs[0].def_name, "Steel");
    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn load_cached_index_only_returns_a_hit_with_usable_fingerprints_and_computed_fields() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");

    rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();

    let cached = load_cached_index_only(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
    )
    .expect("expected a cache hit");

    assert_eq!(cached.index.defs.len(), 1);
    assert_eq!(cached.index.defs[0].def_name, "Steel");
    assert!(!cached.file_fingerprints.is_empty());
    assert!(!cached.settings_fingerprint.is_empty());
    // Computed fields are `#[serde(skip)]`, so a real roundtrip through disk leaves them
    // empty until `rebuild_computed_fields()` restores them.
    assert_eq!(cached.index.defs[0].def_name_lower, "steel");
    assert!(cached.index.by_type.contains_key("ThingDef"));

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn load_cached_index_only_misses_without_creating_a_cache_file_when_none_exists() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");

    let cached = load_cached_index_only(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
    );

    assert!(cached.is_none());
    assert!(!cache_file_path(&app_dir).exists());

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn load_cached_index_only_misses_when_an_indexed_file_changes_after_storing() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");

    rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();

    // Mutate the indexed file's content after the cache was stored.
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>SteelChanged</defName></ThingDef></Defs>",
    )
    .unwrap();

    let cached = load_cached_index_only(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
    );

    assert!(
        cached.is_none(),
        "a changed file's content hash must invalidate the cache"
    );

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn load_cached_index_only_misses_when_the_game_version_setting_changes() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");

    rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();

    let changed_settings = steel_project_settings(&project_dir, "1.5");
    let cached = load_cached_index_only(
        &app_dir,
        &changed_settings,
        &DefIndexBuildOptions::for_project("project"),
    );

    assert!(
        cached.is_none(),
        "a game version change must invalidate the cache via the settings fingerprint"
    );

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn load_cached_index_only_treats_invalid_json_and_an_incompatible_version_as_a_non_fatal_miss() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");
    let cache_path = cache_file_path(&app_dir);

    fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
    fs::write(&cache_path, b"not a valid cache file").unwrap();
    assert!(load_cached_index_only(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
    )
    .is_none());

    let mismatched = crate::def_index::cache_v3::encode_with_version_for_test(
        "x".to_string(),
        vec![],
        &crate::def_index::DefIndex::default(),
        999,
    );
    fs::write(&cache_path, mismatched).unwrap();
    assert!(load_cached_index_only(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
    )
    .is_none());

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn unverified_hydration_returns_a_saved_index_without_scanning_indexed_files() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");

    rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();

    // Delete the indexed root entirely after storing. A scan-based reader (`load_cached_index_only`)
    // must now miss; the unverified reader must still hit, proving it never touches indexed files.
    fs::remove_dir_all(project_dir.join("Defs")).unwrap();

    let unverified = load_cached_index_unverified(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
        None,
    )
    .expect("unverified hydration must hit even though the indexed root is gone");
    assert_eq!(unverified.index.defs.len(), 1);
    assert_eq!(unverified.index.defs[0].def_name, "Steel");
    // Computed fields (`#[serde(skip)]`) must be rebuilt after deserialization.
    assert_eq!(unverified.index.defs[0].def_name_lower, "steel");
    assert!(unverified.index.by_type.contains_key("ThingDef"));
    assert!(!unverified.file_fingerprints.is_empty());

    let strict = load_cached_index_only(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
    );
    assert!(
        strict.is_none(),
        "strict validation must miss once the indexed root is gone"
    );

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn unverified_hydration_is_a_benign_miss_for_absent_malformed_and_version_mismatched_caches() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");
    let cache_path = cache_file_path(&app_dir);

    // Absent.
    assert!(load_cached_index_unverified(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
        None,
    )
    .is_none());

    // Malformed/corrupt bytes.
    fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
    fs::write(&cache_path, b"not a valid cache file").unwrap();
    assert!(load_cached_index_unverified(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
        None,
    )
    .is_none());

    // Version mismatch: valid CBOR+zstd framing, but a `version` this build doesn't support.
    let mismatched = crate::def_index::cache_v3::encode_with_version_for_test(
        "x".to_string(),
        vec![],
        &crate::def_index::DefIndex::default(),
        999,
    );
    fs::write(&cache_path, mismatched).unwrap();
    assert!(load_cached_index_unverified(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
        None,
    )
    .is_none());

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn a_pre_phase_2_v2_json_cache_at_the_old_filename_is_silently_ignored_not_migrated() {
    // `index-cache-v3.bin.zst` replaced `index-cache-v1.json` at a *different* filename rather
    // than converting in place -- a leftover pre-upgrade file must be a benign miss, exactly
    // like any other absent cache, never read or migrated.
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");
    let old_v2_path = app_dir.join("def-index").join("index-cache-v1.json");
    fs::create_dir_all(old_v2_path.parent().unwrap()).unwrap();
    fs::write(
        &old_v2_path,
        r#"{"version":2,"settingsFingerprint":"whatever","fileFingerprints":[],"index":{"defs":[],"errors":[],"builtAtUnixMs":0}}"#,
    )
    .unwrap();

    assert!(
        load_cached_index_only(
            &app_dir,
            &settings,
            &DefIndexBuildOptions::for_project("project"),
        )
        .is_none(),
        "a leftover v2 JSON cache at the old filename must never be read"
    );
    assert!(
        old_v2_path.exists(),
        "the old file must be left in place as harmless obsolete data, not deleted"
    );

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn rebuild_and_store_def_index_returns_ok_even_when_the_cache_write_fails() {
    // A disk-cache write failure (a full/locked disk, a permissions problem, or the size
    // guardrail in `cache_v3::encode`) must never discard the freshly built in-memory index --
    // see `rebuild_and_store_def_index_with_progress`'s handling. Forced here by making
    // `def-index` exist as a *file* instead of a directory, so `write_cache`'s `create_dir_all`
    // fails deterministically and portably (no real disk-full/permission trickery needed).
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");
    fs::write(app_dir.join("def-index"), b"not a directory").unwrap();

    let index = rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .expect("a disk-cache write failure must not discard the freshly built in-memory index");

    assert_eq!(index.defs.len(), 1);
    assert_eq!(index.defs[0].def_name, "Steel");

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn unverified_hydration_misses_when_settings_fingerprint_does_not_match() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");

    rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();

    let changed_settings = steel_project_settings(&project_dir, "1.5");
    let cached = load_cached_index_unverified(
        &app_dir,
        &changed_settings,
        &DefIndexBuildOptions::for_project("project"),
        None,
    );
    assert!(cached.is_none());

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn verify_fingerprints_detects_match_and_mismatch() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");

    rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();
    let cached = load_cached_index_unverified(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
        None,
    )
    .unwrap();

    // Match: nothing changed since the fingerprints were captured.
    assert_eq!(
        verify_fingerprints(
            &settings,
            &DefIndexBuildOptions::for_project("project"),
            &cached.file_fingerprints,
        ),
        FingerprintVerification::Match
    );

    // Mismatch: content changed without necessarily changing metadata.
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>SteelChanged</defName></ThingDef></Defs>",
    )
    .unwrap();
    assert_eq!(
        verify_fingerprints(
            &settings,
            &DefIndexBuildOptions::for_project("project"),
            &cached.file_fingerprints,
        ),
        FingerprintVerification::Mismatch
    );

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}

#[test]
fn load_or_rebuild_def_index_bypasses_the_cache_for_force_rebuild_and_replacement() {
    let project_dir = temp_dir();
    let app_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = steel_project_settings(&project_dir, "1.6");

    rebuild_and_store_def_index(
        &app_dir,
        &settings,
        DefIndexBuildOptions::for_project("project"),
    )
    .unwrap();

    // A stored, valid cache exists at this point -- `load_or_rebuild_def_index` would normally
    // hit it. `force_rebuild: true` must still read+parse the collection instead.
    let mut force_options = DefIndexBuildOptions::for_project("project");
    force_options.force_rebuild = true;
    let forced = load_or_rebuild_def_index(&app_dir, &settings, force_options).unwrap();
    assert_eq!(forced.defs.len(), 1);

    // A replacement overlay must also bypass the disk-cache read path.
    let replacement_options = DefIndexBuildOptions {
        project_id: Some("project"),
        include_sources: true,
        replacement: Some(DefIndexReplacement {
            location_id: "project",
            relative_path: "Defs/a.xml",
            source: "<Defs><ThingDef><defName>Overlay</defName></ThingDef></Defs>",
        }),
        force_rebuild: false,
    };
    let overlaid = load_or_rebuild_def_index(&app_dir, &settings, replacement_options).unwrap();
    assert_eq!(overlaid.defs[0].def_name, "Overlay");

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&app_dir).ok();
}
