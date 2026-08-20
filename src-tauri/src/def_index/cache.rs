use crate::project_model::{AppError, ProjectSettings};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

use super::builder::{build_def_index, build_def_index_with_progress, DefIndexBuildOptions};
use super::cache_v3::{self, CacheCodecError, CACHE_VERSION_V3};
use super::fingerprint::{file_fingerprints, settings_fingerprint, IndexedFileFingerprint};
use super::model::DefIndex;

const CACHE_VERSION: u16 = CACHE_VERSION_V3;

/// The hydrated, logical shape of a disk cache -- distinct from `cache_v3::DefIndexCacheDtoV3`,
/// which is the deduplicated *physical* on-disk encoding. `read_cache_file`/`write_cache`
/// translate between the two via `cache_v3::from_dto`/`to_dto`.
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
    #[error("Def index cache exceeds the {limit}-byte safety limit ({actual} bytes)")]
    TooLarge { actual: usize, limit: usize },
    #[error("Def index cache exceeds the {limit}-record safety limit ({actual} records)")]
    TooManyRecords { actual: usize, limit: usize },
}

impl From<CacheCodecError> for DefIndexCacheError {
    fn from(value: CacheCodecError) -> Self {
        match value {
            CacheCodecError::Encode(msg) => DefIndexCacheError::WriteFailed(msg),
            CacheCodecError::TooLarge { actual, limit } => {
                DefIndexCacheError::TooLarge { actual, limit }
            }
            CacheCodecError::TooManyRecords { actual, limit } => {
                DefIndexCacheError::TooManyRecords { actual, limit }
            }
        }
    }
}

impl From<DefIndexCacheError> for AppError {
    fn from(value: DefIndexCacheError) -> Self {
        let code = match &value {
            DefIndexCacheError::WriteFailed(_) => "def_index_cache_write_failed",
            DefIndexCacheError::TooLarge { .. } => "def_index_cache_too_large",
            DefIndexCacheError::TooManyRecords { .. } => "def_index_cache_too_large",
        };
        AppError {
            code: code.to_string(),
            message: value.to_string(),
            details: None,
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }
}

/// A cache hit from [`load_cached_index_only`]: the deserialized index plus the fingerprints
/// that were computed to validate it, so callers can populate `DefIndexState` without
/// recomputing the (expensive) project-wide file-fingerprint scan.
pub struct CachedDefIndex {
    pub index: DefIndex,
    pub settings_fingerprint: String,
    pub file_fingerprints: Vec<IndexedFileFingerprint>,
}

/// Reads a valid disk-cached def index checking only the settings fingerprint -- never reads or
/// hashes any indexed Def XML file. Returns `None` for any benign miss -- absent cache file,
/// unreadable/malformed JSON, unsupported cache version, or a settings-fingerprint mismatch --
/// never as an error. Computed fields are rebuilt on the deserialized index before it is
/// returned, exactly like the strict reader.
///
/// Ignores `options.replacement` / `options.force_rebuild`, like [`load_cached_index_only`].
///
/// This is the "trusted cache" read used at startup: the returned `file_fingerprints` are
/// whatever was persisted alongside the index and have *not* been verified against the current
/// collection -- callers that need that guarantee must run [`verify_fingerprints`] (typically in
/// the background, off the caller's critical path).
///
/// `app` is used only for dev-only instrumentation (see `docs/Instrumentation.md`) around this
/// startup hydration path -- pass `None` from tests and any other caller with no `AppHandle` on
/// hand; instrumentation is simply skipped in that case.
pub fn load_cached_index_unverified(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: &DefIndexBuildOptions<'_>,
    app: Option<&AppHandle>,
) -> Option<CachedDefIndex> {
    let (mut cache, expected_settings) = read_cache_file(app_data_dir, settings, options, app)?;
    {
        let mut span =
            app.map(|app| crate::instrumentation::span(app, "defIndex.rebuildComputedFields"));
        cache.index.rebuild_computed_fields();
        if let Some(span) = span.as_mut() {
            span.set_tag("defCount", cache.index.defs.len().to_string());
            span.set_tag("defTypeCount", cache.index.by_type.len().to_string());
        }
    }
    Some(CachedDefIndex {
        index: cache.index,
        settings_fingerprint: expected_settings,
        file_fingerprints: cache.file_fingerprints,
    })
}

/// Reads and deserializes the disk cache file, checking only the settings fingerprint -- shared by
/// `load_cached_index_unverified` and `load_cached_index_only`. Deliberately does *not* call
/// `rebuild_computed_fields` on the deserialized index: that rebuild (lowercasing every def
/// name/label, rebuilding the by-type map) is real, non-trivial work for a large collection, and
/// `load_cached_index_only`'s common case -- a `RequireFresh` caller hitting a settings-matching
/// but file-content-stale cache -- would otherwise pay it just to immediately discard the result on
/// a fingerprint mismatch. Callers that actually need the returned index usable decide when to pay
/// that cost themselves.
fn read_cache_file(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: &DefIndexBuildOptions<'_>,
    app: Option<&AppHandle>,
) -> Option<(DefIndexCacheFile, String)> {
    let expected_settings = settings_fingerprint(settings, options);
    let cache_path = cache_path(app_data_dir);
    if !cache_path.exists() {
        return None;
    }

    let raw = {
        let mut span = app.map(|app| crate::instrumentation::span(app, "defIndex.cacheRead"));
        let raw = std::fs::read(&cache_path).ok()?;
        if let Some(span) = span.as_mut() {
            span.set_tag("bytes", raw.len().to_string());
        }
        raw
    };

    let cache = {
        let mut span =
            app.map(|app| crate::instrumentation::span(app, "defIndex.cacheDeserialize"));
        let parsed = cache_v3::decode(&raw);
        let dto = match parsed {
            Some(dto) => dto,
            None => {
                if let Some(span) = span.as_mut() {
                    span.set_tag("errorCount", "1");
                }
                return None;
            }
        };
        let version = dto.version;
        let fingerprint_count = dto.file_fingerprints.len();
        let (settings_fingerprint, file_fingerprints, index) = cache_v3::from_dto(dto);
        if let Some(span) = span.as_mut() {
            span.set_tag("version", version.to_string());
            span.set_tag("defCount", index.defs.len().to_string());
            span.set_tag("fingerprintCount", fingerprint_count.to_string());
            span.set_tag("errorCount", index.errors.len().to_string());
        }
        DefIndexCacheFile {
            version,
            settings_fingerprint,
            file_fingerprints,
            index,
        }
    };

    if cache.version != CACHE_VERSION || cache.settings_fingerprint != expected_settings {
        return None;
    }
    Some((cache, expected_settings))
}

/// Reads a valid disk-cached def index, strictly verifying it against the current collection.
/// Returns `None` for any benign miss -- everything [`load_cached_index_unverified`] treats as a
/// miss, plus a fingerprint scan failure or any fingerprint mismatch -- never as an error.
///
/// Ignores `options.replacement` / `options.force_rebuild`; callers that need those bypass
/// semantics (like [`load_or_rebuild_def_index`]) must check them before calling this.
pub fn load_cached_index_only(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: &DefIndexBuildOptions<'_>,
) -> Option<CachedDefIndex> {
    let (mut cache, expected_settings) = read_cache_file(app_data_dir, settings, options, None)?;
    // Check the (potentially slow, project-wide) file-fingerprint scan against the *raw*
    // deserialized fingerprints before paying `rebuild_computed_fields`'s cost -- on the common
    // stale-cache path (any edit since the cache was written), that rebuild would otherwise be
    // performed only to be thrown away immediately below.
    let expected_files = file_fingerprints(settings, options).ok()?;
    if cache.file_fingerprints != expected_files {
        return None;
    }
    cache.index.rebuild_computed_fields();
    Some(CachedDefIndex {
        index: cache.index,
        settings_fingerprint: expected_settings,
        file_fingerprints: cache.file_fingerprints,
    })
}

/// Outcome of comparing a previously-stored fingerprint list against the current collection --
/// used by the background cache verifier so it can distinguish "still fresh" from "stale" from
/// "couldn't tell" for logging/status handling, without re-reading or re-deserializing the disk
/// cache (the caller already has the fingerprints from an earlier hydration).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FingerprintVerification {
    Match,
    Mismatch,
    ScanFailed,
}

/// Compares `expected` (fingerprints captured at a prior hydration/build) against a fresh
/// project-wide file-fingerprint scan. This is the only place that scan runs for verification --
/// it never reads or deserializes the disk cache file itself.
pub fn verify_fingerprints(
    settings: &ProjectSettings,
    options: &DefIndexBuildOptions<'_>,
    expected: &[IndexedFileFingerprint],
) -> FingerprintVerification {
    match file_fingerprints(settings, options) {
        Ok(current) => {
            if current == *expected {
                FingerprintVerification::Match
            } else {
                FingerprintVerification::Mismatch
            }
        }
        Err(_) => FingerprintVerification::ScanFailed,
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

    if let Some(cached) = load_cached_index_only(app_data_dir, settings, &options) {
        return Ok(cached.index);
    }

    rebuild_and_store_def_index(app_data_dir, settings, options)
}

pub fn rebuild_and_store_def_index(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: DefIndexBuildOptions<'_>,
) -> Result<DefIndex, DefIndexCacheError> {
    rebuild_and_store_def_index_with_progress(app_data_dir, settings, options, None)
}

/// Same as `rebuild_and_store_def_index`, but threads a live-progress callback through to
/// `build_def_index_with_progress` -- see that function's doc comment.
pub fn rebuild_and_store_def_index_with_progress(
    app_data_dir: &Path,
    settings: &ProjectSettings,
    options: DefIndexBuildOptions<'_>,
    on_file_indexed: Option<&dyn Fn()>,
) -> Result<DefIndex, DefIndexCacheError> {
    let should_store = options.replacement.is_none();
    let settings_hash = settings_fingerprint(settings, &options);
    let files = file_fingerprints(settings, &options).unwrap_or_default();
    let index = build_def_index_with_progress(settings, options, on_file_indexed);

    if should_store {
        let cache = DefIndexCacheFile {
            version: CACHE_VERSION,
            settings_fingerprint: settings_hash,
            file_fingerprints: files,
            index: index.clone(),
        };
        // A disk-cache write failure (including the size guardrail in `write_cache`) must never
        // discard the freshly built in-memory index -- report it and continue. The next launch
        // simply runs as an ordinary cache miss (a normal, already-handled background rebuild)
        // rather than this whole rebuild failing outright over a problem that has nothing to do
        // with whether the collection itself parsed correctly.
        if let Err(e) = write_cache(app_data_dir, &cache) {
            eprintln!(
                "[rimedit] Failed to persist def-index cache; continuing without it: {}",
                e
            );
        }
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
    let dto = cache_v3::to_dto(
        cache.settings_fingerprint.clone(),
        cache.file_fingerprints.clone(),
        &cache.index,
    );
    let bytes = cache_v3::encode(&dto)?;
    // Unique per call (not a fixed name): the background worker's own rebuild/persist path and a
    // `RequireFresh` caller's cache-miss rebuild (commands like save/duplicate-check, invoked
    // directly, not funneled through the worker's single queue) can call this concurrently. A
    // shared fixed tmp filename would let one writer's `std::fs::write` (not itself guaranteed
    // atomic for larger payloads) interleave with another's, risking a `rename` moving
    // already-overwritten, inconsistent bytes into place. Each writer getting its own tmp path
    // means the final `rename` -- atomic at the filesystem level -- is the only point of
    // contention, and concurrent renames to the same destination simply resolve to "last one
    // wins" with no interleaved-content risk.
    let tmp = parent.join(format!("index-cache-v3-{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, bytes).map_err(|e| DefIndexCacheError::WriteFailed(e.to_string()))?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        // Unlike the old fixed-filename scheme (where the next write would simply overwrite the
        // same stale tmp file), a unique-per-call name left behind by a failed rename would
        // otherwise never get cleaned up by anything in this codebase -- write_cache runs on
        // essentially every save, so repeated failures (e.g. a persistently AV-locked cache
        // directory) would accumulate unbounded orphaned full-index cache files. Best-effort only:
        // if the cleanup itself fails, there's nothing more useful to do than report the original
        // rename error.
        let _ = std::fs::remove_file(&tmp);
        return Err(DefIndexCacheError::WriteFailed(e.to_string()));
    }
    Ok(())
}

/// `.bin.zst` (not the prior `index-cache-v1.json`, despite that file's `CACHE_VERSION` internally
/// already having moved to `2` before this cache format existed): a distinct filename, not a
/// migration, is what makes an old on-disk cache written by a pre-Phase-2 build a silent, benign
/// miss here -- `cache_path.exists()` in `read_cache_file` simply never finds it. The old file (if
/// present) is left alone as harmless obsolete data; nothing in this module ever deletes it.
fn cache_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir
        .join("def-index")
        .join("index-cache-v3.bin.zst")
}
