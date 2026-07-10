use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use crate::def_index::{
    apply_file_change, settings_fingerprint, DefIndex, DefIndexBuildOptions, DefIndexState,
    IndexingStatus,
};
use crate::project_files::scan_indexable_def_xml_files;
use crate::project_model::AppError;
use crate::services::def_index_cache;
use crate::settings_store::load_settings;

use super::debounce::{DebouncedIndexEvents, PendingFileEvent};
use super::events::emit_indexing_status;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) enum IndexJobReason {
    InitialProjectOpen,
    SettingsChanged,
    WatcherEvent,
    SavedProjectFile,
    ProjectFileMutation,
    ManualRebuild,
}

#[derive(Debug)]
pub(super) enum IndexJob {
    FullRebuild {
        project_id: Option<String>,
        generation: u64,
        #[allow(dead_code)]
        reason: IndexJobReason,
    },
    FileChanged {
        location_id: String,
        relative_path: String,
        generation: u64,
        #[allow(dead_code)]
        reason: IndexJobReason,
    },
    FileDeleted {
        location_id: String,
        relative_path: String,
        generation: u64,
        #[allow(dead_code)]
        reason: IndexJobReason,
    },
    FolderDeleted {
        location_id: String,
        folder_prefix: String,
        generation: u64,
        #[allow(dead_code)]
        reason: IndexJobReason,
    },
}

#[derive(Debug)]
pub(crate) enum WatcherRawEvent {
    Changed {
        location_id: String,
        relative_path: String,
    },
    Deleted {
        location_id: String,
        relative_path: String,
    },
    FolderDeleted {
        location_id: String,
        folder_prefix: String,
    },
}

fn job_generation(job: &IndexJob) -> u64 {
    match job {
        IndexJob::FullRebuild { generation, .. } => *generation,
        IndexJob::FileChanged { generation, .. } => *generation,
        IndexJob::FileDeleted { generation, .. } => *generation,
        IndexJob::FolderDeleted { generation, .. } => *generation,
    }
}

// ---------------------------------------------------------------------------
// Managed state
// ---------------------------------------------------------------------------

pub(crate) struct IndexingServiceState {
    pub(super) sender: mpsc::UnboundedSender<IndexJob>,
    pub(super) watcher_sender: mpsc::UnboundedSender<WatcherRawEvent>,
    receiver: Mutex<Option<mpsc::UnboundedReceiver<IndexJob>>>,
    watcher_receiver: Mutex<Option<mpsc::UnboundedReceiver<WatcherRawEvent>>>,
}

impl IndexingServiceState {
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (wtx, wrx) = mpsc::unbounded_channel();
        Self {
            sender: tx,
            watcher_sender: wtx,
            receiver: Mutex::new(Some(rx)),
            watcher_receiver: Mutex::new(Some(wrx)),
        }
    }
}

// ---------------------------------------------------------------------------
// Public enqueue API
// ---------------------------------------------------------------------------

pub(crate) fn enqueue_full_rebuild(
    app: &AppHandle,
    project_id: Option<String>,
    reason: IndexJobReason,
) {
    let gen = app.state::<DefIndexState>().current_generation();
    let svc = app.state::<IndexingServiceState>();
    let _ = svc.sender.send(IndexJob::FullRebuild {
        project_id,
        generation: gen,
        reason,
    });
}

pub(crate) fn enqueue_file_change(
    app: &AppHandle,
    location_id: String,
    relative_path: String,
    reason: IndexJobReason,
) {
    let gen = app.state::<DefIndexState>().current_generation();
    let svc = app.state::<IndexingServiceState>();
    let _ = svc.sender.send(IndexJob::FileChanged {
        location_id,
        relative_path,
        generation: gen,
        reason,
    });
}

pub(crate) fn enqueue_file_delete(
    app: &AppHandle,
    location_id: String,
    relative_path: String,
    reason: IndexJobReason,
) {
    let gen = app.state::<DefIndexState>().current_generation();
    let svc = app.state::<IndexingServiceState>();
    let _ = svc.sender.send(IndexJob::FileDeleted {
        location_id,
        relative_path,
        generation: gen,
        reason,
    });
}

pub(crate) fn enqueue_folder_delete(
    app: &AppHandle,
    location_id: String,
    folder_prefix: String,
    reason: IndexJobReason,
) {
    let gen = app.state::<DefIndexState>().current_generation();
    let svc = app.state::<IndexingServiceState>();
    let _ = svc.sender.send(IndexJob::FolderDeleted {
        location_id,
        folder_prefix,
        generation: gen,
        reason,
    });
}

pub(crate) fn get_indexing_status(app: &AppHandle) -> IndexingStatus {
    app.state::<DefIndexState>().status()
}

pub(crate) fn start_worker(app: &AppHandle) -> Result<(), AppError> {
    let svc = app.state::<IndexingServiceState>();
    let job_rx = svc
        .receiver
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| AppError {
            code: "indexing_already_started".into(),
            message: "Indexing worker already started.".into(),
            details: None,
        })?;
    let watcher_rx = svc
        .watcher_receiver
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| AppError {
            code: "indexing_already_started".into(),
            message: "Indexing watcher channel already taken.".into(),
            details: None,
        })?;
    let app_clone = app.clone();
    tauri::async_runtime::spawn(run_worker(app_clone, job_rx, watcher_rx));
    Ok(())
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

/// Routes a watcher event: file events go through the debouncer; folder deletes are
/// returned as immediate jobs (directories can't be debounced by path).
fn route_watcher_event(
    app: &AppHandle,
    debouncer: &mut DebouncedIndexEvents,
    ev: WatcherRawEvent,
    at: Instant,
    immediate_jobs: &mut Vec<IndexJob>,
) {
    match ev {
        WatcherRawEvent::Changed {
            location_id,
            relative_path,
        } => {
            debouncer.record_change(location_id, relative_path, at);
        }
        WatcherRawEvent::Deleted {
            location_id,
            relative_path,
        } => {
            debouncer.record_delete(location_id, relative_path, at);
        }
        WatcherRawEvent::FolderDeleted {
            location_id,
            folder_prefix,
        } => {
            let gen = app.state::<DefIndexState>().current_generation();
            immediate_jobs.push(IndexJob::FolderDeleted {
                location_id,
                folder_prefix,
                generation: gen,
                reason: IndexJobReason::WatcherEvent,
            });
        }
    }
}

async fn run_worker(
    app: AppHandle,
    mut job_rx: mpsc::UnboundedReceiver<IndexJob>,
    mut watcher_rx: mpsc::UnboundedReceiver<WatcherRawEvent>,
) {
    let mut debouncer = DebouncedIndexEvents::new(Duration::from_millis(400));

    loop {
        let deadline = debouncer
            .next_deadline()
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600));

        tokio::select! {
            result = job_rx.recv() => {
                let Some(job) = result else { break };
                let mut batch = vec![job];
                while let Ok(more) = job_rx.try_recv() { batch.push(more); }
                let coalesced = coalesce_batch(&app, batch);
                if !coalesced.is_empty() {
                    process_batch(&app, coalesced).await;
                }
            }
            result = watcher_rx.recv() => {
                let Some(ev) = result else { break };
                let now = Instant::now();
                let mut immediate_folder_jobs: Vec<IndexJob> = Vec::new();
                route_watcher_event(&app, &mut debouncer, ev, now, &mut immediate_folder_jobs);
                while let Ok(more) = watcher_rx.try_recv() {
                    route_watcher_event(&app, &mut debouncer, more, now, &mut immediate_folder_jobs);
                }
                if !immediate_folder_jobs.is_empty() {
                    let coalesced = coalesce_batch(&app, immediate_folder_jobs);
                    if !coalesced.is_empty() {
                        process_batch(&app, coalesced).await;
                    }
                }
            }
            _ = tokio::time::sleep_until(deadline.into()) => {
                let ready = debouncer.drain_ready(Instant::now());
                if !ready.is_empty() {
                    let gen = app.state::<DefIndexState>().current_generation();
                    let batch: Vec<IndexJob> = ready
                        .into_iter()
                        .map(|(loc, path, ev)| match ev {
                            PendingFileEvent::Changed => IndexJob::FileChanged {
                                location_id: loc,
                                relative_path: path,
                                generation: gen,
                                reason: IndexJobReason::WatcherEvent,
                            },
                            PendingFileEvent::Deleted => IndexJob::FileDeleted {
                                location_id: loc,
                                relative_path: path,
                                generation: gen,
                                reason: IndexJobReason::WatcherEvent,
                            },
                        })
                        .collect();
                    let coalesced = coalesce_batch(&app, batch);
                    if !coalesced.is_empty() {
                        process_batch(&app, coalesced).await;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Batch coalescing
// ---------------------------------------------------------------------------

fn coalesce_batch(app: &AppHandle, batch: Vec<IndexJob>) -> Vec<IndexJob> {
    let current_gen = app.state::<DefIndexState>().current_generation();
    let batch: Vec<IndexJob> = batch
        .into_iter()
        .filter(|j| job_generation(j) == current_gen)
        .collect();

    if batch.is_empty() {
        return Vec::new();
    }

    // If any FullRebuild is present, return only one FullRebuild
    let rebuild = batch.iter().find_map(|j| {
        if let IndexJob::FullRebuild {
            project_id,
            generation,
            ..
        } = j
        {
            Some((project_id.clone(), *generation))
        } else {
            None
        }
    });
    if let Some((project_id, generation)) = rebuild {
        return vec![IndexJob::FullRebuild {
            project_id,
            generation,
            reason: IndexJobReason::ManualRebuild,
        }];
    }

    // Coalesce file jobs: map (location_id, path) -> most severe event
    use std::collections::HashMap;
    let mut file_jobs: HashMap<(String, String), IndexJob> = HashMap::new();
    for job in batch {
        match &job {
            IndexJob::FileChanged {
                location_id,
                relative_path,
                ..
            } => {
                let key = (location_id.clone(), relative_path.clone());
                // Only insert Changed if no Deleted/FolderDeleted already present
                file_jobs.entry(key).or_insert(job);
            }
            IndexJob::FileDeleted {
                location_id,
                relative_path,
                ..
            } => {
                let key = (location_id.clone(), relative_path.clone());
                file_jobs.insert(key, job);
            }
            IndexJob::FolderDeleted {
                location_id,
                folder_prefix,
                generation,
                ..
            } => {
                let loc = location_id.clone();
                let prefix = folder_prefix.clone();
                let prefix_slash = format!("{}/", prefix);
                // Remove any file jobs under this folder prefix
                file_jobs.retain(|(l, p), _| {
                    l != &loc || (!p.starts_with(&prefix_slash) && p != &prefix)
                });
                let key = (format!("__folder__{}", loc), prefix.clone());
                file_jobs.insert(
                    key,
                    IndexJob::FolderDeleted {
                        location_id: loc,
                        folder_prefix: prefix,
                        generation: *generation,
                        reason: IndexJobReason::ManualRebuild,
                    },
                );
            }
            IndexJob::FullRebuild { .. } => unreachable!(),
        }
    }
    file_jobs.into_values().collect()
}

// ---------------------------------------------------------------------------
// Job execution
// ---------------------------------------------------------------------------

async fn process_batch(app: &AppHandle, batch: Vec<IndexJob>) {
    if batch.is_empty() {
        return;
    }

    // Determine if this is a full rebuild
    if let Some(IndexJob::FullRebuild { project_id, .. }) = batch.first() {
        execute_full_rebuild(app, project_id.clone()).await;
        return;
    }

    execute_file_jobs(app, batch).await;
}

async fn execute_full_rebuild(app: &AppHandle, project_id: Option<String>) {
    let _span = crate::instrumentation::span_with_tags(
        app,
        "indexing.executeFullRebuild",
        [(
            "projectPresent".to_string(),
            project_id.is_some().to_string(),
        )],
    );
    let state = app.state::<DefIndexState>();
    let current_gen = state.current_generation();

    state.set_status_pending(project_id.clone(), 0);
    emit_indexing_status(app, &state.status());

    let settings = match load_settings(app) {
        Ok(s) => s,
        Err(e) => {
            state.set_status_failed(project_id, e.message);
            emit_indexing_status(app, &state.status());
            return;
        }
    };

    state.set_status_running(project_id.clone(), 0);
    emit_indexing_status(app, &state.status());

    // Check generation hasn't changed while we set up
    if state.current_generation() != current_gen {
        return;
    }

    match def_index_cache::rebuild_for_project(app, &settings, project_id.as_deref()) {
        Ok(_summary) => {
            // The index was stored by rebuild_for_project; read it back via get_if_settings_match
            let options = DefIndexBuildOptions {
                project_id: project_id
                    .as_deref()
                    .or(settings.active_project_id.as_deref()),
                include_sources: true,
                replacement: None,
                force_rebuild: false,
            };
            let fp = settings_fingerprint(&settings, &options);
            if let Some(index) = state.get_if_settings_match(&fp) {
                state.set_status_complete(&index);
            } else {
                // Fallback: build a minimal status from the summary
                state.set_status_complete(&DefIndex::default());
            }
            emit_indexing_status(app, &state.status());
        }
        Err(e) => {
            state.set_status_failed(project_id, e.message);
            emit_indexing_status(app, &state.status());
        }
    }
}

async fn execute_file_jobs(app: &AppHandle, batch: Vec<IndexJob>) {
    let _span = crate::instrumentation::span_with_tags(
        app,
        "indexing.executeFileJobs",
        [("batchSize".to_string(), batch.len().to_string())],
    );
    let state = app.state::<DefIndexState>();
    let current_gen = state.current_generation();

    let settings = match load_settings(app) {
        Ok(s) => s,
        Err(_) => {
            // No settings: nothing to do
            return;
        }
    };

    let options = DefIndexBuildOptions {
        project_id: settings.active_project_id.as_deref(),
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    let fp = settings_fingerprint(&settings, &options);

    let mut index = match state.get_if_settings_match(&fp) {
        Some(arc) => (*arc).clone(),
        None => {
            // No usable base index: escalate to full rebuild
            execute_full_rebuild(app, settings.active_project_id.clone()).await;
            return;
        }
    };

    state.set_status_running(settings.active_project_id.clone(), batch.len());
    emit_indexing_status(app, &state.status());

    for job in batch {
        // Check generation hasn't changed
        if state.current_generation() != current_gen {
            return;
        }
        match job {
            IndexJob::FileChanged {
                location_id,
                relative_path,
                ..
            } => {
                // A change to LoadFolders.xml alters which files are active - force a full rebuild.
                let basename = std::path::Path::new(&relative_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if basename.eq_ignore_ascii_case("LoadFolders.xml") {
                    execute_full_rebuild(app, settings.active_project_id.clone()).await;
                    return;
                }

                if let Some(location) = settings.locations.iter().find(|l| l.id == location_id) {
                    // Only apply the change if the path is currently in the active load set.
                    let norm_changed = relative_path.replace('\\', "/");
                    let is_active = scan_indexable_def_xml_files(&settings, location)
                        .map(|scan| {
                            scan.files
                                .iter()
                                .any(|f| f.relative_path.replace('\\', "/") == norm_changed)
                        })
                        .unwrap_or(false);

                    if is_active {
                        let full_path =
                            std::path::PathBuf::from(&location.root_path).join(&relative_path);
                        match std::fs::read_to_string(&full_path) {
                            Ok(raw) => {
                                apply_file_change(&mut index, location, &relative_path, &raw)
                            }
                            Err(_) => {
                                index.remove_file(&location_id, &relative_path);
                                index.mark_rebuilt_now();
                            }
                        }
                    } else {
                        // File is outside the active load set - remove stale entries.
                        index.remove_file(&location_id, &relative_path);
                        index.mark_rebuilt_now();
                    }
                }
            }
            IndexJob::FileDeleted {
                location_id,
                relative_path,
                ..
            } => {
                index.remove_file(&location_id, &relative_path);
                index.mark_rebuilt_now();
            }
            IndexJob::FolderDeleted {
                location_id,
                folder_prefix,
                ..
            } => {
                index.remove_folder_prefix(&location_id, &folder_prefix);
                index.mark_rebuilt_now();
            }
            IndexJob::FullRebuild { .. } => unreachable!(),
        }
    }

    index.rebuild_computed_fields();
    state.set_status_complete(&index);
    emit_indexing_status(app, &state.status());
    def_index_cache::persist_incremental(
        app,
        &settings,
        settings.active_project_id.as_deref(),
        index,
    );
}

// ---------------------------------------------------------------------------
// Coalescing tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn file_changed(loc: &str, path: &str, gen: u64) -> IndexJob {
        IndexJob::FileChanged {
            location_id: loc.into(),
            relative_path: path.into(),
            generation: gen,
            reason: IndexJobReason::WatcherEvent,
        }
    }
    fn file_deleted(loc: &str, path: &str, gen: u64) -> IndexJob {
        IndexJob::FileDeleted {
            location_id: loc.into(),
            relative_path: path.into(),
            generation: gen,
            reason: IndexJobReason::WatcherEvent,
        }
    }
    fn full_rebuild(gen: u64) -> IndexJob {
        IndexJob::FullRebuild {
            project_id: None,
            generation: gen,
            reason: IndexJobReason::ManualRebuild,
        }
    }
    fn folder_deleted(loc: &str, prefix: &str, gen: u64) -> IndexJob {
        IndexJob::FolderDeleted {
            location_id: loc.into(),
            folder_prefix: prefix.into(),
            generation: gen,
            reason: IndexJobReason::ProjectFileMutation,
        }
    }

    fn coalesce_with_gen(batch: Vec<IndexJob>, current_gen: u64) -> Vec<IndexJob> {
        // Simplified coalesce that doesn't need AppHandle - replicate logic inline
        let batch: Vec<IndexJob> = batch
            .into_iter()
            .filter(|j| job_generation(j) == current_gen)
            .collect();
        if batch.is_empty() {
            return Vec::new();
        }
        let rebuild = batch.iter().find_map(|j| {
            if let IndexJob::FullRebuild {
                project_id,
                generation,
                ..
            } = j
            {
                Some((project_id.clone(), *generation))
            } else {
                None
            }
        });
        if let Some((project_id, generation)) = rebuild {
            return vec![IndexJob::FullRebuild {
                project_id,
                generation,
                reason: IndexJobReason::ManualRebuild,
            }];
        }
        use std::collections::HashMap;
        let mut file_jobs: HashMap<(String, String), IndexJob> = HashMap::new();
        for job in batch {
            match &job {
                IndexJob::FileChanged {
                    location_id,
                    relative_path,
                    ..
                } => {
                    let key = (location_id.clone(), relative_path.clone());
                    file_jobs.entry(key).or_insert(job);
                }
                IndexJob::FileDeleted {
                    location_id,
                    relative_path,
                    ..
                } => {
                    let key = (location_id.clone(), relative_path.clone());
                    file_jobs.insert(key, job);
                }
                IndexJob::FolderDeleted {
                    location_id,
                    folder_prefix,
                    generation,
                    ..
                } => {
                    let loc = location_id.clone();
                    let prefix = folder_prefix.clone();
                    let prefix_slash = format!("{}/", prefix);
                    file_jobs.retain(|(l, p), _| {
                        l != &loc || (!p.starts_with(&prefix_slash) && p != &prefix)
                    });
                    let key = (format!("__folder__{}", loc), prefix.clone());
                    file_jobs.insert(
                        key,
                        IndexJob::FolderDeleted {
                            location_id: loc,
                            folder_prefix: prefix,
                            generation: *generation,
                            reason: IndexJobReason::ManualRebuild,
                        },
                    );
                }
                IndexJob::FullRebuild { .. } => unreachable!(),
            }
        }
        file_jobs.into_values().collect()
    }

    #[test]
    fn stale_generation_jobs_are_dropped() {
        let batch = vec![
            file_changed("loc", "a.xml", 1),
            file_changed("loc", "b.xml", 1),
        ];
        let result = coalesce_with_gen(batch, 2); // current gen is 2, jobs have gen 1
        assert!(result.is_empty());
    }

    #[test]
    fn full_rebuild_supersedes_file_jobs() {
        let batch = vec![
            file_changed("loc", "a.xml", 1),
            full_rebuild(1),
            file_deleted("loc", "b.xml", 1),
        ];
        let result = coalesce_with_gen(batch, 1);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], IndexJob::FullRebuild { .. }));
    }

    #[test]
    fn multiple_changes_to_same_file_collapse() {
        let batch = vec![
            file_changed("loc", "a.xml", 1),
            file_changed("loc", "a.xml", 1),
            file_changed("loc", "a.xml", 1),
        ];
        let result = coalesce_with_gen(batch, 1);
        let changed: Vec<_> = result
            .iter()
            .filter(|j| matches!(j, IndexJob::FileChanged { .. }))
            .collect();
        assert_eq!(changed.len(), 1);
    }

    #[test]
    fn delete_wins_over_change_in_batch() {
        let batch = vec![
            file_changed("loc", "a.xml", 1),
            file_deleted("loc", "a.xml", 1),
        ];
        let result = coalesce_with_gen(batch, 1);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], IndexJob::FileDeleted { .. }));
    }

    #[test]
    fn folder_delete_removes_child_file_jobs() {
        let batch = vec![
            file_changed("loc", "Defs/Weapons/a.xml", 1),
            file_changed("loc", "Defs/Other/b.xml", 1),
            folder_deleted("loc", "Defs/Weapons", 1),
        ];
        let result = coalesce_with_gen(batch, 1);
        // Should have: FolderDeleted for Weapons, FileChanged for Other/b.xml
        assert_eq!(result.len(), 2);
        let has_folder = result
            .iter()
            .any(|j| matches!(j, IndexJob::FolderDeleted { folder_prefix, .. } if folder_prefix == "Defs/Weapons"));
        let has_other = result
            .iter()
            .any(|j| matches!(j, IndexJob::FileChanged { relative_path, .. } if relative_path == "Defs/Other/b.xml"));
        assert!(has_folder, "expected FolderDeleted for Defs/Weapons");
        assert!(has_other, "expected FileChanged for Defs/Other/b.xml");
    }
}
