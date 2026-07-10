use crate::def_index::{
    cache_state_inputs, load_or_rebuild_def_index, rebuild_and_store_def_index,
    settings_fingerprint, store_prebuilt_index, summarize_index, DefIndex, DefIndexBuildOptions,
    DefIndexState, DefIndexSummary,
};
use crate::project_model::{AppError, ProjectSettings};
use crate::services::app_paths;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub(crate) fn load_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    force_rebuild: bool,
) -> Result<Arc<DefIndex>, AppError> {
    let app_data_dir = app_paths::app_storage_dir(app, "def_index_load_failed")?;
    let state = app.state::<DefIndexState>();
    let options = DefIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        replacement: None,
        force_rebuild,
    };

    if !force_rebuild {
        // Hot path: the indexing service keeps this state current on file changes, so avoid
        // recomputing project-wide file fingerprints before returning the in-memory index.
        let fp = settings_fingerprint(settings, &options);
        if let Some(index) = state.get_if_settings_match(&fp) {
            return Ok(index);
        }
    }

    let index = load_or_rebuild_def_index(&app_data_dir, settings, options)?;
    let options = DefIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    if let Some((settings_hash, file_hashes)) = cache_state_inputs(settings, &options) {
        return Ok(state.store(settings_hash, file_hashes, index));
    }
    Ok(Arc::new(index))
}

pub(crate) fn load_for_project_query(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
) -> Result<Arc<DefIndex>, AppError> {
    let state = app.state::<DefIndexState>();
    let options = DefIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    let settings_fp = settings_fingerprint(settings, &options);
    if let Some(index) = state.get_if_settings_match(&settings_fp) {
        return Ok(index);
    }
    // Settings changed or no index yet: serve whatever is cached rather than blocking on rebuild.
    // Background indexing will populate a fresh index shortly.
    if let Some(stale) = state.get_any_cached() {
        return Ok(stale);
    }
    Ok(Arc::new(DefIndex::default()))
}

/// Writes an incrementally-updated index to both disk and in-memory state with fresh
/// file fingerprints, so subsequent `load_for_project` calls succeed without triggering
/// a full rescan.
pub(crate) fn persist_incremental(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: Option<&str>,
    index: DefIndex,
) {
    let Ok(app_data_dir) = app_paths::app_storage_dir(app, "persist_incremental") else {
        return;
    };
    let state = app.state::<DefIndexState>();
    let options = DefIndexBuildOptions {
        project_id,
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    match store_prebuilt_index(&app_data_dir, settings, options, index.clone()) {
        Ok((settings_fp, file_fps)) => {
            state.store(settings_fp, file_fps, index);
        }
        Err(_) => {
            // Disk write failed; settings-match still works via whatever was stored before
        }
    }
}

pub(crate) fn rebuild_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: Option<&str>,
) -> Result<DefIndexSummary, AppError> {
    let app_data_dir = app_paths::app_storage_dir(app, "def_index_rebuild_failed")?;
    let effective_project_id = project_id.or(settings.active_project_id.as_deref());
    let options = DefIndexBuildOptions {
        project_id: effective_project_id,
        include_sources: true,
        replacement: None,
        force_rebuild: true,
    };
    let index = rebuild_and_store_def_index(&app_data_dir, settings, options)?;
    let summary = summarize_index(&index);
    // Update in-memory state so subsequent query calls return the fresh index.
    let state = app.state::<DefIndexState>();
    let store_options = DefIndexBuildOptions {
        project_id: effective_project_id,
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    if let Some((settings_hash, file_hashes)) = cache_state_inputs(settings, &store_options) {
        state.store(settings_hash, file_hashes, index);
    }
    Ok(summary)
}
