mod builder;
mod cache;
mod query;

use crate::project_model::{LocationKind, RegisteredLocation, SourceType};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

pub(super) fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rimedit_def_index_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

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
