//! Covers `scan_def_files_in_load_order` for a `SteamWorkshop` collection root, proving the
//! combined preview load input doesn't cross-shadow same-named Def files from two different
//! Workshop items -- see `rimworld_load_folders`'s `scope` field and
//! `project_files::tests`/`patches::tests::scan`'s equivalent coverage for indexing and patch
//! scanning.

use std::path::Path;

use crate::project_model::{LocationKind, ProjectSettings, SourceType};
use crate::services::patch_preview::scan::scan_def_files_in_load_order;

use super::support::location;

fn settings_with_workshop_collection(collection_root: &Path) -> ProjectSettings {
    let mut loc = location(collection_root, "workshop", LocationKind::Source);
    loc.source_type = SourceType::SteamWorkshop;
    ProjectSettings {
        schema_version: 3,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations: vec![loc],
        active_project_id: None,
    }
}

#[test]
fn duplicate_def_relative_path_across_two_workshop_items_is_not_cross_shadowed() {
    let collection_root =
        std::env::temp_dir().join(format!("rimedit_patch_preview_wk_{}", uuid::Uuid::new_v4()));
    let item_a = collection_root.join("111");
    let item_b = collection_root.join("222");
    std::fs::create_dir_all(item_a.join("Defs")).unwrap();
    std::fs::create_dir_all(item_b.join("Defs")).unwrap();
    std::fs::write(item_a.join("Defs").join("Same.xml"), "<Defs/>").unwrap();
    std::fs::write(item_b.join("Defs").join("Same.xml"), "<Defs/>").unwrap();

    let settings = settings_with_workshop_collection(&collection_root);
    let location = &settings.locations[0];

    let files = scan_def_files_in_load_order(&settings, location);
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();

    assert_eq!(paths, vec!["111/Defs/Same.xml", "222/Defs/Same.xml"]);
    std::fs::remove_dir_all(&collection_root).ok();
}

#[test]
fn intra_item_load_folder_shadowing_is_still_applied() {
    let collection_root =
        std::env::temp_dir().join(format!("rimedit_patch_preview_wk_{}", uuid::Uuid::new_v4()));
    let item = collection_root.join("333");
    std::fs::create_dir_all(item.join("Low").join("Defs")).unwrap();
    std::fs::create_dir_all(item.join("High").join("Defs")).unwrap();
    std::fs::write(
        item.join("LoadFolders.xml"),
        r#"<loadFolders><v1.6><li>Low</li><li>High</li></v1.6></loadFolders>"#,
    )
    .unwrap();
    std::fs::write(item.join("Low").join("Defs").join("Same.xml"), "<Defs/>").unwrap();
    std::fs::write(item.join("High").join("Defs").join("Same.xml"), "<Defs/>").unwrap();

    let settings = settings_with_workshop_collection(&collection_root);
    let location = &settings.locations[0];

    let files = scan_def_files_in_load_order(&settings, location);
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();

    assert_eq!(paths, vec!["333/High/Defs/Same.xml"]);
    std::fs::remove_dir_all(&collection_root).ok();
}
