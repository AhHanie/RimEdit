use crate::project_model::{AppError, ProjectSettings};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::fingerprint::{
    custom_operations_fingerprint, file_fingerprints, settings_fingerprint,
    IndexedPatchFileFingerprint,
};
use super::index::{
    build_patch_index, CustomOperationMetadataMap, PatchIndex, PatchIndexBuildOptions,
};

// Bumped from 1 to 2 when `customOperationsFingerprint` was added -- a v1 cache file lacks the
// field entirely, so it must not be misread as an empty-string match.
//
// Bumped from 2 to 3 when `impact_graph::infer_xpath_target` learned to resolve OR-chained
// `defName="A" or defName="B"` predicates (previously classified `XPathTarget::Unsupported`). The
// cache key is derived only from settings/file-content/custom-ops fingerprints, none of which
// change when the *classification logic* changes -- so a v2 cache file's stored `PatchIndex`
// would otherwise keep serving stale `Unsupported` targets for every OR-chained operation
// indefinitely, even across a full app restart, until something else invalidated it.
//
// Bumped from 3 to 4 when `XPathTarget`'s struct variants (`Def`/`DefType`/`Defs`) picked up
// their own `#[serde(rename_all = "camelCase")]` (see that enum's doc comment -- the enum-level
// attribute alone never renamed struct-variant fields, only the `kind` tag). A v3 cache file's
// `target` values were serialized with the old, un-renamed field keys; without this bump they'd
// fail to deserialize under the corrected schema on next load and silently look like a plain
// cache miss, which is harmless but this is clearer.
const CACHE_VERSION: u16 = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchIndexCacheFile {
    version: u16,
    settings_fingerprint: String,
    file_fingerprints: Vec<IndexedPatchFileFingerprint>,
    custom_operations_fingerprint: String,
    index: PatchIndex,
}

#[derive(Debug, thiserror::Error)]
pub enum PatchIndexCacheError {
    #[error("Failed to write patch index cache: {0}")]
    WriteFailed(String),
}

impl From<PatchIndexCacheError> for AppError {
    fn from(value: PatchIndexCacheError) -> Self {
        let code = match &value {
            PatchIndexCacheError::WriteFailed(_) => "patch_index_cache_write_failed",
        };
        AppError {
            code: code.to_string(),
            message: value.to_string(),
            details: None,
        }
    }
}

pub fn load_or_rebuild_patch_index(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: PatchIndexBuildOptions<'_>,
    custom_operations: &CustomOperationMetadataMap,
) -> Result<PatchIndex, PatchIndexCacheError> {
    if options.force_rebuild {
        return rebuild_and_store_patch_index(app_data_dir, settings, options, custom_operations);
    }

    let expected_settings = settings_fingerprint(settings, &options);
    let expected_files = file_fingerprints(settings, &options).ok();
    let expected_custom_ops = custom_operations_fingerprint(custom_operations);
    let cache_path = cache_path(app_data_dir);

    if let Some(expected_files) = expected_files {
        if cache_path.exists() {
            if let Ok(raw) = std::fs::read_to_string(&cache_path) {
                if let Ok(cache) = serde_json::from_str::<PatchIndexCacheFile>(&raw) {
                    if cache.version == CACHE_VERSION
                        && cache.settings_fingerprint == expected_settings
                        && cache.file_fingerprints == expected_files
                        && cache.custom_operations_fingerprint == expected_custom_ops
                    {
                        return Ok(cache.index);
                    }
                }
            }
        }
    }

    rebuild_and_store_patch_index(app_data_dir, settings, options, custom_operations)
}

pub fn rebuild_and_store_patch_index(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: PatchIndexBuildOptions<'_>,
    custom_operations: &CustomOperationMetadataMap,
) -> Result<PatchIndex, PatchIndexCacheError> {
    let settings_hash = settings_fingerprint(settings, &options);
    let files = file_fingerprints(settings, &options).unwrap_or_default();
    let custom_ops_hash = custom_operations_fingerprint(custom_operations);
    let index = build_patch_index(settings, options, custom_operations);

    let cache = PatchIndexCacheFile {
        version: CACHE_VERSION,
        settings_fingerprint: settings_hash,
        file_fingerprints: files,
        custom_operations_fingerprint: custom_ops_hash,
        index: index.clone(),
    };
    write_cache(app_data_dir, &cache)?;

    Ok(index)
}

pub fn cache_state_inputs(
    settings: &ProjectSettings,
    options: &PatchIndexBuildOptions<'_>,
    custom_operations: &CustomOperationMetadataMap,
) -> Option<(String, Vec<IndexedPatchFileFingerprint>, String)> {
    Some((
        settings_fingerprint(settings, options),
        file_fingerprints(settings, options).ok()?,
        custom_operations_fingerprint(custom_operations),
    ))
}

fn write_cache(
    app_data_dir: &Path,
    cache: &PatchIndexCacheFile,
) -> Result<(), PatchIndexCacheError> {
    let path = cache_path(app_data_dir);
    let parent = path.parent().ok_or_else(|| {
        PatchIndexCacheError::WriteFailed("cache path has no parent directory".to_string())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|e| PatchIndexCacheError::WriteFailed(e.to_string()))?;
    let json = serde_json::to_string_pretty(cache)
        .map_err(|e| PatchIndexCacheError::WriteFailed(e.to_string()))?;
    let tmp = parent.join("patch-index-cache-v1.tmp");
    std::fs::write(&tmp, json).map_err(|e| PatchIndexCacheError::WriteFailed(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| PatchIndexCacheError::WriteFailed(e.to_string()))?;
    Ok(())
}

fn cache_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("patch-index").join("index-cache-v1.json")
}
