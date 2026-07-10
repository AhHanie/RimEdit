use crate::project_model::ProjectSettings;

use super::builder::{add_document_defs, indexed_source_for_location, normalize_relative_path};
use super::model::{DefIndex, DefIndexError, DefIndexReplacement, IndexedSourceKind};

pub fn apply_replacement_overlay(
    mut index: DefIndex,
    settings: &ProjectSettings,
    replacement: DefIndexReplacement<'_>,
) -> DefIndex {
    let normalized_path = normalize_relative_path(replacement.relative_path);
    index.defs.retain(|def| {
        def.source.location_id != replacement.location_id || def.relative_path != normalized_path
    });
    index.errors.retain(|error| {
        error.location_id != replacement.location_id
            || error.relative_path.as_deref() != Some(normalized_path.as_str())
    });

    let Some(location) = settings
        .locations
        .iter()
        .find(|location| location.id == replacement.location_id)
    else {
        index.errors.push(DefIndexError {
            location_id: replacement.location_id.to_string(),
            location_name: replacement.location_id.to_string(),
            source_kind: IndexedSourceKind::Project,
            relative_path: Some(normalized_path),
            code: "def_index_overlay_location_missing".to_string(),
            message: format!("Location not found: {}", replacement.location_id),
            line: None,
            column: None,
        });
        return index;
    };

    let source = indexed_source_for_location(location);
    add_document_defs(&mut index, &normalized_path, replacement.source, &source);
    index.rebuild_computed_fields();
    index
}
