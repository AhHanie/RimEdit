use std::sync::{Arc, Mutex};
use time::OffsetDateTime;

use super::fingerprint::IndexedFileFingerprint;
use super::model::{DefIndex, IndexedSourceKind};
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
    /// Cheap, parse-free directory-listing pass computing scan statistics (locations, resolved
    /// Workshop items, selected load folders, discovered file count) before the slower
    /// read/parse phase -- see `def_index::discover_scan_stats`.
    Discovering,
    /// Reading and parsing the discovered Def XML files (and, internally, persisting the
    /// resulting index once every file has been attempted) -- see `builder::build_def_index_with_progress`.
    Indexing,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexingStatus {
    pub project_id: Option<String>,
    pub phase: IndexingPhase,
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
}

impl IndexingStatus {
    fn idle() -> Self {
        Self {
            project_id: None,
            phase: IndexingPhase::Idle,
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
        }
    }
}

fn now_ms() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000
}

fn complete_status(project_id: Option<String>, index: &DefIndex) -> IndexingStatus {
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
    }
}

struct DefIndexStateEntry {
    settings_fingerprint: String,
    file_fingerprints: Vec<IndexedFileFingerprint>,
    index: Arc<DefIndex>,
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
    /// Stores the index (wrapped in Arc) and returns the Arc so callers can reuse it.
    pub fn store(
        &self,
        settings_fingerprint: String,
        file_fingerprints: Vec<IndexedFileFingerprint>,
        index: DefIndex,
    ) -> Arc<DefIndex> {
        let arc = Arc::new(index);
        if let Ok(mut guard) = self.inner.lock() {
            guard.entry = Some(DefIndexStateEntry {
                settings_fingerprint,
                file_fingerprints,
                index: Arc::clone(&arc),
            });
        }
        arc
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

    /// Returns whatever index is cached regardless of fingerprints, for non-blocking query fallback.
    pub fn get_any_cached(&self) -> Option<Arc<DefIndex>> {
        let guard = self.inner.lock().ok()?;
        guard.entry.as_ref().map(|e| Arc::clone(&e.index))
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
            };
        }
    }

    pub fn set_status_running(&self, project_id: Option<String>, pending_files: usize) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Running,
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

    pub fn set_status_complete(&self, index: &DefIndex) {
        if let Ok(mut guard) = self.inner.lock() {
            let project_id = guard.status.project_id.clone();
            guard.status = complete_status(project_id, index);
        }
    }

    /// Same as `set_status_complete`, but explicitly assigns `project_id` instead of carrying
    /// it forward from the prior status. Used to report a hydrated disk-cache hit as completed
    /// indexing for the active project, since fresh state begins with `project_id: None`.
    pub fn set_status_complete_for_project(&self, project_id: Option<String>, index: &DefIndex) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = complete_status(project_id, index);
        }
    }

    pub fn set_status_failed(&self, project_id: Option<String>, message: String) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Failed,
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

        state.set_status_complete_for_project(Some("proj".to_string()), &index);

        let status = state.status();
        assert_eq!(status.phase, IndexingPhase::Complete);
        assert_eq!(status.project_id.as_deref(), Some("proj"));
        assert_eq!(status.indexed_defs, 3);
        assert_eq!(status.project_defs, 1);
        assert_eq!(status.source_defs, 2);
        assert_eq!(status.errors, 0);
        assert_eq!(status.pending_files, 0);
        assert_eq!(status.total_files, None);
        assert_eq!(status.processed_files, None);
        assert_eq!(status.current_stage, None);
        assert_eq!(status.current_location_name, None);
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
    }
}
