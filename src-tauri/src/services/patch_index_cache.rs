use crate::patches::{
    build_patch_index_with_files, cache_state_inputs, load_or_rebuild_patch_index,
    rebuild_and_store_patch_index, summarize_patch_index, PatchFile, PatchFilesState, PatchIndex,
    PatchIndexBuildOptions, PatchIndexState, PatchIndexSummary,
};
use crate::project_model::{AppError, ProjectSettings};
use crate::schema_pack::{build_schema_catalog, PatchOperationMetadata};
use crate::services::app_paths;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Build the patch operation metadata catalog used to classify `Custom` operations, discovering
/// schema packs from every registered project/source location root (not just the built-in
/// packs). Mod authors ship a `schema-pack.json` (optionally under `About/` or `SchemaPacks/<name>/`,
/// see `schema_pack::loader::discover_manifest_paths_in_root`) inside their own mod folder, so
/// each registered location's `root_path` is a schema pack root candidate, mirroring how
/// `extraSchemaRoots` works for the schema-catalog UI.
fn custom_operations_for_settings(
    settings: &ProjectSettings,
) -> BTreeMap<String, PatchOperationMetadata> {
    let roots: Vec<PathBuf> = settings
        .locations
        .iter()
        .map(|location| PathBuf::from(&location.root_path))
        .collect();
    build_schema_catalog(&roots, None).catalog.patch_operations
}

pub(crate) fn load_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    force_rebuild: bool,
) -> Result<Arc<PatchIndex>, AppError> {
    let app_data_dir = app_paths::app_storage_dir(app, "patch_index_load_failed")?;
    let state = app.state::<PatchIndexState>();
    let options = PatchIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        force_rebuild,
    };
    let custom_operations = custom_operations_for_settings(settings);

    if !force_rebuild {
        // Patches have no background file-watcher keeping this in sync, so the in-memory
        // shortcut must verify per-file content fingerprints too, not just settings identity --
        // otherwise an edited patch file would silently serve stale results until an explicit
        // rebuild. This still recomputes fingerprints (metadata + content hash reads) rather
        // than a full reparse, so it stays cheaper than a full rebuild when nothing changed.
        if let Some((settings_hash, file_hashes, custom_ops_hash)) =
            cache_state_inputs(settings, &options, &custom_operations)
        {
            if let Some(index) =
                state.get_if_fingerprints_match(&settings_hash, &file_hashes, &custom_ops_hash)
            {
                return Ok(index);
            }
        }
    }

    let index = load_or_rebuild_patch_index(&app_data_dir, settings, options, &custom_operations)?;
    let options = PatchIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        force_rebuild: false,
    };
    if let Some((settings_hash, file_hashes, custom_ops_hash)) =
        cache_state_inputs(settings, &options, &custom_operations)
    {
        return Ok(state.store(settings_hash, file_hashes, custom_ops_hash, index));
    }
    Ok(Arc::new(index))
}

/// Loads the full parsed patch operation ASTs (`Vec<PatchFile>`), reusing the in-memory
/// `PatchFilesState` cache when nothing has changed since it was last built. Unlike
/// [`load_for_project`], this is infallible and has no disk-backed cache of its own -- it exists
/// purely to avoid re-reading and re-parsing every patch file from disk on every call, which
/// previously happened on every `preview_def_patches` invocation (including every checkbox
/// toggle/reorder click inside an already-open preview dialog, not just on open).
pub(crate) fn load_patch_files_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
) -> Arc<Vec<PatchFile>> {
    let state = app.state::<PatchFilesState>();
    let options = PatchIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        force_rebuild: false,
    };
    let custom_operations = custom_operations_for_settings(settings);

    if let Some((settings_hash, file_hashes, custom_ops_hash)) =
        cache_state_inputs(settings, &options, &custom_operations)
    {
        if let Some(files) =
            state.get_if_fingerprints_match(&settings_hash, &file_hashes, &custom_ops_hash)
        {
            return files;
        }
    }

    let (_, patch_files) = build_patch_index_with_files(settings, options, &custom_operations);

    let store_options = PatchIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        force_rebuild: false,
    };
    if let Some((settings_hash, file_hashes, custom_ops_hash)) =
        cache_state_inputs(settings, &store_options, &custom_operations)
    {
        return state.store(settings_hash, file_hashes, custom_ops_hash, patch_files);
    }
    Arc::new(patch_files)
}

pub(crate) fn rebuild_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: Option<&str>,
) -> Result<PatchIndexSummary, AppError> {
    let app_data_dir = app_paths::app_storage_dir(app, "patch_index_rebuild_failed")?;
    let effective_project_id = project_id.or(settings.active_project_id.as_deref());
    let options = PatchIndexBuildOptions {
        project_id: effective_project_id,
        include_sources: true,
        force_rebuild: true,
    };
    let custom_operations = custom_operations_for_settings(settings);
    let index =
        rebuild_and_store_patch_index(&app_data_dir, settings, options, &custom_operations)?;
    let summary = summarize_patch_index(&index);

    let state = app.state::<PatchIndexState>();
    let store_options = PatchIndexBuildOptions {
        project_id: effective_project_id,
        include_sources: true,
        force_rebuild: false,
    };
    if let Some((settings_hash, file_hashes, custom_ops_hash)) =
        cache_state_inputs(settings, &store_options, &custom_operations)
    {
        state.store(settings_hash, file_hashes, custom_ops_hash, index);
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::custom_operations_for_settings;
    use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};
    use time::OffsetDateTime;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rimedit_patch_index_cache_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn mod_location(root: &std::path::Path) -> RegisteredLocation {
        RegisteredLocation {
            id: "mod-a".to_string(),
            display_name: "Mod A".to_string(),
            root_path: root.to_string_lossy().to_string(),
            kind: LocationKind::Source,
            source_type: SourceType::Folder,
            read_only: true,
            mod_id: Some("mod-a".to_string()),
            game_version: None,
            expansion_name: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    /// Reproduces a bug where a mod that ships its own
    /// `schema-pack.json` (with a `patch-operations/` directory) inside its registered location
    /// root wasn't discovered by patch indexing, only by the schema-catalog UI's
    /// `extraSchemaRoots` parameter.
    #[test]
    fn discovers_custom_operation_metadata_from_a_registered_location_root() {
        let root = temp_dir();
        std::fs::write(
            root.join("schema-pack.json"),
            r#"{
                "formatVersion": 2,
                "packId": "test.mod-a",
                "name": "Mod A",
                "version": "1.0.0",
                "defTypeDirectories": ["def-types"],
                "patchOperationDirectories": ["patch-operations"]
            }"#,
        )
        .unwrap();
        std::fs::create_dir(root.join("patch-operations")).unwrap();
        std::fs::write(
            root.join("patch-operations").join("Custom.json"),
            r#"{
                "formatVersion": 1,
                "className": "ModA.PatchOperationCustom",
                "fields": {}
            }"#,
        )
        .unwrap();

        let settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![mod_location(&root)],
            active_project_id: None,
        };

        let custom_operations = custom_operations_for_settings(&settings);
        assert!(
            custom_operations.contains_key("ModA.PatchOperationCustom"),
            "expected custom operation metadata from the mod's own schema-pack.json to be discovered, got: {:?}",
            custom_operations.keys().collect::<Vec<_>>()
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
