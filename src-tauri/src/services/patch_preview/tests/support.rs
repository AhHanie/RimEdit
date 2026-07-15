use std::path::{Path, PathBuf};

use time::OffsetDateTime;

use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};
use crate::services::patch_preview::PatchPreviewTarget;

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
        schema_version: 3,
        game_version: "1.6".to_string(),
        locale: "en".to_string(),
        locations: vec![location(root, "proj", LocationKind::Project)],
        active_project_id: Some("proj".to_string()),
    }
}

/// Builds a [`PatchPreviewTarget`] for the common case in this test suite: a Def declared in the
/// project location's `Defs/Things.xml`, identified by its zero-based position among that file's
/// own top-level Defs (`ordinal`).
pub(super) fn target(def_type: &str, identity: &str, ordinal: usize) -> PatchPreviewTarget {
    target_at("proj", "Defs/Things.xml", def_type, identity, ordinal)
}

/// Builds a [`PatchPreviewTarget`] naming an exact file origin -- used by tests proving target
/// resolution is independent of which location/file happens to share a Def's identity.
pub(super) fn target_at(
    location_id: &str,
    relative_path: &str,
    def_type: &str,
    identity: &str,
    ordinal: usize,
) -> PatchPreviewTarget {
    PatchPreviewTarget {
        location_id: location_id.to_string(),
        relative_path: relative_path.to_string(),
        def_type: def_type.to_string(),
        identity: identity.to_string(),
        ordinal,
    }
}
