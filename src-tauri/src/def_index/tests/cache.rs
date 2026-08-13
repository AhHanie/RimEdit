use crate::def_index::{
    load_cached_index_only, load_or_rebuild_def_index, rebuild_and_store_def_index,
    DefIndexBuildOptions, DefIndexReplacement,
};
use crate::project_model::{LocationKind, ProjectSettings};
use std::fs;

use super::{location, temp_dir};

fn steel_project_settings(project_dir: &std::path::Path, game_version: &str) -> ProjectSettings {
    ProjectSettings {
        schema_version: 3,
        game_version: game_version.to_string(),
        locale: "en".to_string(),
        locations: vec![location(project_dir, "project", LocationKind::Project)],
        active_project_id: Some("project".to_string()),
    }
}

fn cache_file_path(app_dir: &std::path::Path) -> std::path::PathBuf {
    app_dir.join("def-index").join("index-cache-v1.json")
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
        schema_version: 3,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations: vec![location(&project_dir, "project", LocationKind::Project)],
        active_project_id: Some("project".to_string()),
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
    fs::write(&cache_path, "{ not valid json").unwrap();
    assert!(load_cached_index_only(
        &app_dir,
        &settings,
        &DefIndexBuildOptions::for_project("project"),
    )
    .is_none());

    fs::write(
        &cache_path,
        r#"{"version":999999,"settingsFingerprint":"x","fileFingerprints":[],"index":{"defs":[],"errors":[],"builtAtUnixMs":0}}"#,
    )
    .unwrap();
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
