use std::path::{Path, PathBuf};

use time::OffsetDateTime;

use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};

pub(super) fn location(root: &Path, id: &str, kind: LocationKind) -> RegisteredLocation {
    RegisteredLocation {
        id: id.to_string(),
        display_name: id.to_string(),
        root_path: root.to_string_lossy().to_string(),
        kind: kind.clone(),
        source_type: SourceType::Folder,
        read_only: kind == LocationKind::Source,
        mod_id: Some(format!("mod-{}", id)),
        game_version: None,
        expansion_name: None,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

pub(super) fn temp_project_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rimedit_patch_preview_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("Defs")).unwrap();
    std::fs::create_dir_all(dir.join("Patches")).unwrap();
    dir
}

pub(super) fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

pub(super) fn settings_for(root: &Path) -> ProjectSettings {
    ProjectSettings {
        schema_version: 2,
        game_version: "1.6".to_string(),
        locations: vec![location(root, "proj", LocationKind::Project)],
        active_project_id: Some("proj".to_string()),
    }
}
