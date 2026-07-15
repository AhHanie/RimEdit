use crate::project_model::{AppError, ProjectSettings};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::builder::{build_def_index, DefIndexBuildOptions};
use super::fingerprint::{file_fingerprints, settings_fingerprint, IndexedFileFingerprint};
use super::model::DefIndex;

const CACHE_VERSION: u16 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DefIndexCacheFile {
    version: u16,
    settings_fingerprint: String,
    file_fingerprints: Vec<IndexedFileFingerprint>,
    index: DefIndex,
}

#[derive(Debug, thiserror::Error)]
pub enum DefIndexCacheError {
    #[error("Failed to write def index cache: {0}")]
    WriteFailed(String),
}

impl From<DefIndexCacheError> for AppError {
    fn from(value: DefIndexCacheError) -> Self {
        let code = match &value {
            DefIndexCacheError::WriteFailed(_) => "def_index_cache_write_failed",
        };
        AppError {
            code: code.to_string(),
            message: value.to_string(),
            details: None,
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }
}

pub fn load_or_rebuild_def_index(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: DefIndexBuildOptions<'_>,
) -> Result<DefIndex, DefIndexCacheError> {
    if options.replacement.is_some() || options.force_rebuild {
        return rebuild_and_store_def_index(app_data_dir, settings, options);
    }

    let expected_settings = settings_fingerprint(settings, &options);
    let expected_files = file_fingerprints(settings, &options).ok();
    let cache_path = cache_path(app_data_dir);

    if let Some(expected_files) = expected_files {
        if cache_path.exists() {
            if let Ok(raw) = std::fs::read_to_string(&cache_path) {
                if let Ok(cache) = serde_json::from_str::<DefIndexCacheFile>(&raw) {
                    if cache.version == CACHE_VERSION
                        && cache.settings_fingerprint == expected_settings
                        && cache.file_fingerprints == expected_files
                    {
                        let mut index = cache.index;
                        index.rebuild_computed_fields();
                        return Ok(index);
                    }
                }
            }
        }
    }

    rebuild_and_store_def_index(app_data_dir, settings, options)
}

pub fn rebuild_and_store_def_index(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: DefIndexBuildOptions<'_>,
) -> Result<DefIndex, DefIndexCacheError> {
    let should_store = options.replacement.is_none();
    let settings_hash = settings_fingerprint(settings, &options);
    let files = file_fingerprints(settings, &options).unwrap_or_default();
    let index = build_def_index(settings, options);

    if should_store {
        let cache = DefIndexCacheFile {
            version: CACHE_VERSION,
            settings_fingerprint: settings_hash,
            file_fingerprints: files,
            index: index.clone(),
        };
        write_cache(app_data_dir, &cache)?;
    }

    Ok(index)
}

/// Writes a pre-built index to the disk cache with freshly computed file fingerprints
/// and returns the (settings_fingerprint, file_fingerprints) so the caller can update
/// the in-memory state with `DefIndexState::store`.
pub fn store_prebuilt_index(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: DefIndexBuildOptions<'_>,
    index: DefIndex,
) -> Result<(String, Vec<IndexedFileFingerprint>), DefIndexCacheError> {
    let settings_fp = settings_fingerprint(settings, &options);
    let file_fps = file_fingerprints(settings, &options).unwrap_or_default();
    let cache = DefIndexCacheFile {
        version: CACHE_VERSION,
        settings_fingerprint: settings_fp.clone(),
        file_fingerprints: file_fps.clone(),
        index,
    };
    write_cache(app_data_dir, &cache)?;
    Ok((settings_fp, file_fps))
}

pub fn cache_state_inputs(
    settings: &ProjectSettings,
    options: &DefIndexBuildOptions<'_>,
) -> Option<(String, Vec<IndexedFileFingerprint>)> {
    if options.replacement.is_some() {
        return None;
    }
    Some((
        settings_fingerprint(settings, options),
        file_fingerprints(settings, options).ok()?,
    ))
}

fn write_cache(app_data_dir: &Path, cache: &DefIndexCacheFile) -> Result<(), DefIndexCacheError> {
    let path = cache_path(app_data_dir);
    let parent = path.parent().ok_or_else(|| {
        DefIndexCacheError::WriteFailed("cache path has no parent directory".to_string())
    })?;
    std::fs::create_dir_all(parent).map_err(|e| DefIndexCacheError::WriteFailed(e.to_string()))?;
    let json = serde_json::to_string_pretty(cache)
        .map_err(|e| DefIndexCacheError::WriteFailed(e.to_string()))?;
    let tmp = parent.join("index-cache-v1.tmp");
    std::fs::write(&tmp, json).map_err(|e| DefIndexCacheError::WriteFailed(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| DefIndexCacheError::WriteFailed(e.to_string()))?;
    Ok(())
}

fn cache_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("def-index").join("index-cache-v1.json")
}
