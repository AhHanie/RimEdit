use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use crate::def_index::{
    apply_file_change, settings_fingerprint, DefIndexBuildOptions, DefIndexState,
    FingerprintVerification, IndexedFileFingerprint, IndexingStatus,
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
    /// A background `VerifyCache` job found a fingerprint mismatch (or scan failure) and handed
    /// off to a full rebuild.
    CacheVerification,
}

/// Identifies one in-flight cache-hydration/verification initialization -- see
/// `IndexingServiceState::try_schedule_init`/`finish_init`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct InitKey {
    pub(crate) project_id: Option<String>,
    pub(crate) generation: u64,
    pub(crate) settings_fingerprint: String,
}

#[derive(Debug)]
pub(super) enum IndexJob {
    FullRebuild {
        project_id: Option<String>,
        generation: u64,
        #[allow(dead_code)]
        reason: IndexJobReason,
    },
    /// Verifies a trusted (unverified) hydrated cache's stored fingerprints against the current
    /// collection in the background -- see `execute_verify_cache`. A confirmed match flips the
    /// matching `DefIndexState` entry's status to `CacheVerification::Verified`; a confirmed
    /// mismatch hands off to exactly one `FullRebuild` for the same project/generation. A scan
    /// failure (as opposed to a confirmed mismatch) retries up to `MAX_VERIFY_ATTEMPTS` times
    /// (`attempt` tracks how many scans have already been attempted for this entry) before falling
    /// back to the same `FullRebuild` hand-off as a confirmed mismatch.
    VerifyCache {
        project_id: Option<String>,
        generation: u64,
        settings_fingerprint: String,
        attempt: u8,
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
    /// A direct child of a `SteamWorkshop` collection root was created, removed, or renamed --
    /// i.e. the set of subscribed Workshop items changed. Always routed to a full rebuild (see
    /// `watchers::is_workshop_collection_membership_change`).
    CollectionMembershipChanged {
        #[allow(dead_code)]
        location_id: String,
    },
}

fn job_generation(job: &IndexJob) -> u64 {
    match job {
        IndexJob::FullRebuild { generation, .. } => *generation,
        IndexJob::VerifyCache { generation, .. } => *generation,
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
    /// Single-flight tracking for cache-hydration/verification initialization -- see
    /// `try_schedule_init`/`finish_init`. Distinct from the job queue itself: this dedupes the
    /// *decision* to hydrate/verify/rebuild across concurrent callers (setup, the frontend's
    /// `start_background_indexing`, and interactive editor loads), while the worker's own
    /// generation/coalescing rules remain the sole authority over which jobs actually run.
    in_flight_init: Mutex<HashSet<InitKey>>,
    /// Notified every time the worker publishes a terminal (`Complete`/`Failed`) status for a
    /// `FullRebuild`, `VerifyCache`, or file-change job -- see `notify_job_completed`. Lets
    /// `IndexLoadPolicy::RequireFresh` callers (`def_index_cache::load_for_project`) wait for the
    /// worker's own in-flight scan/rebuild to finish instead of always launching a second,
    /// independent one for the same collection, per Plan.md section 5: "If a full rebuild is
    /// already in flight, await the coordinated result rather than launching a second build. Add
    /// a completion notification/oneshot in the indexing coordinator if necessary; do not poll or
    /// busy-wait." A single shared `Notify` (not scoped per project/generation) is deliberately
    /// coarse: a waiter always re-checks the actual condition it cares about
    /// (`get_verified_if_settings_match`) after being woken, so an irrelevant job's completion
    /// merely costs one extra wasted wakeup+recheck, never a correctness issue -- and avoids the
    /// complexity of a keyed-notifier registry for what is, in practice, always at most one
    /// worker job in flight at a time.
    job_completed: tokio::sync::Notify,
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
            in_flight_init: Mutex::new(HashSet::new()),
            job_completed: tokio::sync::Notify::new(),
        }
    }

    /// Wakes every current waiter registered via `job_completed_notified` -- called after every
    /// terminal (`Complete`/`Failed`) status publish in `execute_full_rebuild`, `execute_verify_cache`
    /// (both the `Match`/`confirm_cache_verified` success path and `invalidate_entry_and_rebuild`'s
    /// hand-off), and `execute_file_jobs`.
    pub(crate) fn notify_job_completed(&self) {
        self.job_completed.notify_waiters();
    }

    /// Returns a future that resolves the next time `notify_job_completed` is called. Callers
    /// MUST obtain this *before* re-checking whatever condition they're waiting on (the standard
    /// `tokio::sync::Notify` pattern) -- registering interest first means a `notify_job_completed`
    /// call that races in between the check and the `.await` is still observed, instead of being
    /// silently missed.
    pub(crate) fn job_completed_notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.job_completed.notified()
    }

    /// Registers `key` as in-flight and returns `true` if this call is the one that registered
    /// it (the caller should proceed with hydration/verification/rebuild), or `false` if a
    /// matching initialization was already in flight (the caller should skip and let the
    /// existing one finish). Callers MUST call `finish_init` with the same key on every terminal
    /// path once `true` is returned, or the key leaks and blocks all future initialization for
    /// that `{project_id, generation, settings_fingerprint}`.
    pub(crate) fn try_schedule_init(&self, key: InitKey) -> bool {
        self.in_flight_init
            .lock()
            .map(|mut guard| guard.insert(key))
            .unwrap_or(true)
    }

    pub(crate) fn finish_init(&self, key: &InitKey) {
        if let Ok(mut guard) = self.in_flight_init.lock() {
            guard.remove(key);
        }
    }

    #[cfg(test)]
    pub(crate) fn has_in_flight_init(&self, key: &InitKey) -> bool {
        self.in_flight_init
            .lock()
            .map(|guard| guard.contains(key))
            .unwrap_or(false)
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

/// Same as `enqueue_full_rebuild`, but tags the job with `expected_generation` (the caller's
/// captured value) instead of reading the *current* generation fresh -- and skips enqueueing
/// entirely if the current generation has already moved past it. Reading the generation fresh at
/// enqueue time is safe for a caller acting on the current, live state (e.g. a watcher event just
/// received, or a fresh interactive miss), but a caller whose `project_id` was captured earlier
/// (e.g. a background `VerifyCache` job that ran for a while) must not have that stale
/// `project_id` silently "laundered" into a job tagged with whatever generation happens to be
/// current by the time it enqueues -- that would defeat the worker's own generation-based
/// coalescing, which is exactly what dropped/superseded work is supposed to rely on. Returns
/// whether the job was actually enqueued.
pub(crate) fn enqueue_full_rebuild_if_generation_matches(
    app: &AppHandle,
    expected_generation: u64,
    project_id: Option<String>,
    reason: IndexJobReason,
) -> bool {
    let state = app.state::<DefIndexState>();
    if state.current_generation() != expected_generation {
        return false;
    }
    let svc = app.state::<IndexingServiceState>();
    let _ = svc.sender.send(IndexJob::FullRebuild {
        project_id,
        generation: expected_generation,
        reason,
    });
    true
}

/// Schedules a background fingerprint check for a trusted (unverified) hydrated cache. `project_id`
/// and `settings_fingerprint` identify which `DefIndexState` entry to verify; `expected_generation`
/// is the caller's own already-captured generation (not read fresh here), mirroring
/// `enqueue_full_rebuild_if_generation_matches` -- so a generation bump landing between the
/// caller's hydration and this call can't get laundered into a job tagged with whatever generation
/// happens to be current by the time it enqueues, which would defeat `execute_verify_cache`'s own
/// generation recheck (the entire point of tagging jobs with a captured generation in the first
/// place). `attempt` is `0` for a fresh hydration-triggered verification; `execute_verify_cache`
/// passes a higher value when retrying after a `ScanFailed` result.
pub(crate) fn enqueue_cache_verification(
    app: &AppHandle,
    expected_generation: u64,
    project_id: Option<String>,
    settings_fingerprint: String,
    attempt: u8,
) {
    let svc = app.state::<IndexingServiceState>();
    let _ = svc.sender.send(IndexJob::VerifyCache {
        project_id,
        generation: expected_generation,
        settings_fingerprint,
        attempt,
        reason: IndexJobReason::CacheVerification,
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
            args: crate::diagnostics::DiagnosticArgs::new(),
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
            args: crate::diagnostics::DiagnosticArgs::new(),
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
        WatcherRawEvent::CollectionMembershipChanged { .. } => {
            let gen = app.state::<DefIndexState>().current_generation();
            immediate_jobs.push(IndexJob::FullRebuild {
                project_id: None,
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
    coalesce_batch_for_generation(batch, current_gen)
}

/// The actual coalescing logic, parameterized on the current generation instead of reading it
/// from `AppHandle` -- shared verbatim between `coalesce_batch` (production) and this module's
/// tests, which have no `AppHandle` to construct. Keeping this as the single implementation (not
/// a second copy in `#[cfg(test)] mod tests`) means a future change to the real coalescing rules
/// can't silently stop being reflected in what the tests exercise.
fn coalesce_batch_for_generation(batch: Vec<IndexJob>, current_gen: u64) -> Vec<IndexJob> {
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

    // Coalesce file jobs: map (location_id, path) -> most severe event. Verification jobs are
    // deduped separately by (project_id, settings_fingerprint) -- they don't interact with file
    // paths at all.
    use std::collections::HashMap;
    let mut verify_jobs: HashMap<(Option<String>, String), IndexJob> = HashMap::new();
    let mut file_jobs: HashMap<(String, String), IndexJob> = HashMap::new();
    for job in batch {
        match &job {
            IndexJob::VerifyCache {
                project_id,
                settings_fingerprint,
                ..
            } => {
                let key = (project_id.clone(), settings_fingerprint.clone());
                verify_jobs.insert(key, job);
            }
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
    verify_jobs
        .into_values()
        .chain(file_jobs.into_values())
        .collect()
}

// ---------------------------------------------------------------------------
// Job execution
// ---------------------------------------------------------------------------

async fn process_batch(app: &AppHandle, batch: Vec<IndexJob>) {
    if batch.is_empty() {
        return;
    }

    // Determine if this is a full rebuild -- coalescing guarantees a batch containing any
    // FullRebuild is exactly `[FullRebuild]`.
    if let Some(IndexJob::FullRebuild { project_id, .. }) = batch.first() {
        execute_full_rebuild(app, project_id.clone()).await;
        return;
    }

    // Otherwise the batch is a mix of (deduped) VerifyCache jobs and file-change jobs.
    // Verifications run first and independently; they never touch `execute_file_jobs`'s base
    // index, so ordering between the two doesn't matter for correctness.
    let mut file_jobs = Vec::new();
    for job in batch {
        match job {
            IndexJob::VerifyCache {
                project_id,
                generation,
                settings_fingerprint,
                attempt,
                ..
            } => {
                execute_verify_cache(app, project_id, generation, settings_fingerprint, attempt)
                    .await;
            }
            other => file_jobs.push(other),
        }
    }
    if !file_jobs.is_empty() {
        execute_file_jobs(app, file_jobs).await;
    }
}

/// Verifies a trusted (unverified) hydrated cache's stored fingerprints against the current
/// collection on a blocking thread, since this walks and hashes every indexed file. On a match,
/// flips the matching `DefIndexState` entry to `CacheVerification::Verified` in place. On a
/// confirmed mismatch, invalidates the entry and enqueues exactly one `FullRebuild` for the same
/// project/generation (never rebuilds directly), so the worker's normal generation/coalescing
/// rules stay authoritative. A scan failure (as opposed to a confirmed mismatch) is treated as
/// inconclusive rather than evidence of staleness, and retries up to `MAX_VERIFY_ATTEMPTS` times
/// before falling back to the same mismatch handling -- see the `ScanFailed` match arm. Re-checks
/// the captured generation and that `DefIndexState` still holds an entry matching
/// `settings_fingerprint_expected` both before and after the blocking scan, so a settings/project
/// change or a newer watcher-driven update can't be overwritten by a stale verification result.
/// A `ScanFailed` result retries this many times (attempts `0..MAX_VERIFY_ATTEMPTS - 1`) before
/// being treated as a confirmed `Mismatch` -- see the `ScanFailed` match arm in
/// `execute_verify_cache`.
const MAX_VERIFY_ATTEMPTS: u8 = 3;

/// Delay before a `ScanFailed` retry -- gives a transient condition (a momentarily locked file, a
/// Steam Workshop item briefly unmounting) a moment to actually clear, instead of exhausting every
/// attempt back-to-back within the same instant.
const VERIFY_RETRY_BACKOFF: Duration = Duration::from_secs(2);

async fn execute_verify_cache(
    app: &AppHandle,
    project_id: Option<String>,
    captured_generation: u64,
    settings_fingerprint_expected: String,
    attempt: u8,
) {
    let _span = crate::instrumentation::span_with_tags(
        app,
        "indexing.verifyCache",
        [(
            "projectPresent".to_string(),
            project_id.is_some().to_string(),
        )],
    );
    // No `IndexingServiceState`/`InitKey` bookkeeping here: `schedule_initialization`
    // already calls `finish_init` for this exact key synchronously, right after enqueuing this
    // job -- by the time this job runs (asynchronously, off the worker queue), the key is already
    // released. That's safe because a concurrent caller reaching `schedule_initialization`
    // in the meantime sees the just-hydrated `DefIndexState` entry via its own
    // `get_if_settings_match` check and short-circuits to `HydratedHit` before it would even
    // consult the single-flight map, so holding the lock across this job would add nothing.
    let state = app.state::<DefIndexState>();

    if state.current_generation() != captured_generation {
        return;
    }

    let Some(expected_fingerprints) =
        state.get_file_fingerprints_if_settings_match(&settings_fingerprint_expected)
    else {
        // The hydrated entry this verification was scheduled for is no longer current (e.g. an
        // incremental watcher update or a settings change already replaced it) -- nothing to
        // verify against.
        return;
    };
    // Wrapped in an `Arc` so the (potentially large, for a Steam Workshop-sized collection) list
    // can be handed into the `spawn_blocking` closure below with a cheap refcount bump instead of a
    // second full `Vec` clone, while the original is still needed afterward (for
    // `invalidate_entry_and_rebuild`'s own comparison against the entry's *current* fingerprints).
    let expected_fingerprints = Arc::new(expected_fingerprints);

    let settings = match load_settings(app) {
        Ok(s) => s,
        Err(_) => {
            // Treated identically to `FingerprintVerification::ScanFailed` (retry with backoff,
            // then fall back to invalidate-and-rebuild), not a silent bail-out: a transient
            // settings-read failure (e.g. Windows briefly locking settings.json during a
            // concurrent `save_settings` rename) says nothing about whether the collection
            // actually changed, but a bare `return` here would strand this entry permanently
            // unverified -- `should_attempt_initialization` never re-schedules verification for a
            // `settings_fingerprint` that already has *any* matching entry, hydrated or not, so
            // nothing else would ever retry it. Every `IndexLoadPolicy::RequireFresh` caller would
            // then keep missing `get_verified_if_settings_match` and redo a full project-wide scan
            // on every save/validation for the rest of the session, defeating the fast-cached-
            // startup goal this diff exists for, over a hiccup that would likely succeed on retry.
            handle_verify_scan_failure(
                app,
                &state,
                VerifyFailureContext {
                    project_id,
                    captured_generation,
                    settings_fingerprint_expected,
                    expected_fingerprints: Arc::clone(&expected_fingerprints),
                    attempt,
                },
                "settings unreadable",
            )
            .await;
            return;
        }
    };

    let blocking_settings = settings.clone();
    let blocking_project_id = project_id.clone();
    let blocking_expected = Arc::clone(&expected_fingerprints);
    let verification = tauri::async_runtime::spawn_blocking(move || {
        def_index_cache::verify_hydrated_project(
            &blocking_settings,
            blocking_project_id.as_deref(),
            &blocking_expected,
        )
    })
    .await
    .unwrap_or(FingerprintVerification::ScanFailed);

    if state.current_generation() != captured_generation
        || !state.has_entry_with_settings_fingerprint(&settings_fingerprint_expected)
    {
        return;
    }

    match verification {
        FingerprintVerification::Match => {
            // Atomic re-check-and-write, not the plain `set_cache_verified()`: the generation/
            // fingerprint recheck just above and this write are still two separate lock
            // acquisitions, so a concurrent writer (a newer `schedule_initialization`, or a
            // watcher-driven `persist_incremental`) could replace `status`/`entry` in between --
            // `confirm_cache_verified` re-verifies both under the same lock as the write, so a
            // losing race is a no-op instead of stamping `Verified` onto unrelated newer state.
            if state.confirm_cache_verified(captured_generation, &settings_fingerprint_expected) {
                emit_indexing_status(app, &state.status());
                app.state::<IndexingServiceState>().notify_job_completed();
            }
        }
        FingerprintVerification::Mismatch => {
            invalidate_entry_and_rebuild(
                app,
                &state,
                project_id,
                captured_generation,
                &settings_fingerprint_expected,
                &expected_fingerprints,
            )
            .await;
        }
        FingerprintVerification::ScanFailed => {
            // A scan failure (e.g. a momentarily locked file, a Steam Workshop item briefly
            // unmounting) says nothing about whether the collection actually changed, unlike a
            // `Mismatch` -- retry a bounded number of times before drawing any conclusion.
            // Escalating to a full project-wide rebuild (or even invalidating the still-usable
            // cached entry) on the very first transient I/O hiccup would be exactly the wasteful
            // cost this cache exists to avoid. Shared with the `load_settings` failure path above,
            // which is the same class of transient, inconclusive I/O failure.
            handle_verify_scan_failure(
                app,
                &state,
                VerifyFailureContext {
                    project_id,
                    captured_generation,
                    settings_fingerprint_expected,
                    expected_fingerprints,
                    attempt,
                },
                "scan failed",
            )
            .await;
        }
    }
}

/// Bundles `handle_verify_scan_failure`'s per-call context, so the function itself doesn't exceed
/// clippy's argument-count lint -- these five values are always threaded through together from
/// both of `execute_verify_cache`'s call sites (the `load_settings` failure path and the
/// `ScanFailed` match arm).
struct VerifyFailureContext {
    project_id: Option<String>,
    captured_generation: u64,
    settings_fingerprint_expected: String,
    expected_fingerprints: Arc<Vec<IndexedFileFingerprint>>,
    attempt: u8,
}

/// Shared by `execute_verify_cache`'s `ScanFailed` verification result and its `load_settings`
/// failure path -- both are transient, inconclusive I/O failures, not evidence the collection
/// actually changed. Retries with backoff up to `MAX_VERIFY_ATTEMPTS` times; once exhausted, falls
/// back to the same handling as a confirmed `Mismatch` rather than leaving the entry permanently
/// unverified (per `invalidate_entry_and_rebuild`'s doc comment, nothing else would ever retry it).
async fn handle_verify_scan_failure(
    app: &AppHandle,
    state: &DefIndexState,
    ctx: VerifyFailureContext,
    reason: &str,
) {
    let VerifyFailureContext {
        project_id,
        captured_generation,
        settings_fingerprint_expected,
        expected_fingerprints,
        attempt,
    } = ctx;
    if attempt + 1 < MAX_VERIFY_ATTEMPTS {
        eprintln!(
            "[rimedit] Background cache verification failed ({}, attempt {}/{}); retrying: project_present={}",
            reason,
            attempt + 1,
            MAX_VERIFY_ATTEMPTS,
            project_id.is_some()
        );
        // A short backoff before re-enqueueing: the whole premise of retrying (rather than
        // treating this as a confirmed mismatch immediately) is that the failure is transient and
        // needs a moment to clear -- retrying instantly, back-to-back on the very next worker
        // batch, gives a condition lasting even a fraction of a second no chance to resolve before
        // every attempt is already exhausted.
        //
        // Spawned as its own detached task rather than `.await`ed inline: this function runs
        // directly inside `run_worker`'s single select loop (via `process_batch`/
        // `execute_verify_cache`), so sleeping in place here would block that one worker task from
        // dequeuing *any* other job -- file changes, other verify jobs, full rebuilds -- for the
        // whole backoff. A `VerifyCache` retry is exactly the kind of low-priority background work
        // that must never hold up higher-priority work like a user's own save landing in the index.
        let retry_app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(VERIFY_RETRY_BACKOFF).await;
            let retry_state = retry_app.state::<DefIndexState>();
            if retry_state.current_generation() == captured_generation {
                enqueue_cache_verification(
                    &retry_app,
                    captured_generation,
                    project_id,
                    settings_fingerprint_expected,
                    attempt + 1,
                );
            }
        });
    } else {
        // Repeated failures stop looking like a transient hiccup and start looking like something
        // genuinely wrong (e.g. a Workshop item that isn't coming back, or a persistently
        // unreadable settings file) -- at that point, leaving the entry unverified forever (with
        // no other trigger that will ever retry it, per `should_attempt_initialization` treating
        // any settings-matching entry as "nothing to do") is worse than falling back to the same
        // handling as a confirmed mismatch.
        eprintln!(
            "[rimedit] Background cache verification failed ({}) {} times; treating the cache as stale: project_present={}",
            reason,
            MAX_VERIFY_ATTEMPTS,
            project_id.is_some()
        );
        invalidate_entry_and_rebuild(
            app,
            state,
            project_id,
            captured_generation,
            &settings_fingerprint_expected,
            &expected_fingerprints,
        )
        .await;
    }
}

/// Shared by `execute_verify_cache`'s `Mismatch` arm and its exhausted-`ScanFailed` fallback:
/// invalidates the now-known-stale entry before flipping status -- but only if it's still *exactly*
/// the entry this scan actually observed (generation, settings fingerprint, and file fingerprints
/// all still match what was captured before the scan ran): a watcher-driven incremental update can
/// legitimately replace the entry with fresh, already-verified data without bumping the generation
/// at all (content edits never change `settings_fingerprint`), and if that happened concurrently,
/// the entry it replaced-with is already correct -- clearing it here would destroy that fresher
/// data based on a now-stale result. `set_status_pending` alone changes `cache_verification` to
/// `NotRequired` for the in-progress rebuild phase (not `Checking`), and leaves `DefIndexState`'s
/// cached entry untouched -- so without this, `get_verified_if_settings_match` would otherwise keep
/// serving the exact index this scan just proved wrong to `IndexLoadPolicy::RequireFresh` callers
/// until the rebuild below completes.
async fn invalidate_entry_and_rebuild(
    app: &AppHandle,
    state: &DefIndexState,
    project_id: Option<String>,
    captured_generation: u64,
    settings_fingerprint_expected: &str,
    expected_fingerprints: &[IndexedFileFingerprint],
) {
    let invalidated = state.invalidate_entry_if_unchanged(
        captured_generation,
        settings_fingerprint_expected,
        expected_fingerprints,
    );
    if !invalidated {
        // Either a project switch raced this, or a concurrent write already replaced the entry
        // with fresher (correct) data -- either way, this scan's result is stale and there's
        // nothing left to invalidate or rebuild for.
        return;
    }
    // Re-check right before writing status: a project switch could still race in between the
    // top-level recheck in `execute_verify_cache` and here (no lock is held across these
    // statements). `enqueue_full_rebuild_if_generation_matches` tags the job with
    // `captured_generation` rather than reading it fresh, so even a residual race in its own tiny
    // window can't "launder" this stale `project_id` into a job the worker's coalescing would treat
    // as current -- see that function's doc comment for why a plain `enqueue_full_rebuild` here
    // would let a raced project switch's rebuild get clobbered by this one.
    if state.current_generation() == captured_generation {
        state.set_status_pending(project_id.clone(), 0);
        emit_indexing_status(app, &state.status());
    }
    enqueue_full_rebuild_if_generation_matches(
        app,
        captured_generation,
        project_id,
        IndexJobReason::CacheVerification,
    );
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
            // Generation-gated like the three checkpoints later in this function: a bump landing
            // in the (small, but non-zero) gap between capturing `current_gen` above and this
            // failure means a *newer* job already owns the outcome for the current generation --
            // publishing `Failed` for this stale attempt would transiently clobber whatever status
            // that newer job has already published or is about to.
            if state.current_generation() == current_gen {
                state.set_status_failed(project_id, e.message);
                emit_indexing_status(app, &state.status());
                app.state::<IndexingServiceState>().notify_job_completed();
            }
            return;
        }
    };

    state.set_status_discovering(project_id.clone(), None);
    emit_indexing_status(app, &state.status());

    // Check generation hasn't changed while we set up
    if state.current_generation() != current_gen {
        return;
    }

    let discovery_options = DefIndexBuildOptions {
        project_id: project_id
            .as_deref()
            .or(settings.active_project_id.as_deref()),
        include_sources: true,
        replacement: None,
        force_rebuild: true,
    };
    let discovery = {
        let mut discovery_span = crate::instrumentation::span_with_tags(
            app,
            "indexing.discoverScanStats",
            [(
                "projectPresent".to_string(),
                project_id.is_some().to_string(),
            )],
        );
        let discovery = crate::def_index::discover_scan_stats(&settings, &discovery_options);
        discovery_span.set_tag(
            "includedLocations",
            discovery.included_locations.to_string(),
        );
        discovery_span.set_tag(
            "resolvedWorkshopItemCount",
            discovery.resolved_workshop_item_count.to_string(),
        );
        discovery_span.set_tag(
            "selectedLoadFolderCount",
            discovery.selected_load_folder_count.to_string(),
        );
        discovery_span.set_tag("discoveredFiles", discovery.discovered_files.to_string());
        discovery
    };

    if state.current_generation() != current_gen {
        return;
    }

    state.set_status_indexing(project_id.clone(), discovery.discovered_files);
    emit_indexing_status(app, &state.status());

    if state.current_generation() != current_gen {
        return;
    }

    // Live progress: ticks `processed_files` as each discovered file is attempted, throttled to
    // roughly 100 emissions for the whole rebuild (never per-file) regardless of collection
    // size. `set_status_indexing_progress` itself no-ops once the rebuild has left the
    // `Indexing` stage, and the generation check here additionally stops a stale tick from a
    // superseded rebuild from being emitted at all -- belt-and-suspenders, since in practice the
    // single-worker job queue never runs two full rebuilds concurrently.
    let processed_counter = std::sync::atomic::AtomicUsize::new(0);
    let total_files = discovery.discovered_files;
    let emit_every = (total_files / 100).max(1);
    let progress_app = app.clone();
    let on_file_indexed = move || {
        let n = processed_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if !n.is_multiple_of(emit_every) && n != total_files {
            return;
        }
        let progress_state = progress_app.state::<DefIndexState>();
        if progress_state.current_generation() != current_gen {
            return;
        }
        progress_state.set_status_indexing_progress(n);
        emit_indexing_status(&progress_app, &progress_state.status());
    };

    let rebuild_result = def_index_cache::rebuild_for_project_with_progress(
        app,
        &settings,
        project_id.as_deref(),
        Some(&on_file_indexed),
    );

    // Generation-gated like the three checkpoints above, but this is the one that matters most:
    // `rebuild_for_project_with_progress` is this function's single longest-running operation (a
    // full collection parse, potentially many seconds for a Steam Workshop-sized collection), so
    // it has by far the largest window for a generation bump (a settings/project change) to land
    // while it runs. Without this check, the terminal `set_status_complete`/
    // `set_status_complete_from_summary`/`set_status_failed` calls below would publish
    // unconditionally -- and since those setters stamp `generation: guard.generation` (the *live*
    // value, now past `current_gen`), the resulting status would look perfectly current to every
    // consumer that compares `status.generation` against `state.current_generation()`
    // (`should_attempt_initialization`, `load_for_project_query`'s in-flight guard), transiently
    // overwriting whatever a newer job for the new generation has already published or is about to.
    if state.current_generation() != current_gen {
        return;
    }

    match rebuild_result {
        Ok(summary) => {
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
                // The just-stored entry was evicted by a concurrent write (e.g. a project switch's
                // own hydration landing in `DefIndexState`'s single global slot) before this
                // readback ran. Report the rebuild's own summary -- real counts and build
                // timestamp -- rather than `DefIndex::default()`, which would falsely publish "0
                // defs, Verified" for a project whose rebuild actually just succeeded (and which
                // `shouldBumpValidationRefreshRevision` on the frontend would then treat as a
                // genuine, if bogus, index change).
                state.set_status_complete_from_summary(project_id.clone(), &summary);
            }
            emit_indexing_status(app, &state.status());
            app.state::<IndexingServiceState>().notify_job_completed();
        }
        Err(e) => {
            state.set_status_failed(project_id, e.message);
            emit_indexing_status(app, &state.status());
            app.state::<IndexingServiceState>().notify_job_completed();
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

    // `get_verified_if_settings_match`, not `get_if_settings_match`: merging one changed file into
    // an *unverified* trusted hydration is unsound, because the base's other files may already be
    // stale relative to disk (they simply haven't been fingerprint-checked yet). This job's own
    // fresh fingerprint scan below only covers the *whole collection*, not "which specific files
    // actually changed" -- so `persist_incremental`'s subsequent `store()` would then unconditionally
    // stamp the merged result `verified: true`, permanently retiring the still-pending `VerifyCache`
    // job's ability to catch the real staleness (its own captured expected fingerprints would no
    // longer match the entry `persist_incremental` just overwrote, so its post-scan recheck bails
    // out treating this as "someone else already fixed it"). Escalating to a full rebuild here is
    // conservative, but this only matters in the narrow window between a hydration-hit and its
    // `VerifyCache` job completing -- a single incremental watcher event landing in that window is
    // expected to be rare.
    let mut index = match state.get_verified_if_settings_match(&fp) {
        Some(arc) => (*arc).clone(),
        None => {
            // No usable verified base index: escalate to full rebuild
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
            IndexJob::FullRebuild { .. } | IndexJob::VerifyCache { .. } => unreachable!(
                "process_batch routes FullRebuild/VerifyCache jobs before calling execute_file_jobs"
            ),
        }
    }

    // Generation-gated, matching `execute_full_rebuild`'s equivalent check right after its own
    // longest-running operation: the loop above only checks generation at the *start* of each
    // iteration (line ~1118), not after -- so a bump landing during the last job's own per-file
    // I/O (`std::fs::read_to_string`, `scan_indexable_def_xml_files`) is never caught before this
    // point. Without this, `set_status_complete` would publish a "Complete" status for this stale
    // batch's project stamped with the *current* (bumped) generation -- indistinguishable from a
    // legitimate current-generation result to any consumer comparing `status.generation` against
    // `state.current_generation()` -- and `persist_incremental` would write/store the stale
    // batch's index into `DefIndexState`'s single global slot, transiently overwriting whatever a
    // newer generation's own in-flight job has already published or is about to.
    if state.current_generation() != current_gen {
        return;
    }
    index.rebuild_computed_fields();
    state.set_status_complete(&index);
    emit_indexing_status(app, &state.status());
    // `persist_incremental` (not `notify_job_completed` above it) is what actually calls
    // `DefIndexState::store` to write the fresh entry -- notifying before that store would let a
    // waiter woken concurrently (on another tokio worker thread) see `phase: Complete` already
    // published but `get_verified_if_settings_match` still miss, wrongly concluding this job
    // isn't actually done and falling through to its own redundant scan.
    def_index_cache::persist_incremental(
        app,
        &settings,
        settings.active_project_id.as_deref(),
        index,
    );
    app.state::<IndexingServiceState>().notify_job_completed();
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
    fn verify_cache(project_id: Option<&str>, fp: &str, gen: u64) -> IndexJob {
        IndexJob::VerifyCache {
            project_id: project_id.map(str::to_string),
            generation: gen,
            settings_fingerprint: fp.into(),
            attempt: 0,
            reason: IndexJobReason::CacheVerification,
        }
    }

    #[test]
    fn stale_generation_jobs_are_dropped() {
        let batch = vec![
            file_changed("loc", "a.xml", 1),
            file_changed("loc", "b.xml", 1),
        ];
        let result = coalesce_batch_for_generation(batch, 2); // current gen is 2, jobs have gen 1
        assert!(result.is_empty());
    }

    #[test]
    fn full_rebuild_supersedes_file_jobs() {
        let batch = vec![
            file_changed("loc", "a.xml", 1),
            full_rebuild(1),
            file_deleted("loc", "b.xml", 1),
        ];
        let result = coalesce_batch_for_generation(batch, 1);
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
        let result = coalesce_batch_for_generation(batch, 1);
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
        let result = coalesce_batch_for_generation(batch, 1);
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
        let result = coalesce_batch_for_generation(batch, 1);
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

    #[test]
    fn duplicate_verify_cache_requests_for_the_same_key_coalesce() {
        let batch = vec![
            verify_cache(Some("proj"), "fp1", 1),
            verify_cache(Some("proj"), "fp1", 1),
            verify_cache(Some("proj"), "fp1", 1),
        ];
        let result = coalesce_batch_for_generation(batch, 1);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], IndexJob::VerifyCache { .. }));
    }

    #[test]
    fn verify_cache_jobs_for_different_settings_fingerprints_do_not_collapse() {
        let batch = vec![
            verify_cache(Some("proj"), "fp1", 1),
            verify_cache(Some("proj"), "fp2", 1),
        ];
        let result = coalesce_batch_for_generation(batch, 1);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn full_rebuild_supersedes_verify_cache_jobs() {
        let batch = vec![verify_cache(Some("proj"), "fp1", 1), full_rebuild(1)];
        let result = coalesce_batch_for_generation(batch, 1);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], IndexJob::FullRebuild { .. }));
    }

    #[test]
    fn verify_cache_and_file_jobs_coexist_in_the_same_coalesced_batch() {
        let batch = vec![
            verify_cache(Some("proj"), "fp1", 1),
            file_changed("loc", "a.xml", 1),
        ];
        let result = coalesce_batch_for_generation(batch, 1);
        assert_eq!(result.len(), 2);
        assert!(result
            .iter()
            .any(|j| matches!(j, IndexJob::VerifyCache { .. })));
        assert!(result
            .iter()
            .any(|j| matches!(j, IndexJob::FileChanged { .. })));
    }

    #[test]
    fn a_stale_generation_verify_cache_job_is_dropped_by_coalescing() {
        let batch = vec![verify_cache(Some("proj"), "fp1", 1)];
        let result = coalesce_batch_for_generation(batch, 2);
        assert!(result.is_empty());
    }

    #[test]
    fn single_flight_try_schedule_init_returns_true_only_for_the_first_caller() {
        let state = IndexingServiceState::new();
        let key = InitKey {
            project_id: Some("proj".to_string()),
            generation: 1,
            settings_fingerprint: "fp1".to_string(),
        };
        assert!(
            state.try_schedule_init(key.clone()),
            "first caller must win the single-flight race"
        );
        assert!(
            !state.try_schedule_init(key.clone()),
            "a second concurrent caller for the same key must be told to skip"
        );
        assert!(state.has_in_flight_init(&key));

        state.finish_init(&key);
        assert!(!state.has_in_flight_init(&key));
        assert!(
            state.try_schedule_init(key),
            "after finish_init, a new initialization for the same key may proceed"
        );
    }

    #[test]
    fn single_flight_keys_differing_only_by_generation_are_independent() {
        let state = IndexingServiceState::new();
        let key_gen1 = InitKey {
            project_id: Some("proj".to_string()),
            generation: 1,
            settings_fingerprint: "fp1".to_string(),
        };
        let key_gen2 = InitKey {
            project_id: Some("proj".to_string()),
            generation: 2,
            settings_fingerprint: "fp1".to_string(),
        };
        assert!(state.try_schedule_init(key_gen1));
        assert!(
            state.try_schedule_init(key_gen2),
            "a newer generation must not be blocked by an in-flight older-generation init"
        );
    }
}
