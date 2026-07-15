use crate::def_index::{
    load_or_rebuild_def_index, rebuild_and_store_def_index, DefIndexBuildOptions,
};
use crate::project_model::{LocationKind, ProjectSettings};
use std::fs;

use super::{location, temp_dir};

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
