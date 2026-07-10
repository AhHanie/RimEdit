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
        }
    }
}

fn now_ms() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000
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
            };
        }
    }

    pub fn set_status_complete(&self, index: &DefIndex) {
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
        if let Ok(mut guard) = self.inner.lock() {
            let project_id = guard.status.project_id.clone();
            guard.status = IndexingStatus {
                project_id,
                phase: IndexingPhase::Complete,
                pending_files: 0,
                indexed_defs: index.defs.len(),
                project_defs,
                source_defs,
                errors: index.errors.len(),
                message: None,
                updated_at_unix_ms: now_ms(),
            };
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
