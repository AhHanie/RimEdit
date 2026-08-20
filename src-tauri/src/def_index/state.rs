use std::sync::{Arc, Mutex};
use time::OffsetDateTime;

use super::fingerprint::IndexedFileFingerprint;
use super::model::{DefIndex, DefIndexSummary, IndexedSourceKind};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexingPhase {
    Idle,
    Pending,
    Running,
    Complete,
    Failed,
}

/// Coarse stage within a `Running` full rebuild, for status-bar progress display on large
/// (e.g. Steam Workshop) collections. `None` outside a full rebuild (incremental file-change
/// jobs and every other phase leave `current_stage` unset).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexingStage {
    /// Reading, deserializing, and rebuilding computed fields for the trusted-but-unverified disk
    /// cache during startup/interactive hydration, before any collection scan has run -- see
    /// `services::def_index_cache::schedule_initialization`. Precedes `Discovering`/`Indexing`,
    /// which only begin once hydration has produced a `Miss` and a `FullRebuild` is enqueued. Never
    /// carries a file counter (there is nothing to count): `set_status_hydrating_cache_if_generation_matches`
    /// always clears `total_files`/`processed_files`/`pending_files`.
    HydratingCache,
    /// Cheap, parse-free directory-listing pass computing scan statistics (locations, resolved
    /// Workshop items, selected load folders, discovered file count) before the slower
    /// read/parse phase -- see `def_index::discover_scan_stats`.
    Discovering,
    /// Reading and parsing the discovered Def XML files (and, internally, persisting the
    /// resulting index once every file has been attempted) -- see `builder::build_def_index_with_progress`.
    Indexing,
}

/// Whether the def-index cache backing the current status has been verified against the actual
/// collection on disk. A fast hydrated-cache startup reports `Checking` (real Def/error counts,
/// but not yet fingerprint-verified); a background `VerifyCache` job flips a matching entry to
/// `Verified` on a fingerprint match, or triggers a `NotRequired`-labeled full rebuild on a
/// mismatch that itself ends `Verified`. Every other status constructor -- explicit
/// settings-triggered/manual rebuilds while in progress -- reports `NotRequired`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheVerification {
    NotRequired,
    Checking,
    Verified,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexingStatus {
    pub project_id: Option<String>,
    pub phase: IndexingPhase,
    pub cache_verification: CacheVerification,
    pub pending_files: usize,
    pub indexed_defs: usize,
    pub project_defs: usize,
    pub source_defs: usize,
    pub errors: usize,
    pub message: Option<String>,
    pub updated_at_unix_ms: i64,
    /// Total files discovered for this full rebuild, known once the `Discovering` stage
    /// completes. `None` before discovery finishes, during incremental jobs, or once the
    /// rebuild reaches `Complete`/`Failed` (superseded by `indexed_defs`/`errors` at that point).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_files: Option<usize>,
    /// Ticks up as each discovered file is attempted (read+parsed or failed to read), throttled
    /// to a bounded number of emissions per rebuild -- see
    /// `services::indexing::jobs::execute_full_rebuild`'s progress closure and
    /// `builder::build_def_index_with_progress`. `Some(0)` immediately after `total_files` is
    /// set, ticking toward (and clamped to) `total_files` as parsing proceeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_files: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_stage: Option<IndexingStage>,
    /// Display name only -- never a filesystem path. `Some` only while discovering a specific
    /// location's scan stats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_location_name: Option<String>,
    /// The completed index's own `DefIndex::built_at_unix_ms`, `Some` only on a `Complete` status.
    /// Distinct from `updated_at_unix_ms` (which changes on *any* status write, including a mere
    /// `confirm_cache_verified` checking->verified flip that leaves the underlying index
    /// untouched): this field only changes when the index itself was actually (re)built, so it's
    /// the exact signal a "did the collection's data actually change" consumer needs -- see the
    /// frontend's `shouldBumpValidationRefreshRevision`, which would otherwise have no way to tell
    /// a genuine rebuild whose def/error counts happen to coincide with the prior complete
    /// snapshot apart from a same-index verification-only confirmation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_built_at_unix_ms: Option<i64>,
    /// The `DefIndexState` generation this status was published under -- not serialized to the
    /// frontend (backend-internal only). A job superseded by a generation bump can bail out
    /// mid-setup after already publishing an intermediate `Pending`/`Running` status (e.g.
    /// `execute_full_rebuild`'s early-return generation checks run *after* `set_status_discovering`/
    /// `set_status_indexing` have already written it) and never gets to publish a terminating
    /// `Complete`/`Failed` for that abandoned attempt -- leaving the status frozen at
    /// `Pending`/`Running` for a `project_id` that no active job is actually working on anymore,
    /// until some unrelated job happens to overwrite the whole status object. Comparing this field
    /// against `DefIndexState::current_generation()` lets a consumer (`should_attempt_initialization`,
    /// `load_for_project_query`'s in-flight guard) recognize such a stale status instead of trusting
    /// it as evidence of a live in-flight job.
    #[serde(skip)]
    pub generation: u64,
}

impl IndexingStatus {
    fn idle() -> Self {
        Self {
            project_id: None,
            phase: IndexingPhase::Idle,
            cache_verification: CacheVerification::NotRequired,
            pending_files: 0,
            indexed_defs: 0,
            project_defs: 0,
            source_defs: 0,
            errors: 0,
            message: None,
            updated_at_unix_ms: now_ms(),
            total_files: None,
            processed_files: None,
            current_stage: None,
            current_location_name: None,
            index_built_at_unix_ms: None,
            generation: 0,
        }
    }
}

fn now_ms() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000
}

fn complete_status(
    project_id: Option<String>,
    index: &DefIndex,
    cache_verification: CacheVerification,
    generation: u64,
) -> IndexingStatus {
    let project_defs = index
        .defs
        .iter()
        .filter(|d| d.source.source_kind == IndexedSourceKind::Project)
        .count();
    let source_defs = index
        .defs
        .iter()
        .filter(|d| d.source.source_kind == IndexedSourceKind::Source)
        .count();
    IndexingStatus {
        project_id,
        phase: IndexingPhase::Complete,
        cache_verification,
        pending_files: 0,
        indexed_defs: index.defs.len(),
        project_defs,
        source_defs,
        errors: index.errors.len(),
        message: None,
        updated_at_unix_ms: now_ms(),
        total_files: None,
        processed_files: None,
        current_stage: None,
        current_location_name: None,
        index_built_at_unix_ms: Some(index.built_at_unix_ms),
        generation,
    }
}

struct DefIndexStateEntry {
    /// The project this entry was built/hydrated for, if any -- lets `get_cached_for_project`
    /// refuse to hand back a *different* project's index from this single global slot.
    project_id: Option<String>,
    settings_fingerprint: String,
    file_fingerprints: Vec<IndexedFileFingerprint>,
    index: Arc<DefIndex>,
    /// Whether *this specific entry's* fingerprints are known-fresh -- a genuinely fresh build
    /// (`store`), or a hydrated cache (`store_if_generation_matches`) that has since passed
    /// background verification (`confirm_cache_verified`). Deliberately tracked on the entry
    /// itself, not derived from `DefIndexStateInner::status.cache_verification`: `status` is a
    /// single global snapshot that a *different* concurrent operation (e.g. another
    /// `IndexLoadPolicy::RequireFresh` caller's own fresh rebuild, which updates the entry via
    /// `store` without touching `status` at all) can leave stale relative to whichever entry is
    /// actually cached right now. `get_verified_if_settings_match` checks this field directly so
    /// it can never desync from the entry it describes.
    verified: bool,
}

struct DefIndexStateInner {
    entry: Option<DefIndexStateEntry>,
    generation: u64,
    status: IndexingStatus,
}

impl Default for DefIndexStateInner {
    fn default() -> Self {
        Self {
            entry: None,
            generation: 0,
            status: IndexingStatus::idle(),
        }
    }
}

#[derive(Default)]
pub struct DefIndexState {
    inner: Mutex<DefIndexStateInner>,
}

impl DefIndexState {
    /// Stores the index (wrapped in Arc) and returns the Arc so callers can reuse it. Every
    /// current caller passes an index that was just read/parsed/rebuilt directly (a fresh full
    /// rebuild, a re-verified `RequireFresh` load, or an incremental update applied to an
    /// already-current base) -- there is nothing left to verify -- so the stored entry is always
    /// marked `verified`. `store_if_generation_matches` is the counterpart for a trusted-but-not-
    /// yet-verified hydration.
    pub fn store(
        &self,
        project_id: Option<String>,
        settings_fingerprint: String,
        file_fingerprints: Vec<IndexedFileFingerprint>,
        index: DefIndex,
    ) -> Arc<DefIndex> {
        let arc = Arc::new(index);
        if let Ok(mut guard) = self.inner.lock() {
            guard.entry = Some(DefIndexStateEntry {
                project_id,
                settings_fingerprint,
                file_fingerprints,
                index: Arc::clone(&arc),
                verified: true,
            });
        }
        arc
    }

    /// Same as `store`, but atomically no-ops (returning `None`, leaving any existing entry
    /// untouched) if `expected_generation` no longer matches the current generation, or if an
    /// already-*verified* entry for the same `{project_id, settings_fingerprint}` is already
    /// cached. The check and the write happen under one lock acquisition, so a settings/project
    /// change racing a slow disk read (e.g. cache hydration) can never land its now-stale result
    /// *after* a legitimate newer write, silently clobbering it -- unlike checking the generation
    /// before and/or after a plain `store()` call, which still leaves a race window around the
    /// write itself.
    ///
    /// The same-scope-already-verified check guards a distinct race: this is the *trusted, not
    /// yet verified* hydration path, and `settings_fingerprint` never reflects file content (only
    /// location/game-version metadata -- see `fingerprint::settings_fingerprint`), so a watcher-
    /// driven incremental update (`persist_incremental`'s plain `store()`, always `verified: true`)
    /// racing ahead of a slow disk-cache read for the *same* project/settings would otherwise get
    /// silently downgraded back to unverified, stale data by this call landing second. A verified
    /// entry is never less trustworthy than a fresh hydration for the same scope, so refusing to
    /// overwrite it here is always safe -- unlike a *different* scope (a genuine project switch),
    /// where the mismatched `project_id`/`settings_fingerprint` lets this through as intended.
    pub fn store_if_generation_matches(
        &self,
        expected_generation: u64,
        project_id: Option<String>,
        settings_fingerprint: String,
        file_fingerprints: Vec<IndexedFileFingerprint>,
        index: DefIndex,
    ) -> Option<Arc<DefIndex>> {
        let mut guard = self.inner.lock().ok()?;
        if guard.generation != expected_generation {
            return None;
        }
        let already_verified_for_same_scope = guard.entry.as_ref().is_some_and(|e| {
            e.verified
                && e.project_id == project_id
                && e.settings_fingerprint == settings_fingerprint
        });
        if already_verified_for_same_scope {
            return None;
        }
        let arc = Arc::new(index);
        guard.entry = Some(DefIndexStateEntry {
            project_id,
            settings_fingerprint,
            file_fingerprints,
            index: Arc::clone(&arc),
            // This is the trusted-but-unverified hydration path (`hydrate_unverified_for_project`
            // is its only caller) -- `confirm_cache_verified` flips this once a background
            // `VerifyCache` job confirms the fingerprints still match the collection.
            verified: false,
        });
        Some(arc)
    }

    pub fn get_if_settings_match(&self, settings_fingerprint: &str) -> Option<Arc<DefIndex>> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.entry.as_ref()?;
        if entry.settings_fingerprint == settings_fingerprint {
            Some(Arc::clone(&entry.index))
        } else {
            None
        }
    }

    /// Same as `get_if_settings_match`, but additionally refuses to return the entry unless *it
    /// specifically* (`entry.verified`, not the global `status.cache_verification`) is known-fresh
    /// -- either a genuinely fresh build/rebuild, or a trusted hydration that has since passed
    /// background verification. `IndexLoadPolicy::RequireFresh` (`load_for_project`'s hot path)
    /// uses this instead of `get_if_settings_match`: before the verified/unverified distinction
    /// existed, every entry `DefIndexState` ever held had already passed a full fingerprint scan
    /// by construction (the old strict-only hydration path never stored anything unverified), so
    /// trusting a settings-fingerprint match alone was safe. `hydrate_unverified_for_project` broke
    /// that invariant; RequireFresh callers must not silently inherit an unverified index just
    /// because `Interactive` loads populated the same global slot first.
    pub fn get_verified_if_settings_match(
        &self,
        settings_fingerprint: &str,
    ) -> Option<Arc<DefIndex>> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.entry.as_ref()?;
        if entry.verified && entry.settings_fingerprint == settings_fingerprint {
            Some(Arc::clone(&entry.index))
        } else {
            None
        }
    }

    /// Returns whatever index is cached for `project_id`, regardless of settings-fingerprint
    /// match, for a non-blocking query/interactive fallback -- e.g. a fresh index isn't ready yet
    /// but a same-project stale one is better than nothing. `DefIndexState` holds a single global
    /// slot, so this deliberately refuses to hand back a *different* project's index (e.g. right
    /// after a project switch): that would silently attribute another project's Defs to this
    /// one's search/facet/reference/validation results instead of merely showing stale data.
    pub fn get_cached_for_project(&self, project_id: &str) -> Option<Arc<DefIndex>> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.entry.as_ref()?;
        if entry.project_id.as_deref() == Some(project_id) {
            Some(Arc::clone(&entry.index))
        } else {
            None
        }
    }

    /// Returns the stored file fingerprints without rescanning any files, if the cached entry
    /// matches `settings_fp`. Used by the save fast path to verify validation tokens without
    /// triggering a full project-wide file scan.
    pub fn get_file_fingerprints_if_settings_match(
        &self,
        settings_fp: &str,
    ) -> Option<Vec<IndexedFileFingerprint>> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.entry.as_ref()?;
        if entry.settings_fingerprint == settings_fp {
            Some(entry.file_fingerprints.clone())
        } else {
            None
        }
    }

    /// Same presence check as `get_file_fingerprints_if_settings_match(..).is_some()`, without
    /// cloning the (potentially large, for a Steam Workshop-sized collection) fingerprint list just
    /// to immediately discard it -- for callers that only need to know an entry is still there, not
    /// its contents.
    pub fn has_entry_with_settings_fingerprint(&self, settings_fp: &str) -> bool {
        let Ok(guard) = self.inner.lock() else {
            return false;
        };
        guard
            .entry
            .as_ref()
            .is_some_and(|e| e.settings_fingerprint == settings_fp)
    }

    pub fn current_generation(&self) -> u64 {
        self.inner.lock().map(|g| g.generation).unwrap_or(0)
    }

    pub fn increment_generation(&self) -> u64 {
        if let Ok(mut guard) = self.inner.lock() {
            guard.generation += 1;
            guard.generation
        } else {
            0
        }
    }

    pub fn set_status_pending(&self, project_id: Option<String>, pending_files: usize) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Pending,
                cache_verification: CacheVerification::NotRequired,
                pending_files,
                indexed_defs: 0,
                project_defs: 0,
                source_defs: 0,
                errors: 0,
                message: None,
                updated_at_unix_ms: now_ms(),
                total_files: None,
                processed_files: None,
                current_stage: None,
                current_location_name: None,
                index_built_at_unix_ms: None,
                generation: guard.generation,
            };
        }
    }

    /// Publishes the `HydratingCache` sub-stage, used while `schedule_initialization`'s spawned
    /// background work reads/deserializes the trusted disk cache and rebuilds its computed fields
    /// -- before any collection scan has run, and before it's known whether this will resolve to a
    /// `Hit` or a `Miss`. Every file-progress field (`total_files`, `processed_files`,
    /// `pending_files`) is cleared, so the frontend never renders a `Discovering`/`Indexing`-style
    /// file counter for a stage that has none.
    ///
    /// Atomically no-ops (returning `false`, leaving `status` untouched) unless
    /// `expected_generation` still matches the current generation -- generation-guarded like the
    /// other pre-hydration status setters here: a project/settings change racing the claim of
    /// `InitKey` (a separate operation, done just before this call in `schedule_initialization`)
    /// must not let a stale-generation status overwrite whatever a newer initialization has already
    /// published.
    pub fn set_status_hydrating_cache_if_generation_matches(
        &self,
        expected_generation: u64,
        project_id: Option<String>,
    ) -> bool {
        if let Ok(mut guard) = self.inner.lock() {
            if guard.generation != expected_generation {
                return false;
            }
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Running,
                cache_verification: CacheVerification::NotRequired,
                pending_files: 0,
                indexed_defs: 0,
                project_defs: 0,
                source_defs: 0,
                errors: 0,
                message: None,
                updated_at_unix_ms: now_ms(),
                total_files: None,
                processed_files: None,
                current_stage: Some(IndexingStage::HydratingCache),
                current_location_name: None,
                index_built_at_unix_ms: None,
                generation: expected_generation,
            };
            true
        } else {
            false
        }
    }

    pub fn set_status_running(&self, project_id: Option<String>, pending_files: usize) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Running,
                cache_verification: CacheVerification::NotRequired,
                pending_files,
                indexed_defs: 0,
                project_defs: 0,
                source_defs: 0,
                errors: 0,
                message: None,
                updated_at_unix_ms: now_ms(),
                total_files: None,
                processed_files: None,
                current_stage: None,
                current_location_name: None,
                index_built_at_unix_ms: None,
                generation: guard.generation,
            };
        }
    }

    /// Enters the `Discovering` sub-stage of a `Running` full rebuild -- see
    /// `def_index::discover_scan_stats`. `current_location_name` is display-name-only.
    pub fn set_status_discovering(
        &self,
        project_id: Option<String>,
        current_location_name: Option<String>,
    ) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Running,
                cache_verification: CacheVerification::NotRequired,
                pending_files: 0,
                indexed_defs: 0,
                project_defs: 0,
                source_defs: 0,
                errors: 0,
                message: None,
                updated_at_unix_ms: now_ms(),
                total_files: None,
                processed_files: None,
                current_stage: Some(IndexingStage::Discovering),
                current_location_name,
                index_built_at_unix_ms: None,
                generation: guard.generation,
            };
        }
    }

    /// Enters the `Indexing` sub-stage of a `Running` full rebuild once discovery has produced
    /// a total file count.
    pub fn set_status_indexing(&self, project_id: Option<String>, total_files: usize) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Running,
                cache_verification: CacheVerification::NotRequired,
                pending_files: 0,
                indexed_defs: 0,
                project_defs: 0,
                source_defs: 0,
                errors: 0,
                message: None,
                updated_at_unix_ms: now_ms(),
                total_files: Some(total_files),
                processed_files: Some(0),
                current_stage: Some(IndexingStage::Indexing),
                current_location_name: None,
                index_built_at_unix_ms: None,
                generation: guard.generation,
            };
        }
    }

    /// Ticks `processed_files` up during the `Indexing` sub-stage, leaving every other field
    /// (`total_files`, `phase`, `current_stage`, ...) untouched. `processed_files` is clamped to
    /// `total_files` (if set) so a discovery/indexing file-count mismatch -- e.g. a file added or
    /// removed on disk between the two passes -- can never render as "over 100%". A no-op if the
    /// rebuild has already left the `Indexing` stage (e.g. a stale/superseded progress tick
    /// arriving after `set_status_complete`/`set_status_failed`).
    pub fn set_status_indexing_progress(&self, processed_files: usize) {
        if let Ok(mut guard) = self.inner.lock() {
            if guard.status.current_stage != Some(IndexingStage::Indexing) {
                return;
            }
            let clamped = match guard.status.total_files {
                Some(total) => processed_files.min(total),
                None => processed_files,
            };
            guard.status.processed_files = Some(clamped);
            guard.status.updated_at_unix_ms = now_ms();
        }
    }

    /// Reports a freshly built/updated index (a completed full rebuild or incremental file job)
    /// as complete and verified -- unlike a hydrated disk-cache hit, this index was just read
    /// straight off disk, so there is nothing left to verify in the background.
    pub fn set_status_complete(&self, index: &DefIndex) {
        if let Ok(mut guard) = self.inner.lock() {
            let project_id = guard.status.project_id.clone();
            let generation = guard.generation;
            guard.status =
                complete_status(project_id, index, CacheVerification::Verified, generation);
        }
    }

    /// Same as `set_status_complete`, but derived from a `DefIndexSummary` instead of a full
    /// `DefIndex` -- for a caller that just successfully rebuilt an index but no longer has (or
    /// can no longer look up) the built `DefIndex` itself. `execute_full_rebuild` uses this as its
    /// fallback when its post-rebuild `get_if_settings_match` readback misses (a concurrent write
    /// -- e.g. a project switch's own hydration landing in `DefIndexState`'s single global slot --
    /// evicted the just-stored entry before the readback ran): reporting the rebuild's own summary
    /// (real counts and build timestamp) is still correct, unlike falling back to
    /// `set_status_complete(&DefIndex::default())`, which would falsely publish "0 defs" for a
    /// project whose rebuild actually just succeeded.
    pub fn set_status_complete_from_summary(
        &self,
        project_id: Option<String>,
        summary: &DefIndexSummary,
    ) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Complete,
                cache_verification: CacheVerification::Verified,
                pending_files: 0,
                indexed_defs: summary.indexed_defs,
                project_defs: summary.project_defs,
                source_defs: summary.source_defs,
                errors: summary.errors,
                message: None,
                updated_at_unix_ms: now_ms(),
                total_files: None,
                processed_files: None,
                current_stage: None,
                current_location_name: None,
                index_built_at_unix_ms: Some(summary.built_at_unix_ms),
                generation: guard.generation,
            };
        }
    }

    /// Reports a hydrated disk-cache hit as completed indexing for the active project, explicitly
    /// assigning `project_id` instead of carrying it forward from the prior status (fresh state
    /// begins with `project_id: None`). The cache has not been fingerprint-verified yet, so this
    /// always reports `cacheVerification: Checking` -- callers must ensure a `VerifyCache` job is
    /// scheduled alongside this call.
    ///
    /// Atomically no-ops (returning `false`, leaving `status` untouched) unless
    /// `expected_generation` still matches the current generation AND the stored entry's settings
    /// fingerprint still matches `expected_settings_fingerprint`. `schedule_initialization`
    /// calls this right after a successful `store_if_generation_matches` -- a separate lock
    /// acquisition, with a (small but non-zero) gap in between -- so a generation bump landing in
    /// that gap (e.g. a concurrent project switch) can't let a stale hydration overwrite a newer,
    /// already-published status, the same TOCTOU class `confirm_cache_verified` closes for
    /// verification. The generation check alone is not enough: a watcher-driven incremental update
    /// for the *same* project/settings can legitimately replace the entry without bumping the
    /// generation at all (content edits never change `settings_fingerprint`), so this derives the
    /// published counts/build-timestamp from whatever the *currently stored* entry actually is
    /// (re-read under this same lock) rather than trusting the possibly-now-stale index the caller
    /// captured before its first lock acquisition -- otherwise a status update naming the right
    /// project could still describe an index that's already been superseded.
    pub fn set_status_complete_for_project_if_generation_matches(
        &self,
        expected_generation: u64,
        expected_settings_fingerprint: &str,
        project_id: Option<String>,
    ) -> bool {
        if let Ok(mut guard) = self.inner.lock() {
            if guard.generation != expected_generation {
                return false;
            }
            let Some(entry) = guard.entry.as_ref() else {
                return false;
            };
            if entry.settings_fingerprint != expected_settings_fingerprint {
                return false;
            }
            let index = Arc::clone(&entry.index);
            guard.status = complete_status(
                project_id,
                &index,
                CacheVerification::Checking,
                expected_generation,
            );
            true
        } else {
            false
        }
    }

    /// Removes the cached entry, but only if it is still *exactly* the one a background
    /// `VerifyCache` job scanned -- `expected_generation` matches the current generation AND the
    /// entry's `settings_fingerprint`/`file_fingerprints` still equal what was captured before the
    /// (slow, off-thread) scan ran. Called on a confirmed `Mismatch`/`ScanFailed`: flipping
    /// `status` to `Pending`/`Running` alone leaves the now-known-stale `entry` in place, and since
    /// `status.cache_verification` moves to `NotRequired` for that in-progress phase (not
    /// `Checking`), `get_verified_if_settings_match` would otherwise keep serving that exact index
    /// -- the one just proven wrong -- to `IndexLoadPolicy::RequireFresh` callers until the
    /// resulting rebuild completes.
    ///
    /// The generation check alone is not enough: `settings_fingerprint` never reflects file content
    /// (only location/game-version metadata), so a watcher-driven incremental update can legitimately
    /// replace the entry's `file_fingerprints` (via `persist_incremental`'s `store()`) with fresh,
    /// already-verified data *without bumping the generation at all* -- and can race ahead of this
    /// call, landing between the top-level recheck in `execute_verify_cache` and this one. Comparing
    /// the entry's current `file_fingerprints` against the ones captured before the scan closes that
    /// gap: if they differ, some other write already corrected the entry, and clearing it here would
    /// destroy that fresher, correct data instead of the stale one the scan actually observed.
    pub fn invalidate_entry_if_unchanged(
        &self,
        expected_generation: u64,
        expected_settings_fingerprint: &str,
        expected_file_fingerprints: &[IndexedFileFingerprint],
    ) -> bool {
        if let Ok(mut guard) = self.inner.lock() {
            if guard.generation != expected_generation {
                return false;
            }
            let still_the_scanned_entry = guard.entry.as_ref().is_some_and(|e| {
                e.settings_fingerprint == expected_settings_fingerprint
                    && e.file_fingerprints == expected_file_fingerprints
            });
            if still_the_scanned_entry {
                guard.entry = None;
                return true;
            }
        }
        false
    }

    /// Removes the cached entry only if it is still exactly the *unverified* hydration identified
    /// by `settings_fingerprint` -- cleanup for `schedule_initialization` losing the race to
    /// publish `status` for an entry it just hydrated (a generation bump landed between the
    /// `store_if_generation_matches` call and the `set_status_complete_for_project_if_generation_matches`
    /// call right after it, so the latter fails and no `VerifyCache` job ever gets scheduled for
    /// this entry). Not generation-gated like `invalidate_entry_if_unchanged`: by the time this
    /// runs, the caller's captured generation is already known stale, so checking it would always
    /// no-op. The `verified == false` check keeps this from ever clearing a legitimate, already-
    /// verified entry that might coincidentally share the same settings fingerprint (e.g. the same
    /// project reactivated under a still-newer generation, with a genuinely fresh entry already in
    /// place by the time this cleanup call runs).
    pub fn remove_unverified_entry_if_settings_match(&self, settings_fingerprint: &str) -> bool {
        if let Ok(mut guard) = self.inner.lock() {
            let should_clear = guard
                .entry
                .as_ref()
                .is_some_and(|e| !e.verified && e.settings_fingerprint == settings_fingerprint);
            if should_clear {
                guard.entry = None;
            }
            should_clear
        } else {
            false
        }
    }

    /// Flips a `Checking` completed status to `Verified` in place, leaving the index and every
    /// other status field (counts, `phase`, `project_id`, ...) untouched -- but only if
    /// `expected_generation` matches the current generation AND the stored entry's settings
    /// fingerprint still matches `expected_settings_fingerprint`. Called once a background
    /// `VerifyCache` job confirms the hydrated cache's fingerprints still match the collection.
    ///
    /// The check and the write happen under one lock acquisition. `execute_verify_cache` already
    /// rechecks both conditions right before calling this, but as a separate lock acquisition from
    /// this one, with a (potentially slow, off-thread) filesystem scan in between -- a concurrent
    /// writer (a newer `schedule_initialization`, or a watcher-driven `persist_incremental`)
    /// could replace `status`/`entry` in that gap, and an unconditional write here would then
    /// blindly stamp `Verified` onto that unrelated newer state. Combining the recheck and the
    /// write into one lock acquisition closes that gap, the same way `store_if_generation_matches`
    /// does for `store`.
    pub fn confirm_cache_verified(
        &self,
        expected_generation: u64,
        expected_settings_fingerprint: &str,
    ) -> bool {
        if let Ok(mut guard) = self.inner.lock() {
            if guard.generation != expected_generation {
                return false;
            }
            let matches_entry = guard
                .entry
                .as_ref()
                .is_some_and(|e| e.settings_fingerprint == expected_settings_fingerprint);
            if !matches_entry {
                return false;
            }
            // Flip both: `entry.verified` is the authoritative signal `get_verified_if_settings_match`
            // checks, and `status.cache_verification` is what the frontend displays.
            if let Some(entry) = guard.entry.as_mut() {
                entry.verified = true;
            }
            guard.status.cache_verification = CacheVerification::Verified;
            guard.status.updated_at_unix_ms = now_ms();
            true
        } else {
            false
        }
    }

    pub fn set_status_failed(&self, project_id: Option<String>, message: String) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Failed,
                cache_verification: CacheVerification::NotRequired,
                pending_files: 0,
                indexed_defs: 0,
                project_defs: 0,
                source_defs: 0,
                errors: 0,
                message: Some(message),
                updated_at_unix_ms: now_ms(),
                total_files: None,
                processed_files: None,
                current_stage: None,
                current_location_name: None,
                index_built_at_unix_ms: None,
                generation: guard.generation,
            };
        }
    }

    pub fn status(&self) -> IndexingStatus {
        self.inner
            .lock()
            .map(|g| g.status.clone())
            .unwrap_or_else(|_| IndexingStatus::idle())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def_index::{DefIdentityKey, IndexedDef, IndexedDefSource};
    use crate::project_model::SourceType;

    fn indexed_def(def_name: &str, kind: IndexedSourceKind) -> IndexedDef {
        IndexedDef {
            key: DefIdentityKey {
                def_type: "ThingDef".to_string(),
                def_name: def_name.to_string(),
            },
            def_type: "ThingDef".to_string(),
            def_name: def_name.to_string(),
            label: None,
            parent_name: None,
            relative_path: "a.xml".to_string(),
            node_id: None,
            line: None,
            column: None,
            source: IndexedDefSource {
                location_id: "loc".to_string(),
                location_name: "loc".to_string(),
                source_kind: kind,
                source_type: SourceType::Folder,
                read_only: false,
                mod_id: None,
                game_version: None,
                expansion_name: None,
            },
            fields: Vec::new(),
            def_name_lower: def_name.to_lowercase(),
            label_lower: String::new(),
        }
    }

    #[test]
    fn set_status_complete_for_project_reports_the_supplied_project_id_and_counts() {
        let state = DefIndexState::default();
        // Fresh state begins with `project_id: None`; `set_status_complete` alone would carry
        // that forward instead of reporting the hydrated project.
        let index = DefIndex {
            defs: vec![
                indexed_def("Steel", IndexedSourceKind::Project),
                indexed_def("Wood", IndexedSourceKind::Source),
                indexed_def("Plastic", IndexedSourceKind::Source),
            ],
            errors: Vec::new(),
            built_at_unix_ms: 1234,
            by_type: Default::default(),
        };

        let generation = state.current_generation();
        state.store_if_generation_matches(
            generation,
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            index,
        );
        state.set_status_complete_for_project_if_generation_matches(
            generation,
            "fp1",
            Some("proj".to_string()),
        );

        let status = state.status();
        assert_eq!(status.phase, IndexingPhase::Complete);
        assert_eq!(status.project_id.as_deref(), Some("proj"));
        assert_eq!(status.indexed_defs, 3);
        assert_eq!(status.project_defs, 1);
        assert_eq!(status.source_defs, 2);
        assert_eq!(status.errors, 0);
        assert_eq!(status.pending_files, 0);
        assert_eq!(
            status.cache_verification,
            CacheVerification::Checking,
            "a hydrated-cache completion must report checking, not verified, until a VerifyCache job confirms it"
        );
        assert_eq!(status.total_files, None);
        assert_eq!(status.processed_files, None);
        assert_eq!(status.current_stage, None);
        assert_eq!(status.current_location_name, None);
    }

    #[test]
    fn set_status_complete_for_project_reports_a_concurrently_replaced_entry_not_a_stale_snapshot()
    {
        // Regression: a watcher-driven incremental update landing between
        // `hydrate_unverified_for_project`'s `store_if_generation_matches` call and this status
        // publish (two separate lock acquisitions, same generation -- content edits never bump it)
        // must not let this call publish a status describing the *old*, now-superseded entry.
        let state = DefIndexState::default();
        let generation = state.current_generation();
        let stale_index = DefIndex {
            defs: vec![indexed_def("Steel", IndexedSourceKind::Project)],
            errors: Vec::new(),
            built_at_unix_ms: 1000,
            by_type: Default::default(),
        };
        state.store_if_generation_matches(
            generation,
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            stale_index,
        );

        // A concurrent watcher-driven incremental update replaces the entry -- same generation,
        // same settings fingerprint (content edits never change either) -- with fresher data.
        let fresher_index = DefIndex {
            defs: vec![
                indexed_def("Steel", IndexedSourceKind::Project),
                indexed_def("Wood", IndexedSourceKind::Source),
            ],
            errors: Vec::new(),
            built_at_unix_ms: 2000,
            by_type: Default::default(),
        };
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            fresher_index,
        );

        state.set_status_complete_for_project_if_generation_matches(
            generation,
            "fp1",
            Some("proj".to_string()),
        );

        let status = state.status();
        assert_eq!(
            status.indexed_defs, 2,
            "the published status must describe the fresher, currently-stored entry, not the stale index captured before the race"
        );
        assert_eq!(status.index_built_at_unix_ms, Some(2000));
    }

    #[test]
    fn hydrating_cache_publishes_the_stage_with_no_file_counters() {
        let state = DefIndexState::default();
        let generation = state.current_generation();

        let wrote = state
            .set_status_hydrating_cache_if_generation_matches(generation, Some("proj".to_string()));

        assert!(wrote);
        let status = state.status();
        assert_eq!(status.phase, IndexingPhase::Running);
        assert_eq!(status.current_stage, Some(IndexingStage::HydratingCache));
        assert_eq!(status.project_id.as_deref(), Some("proj"));
        assert_eq!(status.total_files, None);
        assert_eq!(status.processed_files, None);
        assert_eq!(status.pending_files, 0);
        assert_eq!(status.cache_verification, CacheVerification::NotRequired);
        assert_eq!(status.generation, generation);
    }

    #[test]
    fn hydrating_cache_rejects_a_stale_generation() {
        let state = DefIndexState::default();
        let gen0 = state.current_generation();
        state.set_status_running(Some("other-proj".to_string()), 0);
        state.increment_generation();

        let wrote =
            state.set_status_hydrating_cache_if_generation_matches(gen0, Some("proj".to_string()));

        assert!(!wrote, "a stale-generation status write must be rejected");
        let status = state.status();
        assert_eq!(
            status.project_id.as_deref(),
            Some("other-proj"),
            "the newer status must not have been overwritten by the stale write"
        );
    }

    #[test]
    fn hydrating_cache_clears_stale_progress_fields_from_a_prior_stage() {
        let state = DefIndexState::default();
        state.set_status_indexing(Some("proj".to_string()), 100);
        state.set_status_indexing_progress(37);
        let generation = state.current_generation();

        state
            .set_status_hydrating_cache_if_generation_matches(generation, Some("proj".to_string()));

        let status = state.status();
        assert_eq!(status.total_files, None);
        assert_eq!(status.processed_files, None);
    }

    #[test]
    fn discovering_then_indexing_then_complete_progresses_through_expected_stages() {
        let state = DefIndexState::default();

        state.set_status_discovering(Some("proj".to_string()), None);
        let discovering = state.status();
        assert_eq!(discovering.phase, IndexingPhase::Running);
        assert_eq!(discovering.current_stage, Some(IndexingStage::Discovering));
        assert_eq!(discovering.total_files, None);
        assert_eq!(discovering.processed_files, None);

        state.set_status_indexing(Some("proj".to_string()), 42);
        let indexing = state.status();
        assert_eq!(indexing.phase, IndexingPhase::Running);
        assert_eq!(indexing.current_stage, Some(IndexingStage::Indexing));
        assert_eq!(indexing.total_files, Some(42));
        assert_eq!(indexing.processed_files, Some(0));

        state.set_status_complete(&DefIndex::default());
        let complete = state.status();
        assert_eq!(complete.phase, IndexingPhase::Complete);
        assert_eq!(complete.current_stage, None);
        assert_eq!(complete.total_files, None);
        assert_eq!(complete.processed_files, None);
    }

    #[test]
    fn indexing_progress_ticks_up_without_disturbing_other_fields() {
        let state = DefIndexState::default();
        state.set_status_indexing(Some("proj".to_string()), 100);

        state.set_status_indexing_progress(37);
        let status = state.status();
        assert_eq!(status.processed_files, Some(37));
        assert_eq!(status.total_files, Some(100));
        assert_eq!(status.current_stage, Some(IndexingStage::Indexing));
        assert_eq!(status.phase, IndexingPhase::Running);
        assert_eq!(status.project_id.as_deref(), Some("proj"));
    }

    #[test]
    fn indexing_progress_is_clamped_to_total_files() {
        let state = DefIndexState::default();
        state.set_status_indexing(None, 10);
        state.set_status_indexing_progress(15);
        assert_eq!(state.status().processed_files, Some(10));
    }

    #[test]
    fn indexing_progress_is_a_no_op_once_the_rebuild_has_completed() {
        let state = DefIndexState::default();
        state.set_status_indexing(None, 10);
        state.set_status_complete(&DefIndex::default());

        state.set_status_indexing_progress(5);

        let status = state.status();
        assert_eq!(status.phase, IndexingPhase::Complete);
        assert_eq!(
            status.processed_files, None,
            "a stale tick must not resurrect progress fields"
        );
    }

    #[test]
    fn indexing_progress_is_a_no_op_during_discovering() {
        let state = DefIndexState::default();
        state.set_status_discovering(None, None);
        state.set_status_indexing_progress(5);
        assert_eq!(state.status().processed_files, None);
    }

    #[test]
    fn discovering_carries_a_display_name_only_location_hint() {
        let state = DefIndexState::default();
        state.set_status_discovering(None, Some("My Mod".to_string()));
        let status = state.status();
        assert_eq!(status.current_location_name.as_deref(), Some("My Mod"));
    }

    #[test]
    fn failed_status_clears_progress_fields() {
        let state = DefIndexState::default();
        state.set_status_indexing(None, 100);
        state.set_status_failed(None, "boom".to_string());
        let status = state.status();
        assert_eq!(status.phase, IndexingPhase::Failed);
        assert_eq!(status.current_stage, None);
        assert_eq!(status.total_files, None);
        assert_eq!(status.cache_verification, CacheVerification::NotRequired);
    }

    #[test]
    fn every_status_setter_stamps_the_generation_current_at_write_time() {
        // Regression: a job superseded by a generation bump can bail out after already publishing
        // an intermediate `Pending`/`Running` status; without a generation stamp on the status
        // itself, that abandoned status is indistinguishable from a genuinely live one to a
        // consumer like `should_attempt_initialization` -- see `IndexingStatus::generation`'s doc
        // comment.
        let state = DefIndexState::default();
        assert_eq!(
            state.status().generation,
            0,
            "fresh state starts at generation 0"
        );

        state.increment_generation();
        state.set_status_pending(None, 0);
        assert_eq!(state.status().generation, 1);

        state.increment_generation();
        let gen2 = state.current_generation();
        state.set_status_hydrating_cache_if_generation_matches(gen2, None);
        assert_eq!(state.status().generation, 2);

        state.increment_generation();
        state.set_status_running(None, 0);
        assert_eq!(state.status().generation, 3);

        state.increment_generation();
        state.set_status_discovering(None, None);
        assert_eq!(state.status().generation, 4);

        state.increment_generation();
        state.set_status_indexing(None, 10);
        assert_eq!(state.status().generation, 5);

        state.increment_generation();
        state.set_status_complete(&DefIndex::default());
        assert_eq!(state.status().generation, 6);

        state.increment_generation();
        state.set_status_failed(None, "boom".to_string());
        assert_eq!(state.status().generation, 7);

        state.increment_generation();
        state.set_status_complete_from_summary(
            None,
            &DefIndexSummary {
                indexed_defs: 0,
                project_defs: 0,
                source_defs: 0,
                errors: 0,
                built_at_unix_ms: 0,
            },
        );
        assert_eq!(state.status().generation, 8);
    }

    #[test]
    fn idle_status_reports_cache_verification_not_required() {
        let state = DefIndexState::default();
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::NotRequired
        );
    }

    #[test]
    fn pending_and_running_statuses_report_cache_verification_not_required() {
        let state = DefIndexState::default();
        state.set_status_pending(None, 0);
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::NotRequired
        );
        state.set_status_running(None, 0);
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::NotRequired
        );
    }

    #[test]
    fn a_fresh_full_rebuild_completion_reports_verified() {
        let state = DefIndexState::default();
        state.set_status_complete(&DefIndex::default());
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::Verified
        );
    }

    #[test]
    fn get_file_fingerprints_if_settings_match_misses_after_a_newer_store_replaces_the_entry() {
        // This is the guard `execute_verify_cache` relies on to avoid overwriting a newer
        // watcher-driven incremental update with a stale background verification result: once
        // `DefIndexState::store` is called again with a different settings fingerprint (e.g. a
        // file change that altered the index and its fingerprints), a verifier still holding the
        // *old* fingerprint must observe a miss here and discard its result instead of publishing
        // `Verified` for state that no longer reflects what's on disk.
        let state = DefIndexState::default();
        state.store(None, "fp-v1".to_string(), Vec::new(), DefIndex::default());
        assert!(state
            .get_file_fingerprints_if_settings_match("fp-v1")
            .is_some());

        state.store(None, "fp-v2".to_string(), Vec::new(), DefIndex::default());

        assert!(
            state
                .get_file_fingerprints_if_settings_match("fp-v1")
                .is_none(),
            "a stale verifier's captured fingerprint must no longer match after a newer store"
        );
        assert!(state
            .get_file_fingerprints_if_settings_match("fp-v2")
            .is_some());
    }

    #[test]
    fn get_cached_for_project_refuses_a_different_projects_entry() {
        let state = DefIndexState::default();
        let mut a = DefIndex::default();
        a.defs
            .push(indexed_def("Steel", IndexedSourceKind::Project));
        state.store(
            Some("proj-a".to_string()),
            "fp-a".to_string(),
            Vec::new(),
            a,
        );

        assert_eq!(
            state.get_cached_for_project("proj-a").map(|i| i.defs.len()),
            Some(1)
        );
        assert!(
            state.get_cached_for_project("proj-b").is_none(),
            "a different project's request must not receive proj-a's cached index"
        );
        assert!(
            state.get_cached_for_project("proj-a").is_some(),
            "the owning project's own request must still succeed"
        );
    }

    #[test]
    fn store_if_generation_matches_writes_only_when_the_generation_is_still_current() {
        let state = DefIndexState::default();
        let gen0 = state.current_generation();

        let stored = state.store_if_generation_matches(
            gen0,
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        assert!(stored.is_some(), "a matching generation must write");
        assert!(state.get_cached_for_project("proj").is_some());

        // A settings/project change bumps the generation -- a slow read captured at `gen0` must
        // no longer be allowed to write, even though it now has data in hand.
        state.increment_generation();
        let mut newer = DefIndex::default();
        newer
            .defs
            .push(indexed_def("Wood", IndexedSourceKind::Source));
        let stale_write = state.store_if_generation_matches(
            gen0,
            Some("proj".to_string()),
            "fp1-stale".to_string(),
            Vec::new(),
            newer,
        );
        assert!(
            stale_write.is_none(),
            "a stale-generation write must be rejected"
        );
        // The entry from the still-current write above must be untouched by the rejected one.
        assert_eq!(
            state.get_cached_for_project("proj").map(|i| i.defs.len()),
            Some(0),
            "a rejected stale write must not clobber the existing entry"
        );
    }

    #[test]
    fn set_status_complete_for_project_if_generation_matches_rejects_a_stale_generation() {
        let state = DefIndexState::default();
        let gen0 = state.current_generation();
        state.set_status_running(Some("other-proj".to_string()), 0);
        state.increment_generation();

        let wrote = state.set_status_complete_for_project_if_generation_matches(
            gen0,
            "fp1",
            Some("proj".to_string()),
        );

        assert!(!wrote, "a stale-generation status write must be rejected");
        let status = state.status();
        assert_eq!(
            status.project_id.as_deref(),
            Some("other-proj"),
            "the newer status must not have been overwritten by the stale write"
        );
        assert_eq!(status.phase, IndexingPhase::Running);
    }

    #[test]
    fn invalidate_entry_if_unchanged_clears_the_entry_only_for_the_current_generation() {
        let state = DefIndexState::default();
        let gen0 = state.current_generation();
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        assert!(state.get_cached_for_project("proj").is_some());

        // A stale-generation invalidation must not touch a still-current entry.
        assert!(!state.invalidate_entry_if_unchanged(gen0.wrapping_sub(1), "fp1", &[]));
        assert!(
            state.get_cached_for_project("proj").is_some(),
            "a stale-generation invalidation must be a no-op"
        );

        assert!(state.invalidate_entry_if_unchanged(gen0, "fp1", &[]));
        assert!(
            state.get_cached_for_project("proj").is_none(),
            "a current-generation, matching-fingerprints invalidation must clear the entry"
        );
    }

    #[test]
    fn invalidate_entry_if_unchanged_refuses_to_clear_an_entry_a_concurrent_write_already_replaced()
    {
        // Regression: a background `VerifyCache` scan captures `file_fingerprints` before its
        // (slow) disk scan; if a watcher-driven incremental update replaces the entry with fresh,
        // already-verified data for the *same* settings fingerprint in the meantime (content
        // changes never alter `settings_fingerprint`), the scan's now-stale Mismatch/ScanFailed
        // result must not destroy that fresher, correct entry.
        let state = DefIndexState::default();
        let gen0 = state.current_generation();
        let old_fingerprints = vec![IndexedFileFingerprint {
            location_id: "loc".to_string(),
            relative_path: "a.xml".to_string(),
            byte_len: 10,
            modified_unix_ms: Some(1),
            content_hash: "old".to_string(),
        }];
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            old_fingerprints.clone(),
            DefIndex::default(),
        );

        // A concurrent watcher-driven update replaces the entry's fingerprints (same settings_fp,
        // same generation -- content edits never touch either) before the verifier's stale-result
        // handling runs.
        let new_fingerprints = vec![IndexedFileFingerprint {
            content_hash: "new".to_string(),
            ..old_fingerprints[0].clone()
        }];
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            new_fingerprints,
            DefIndex::default(),
        );

        assert!(
            !state.invalidate_entry_if_unchanged(gen0, "fp1", &old_fingerprints),
            "must refuse to clear an entry whose file_fingerprints no longer match what was scanned"
        );
        assert!(
            state.get_cached_for_project("proj").is_some(),
            "the fresher, concurrently-written entry must survive"
        );
    }

    #[test]
    fn remove_unverified_entry_if_settings_match_only_clears_a_matching_unverified_entry() {
        let state = DefIndexState::default();
        let generation = state.current_generation();

        // Not present at all: no-op.
        assert!(!state.remove_unverified_entry_if_settings_match("fp1"));

        state.store_if_generation_matches(
            generation,
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        // Wrong fingerprint: must not clear.
        assert!(!state.remove_unverified_entry_if_settings_match("some-other-fp"));
        assert!(state.get_cached_for_project("proj").is_some());

        // A verified entry with the same fingerprint must never be cleared by this method, even if
        // it happens to share the fingerprint (e.g. a genuinely fresh entry already landed).
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        assert!(!state.remove_unverified_entry_if_settings_match("fp1"));
        assert!(state.get_cached_for_project("proj").is_some());

        // An unverified entry with a matching fingerprint is cleared.
        state.store_if_generation_matches(
            state.current_generation(),
            Some("proj2".to_string()),
            "fp2".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        assert!(state.remove_unverified_entry_if_settings_match("fp2"));
        assert!(state.get_cached_for_project("proj2").is_none());
    }

    #[test]
    fn store_if_generation_matches_refuses_to_downgrade_an_already_verified_same_scope_entry() {
        // Regression: a slow, trusted-but-unverified hydration read racing a watcher-driven
        // incremental update (`store`, always verified) for the *same* project/settings must not
        // be allowed to land second and silently replace the fresher verified entry with stale,
        // unverified data.
        let state = DefIndexState::default();
        let generation = state.current_generation();
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            DefIndex::default(),
        );

        let stale_hydration = state.store_if_generation_matches(
            generation,
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        assert!(
            stale_hydration.is_none(),
            "a hydration for a scope that's already verified must be refused"
        );
        assert!(
            state.get_verified_if_settings_match("fp1").is_some(),
            "the existing verified entry must remain in place, untouched"
        );

        // A different project/settings scope must still be allowed through.
        let different_scope = state.store_if_generation_matches(
            generation,
            Some("other-proj".to_string()),
            "fp-other".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        assert!(
            different_scope.is_some(),
            "a hydration for a different scope must not be blocked by an unrelated verified entry"
        );
    }

    #[test]
    fn confirm_cache_verified_writes_only_when_generation_and_entry_still_match() {
        let state = DefIndexState::default();
        let index = DefIndex {
            defs: vec![indexed_def("Steel", IndexedSourceKind::Project)],
            errors: Vec::new(),
            built_at_unix_ms: 1234,
            by_type: Default::default(),
        };
        let generation = state.current_generation();
        state.store_if_generation_matches(
            generation,
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        state.set_status_complete_for_project_if_generation_matches(
            generation,
            "fp1",
            Some("proj".to_string()),
        );
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            index,
        );
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::Checking
        );

        // A wrong settings fingerprint (the entry moved on since the scan was captured) must not
        // confirm verification.
        assert!(!state.confirm_cache_verified(generation, "some-other-fp"));
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::Checking
        );

        // A stale generation must not confirm verification, even with the right fingerprint.
        assert!(!state.confirm_cache_verified(generation + 1, "fp1"));
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::Checking
        );

        // Matching generation and fingerprint: confirms.
        assert!(state.confirm_cache_verified(generation, "fp1"));
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::Verified
        );
    }

    #[test]
    fn confirm_cache_verified_does_not_stamp_verified_onto_a_newer_unrelated_status() {
        // Regression for the TOCTOU this closes: a caller that captured `generation`/`fp1` before
        // a slow scan must not be able to mark a *different*, newer entry/status as verified just
        // because the scan happened to finish after that newer write landed.
        let state = DefIndexState::default();
        let generation = state.current_generation();
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            DefIndex::default(),
        );

        // A newer write for a different project/generation supersedes it.
        state.increment_generation();
        state.set_status_running(Some("other-proj".to_string()), 0);

        assert!(!state.confirm_cache_verified(generation, "fp1"));
        let status = state.status();
        assert_eq!(status.phase, IndexingPhase::Running);
        assert_eq!(status.project_id.as_deref(), Some("other-proj"));
        assert_eq!(
            status.cache_verification,
            CacheVerification::NotRequired,
            "the newer running status must not have been silently stamped Verified"
        );
    }

    #[test]
    fn get_verified_if_settings_match_trusts_a_plain_store_regardless_of_the_global_status() {
        // Regression: `get_verified_if_settings_match` must key off the *entry's own* verified
        // flag, not the single global `status.cache_verification` -- otherwise a RequireFresh
        // caller's own fresh rebuild (which updates the entry via a plain `store()` without
        // touching `status` at all) can get stuck being treated as unverified just because
        // `status` still says "Checking" from an unrelated, earlier hydration.
        let state = DefIndexState::default();
        let generation = state.current_generation();
        state.store_if_generation_matches(
            generation,
            Some("proj".to_string()),
            "fp-pre".to_string(),
            Vec::new(),
            DefIndex::default(),
        );
        state.set_status_complete_for_project_if_generation_matches(
            generation,
            "fp-pre",
            Some("proj".to_string()),
        );
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::Checking,
            "precondition: status is left at Checking from the hydration-hit status update"
        );

        // A RequireFresh caller's own fresh rebuild stores a *verified-by-construction* entry
        // via the plain `store()`, without touching `status` at all.
        state.store(
            Some("proj".to_string()),
            "fp-fresh".to_string(),
            Vec::new(),
            DefIndex::default(),
        );

        assert!(
            state.get_verified_if_settings_match("fp-fresh").is_some(),
            "a freshly stored entry must be trusted even while the unrelated global status \
             still reports Checking"
        );
    }

    #[test]
    fn get_verified_if_settings_match_rejects_an_unverified_hydrated_entry() {
        let state = DefIndexState::default();
        let generation = state.current_generation();
        state.store_if_generation_matches(
            generation,
            Some("proj".to_string()),
            "fp-hydrated".to_string(),
            Vec::new(),
            DefIndex::default(),
        );

        assert!(
            state
                .get_verified_if_settings_match("fp-hydrated")
                .is_none(),
            "an unverified hydration must not be trusted by RequireFresh"
        );

        assert!(state.confirm_cache_verified(generation, "fp-hydrated"));

        assert!(
            state
                .get_verified_if_settings_match("fp-hydrated")
                .is_some(),
            "once confirmed, the same entry must now be trusted"
        );
    }

    #[test]
    fn confirm_cache_verified_flips_a_checking_status_to_verified_without_touching_other_fields() {
        let state = DefIndexState::default();
        let index = DefIndex {
            defs: vec![indexed_def("Steel", IndexedSourceKind::Project)],
            errors: Vec::new(),
            built_at_unix_ms: 1234,
            by_type: Default::default(),
        };
        let generation = state.current_generation();
        state.store_if_generation_matches(
            generation,
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            index.clone(),
        );
        state.set_status_complete_for_project_if_generation_matches(
            generation,
            "fp1",
            Some("proj".to_string()),
        );
        state.store(
            Some("proj".to_string()),
            "fp1".to_string(),
            Vec::new(),
            index,
        );
        assert_eq!(
            state.status().cache_verification,
            CacheVerification::Checking
        );

        assert!(state.confirm_cache_verified(generation, "fp1"));

        let status = state.status();
        assert_eq!(status.cache_verification, CacheVerification::Verified);
        assert_eq!(status.phase, IndexingPhase::Complete);
        assert_eq!(status.project_id.as_deref(), Some("proj"));
        assert_eq!(status.indexed_defs, 1);
    }
}
