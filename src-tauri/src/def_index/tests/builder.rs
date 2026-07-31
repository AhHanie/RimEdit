use crate::def_index::{
    apply_replacement_overlay, build_def_index, build_def_index_with_progress,
    discover_scan_stats, settings_fingerprint, DefIdentityKey, DefIndex, DefIndexBuildOptions,
    DefIndexReplacement, IndexedDef, IndexedSourceKind,
};
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};
use std::fs;
use time::OffsetDateTime;

use super::super::builder::indexed_source_for_location;
use super::{location, temp_dir};

fn settings_with_project(
    project_dir: &std::path::Path,
    source_dir: Option<&std::path::Path>,
) -> ProjectSettings {
    let mut locations = vec![location(project_dir, "project", LocationKind::Project)];
    if let Some(src) = source_dir {
        locations.push(location(src, "source", LocationKind::Source));
    }
    ProjectSettings {
        schema_version: 3,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations,
        active_project_id: Some("project".to_string()),
    }
}

fn settings_with_base_game_source(
    project_dir: &std::path::Path,
    data_dir: &std::path::Path,
) -> ProjectSettings {
    let mut base_game = location(data_dir, "base", LocationKind::Source);
    base_game.display_name = "RimWorld Data".to_string();
    base_game.source_type = SourceType::BaseGame;
    base_game.mod_id = None;
    ProjectSettings {
        schema_version: 3,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations: vec![
            location(project_dir, "project", LocationKind::Project),
            base_game,
        ],
        active_project_id: Some("project".to_string()),
    }
}

fn settings_with_steam_workshop_source(
    project_dir: &std::path::Path,
    collection_dir: &std::path::Path,
) -> ProjectSettings {
    let mut workshop = location(collection_dir, "workshop", LocationKind::Source);
    workshop.display_name = "Steam Workshop".to_string();
    workshop.source_type = SourceType::SteamWorkshop;
    workshop.mod_id = None;
    ProjectSettings {
        schema_version: 3,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations: vec![
            location(project_dir, "project", LocationKind::Project),
            workshop,
        ],
        active_project_id: Some("project".to_string()),
    }
}

#[test]
fn indexes_project_and_source_defs_separately() {
    let project_dir = temp_dir();
    let source_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::create_dir(source_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>Steel</defName><label>steel</label></ThingDef></Defs>",
    )
    .unwrap();
    fs::write(
        source_dir.join("Defs").join("b.xml"),
        "<Defs><ThingDef ParentName=\"Base\"><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = settings_with_project(&project_dir, Some(&source_dir));

    let index = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));

    assert_eq!(index.defs.len(), 2, "errors: {:?}", index.errors);
    assert_eq!(index.find_project_duplicates("ThingDef", "Steel").len(), 1);
    assert_eq!(index.find_all_duplicates("ThingDef", "Steel").len(), 2);
    assert_eq!(index.defs[0].fields[0].name, "defName");
    assert!(index.defs.iter().all(|d| d.node_id.is_some()));
    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&source_dir).ok();
}

#[test]
fn indexes_core_defs_when_base_game_packs_share_relative_paths() {
    let project_dir = temp_dir();
    let data_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    let core_items = data_dir.join("Core").join("Defs").join("ThingDefs_Items");
    let biotech_items = data_dir
        .join("Biotech")
        .join("Defs")
        .join("ThingDefs_Items");
    let core_buildings = data_dir
        .join("Core")
        .join("Defs")
        .join("ThingDefs_Buildings");
    let anomaly_buildings = data_dir
        .join("Anomaly")
        .join("Defs")
        .join("ThingDefs_Buildings");
    fs::create_dir_all(&core_items).unwrap();
    fs::create_dir_all(&biotech_items).unwrap();
    fs::create_dir_all(&core_buildings).unwrap();
    fs::create_dir_all(&anomaly_buildings).unwrap();
    fs::write(
        core_items.join("Items_Unfinished.xml"),
        "<Defs><ThingDef><defName>UnfinishedTechArmor</defName></ThingDef></Defs>",
    )
    .unwrap();
    fs::write(
        biotech_items.join("Items_Unfinished.xml"),
        "<Defs><ThingDef><defName>BiotechUnfinished</defName></ThingDef></Defs>",
    )
    .unwrap();
    fs::write(
        core_buildings.join("Buildings_Production.xml"),
        "<Defs><ThingDef><defName>FabricationBench</defName></ThingDef></Defs>",
    )
    .unwrap();
    fs::write(
        anomaly_buildings.join("Buildings_Production.xml"),
        "<Defs><ThingDef><defName>AnomalyProduction</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = settings_with_base_game_source(&project_dir, &data_dir);

    let index = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));

    assert_eq!(
        index.find_by_key("ThingDef", "UnfinishedTechArmor").len(),
        1
    );
    assert_eq!(index.find_by_key("ThingDef", "FabricationBench").len(), 1);
    assert_eq!(
        index.find_by_key("ThingDef", "UnfinishedTechArmor")[0].relative_path,
        "Core/Defs/ThingDefs_Items/Items_Unfinished.xml"
    );
    assert_eq!(
        index.find_by_key("ThingDef", "FabricationBench")[0].relative_path,
        "Core/Defs/ThingDefs_Buildings/Buildings_Production.xml"
    );
    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&data_dir).ok();
}

#[test]
fn indexes_project_and_steam_workshop_collection_defs() {
    let project_dir = temp_dir();
    let collection_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>ProjectThing</defName></ThingDef></Defs>",
    )
    .unwrap();

    let item_a = collection_dir.join("111");
    fs::create_dir_all(item_a.join("Defs")).unwrap();
    fs::write(
        item_a.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>WorkshopItemAThing</defName></ThingDef></Defs>",
    )
    .unwrap();

    let item_b = collection_dir.join("222");
    fs::create_dir_all(item_b.join("1.6").join("Defs")).unwrap();
    fs::write(
        item_b.join("LoadFolders.xml"),
        r#"<loadFolders><v1.6><li>1.6</li></v1.6></loadFolders>"#,
    )
    .unwrap();
    fs::write(
        item_b.join("1.6").join("Defs").join("b.xml"),
        "<Defs><ThingDef><defName>WorkshopItemBThing</defName></ThingDef></Defs>",
    )
    .unwrap();

    let settings = settings_with_steam_workshop_source(&project_dir, &collection_dir);
    let index = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));

    assert_eq!(index.defs.len(), 3, "errors: {:?}", index.errors);
    assert_eq!(index.find_by_key("ThingDef", "ProjectThing").len(), 1);
    assert_eq!(index.find_by_key("ThingDef", "WorkshopItemAThing").len(), 1);
    assert_eq!(index.find_by_key("ThingDef", "WorkshopItemBThing").len(), 1);

    let item_a_def = &index.find_by_key("ThingDef", "WorkshopItemAThing")[0];
    assert_eq!(item_a_def.relative_path, "111/Defs/a.xml");
    assert_eq!(item_a_def.source.location_id, "workshop");
    assert_eq!(item_a_def.source.source_type, SourceType::SteamWorkshop);

    let item_b_def = &index.find_by_key("ThingDef", "WorkshopItemBThing")[0];
    assert_eq!(item_b_def.relative_path, "222/1.6/Defs/b.xml");
    assert_eq!(item_b_def.source.location_id, "workshop");

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&collection_dir).ok();
}

#[test]
fn discover_scan_stats_counts_locations_workshop_items_and_files_without_parsing() {
    let project_dir = temp_dir();
    let collection_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>ProjectThing</defName></ThingDef></Defs>",
    )
    .unwrap();

    let item_a = collection_dir.join("111");
    fs::create_dir_all(item_a.join("Defs")).unwrap();
    fs::write(item_a.join("Defs").join("a.xml"), "<Defs><ThingDef><defName>A</defName></ThingDef></Defs>").unwrap();

    let item_b = collection_dir.join("222");
    fs::create_dir_all(item_b.join("1.6").join("Defs")).unwrap();
    fs::write(
        item_b.join("LoadFolders.xml"),
        r#"<loadFolders><v1.6><li>1.6</li></v1.6></loadFolders>"#,
    )
    .unwrap();
    fs::write(item_b.join("1.6").join("Defs").join("b.xml"), "<Defs><ThingDef><defName>B</defName></ThingDef></Defs>").unwrap();

    let settings = settings_with_steam_workshop_source(&project_dir, &collection_dir);
    let options = DefIndexBuildOptions::for_project("project");

    let stats = discover_scan_stats(&settings, &options);

    assert_eq!(stats.included_locations, 2, "project + workshop source");
    assert_eq!(stats.resolved_workshop_item_count, 2);
    // 1 project location's own selected folder + item_a's 1 + item_b's 1.
    assert_eq!(stats.selected_load_folder_count, 3);
    assert_eq!(stats.discovered_files, 3, "ProjectThing + A + B");

    // Discovery is parse-free: it must not itself index any Defs (that's `build_def_index`'s job).
    let built = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));
    assert_eq!(built.defs.len(), 3, "errors: {:?}", built.errors);

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&collection_dir).ok();
}

#[test]
fn discover_scan_stats_excludes_sources_when_include_sources_is_false() {
    let project_dir = temp_dir();
    let source_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::create_dir(source_dir.join("Defs")).unwrap();
    fs::write(source_dir.join("Defs").join("a.xml"), "<Defs/>").unwrap();

    let settings = settings_with_project(&project_dir, Some(&source_dir));
    let options = DefIndexBuildOptions {
        project_id: Some("project"),
        include_sources: false,
        replacement: None,
        force_rebuild: false,
    };

    let stats = discover_scan_stats(&settings, &options);
    assert_eq!(stats.included_locations, 1, "only the project location");

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&source_dir).ok();
}

#[test]
fn build_def_index_with_progress_invokes_the_callback_once_per_discovered_file() {
    let project_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>A</defName></ThingDef></Defs>",
    )
    .unwrap();
    fs::write(
        project_dir.join("Defs").join("b.xml"),
        "<Defs><ThingDef><defName>B</defName></ThingDef></Defs>",
    )
    .unwrap();
    // A file that fails to read (not valid UTF-8) still counts as "attempted" for progress
    // purposes -- the callback fires unconditionally before the read is even tried.
    fs::write(project_dir.join("Defs").join("c.xml"), [0xFF, 0xFE, 0x00]).unwrap();

    let settings = settings_with_project(&project_dir, None);
    let calls = std::cell::Cell::new(0usize);
    let on_file_indexed = || calls.set(calls.get() + 1);

    let index = build_def_index_with_progress(
        &settings,
        DefIndexBuildOptions::for_project("project"),
        Some(&on_file_indexed),
    );

    assert_eq!(calls.get(), 3, "expected one callback invocation per discovered file");
    assert_eq!(index.defs.len(), 2);
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn build_def_index_with_progress_none_behaves_identically_to_build_def_index() {
    let project_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>A</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = settings_with_project(&project_dir, None);

    let via_plain = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));
    let via_progress = build_def_index_with_progress(
        &settings,
        DefIndexBuildOptions::for_project("project"),
        None,
    );

    assert_eq!(via_plain.defs.len(), via_progress.defs.len());
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn malformed_workshop_item_load_folders_xml_is_isolated_and_does_not_block_other_items() {
    let project_dir = temp_dir();
    let collection_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>ProjectThing</defName></ThingDef></Defs>",
    )
    .unwrap();

    // A valid Workshop item with no LoadFolders.xml (conventional fallback).
    let item_valid = collection_dir.join("111");
    fs::create_dir_all(item_valid.join("Defs")).unwrap();
    fs::write(
        item_valid.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>ValidItemThing</defName></ThingDef></Defs>",
    )
    .unwrap();

    // A malformed Workshop item: `LoadFolders.xml` exists but cannot be read as a file (it's a
    // directory here, standing in for a permissions/encoding failure). Its own Defs are
    // unreachable (the resolver could not determine which folders to load), but it must not
    // prevent the other, valid item from being indexed.
    let item_broken = collection_dir.join("222");
    fs::create_dir_all(item_broken.join("Defs")).unwrap();
    fs::create_dir(item_broken.join("LoadFolders.xml")).unwrap();
    fs::write(
        item_broken.join("Defs").join("b.xml"),
        "<Defs><ThingDef><defName>BrokenItemThing</defName></ThingDef></Defs>",
    )
    .unwrap();

    let settings = settings_with_steam_workshop_source(&project_dir, &collection_dir);
    let index = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));

    assert_eq!(
        index.find_by_key("ThingDef", "ProjectThing").len(),
        1,
        "errors: {:?}",
        index.errors
    );
    assert_eq!(index.find_by_key("ThingDef", "ValidItemThing").len(), 1);
    assert!(index.find_by_key("ThingDef", "BrokenItemThing").is_empty());

    let load_folders_error = index
        .errors
        .iter()
        .find(|e| e.code == "load_folders_read_failed")
        .unwrap_or_else(|| panic!("expected a load_folders_read_failed error: {:?}", index.errors));
    assert_eq!(load_folders_error.source_kind, IndexedSourceKind::Source);
    assert_eq!(load_folders_error.location_id, "workshop");
    assert_eq!(
        load_folders_error.relative_path.as_deref(),
        Some("222/LoadFolders.xml")
    );

    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&collection_dir).ok();
}

#[test]
fn replacement_overlay_excludes_old_file() {
    let project_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>OldName</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = settings_with_project(&project_dir, None);
    let index = build_def_index(
        &settings,
        DefIndexBuildOptions {
            project_id: Some("project"),
            include_sources: true,
            replacement: Some(DefIndexReplacement {
                location_id: "project",
                relative_path: "Defs/a.xml",
                source: "<Defs><ThingDef><defName>NewName</defName></ThingDef></Defs>",
            }),
            force_rebuild: false,
        },
    );

    assert!(index.find_by_key("ThingDef", "OldName").is_empty());
    assert_eq!(index.find_by_key("ThingDef", "NewName").len(), 1);
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn replacement_overlay_does_not_mutate_base_index() {
    let project_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("a.xml"),
        "<Defs><ThingDef><defName>OldName</defName></ThingDef></Defs>",
    )
    .unwrap();
    let settings = settings_with_project(&project_dir, None);
    let base = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));
    let overlay = apply_replacement_overlay(
        base.clone(),
        &settings,
        DefIndexReplacement {
            location_id: "project",
            relative_path: "Defs/a.xml",
            source: "<Defs><ThingDef><defName>NewName</defName></ThingDef></Defs>",
        },
    );

    assert_eq!(base.find_by_key("ThingDef", "OldName").len(), 1);
    assert!(base.find_by_key("ThingDef", "NewName").is_empty());
    assert!(overlay.find_by_key("ThingDef", "OldName").is_empty());
    assert_eq!(overlay.find_by_key("ThingDef", "NewName").len(), 1);
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn settings_fingerprint_changes_when_location_display_name_changes() {
    let project_dir = temp_dir();
    let mut settings = ProjectSettings {
        schema_version: 3,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations: vec![location(&project_dir, "project", LocationKind::Project)],
        active_project_id: Some("project".to_string()),
    };
    let options = DefIndexBuildOptions::for_project("project");
    let first = settings_fingerprint(&settings, &options);

    settings.locations[0].display_name = "Renamed Project".to_string();
    let second = settings_fingerprint(&settings, &options);

    assert_ne!(first, second);
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn settings_fingerprint_changes_when_game_version_changes() {
    let project_dir = temp_dir();
    let mut settings = ProjectSettings {
        schema_version: 3,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations: vec![location(&project_dir, "project", LocationKind::Project)],
        active_project_id: Some("project".to_string()),
    };
    let options = DefIndexBuildOptions::for_project("project");
    let first = settings_fingerprint(&settings, &options);

    settings.game_version = "1.5".to_string();
    let second = settings_fingerprint(&settings, &options);

    assert_ne!(first, second);
    fs::remove_dir_all(&project_dir).ok();
}

#[test]
fn read_only_project_records_do_not_count_as_project_duplicates() {
    let mut index = DefIndex::default();
    let mut source = indexed_source_for_location(&RegisteredLocation {
        id: "project".to_string(),
        display_name: "Project".to_string(),
        root_path: "C:\\fake".to_string(),
        kind: LocationKind::Project,
        source_type: SourceType::Folder,
        read_only: true,
        mod_id: None,
        game_version: None,
        expansion_name: None,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    });
    source.read_only = true;
    index.defs.push(IndexedDef {
        key: DefIdentityKey {
            def_type: "ThingDef".to_string(),
            def_name: "Steel".to_string(),
        },
        def_type: "ThingDef".to_string(),
        def_name: "Steel".to_string(),
        label: None,
        parent_name: None,
        relative_path: "a.xml".to_string(),
        node_id: None,
        line: None,
        column: None,
        source,
        fields: vec![],
        def_name_lower: String::new(),
        label_lower: String::new(),
    });

    assert!(index
        .find_project_duplicates("ThingDef", "Steel")
        .is_empty());
}

#[test]
fn find_source_duplicates_returns_source_defs_for_matching_key() {
    let mut index = DefIndex::default();
    let source = indexed_source_for_location(&RegisteredLocation {
        id: "core".to_string(),
        display_name: "RimWorld Core".to_string(),
        root_path: "C:\\fake\\core".to_string(),
        kind: LocationKind::Source,
        source_type: SourceType::Folder,
        read_only: true,
        mod_id: None,
        game_version: None,
        expansion_name: None,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    });
    index.defs.push(IndexedDef {
        key: DefIdentityKey {
            def_type: "ThingDef".to_string(),
            def_name: "Steel".to_string(),
        },
        def_type: "ThingDef".to_string(),
        def_name: "Steel".to_string(),
        label: None,
        parent_name: None,
        relative_path: "Defs/Items.xml".to_string(),
        node_id: None,
        line: None,
        column: None,
        source,
        fields: vec![],
        def_name_lower: String::new(),
        label_lower: String::new(),
    });

    let results = index.find_source_duplicates("ThingDef", "Steel");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source.source_kind, IndexedSourceKind::Source);
}

#[test]
fn find_source_duplicates_returns_empty_for_different_def_type() {
    let mut index = DefIndex::default();
    let source = indexed_source_for_location(&RegisteredLocation {
        id: "core".to_string(),
        display_name: "RimWorld Core".to_string(),
        root_path: "C:\\fake\\core".to_string(),
        kind: LocationKind::Source,
        source_type: SourceType::Folder,
        read_only: true,
        mod_id: None,
        game_version: None,
        expansion_name: None,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    });
    index.defs.push(IndexedDef {
        key: DefIdentityKey {
            def_type: "ThingDef".to_string(),
            def_name: "Steel".to_string(),
        },
        def_type: "ThingDef".to_string(),
        def_name: "Steel".to_string(),
        label: None,
        parent_name: None,
        relative_path: "Defs/Items.xml".to_string(),
        node_id: None,
        line: None,
        column: None,
        source,
        fields: vec![],
        def_name_lower: String::new(),
        label_lower: String::new(),
    });

    assert!(index
        .find_source_duplicates("RecipeDef", "Steel")
        .is_empty());
}

#[test]
fn invalid_xml_records_error_and_indexes_valid_files() {
    let project_dir = temp_dir();
    fs::create_dir(project_dir.join("Defs")).unwrap();
    fs::write(
        project_dir.join("Defs").join("good.xml"),
        "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
    )
    .unwrap();
    fs::write(
        project_dir.join("Defs").join("bad.xml"),
        // Unclosed elements at EOF trigger `parse_unexpected_eof`, which (unlike a raw
        // quick-xml syntax error) carries a typed `unclosedCount` arg -- see
        // `xml_document::parser` -- so this fixture also exercises `args` propagation below.
        "<Defs><ThingDef><defName>Bad</defName>",
    )
    .unwrap();
    let settings = settings_with_project(&project_dir, None);

    let index = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));

    assert_eq!(index.defs.len(), 1);
    assert!(!index.errors.is_empty());
    // The underlying `ParseDiagnostic`'s typed args (see `xml_document::diagnostics`) are
    // propagated onto `DefIndexError.args`, not just its English `message`.
    assert!(
        index
            .errors
            .iter()
            .any(|e| e.args.get("unclosedCount").is_some()),
        "expected the underlying ParseDiagnostic's typed args to propagate onto DefIndexError: {:?}",
        index.errors
    );
    fs::remove_dir_all(&project_dir).ok();
}
