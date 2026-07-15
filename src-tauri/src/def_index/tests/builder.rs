use crate::def_index::{
    apply_replacement_overlay, build_def_index, settings_fingerprint, DefIdentityKey, DefIndex,
    DefIndexBuildOptions, DefIndexReplacement, IndexedDef, IndexedSourceKind,
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
