use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::project_files::ProjectFileError;
use crate::project_model::ProjectSettings;

use super::index::{included_locations, CustomOperationMetadataMap, PatchIndexBuildOptions};
use super::scan::scan_indexable_patch_xml_files;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedPatchFileFingerprint {
    pub location_id: String,
    pub relative_path: String,
    pub byte_len: u64,
    pub modified_unix_ms: Option<i64>,
    pub content_hash: String,
}

/// Unlike `def_index::settings_fingerprint`, this does **not** sort location entries before
/// hashing: registered location order changes the stable preview order (see
/// `patches::index`'s module docs), so reordering locations must change the fingerprint even
/// when the location *set* is unchanged. Sorting here would let a stale cached index (with the
/// old `file_order` values) be served after a location reorder.
pub(crate) fn settings_fingerprint(
    settings: &ProjectSettings,
    options: &PatchIndexBuildOptions<'_>,
) -> String {
    let mut entries: Vec<String> = included_locations(settings, options)
        .into_iter()
        .map(|location| {
            format!(
                "{}|{}|{}|{:?}|{:?}|{}|{}|{}",
                location.id,
                location.display_name,
                location.root_path,
                location.kind,
                location.source_type,
                location.read_only,
                location.mod_id.as_deref().unwrap_or(""),
                location.game_version.as_deref().unwrap_or("")
            )
        })
        .collect();
    entries.push(format!("game_version:{}", settings.game_version));
    hash_text(&entries.join("\n"))
}

/// Fingerprint of the resolved custom/built-in patch operation metadata, so a cached index is
/// invalidated when schema-pack contents change classification/preview support even though no
/// project setting or patch file changed. `BTreeMap` and `PatchOperationMetadata`'s own fields
/// serialize in a stable key order, so this is deterministic across runs given the same catalog.
pub(crate) fn custom_operations_fingerprint(
    custom_operations: &CustomOperationMetadataMap,
) -> String {
    let json = serde_json::to_string(custom_operations).unwrap_or_default();
    hash_text(&json)
}

pub(super) fn file_fingerprints(
    settings: &ProjectSettings,
    options: &PatchIndexBuildOptions<'_>,
) -> Result<Vec<IndexedPatchFileFingerprint>, ProjectFileError> {
    let mut fingerprints = Vec::new();
    for location in included_locations(settings, options) {
        let scan = scan_indexable_patch_xml_files(settings, location)?;
        let root = PathBuf::from(scan.root_path);
        for file in scan.files {
            let path = root.join(Path::new(&file.relative_path));
            let metadata = std::fs::metadata(&path)
                .map_err(|e| ProjectFileError::ScanFailed(e.to_string()))?;
            let modified_unix_ms = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);
            let raw =
                std::fs::read(&path).map_err(|e| ProjectFileError::ScanFailed(e.to_string()))?;
            fingerprints.push(IndexedPatchFileFingerprint {
                location_id: location.id.clone(),
                relative_path: file.relative_path,
                byte_len: metadata.len(),
                modified_unix_ms,
                content_hash: hash_bytes(&raw),
            });
        }
    }
    fingerprints.sort_by(|a, b| {
        a.location_id
            .cmp(&b.location_id)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    Ok(fingerprints)
}

fn hash_text(value: &str) -> String {
    hash_bytes(value.as_bytes())
}

fn hash_bytes(value: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value);
    format!("{:x}", hasher.finalize())
}
