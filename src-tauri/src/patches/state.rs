use std::sync::{Arc, Mutex};

use super::fingerprint::IndexedPatchFileFingerprint;
use super::index::PatchIndex;
use super::model::PatchFile;

struct PatchIndexStateEntry {
    settings_fingerprint: String,
    file_fingerprints: Vec<IndexedPatchFileFingerprint>,
    custom_operations_fingerprint: String,
    index: Arc<PatchIndex>,
}

#[derive(Default)]
pub struct PatchIndexState {
    inner: Mutex<Option<PatchIndexStateEntry>>,
}

impl PatchIndexState {
    /// Stores the index (wrapped in Arc) and returns the Arc so callers can reuse it.
    pub fn store(
        &self,
        settings_fingerprint: String,
        file_fingerprints: Vec<IndexedPatchFileFingerprint>,
        custom_operations_fingerprint: String,
        index: PatchIndex,
    ) -> Arc<PatchIndex> {
        let arc = Arc::new(index);
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(PatchIndexStateEntry {
                settings_fingerprint,
                file_fingerprints,
                custom_operations_fingerprint,
                index: Arc::clone(&arc),
            });
        }
        arc
    }

    /// Returns the cached index only if the settings fingerprint, the per-file content
    /// fingerprints, *and* the resolved custom/built-in patch operation metadata fingerprint all
    /// match. Patches have no background file-watcher keeping this state in sync (unlike
    /// `DefIndexState`), so a settings-only match is not sufficient to prove the cached index
    /// reflects the current file contents on disk, and a schema-pack-only metadata change (e.g.
    /// a mod's custom operation metadata is added or edited) must also invalidate the cached
    /// `Custom`/`Unknown` classifications.
    pub fn get_if_fingerprints_match(
        &self,
        settings_fingerprint: &str,
        file_fingerprints: &[IndexedPatchFileFingerprint],
        custom_operations_fingerprint: &str,
    ) -> Option<Arc<PatchIndex>> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.as_ref()?;
        if entry.settings_fingerprint == settings_fingerprint
            && entry.file_fingerprints == file_fingerprints
            && entry.custom_operations_fingerprint == custom_operations_fingerprint
        {
            Some(Arc::clone(&entry.index))
        } else {
            None
        }
    }
}

struct PatchFilesStateEntry {
    settings_fingerprint: String,
    file_fingerprints: Vec<IndexedPatchFileFingerprint>,
    custom_operations_fingerprint: String,
    patch_files: Arc<Vec<PatchFile>>,
}

/// In-memory cache for the full parsed patch operation ASTs (`Vec<PatchFile>`, as returned
/// alongside the lightweight [`PatchIndex`] by `index::build_patch_index_with_files`) --
/// mirrors [`PatchIndexState`] exactly, but for the full ASTs the lightweight index doesn't
/// retain (value XML, attribute name/value, nested operation trees), which the patch preview
/// engine needs to actually apply operations rather than just classify them. Kept as a separate
/// struct rather than folded into `PatchIndexState` so callers that only need the lightweight
/// index (e.g. `query_patch_operations_for_def`) aren't affected by this cache at all.
#[derive(Default)]
pub struct PatchFilesState {
    inner: Mutex<Option<PatchFilesStateEntry>>,
}

impl PatchFilesState {
    /// Stores the parsed patch files (wrapped in Arc) and returns the Arc so callers can reuse
    /// it.
    pub fn store(
        &self,
        settings_fingerprint: String,
        file_fingerprints: Vec<IndexedPatchFileFingerprint>,
        custom_operations_fingerprint: String,
        patch_files: Vec<PatchFile>,
    ) -> Arc<Vec<PatchFile>> {
        let arc = Arc::new(patch_files);
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(PatchFilesStateEntry {
                settings_fingerprint,
                file_fingerprints,
                custom_operations_fingerprint,
                patch_files: Arc::clone(&arc),
            });
        }
        arc
    }

    /// Returns the cached patch files only if the settings fingerprint, the per-file content
    /// fingerprints, *and* the resolved custom/built-in patch operation metadata fingerprint all
    /// match -- same invalidation rule as [`PatchIndexState::get_if_fingerprints_match`], since
    /// this cache must stay in lockstep with the lightweight index it complements.
    pub fn get_if_fingerprints_match(
        &self,
        settings_fingerprint: &str,
        file_fingerprints: &[IndexedPatchFileFingerprint],
        custom_operations_fingerprint: &str,
    ) -> Option<Arc<Vec<PatchFile>>> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.as_ref()?;
        if entry.settings_fingerprint == settings_fingerprint
            && entry.file_fingerprints == file_fingerprints
            && entry.custom_operations_fingerprint == custom_operations_fingerprint
        {
            Some(Arc::clone(&entry.patch_files))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(relative_path: &str) -> IndexedPatchFileFingerprint {
        IndexedPatchFileFingerprint {
            location_id: "project".to_string(),
            relative_path: relative_path.to_string(),
            byte_len: 42,
            modified_unix_ms: Some(0),
            content_hash: "hash-a".to_string(),
        }
    }

    fn patch_file(relative_path: &str) -> PatchFile {
        PatchFile {
            relative_path: relative_path.to_string(),
            xml_declaration: None,
            operations: Vec::new(),
            diagnostics: Vec::new(),
            had_fatal_parse_error: false,
        }
    }

    #[test]
    fn returns_stored_patch_files_when_fingerprints_match() {
        let state = PatchFilesState::default();
        let fingerprints = vec![fingerprint("Patches/a.xml")];
        state.store(
            "settings-hash".to_string(),
            fingerprints.clone(),
            "custom-ops-hash".to_string(),
            vec![patch_file("Patches/a.xml")],
        );

        let hit =
            state.get_if_fingerprints_match("settings-hash", &fingerprints, "custom-ops-hash");
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().len(), 1);
    }

    #[test]
    fn returns_none_when_file_fingerprints_changed() {
        let state = PatchFilesState::default();
        state.store(
            "settings-hash".to_string(),
            vec![fingerprint("Patches/a.xml")],
            "custom-ops-hash".to_string(),
            vec![patch_file("Patches/a.xml")],
        );

        let changed_fingerprints = vec![fingerprint("Patches/b.xml")];
        let miss = state.get_if_fingerprints_match(
            "settings-hash",
            &changed_fingerprints,
            "custom-ops-hash",
        );
        assert!(miss.is_none());
    }

    #[test]
    fn returns_none_before_anything_is_stored() {
        let state = PatchFilesState::default();
        assert!(state
            .get_if_fingerprints_match("settings-hash", &[], "custom-ops-hash")
            .is_none());
    }
}
