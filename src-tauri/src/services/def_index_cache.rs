use crate::def_index::{
    cache_state_inputs, load_cached_index_unverified, load_or_rebuild_def_index,
    rebuild_and_store_def_index_with_progress, settings_fingerprint, store_prebuilt_index,
    summarize_index, verify_fingerprints, CacheVerification, DefIndex, DefIndexBuildOptions,
    DefIndexState, DefIndexSummary, FingerprintVerification, IndexingPhase, IndexingStatus,
};
use crate::project_model::{AppError, ProjectSettings};
use crate::services::app_paths;
use crate::services::indexing::{self, IndexJobReason, IndexingServiceState, InitKey};
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// `async` because the fallback path (a settings-fingerprint miss, or an explicit
/// `force_rebuild`) reads and hashes every indexed file -- and, on a full cache miss, parses
/// the whole collection. Tauri runs non-`async` commands on the main (WebView) thread, so every
/// caller up to its `#[tauri::command]` entry point must be `async` too and `.await` this;
/// otherwise opening/editing/saving a Def file would freeze the UI for the scan's duration. The
/// hot path (an in-memory `DefIndexState` match, the common case once startup indexing has
/// completed) stays synchronous and returns immediately.
pub(crate) async fn load_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    force_rebuild: bool,
) -> Result<Arc<DefIndex>, AppError> {
    let state = app.state::<DefIndexState>();
    let options = DefIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        replacement: None,
        force_rebuild,
    };

    if !force_rebuild {
        // Hot path: the indexing service keeps this state current on file changes, so avoid
        // recomputing project-wide file fingerprints before returning the in-memory index.
        // `get_verified_if_settings_match` (not `get_if_settings_match`): this is the
        // `RequireFresh` policy's only entry point, so it must never trust a still-`Checking`
        // hydrated cache as if it were guaranteed-fresh -- see that method's doc comment.
        let fp = settings_fingerprint(settings, &options);
        if let Some(index) = state.get_verified_if_settings_match(&fp) {
            return Ok(index);
        }

        // A worker job (`FullRebuild`/`VerifyCache`/a file-change batch) may already be scanning
        // or rebuilding this exact collection -- e.g. startup's `schedule_initialization`
        // just enqueued a `FullRebuild` moments before the user opened a document that hit this
        // `RequireFresh` path. Await its result instead of always launching a second, fully
        // independent scan for the same data: if a full rebuild is already in flight, await the
        // coordinated result rather than launching a second build, and never poll or busy-wait.
        if let Some(index) = wait_for_in_flight_job(app, &state, project_id, &fp).await {
            return Ok(index);
        }
    }

    let blocking_app = app.clone();
    let blocking_settings = settings.clone();
    let blocking_project_id = project_id.to_string();
    tauri::async_runtime::spawn_blocking(move || -> Result<Arc<DefIndex>, AppError> {
        let app_data_dir = app_paths::app_storage_dir(&blocking_app, "def_index_load_failed")?;
        let options = DefIndexBuildOptions {
            project_id: Some(&blocking_project_id),
            include_sources: true,
            replacement: None,
            force_rebuild,
        };
        let index = load_or_rebuild_def_index(&app_data_dir, &blocking_settings, options)?;
        let options = DefIndexBuildOptions {
            project_id: Some(&blocking_project_id),
            include_sources: true,
            replacement: None,
            force_rebuild: false,
        };
        let state = blocking_app.state::<DefIndexState>();
        if let Some((settings_hash, file_hashes)) = cache_state_inputs(&blocking_settings, &options)
        {
            return Ok(state.store(
                Some(blocking_project_id.clone()),
                settings_hash,
                file_hashes,
                index,
            ));
        }
        Ok(Arc::new(index))
    })
    .await
    .map_err(|e| AppError {
        code: "def_index_load_join_failed".to_string(),
        message: format!("Def index load task failed: {}", e),
        details: None,
        args: crate::diagnostics::DiagnosticArgs::new(),
    })?
}

/// Bounded wait for an in-flight worker job (`FullRebuild`/`VerifyCache`/a file-change batch)
/// already scanning/rebuilding `project_id`'s collection under the *current* generation --
/// `load_for_project`'s only caller, backing `IndexLoadPolicy::RequireFresh`, so it never launches
/// a second, fully independent scan for data the worker is already producing. Returns `Some`
/// once the wait sees a settings-matching verified entry (either because the in-flight job just
/// produced one, or because it was already there when first checked); returns `None` if no job
/// is (or remains) in flight for this exact scope, or if the bounded wait times out -- in both
/// cases the caller falls through to its own independent scan/rebuild.
///
/// Registers interest via `IndexingServiceState::job_completed_notified` *before* re-checking
/// `get_verified_if_settings_match` each iteration, per the standard `tokio::sync::Notify` pattern:
/// creating the notified future first means a `notify_job_completed` call racing in between the
/// check and the `.await` is still observed, not silently missed. The timeout is a safety fallback
/// only (a job stuck on slow I/O, or one abandoned mid-flight by a generation bump without ever
/// publishing a terminal status for this exact scope -- see `IndexingStatus::generation`'s doc
/// comment) -- the primary wake path is the notification itself, never polling on a timer.
async fn wait_for_in_flight_job(
    app: &AppHandle,
    state: &DefIndexState,
    project_id: &str,
    fp: &str,
) -> Option<Arc<DefIndex>> {
    let svc = app.state::<IndexingServiceState>();
    loop {
        let status = indexing::get_indexing_status(app);
        // A `VerifyCache` scan (`execute_verify_cache`) never publishes `Pending`/`Running` --
        // it has no status write at all for the duration of its (potentially slow, full-
        // collection) fingerprint scan, leaving whatever hydration last published: `phase:
        // Complete, cache_verification: Checking`. Phase alone is therefore not enough to detect
        // it as in-flight; `cache_verification == Checking` is the only signal that distinguishes
        // "verification scan running" from a genuinely settled `Complete` status.
        let in_flight = (matches!(
            status.phase,
            IndexingPhase::Pending | IndexingPhase::Running
        ) || status.cache_verification == CacheVerification::Checking)
            && status.generation == state.current_generation()
            && status.project_id.as_deref() == Some(project_id);
        if !in_flight {
            // Either nothing is in flight for this scope, or the job that was has already
            // published its terminal status -- either way, one last check for the freshest
            // possible answer before handing back to the caller's own fallback.
            return state.get_verified_if_settings_match(fp);
        }
        let notified = svc.job_completed_notified();
        if let Some(index) = state.get_verified_if_settings_match(fp) {
            return Some(index);
        }
        if tokio::time::timeout(std::time::Duration::from_secs(30), notified)
            .await
            .is_err()
        {
            return state.get_verified_if_settings_match(fp);
        }
    }
}

/// Outcome of `hydrate_unverified_for_project`.
pub(crate) enum HydrationOutcome {
    /// A valid trusted cache was found and stored.
    Hit(Arc<DefIndex>),
    /// No valid disk cache exists for `project_id`.
    Miss,
    /// A valid disk cache was found, but `expected_generation` no longer matched
    /// `DefIndexState`'s current generation by the time the (disk) read completed -- a
    /// settings/project change raced this call. Discarded without being stored; the caller must
    /// not enqueue a rebuild for the stale generation it captured (a newer initialization already
    /// owns the outcome for the new one).
    Stale,
}

/// Cache-only, *trusted* startup hydration: reads a disk-cached def index for `project_id`
/// straight into `DefIndexState` checking only the settings fingerprint -- never
/// reading/parsing/hashing indexed Def XML, and never writing the cache file itself. Unlike
/// `load_for_project`, a miss here never falls back to a synchronous rebuild -- callers decide
/// whether to enqueue one.
///
/// The returned index's fingerprints have *not* been verified against the current collection.
/// Callers on a `Hit` must schedule a `VerifyCache` job (`services::indexing::enqueue_cache_verification`)
/// so a change made while RimEdit was closed is still eventually detected and rebuilt --
/// `run_hydration` below does this automatically.
///
/// The store is atomic with the `expected_generation` check (see `DefIndexState::store_if_generation_matches`):
/// a settings/project change that races this call's (small, but non-zero-latency) disk read can
/// never have its now-stale result silently clobber a legitimately newer entry.
///
/// `app_data_dir` is resolved by the caller (rather than internally via `app_paths::app_storage_dir`)
/// so this function's only dependency on `AppHandle` is `DefIndexState` access and instrumentation --
/// letting a test drive it with a temp directory without needing a real app-storage path.
pub(crate) fn hydrate_unverified_for_project(
    app: &AppHandle,
    app_data_dir: &Path,
    settings: &ProjectSettings,
    project_id: Option<&str>,
    expected_generation: u64,
) -> HydrationOutcome {
    let mut span = crate::instrumentation::span(app, "defIndex.cacheHydrate");
    let options = DefIndexBuildOptions {
        project_id,
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    let Some(cached) = load_cached_index_unverified(app_data_dir, settings, &options, Some(app))
    else {
        span.set_tag("outcome", "miss");
        return HydrationOutcome::Miss;
    };
    let state = app.state::<DefIndexState>();
    let stored = state.store_if_generation_matches(
        expected_generation,
        project_id.map(String::from),
        cached.settings_fingerprint,
        cached.file_fingerprints,
        cached.index,
    );
    match stored {
        Some(index) => {
            span.set_tag("outcome", "hit");
            HydrationOutcome::Hit(index)
        }
        None => {
            span.set_tag("outcome", "stale");
            HydrationOutcome::Stale
        }
    }
}

/// Compares `expected_fingerprints` (captured at a prior hydration/build) against a fresh
/// project-wide scan. Never touches `DefIndexState` and never rebuilds -- see
/// `services::indexing::jobs::execute_verify_cache` for the orchestration (generation/settings
/// re-checks, status updates, rebuild hand-off) around this. This performs the same (potentially
/// slow, for a large Steam Workshop collection) read+hash work as `file_fingerprints`, so callers
/// must run it off any latency-sensitive thread -- `execute_verify_cache` does so via
/// `spawn_blocking`.
///
/// Deliberately uses `project_id` exactly as captured at hydration time, with no
/// `settings.active_project_id` fallback: unlike `rebuild_for_project_with_progress` (where "no
/// explicit id" legitimately means "whatever's currently active"), this must verify the *specific*
/// location set the cache was hydrated for -- including a project-less (sources-only) hydration,
/// where `project_id: None` is itself the correct, exact scope. Substituting a since-activated
/// project here would scan files that were never part of what's being verified, and the
/// generation recheck around this call only discards the (correct-but-wasted) resulting scan
/// rather than preventing it.
pub(crate) fn verify_hydrated_project(
    settings: &ProjectSettings,
    project_id: Option<&str>,
    expected_fingerprints: &[crate::def_index::IndexedFileFingerprint],
) -> FingerprintVerification {
    let options = DefIndexBuildOptions {
        project_id,
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    verify_fingerprints(settings, &options, expected_fingerprints)
}

/// Whether `IndexLoadPolicy::Interactive` (`schedule_initialization`) should attempt
/// hydration/rebuild at all, or whether there's already a usable index / matching in-flight job
/// for `effective_id` that makes it redundant. A `Complete` status alone is never sufficient for
/// "nothing to do" -- a stale index (settings-mismatched) must remain eligible. Extracted as a
/// pure predicate (no `AppHandle`) so it's directly unit-testable.
///
/// `current_generation` must be a *freshly read* generation, not derived from `current_status`
/// itself: a job superseded by a generation bump can bail out mid-setup after already publishing
/// an intermediate `Pending`/`Running` status (`execute_full_rebuild`'s early-return generation
/// checks run *after* `set_status_discovering`/`set_status_indexing` have already written it) and
/// never gets to publish a terminating `Complete`/`Failed` for that abandoned attempt -- leaving
/// `current_status` frozen at `Pending`/`Running` for a project no active job is actually working
/// on anymore. Comparing `current_status.generation` against the live generation lets this
/// recognize that case instead of treating a stale status as evidence of a live in-flight job
/// (which would otherwise permanently suppress a genuinely-needed initialization).
pub(crate) fn should_attempt_initialization(
    current_status: &IndexingStatus,
    effective_id: &Option<String>,
    state_has_matching_entry: bool,
    current_generation: u64,
) -> bool {
    if state_has_matching_entry {
        return false;
    }
    let already_in_flight = (current_status.phase == IndexingPhase::Pending
        || current_status.phase == IndexingPhase::Running)
        && current_status.generation == current_generation
        && &current_status.project_id == effective_id;
    !already_in_flight
}

/// Outcome of `schedule_initialization`.
pub(crate) enum InitializationOutcome {
    /// A matching index was already present in `DefIndexState` -- no I/O, no background work
    /// started.
    HydratedHit(Arc<DefIndex>),
    /// This call claimed the `InitKey` and spawned background work
    /// (`run_hydration`) that will hydrate from a trusted disk cache or, on a miss, enqueue a
    /// `FullRebuild` -- neither has completed yet by the time this returns.
    Scheduled,
    /// Another caller already has a matching initialization/rebuild in flight for this
    /// `{project_id, generation, settings_fingerprint}` -- this call did nothing.
    AlreadyInFlightOrRebuilding,
}

/// Single-flight "ensure indexing is scheduled" helper shared by `.setup()`, the frontend's
/// `start_background_indexing` command, and `IndexLoadPolicy::Interactive` loads. At most one
/// hydration-or-rebuild is started per `{project_id, generation, settings_fingerprint}` --
/// concurrent callers for the same key are told `AlreadyInFlightOrRebuilding` and must not start
/// their own.
///
/// Cheap and synchronous: never reads the disk-cache file or scans the collection itself. When
/// there's nothing to do (a matching entry, or an already in-flight job) this is just a couple of
/// lock acquisitions. When hydration is actually needed, this claims the `InitKey`, publishes the
/// `HydratingCache` status synchronously (so a concurrent `RequireFresh` caller's
/// `wait_for_in_flight_job` sees it as in-flight immediately, not only once hydration eventually
/// finishes), and hands the disk read/deserialize/rebuild off to a named background thread
/// (`run_hydration`) -- returning `Scheduled` without waiting for that thread. This is what lets
/// `.setup()` return immediately even against a very large cache.
///
/// Untested directly (neither this function nor `run_hydration` has a unit test driving them
/// end-to-end): both require a real `&AppHandle` for `app.state::<T>()`/instrumentation, and this
/// codebase has no test anywhere that constructs one -- `tauri::test::mock_app()` produces an
/// `App<MockRuntime>`, a different type from the `AppHandle<Wry>` these functions (and everything
/// else in this codebase) are written against, and reconciling that would mean making this whole
/// call chain generic over `tauri::Runtime`. The scheduling decision this function makes is
/// instead covered where it's actually independently testable: `should_attempt_initialization`
/// (the pure predicate below) at the decision-logic level, `IndexingServiceState::try_schedule_init`/
/// `finish_init` (`services/indexing/jobs.rs`) at the single-flight/duplicate-caller level, and
/// `DefIndexState::store_if_generation_matches`/hydration outcome handling (`def_index/state.rs`,
/// `def_index/tests/cache.rs`) at the generation-race/hit-miss-stale level.
pub(crate) fn schedule_initialization(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: Option<String>,
) -> InitializationOutcome {
    // `project_id: None` is a legitimate key (a project-less, sources-only collection), not a
    // no-op -- don't special-case it away. `hydrate_unverified_for_project`/`DefIndexBuildOptions`
    // already accept `Option<&str>` throughout, matching the pre-single-flight behavior of the
    // old `start_background_indexing`, which enqueued a rebuild for `None` too.
    let options = DefIndexBuildOptions {
        project_id: project_id.as_deref(),
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    let settings_fp = settings_fingerprint(settings, &options);
    let state = app.state::<DefIndexState>();

    let matching_entry = state.get_if_settings_match(&settings_fp);
    let current_status = indexing::get_indexing_status(app);
    let generation = state.current_generation();
    if !should_attempt_initialization(
        &current_status,
        &project_id,
        matching_entry.is_some(),
        generation,
    ) {
        return match matching_entry {
            Some(index) => InitializationOutcome::HydratedHit(index),
            None => InitializationOutcome::AlreadyInFlightOrRebuilding,
        };
    }
    let key = InitKey {
        project_id: project_id.clone(),
        generation,
        settings_fingerprint: settings_fp.clone(),
    };
    let svc = app.state::<IndexingServiceState>();
    if !svc.try_schedule_init(key.clone()) {
        return InitializationOutcome::AlreadyInFlightOrRebuilding;
    }
    // Covers the synchronous status-publish work below, before the background thread (which
    // carries its own `FinishInitOnDrop`) exists to cover anything itself -- a panic in either
    // call between here and the successful spawn below would otherwise leak `key` with nothing on
    // this thread's unwind path to release it. Defused once the handoff to that thread succeeds,
    // since from that point on the thread -- not this scope's normal return -- owns releasing the
    // key.
    let panic_guard = FinishInitOnDrop::new(svc.inner(), &key);

    state.set_status_hydrating_cache_if_generation_matches(generation, project_id.clone());
    indexing::events::emit_indexing_status(app, &state.status());

    let thread_app = app.clone();
    let thread_settings = settings.clone();
    let thread_project_id = project_id.clone();
    let thread_settings_fp = settings_fp.clone();
    let thread_key = key.clone();
    let spawned = std::thread::Builder::new()
        .name("rimedit-def-index-hydrate".to_string())
        .spawn(move || {
            run_hydration(
                &thread_app,
                &thread_settings,
                thread_project_id,
                generation,
                thread_settings_fp,
                thread_key,
            );
        });
    if spawned.is_err() {
        // Extremely unlikely (OS thread exhaustion). Release the claimed key so this scope isn't
        // permanently stuck looking "in flight" with nothing actually backing it -- a later caller
        // will see a genuine miss and retry. Then `defuse()` -- not just leaving `panic_guard`
        // armed to do this again on drop -- because a *different* caller could legitimately
        // re-claim this exact key in the gap between this explicit call and the function
        // returning; an armed guard firing after that would silently evict their fresh claim
        // instead of being the harmless no-op it looks like.
        svc.finish_init(&key);
        panic_guard.defuse();
    } else {
        // Handoff succeeded: `run_hydration`'s own `FinishInitOnDrop` now owns releasing `key`.
        panic_guard.defuse();
    }
    InitializationOutcome::Scheduled
}

/// RAII backstop for `try_schedule_init`'s contract ("callers MUST call `finish_init` ... on every
/// terminal path, or the key leaks"). Used in two places: `schedule_initialization` constructs one
/// to cover the synchronous status-publish calls between claiming the key and spawning the
/// background thread; `run_hydration` constructs its own to cover its entire body.
///
/// Every normal (non-panicking) path through both functions already calls `svc.finish_init`
/// explicitly at a deliberately-chosen point (see e.g. `run_hydration`'s `Miss` branch, which
/// releases the key *before* publishing `Pending`) and must `defuse()` its guard immediately
/// after that call -- letting the guard's `Drop` also fire is NOT harmless redundancy here: by
/// the time this scope actually returns, a different caller may have already legitimately
/// reclaimed the identical `{project_id, generation, settings_fingerprint}` key (`in_flight_init`
/// is a plain `HashSet<InitKey>`, not per-claim-tokened), and a still-armed guard firing at that
/// point would silently evict *their* fresh claim, not a no-op. `defuse()` after every explicit
/// call closes that window. What's left un-defused is exactly the path with no explicit call at
/// all: an unexpected panic unwinding out of either function before reaching one, which would
/// otherwise leak `key` and wedge all future initialization for that key forever.
struct FinishInitOnDrop<'a> {
    svc: &'a IndexingServiceState,
    key: &'a InitKey,
    armed: bool,
}

impl<'a> FinishInitOnDrop<'a> {
    fn new(svc: &'a IndexingServiceState, key: &'a InitKey) -> Self {
        Self {
            svc,
            key,
            armed: true,
        }
    }

    /// Disarms the guard without releasing the key -- used once responsibility for eventually
    /// calling `finish_init` has been handed off elsewhere (the spawned hydration thread), so this
    /// scope's own normal return no longer means "finish_init has already happened."
    fn defuse(mut self) {
        self.armed = false;
    }
}

impl Drop for FinishInitOnDrop<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.svc.finish_init(self.key);
        }
    }
}

/// The actual (potentially slow, for a large disk cache) hydration work `schedule_initialization`
/// hands off to a background thread once it has claimed `key`. Mirrors exactly what used to run
/// synchronously inside the old `schedule_initialization` after claiming the same key --
/// only the caller (a background thread instead of whichever thread called this function) and the
/// return type (none; every outcome is published via `DefIndexState`/events instead of returned)
/// have changed. Always calls `svc.finish_init(&key)` on every path, per `try_schedule_init`'s
/// contract, and immediately `defuse()`s `finish_init_on_drop` right after -- that guard is a
/// panic-safety backstop for the same contract (see its own doc comment), not meant to fire on
/// top of an explicit call: a *different* caller could legitimately re-claim this exact key in
/// the gap between the explicit `finish_init` and this function returning, and an armed guard
/// firing after that would silently evict their fresh claim.
fn run_hydration(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: Option<String>,
    generation: u64,
    settings_fp: String,
    key: InitKey,
) {
    let state = app.state::<DefIndexState>();
    let svc = app.state::<IndexingServiceState>();
    let finish_init_on_drop = FinishInitOnDrop::new(svc.inner(), &key);

    let hydration = match app_paths::app_storage_dir(app, "def_index_hydrate_failed") {
        Ok(app_data_dir) => hydrate_unverified_for_project(
            app,
            &app_data_dir,
            settings,
            project_id.as_deref(),
            generation,
        ),
        Err(e) => {
            // Cache-only hydration failing (e.g. the app storage directory can't be resolved) is
            // not fatal -- callers degrade to an empty/stale index rather than blocking document
            // display -- but it's worth a breadcrumb for diagnosing a persistently misconfigured
            // install.
            eprintln!(
                "[rimedit] Unverified def-index hydration failed, treating as a cache miss: {}",
                e.message
            );
            HydrationOutcome::Miss
        }
    };

    match hydration {
        HydrationOutcome::Hit(_index) => {
            // Generation-gated, like the `store_if_generation_matches` call inside
            // `hydrate_unverified_for_project` just above: this is a *separate* lock acquisition
            // from that one, so a generation bump landing in the gap between them (e.g. a
            // concurrent project switch) must not let this stale-generation status overwrite a
            // newer, already-published one.
            if state.set_status_complete_for_project_if_generation_matches(
                generation,
                &settings_fp,
                project_id.clone(),
            ) {
                indexing::events::emit_indexing_status(app, &state.status());
                indexing::enqueue_cache_verification(app, generation, project_id, settings_fp, 0);
            } else {
                // The generation moved between the `store_if_generation_matches` call above and
                // this status publish (a separate lock acquisition), so this hydration lost the
                // race and never got to schedule a `VerifyCache` job for the entry it just stored.
                // Left in place, that entry would sit there forever as an orphaned, permanently-
                // unverified hit: a later call for the exact same `{project_id, settings_fp}` (the
                // same project/settings becoming current again) would see `get_if_settings_match`
                // return it and, per `should_attempt_initialization`, treat that as "nothing to do"
                // -- never scheduling the verification this branch was supposed to kick off.
                // Removing it here (only if it's still exactly the unverified entry this call
                // stored, never a newer legitimate write) instead lets a future call see a genuine
                // cache miss and re-hydrate/re-verify properly.
                state.remove_unverified_entry_if_settings_match(&settings_fp);
            }
            svc.finish_init(&key);
            finish_init_on_drop.defuse();
        }
        HydrationOutcome::Stale => {
            // A settings/project/generation change raced the hydration's disk read; a newer
            // initialization already owns the outcome for the new generation, so discard this
            // one without touching status or scheduling a rebuild under the stale generation.
            svc.finish_init(&key);
            finish_init_on_drop.defuse();
        }
        HydrationOutcome::Miss => {
            svc.finish_init(&key);
            finish_init_on_drop.defuse();
            // Generation-gated: a bump landing between the top-level hydration read and here means
            // a newer initialization already owns the outcome for the current generation. Without
            // this, `set_status_pending` below would publish a "Pending" status for this stale
            // `project_id` -- but stamped with the *current* (not this call's captured)
            // generation, since `set_status_pending` always stamps the live value -- making a
            // phantom pending status that looks perfectly current to any consumer comparing
            // `status.generation` against `state.current_generation()`, with no job actually
            // backing it. Using `enqueue_full_rebuild_if_generation_matches` (not the plain,
            // fresh-generation-read `enqueue_full_rebuild`) additionally closes the "laundering"
            // that function's own doc comment warns about: a fresh read would otherwise tag this
            // stale `project_id` with the *current* generation, and the worker's
            // `coalesce_batch_for_generation` filter would then accept it as legitimately
            // belonging to that generation and actually run it.
            if state.current_generation() != generation {
                return;
            }
            // Publish `Pending` synchronously, before even enqueueing the job:
            // `should_attempt_initialization`'s "already in flight" check reads
            // `DefIndexState.status.phase`, not the job queue, and that phase only flips to
            // `Pending` once the async worker actually dequeues and starts `execute_full_rebuild`
            // -- an unbounded amount of real time after `enqueue_full_rebuild_if_generation_matches`
            // merely sends the job onto the channel. Without this, a concurrent caller (e.g. the
            // frontend's mount-time `start_background_indexing`) racing in after `finish_init`
            // released the key above but before the worker gets around to dequeuing would see
            // neither a matching state entry nor an in-flight status, and would enqueue its own
            // duplicate `FullRebuild` -- which only coalesces away if it lands in the channel
            // before the worker's *next* receive, not if the first job has already been dequeued
            // and started executing by then.
            state.set_status_pending(project_id.clone(), 0);
            indexing::events::emit_indexing_status(app, &state.status());
            indexing::enqueue_full_rebuild_if_generation_matches(
                app,
                generation,
                project_id,
                IndexJobReason::InitialProjectOpen,
            );
        }
    }
}

/// `IndexLoadPolicy::Interactive` (see `load_for_project_with_policy`): never blocks on a disk-cache
/// read or a project-wide file scan. Returns a matching in-memory index immediately; on a miss,
/// schedules hydration/rebuild via `schedule_initialization` -- which claims the `InitKey` and hands
/// the actual disk read off to a background thread -- and immediately returns whatever is already
/// cached for *this same project*, or an empty `DefIndex`, without waiting for that background work
/// to finish.
///
/// `get_cached_for_project` is scoped by project (not an unscoped "whatever's cached" read):
/// `DefIndexState` holds a single global slot, so an unscoped fallback could return a *different*
/// project's index (e.g. right after switching the active project) -- silently validating this
/// document against the wrong project's Defs instead of merely showing stale data for the right
/// one.
async fn load_interactive(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
) -> Arc<DefIndex> {
    let state = app.state::<DefIndexState>();
    let options = DefIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    let fp = settings_fingerprint(settings, &options);
    if let Some(index) = state.get_if_settings_match(&fp) {
        return index;
    }

    // Cache miss: `schedule_initialization` itself is cheap (a couple of lock acquisitions plus a
    // thread spawn, never a disk read) -- no `spawn_blocking` needed to keep it off this async
    // command's Tokio worker thread.
    if let InitializationOutcome::HydratedHit(index) =
        schedule_initialization(app, settings, Some(project_id.to_string()))
    {
        return index;
    }

    state.get_cached_for_project(project_id).unwrap_or_default()
}

/// Policy for loading a def index -- see `load_for_project_with_policy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IndexLoadPolicy {
    /// Never blocks this request on a project-wide file scan (see `load_interactive`). For
    /// document display/validation paths where a moment of stale/partial data is acceptable and
    /// waiting on a Steam Workshop-sized scan is not (initial editor opens, in-buffer parse/edit
    /// validation).
    Interactive,
    /// Strict: blocks on whatever it takes to guarantee freshness -- a project-wide fingerprint
    /// scan and, on a miss, a full parse/rebuild. For save/commit and explicit-validation paths
    /// where a partial/stale index could make a destructive or commit decision unsafe.
    RequireFresh,
}

/// Loads a def index for `project_id` under the given policy -- see `IndexLoadPolicy`.
pub(crate) async fn load_for_project_with_policy(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    policy: IndexLoadPolicy,
) -> Result<Arc<DefIndex>, AppError> {
    match policy {
        IndexLoadPolicy::RequireFresh => load_for_project(app, settings, project_id, false).await,
        IndexLoadPolicy::Interactive => Ok(load_interactive(app, settings, project_id).await),
    }
}

/// Convenience wrapper for `load_for_project_with_policy(.., IndexLoadPolicy::RequireFresh)` --
/// every save/duplicate-check/explicit-validation call site needs the identical call, so this
/// keeps a future change to the `RequireFresh` path (timeout, instrumentation, an extra parameter)
/// from having to be hand-applied across each one.
pub(crate) async fn load_fresh_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
) -> Result<Arc<DefIndex>, AppError> {
    load_for_project_with_policy(app, settings, project_id, IndexLoadPolicy::RequireFresh).await
}

pub(crate) fn load_for_project_query(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
) -> Result<Arc<DefIndex>, AppError> {
    let state = app.state::<DefIndexState>();
    let options = DefIndexBuildOptions {
        project_id: Some(project_id),
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    let settings_fp = settings_fingerprint(settings, &options);
    if let Some(index) = state.get_if_settings_match(&settings_fp) {
        return Ok(index);
    }
    // Settings changed or no index yet: serve whatever is cached for *this project* rather than
    // blocking on rebuild. Background indexing will populate a fresh index shortly. Scoped by
    // project (not an unscoped "whatever's cached" read): right after a project switch,
    // `DefIndexState`'s single global slot may still hold the *previous* project's index --
    // returning it here would attribute another project's search/facet/reference results to
    // this one instead of just showing this project's own stale data.
    if let Some(stale) = state.get_cached_for_project(project_id) {
        return Ok(stale);
    }
    // No cached data at all for this project -- e.g. `DefIndexState`'s single global slot was
    // overwritten by a straggler rebuild for a *different* project that finished after the user
    // switched back to this one (`DefIndexState::store` has no generation gate). Unlike
    // `load_interactive` (used for document opens), this function's callers -- search, facets,
    // reference suggestions, the errors panel -- have no other path that would ever trigger a
    // rehydration, so returning an empty `DefIndex` here without also enqueueing a rebuild would
    // leave search/facets/errors silently and permanently empty for this project (no visible
    // error, and `useIndexingStatus`'s project-scoped event filter would keep showing whatever
    // this project's own last-known status was) until the user happens to open a document tab.
    // A plain enqueue (not `schedule_initialization`) because this function's callers are
    // synchronous Tauri commands that must not block on I/O -- sending a job onto the worker's
    // channel is a cheap, non-blocking, fire-and-forget correction.
    //
    // Guarded by the current status, mirroring `should_attempt_initialization`'s "already in
    // flight" check: these callers can fire once per keystroke (a search box, a reference
    // autocomplete field), and without this guard each call during the miss-recovery window would
    // enqueue its own `FullRebuild`. The worker's own coalescing collapses any that land in the
    // *same* batch, but calls spread out across the window a rebuild actually takes to run would
    // queue up behind it and each still execute afterward -- one redundant full collection rescan
    // per keystroke fired after the first rebuild started, not just one total.
    //
    // `current.generation == state.current_generation()`, not just phase+project_id: a job
    // superseded by a generation bump can bail out after already publishing an intermediate
    // `Pending`/`Running` status and never get to publish `Complete`/`Failed` for it (see
    // `IndexingStatus::generation`'s doc comment) -- without this check, that abandoned status
    // would look exactly like a live in-flight rebuild for this project and permanently suppress
    // the very re-enqueue this fallback exists to perform.
    let already_rebuilding = {
        let current = indexing::get_indexing_status(app);
        matches!(
            current.phase,
            crate::def_index::IndexingPhase::Pending | crate::def_index::IndexingPhase::Running
        ) && current.generation == state.current_generation()
            && current.project_id.as_deref() == Some(project_id)
    };
    if already_rebuilding {
        return Ok(Arc::new(DefIndex::default()));
    }
    indexing::enqueue_full_rebuild(
        app,
        Some(project_id.to_string()),
        IndexJobReason::InitialProjectOpen,
    );
    Ok(Arc::new(DefIndex::default()))
}

/// Writes an incrementally-updated index to both disk and in-memory state with fresh
/// file fingerprints, so subsequent `load_for_project` calls succeed without triggering
/// a full rescan.
pub(crate) fn persist_incremental(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: Option<&str>,
    index: DefIndex,
) {
    let Ok(app_data_dir) = app_paths::app_storage_dir(app, "persist_incremental") else {
        return;
    };
    let state = app.state::<DefIndexState>();
    let options = DefIndexBuildOptions {
        project_id,
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    match store_prebuilt_index(&app_data_dir, settings, options, index.clone()) {
        Ok((settings_fp, file_fps)) => {
            state.store(project_id.map(String::from), settings_fp, file_fps, index);
        }
        Err(_) => {
            // Disk write failed; settings-match still works via whatever was stored before
        }
    }
}

pub(crate) fn rebuild_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: Option<&str>,
) -> Result<DefIndexSummary, AppError> {
    rebuild_for_project_with_progress(app, settings, project_id, None)
}

/// Same as `rebuild_for_project`, but threads a live-progress callback through to
/// `rebuild_and_store_def_index_with_progress` -- see
/// `def_index::build_def_index_with_progress`'s doc comment.
pub(crate) fn rebuild_for_project_with_progress(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: Option<&str>,
    on_file_indexed: Option<&dyn Fn()>,
) -> Result<DefIndexSummary, AppError> {
    let app_data_dir = app_paths::app_storage_dir(app, "def_index_rebuild_failed")?;
    let effective_project_id = project_id.or(settings.active_project_id.as_deref());
    let options = DefIndexBuildOptions {
        project_id: effective_project_id,
        include_sources: true,
        replacement: None,
        force_rebuild: true,
    };
    let index = rebuild_and_store_def_index_with_progress(
        &app_data_dir,
        settings,
        options,
        on_file_indexed,
    )?;
    let summary = summarize_index(&index);
    // Update in-memory state so subsequent query calls return the fresh index.
    let state = app.state::<DefIndexState>();
    let store_options = DefIndexBuildOptions {
        project_id: effective_project_id,
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    if let Some((settings_hash, file_hashes)) = cache_state_inputs(settings, &store_options) {
        state.store(
            effective_project_id.map(String::from),
            settings_hash,
            file_hashes,
            index,
        );
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_hydrated_project_does_not_substitute_the_active_project_for_a_project_less_capture() {
        use crate::project_model::{LocationKind, RegisteredLocation, SourceType};
        use std::fs;
        use time::OffsetDateTime;

        let project_dir =
            std::env::temp_dir().join(format!("rimedit_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(project_dir.join("Defs")).unwrap();
        fs::write(
            project_dir.join("Defs").join("a.xml"),
            "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>",
        )
        .unwrap();

        let location = RegisteredLocation {
            id: "proj-x".to_string(),
            display_name: "proj-x".to_string(),
            root_path: project_dir.to_string_lossy().to_string(),
            kind: LocationKind::Project,
            source_type: SourceType::Folder,
            read_only: false,
            mod_id: None,
            game_version: None,
            expansion_name: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };
        let settings = ProjectSettings {
            schema_version: 4,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![location],
            active_project_id: Some("proj-x".to_string()),
            save_backups_enabled: false,
        };

        // A project-less (sources-only) hydration captured an empty fingerprint list, since only
        // a `Project`-kind location is registered (never included when `project_id` is `None` --
        // see `included_locations`) and no `Source`-kind locations exist.
        let result = verify_hydrated_project(&settings, None, &[]);

        assert_eq!(
            result,
            FingerprintVerification::Match,
            "must verify against the captured None scope, not silently substitute \
             settings.active_project_id and scan that project's own files"
        );

        fs::remove_dir_all(&project_dir).ok();
    }

    fn status(phase: IndexingPhase, project_id: Option<&str>, generation: u64) -> IndexingStatus {
        IndexingStatus {
            project_id: project_id.map(str::to_string),
            phase,
            cache_verification: crate::def_index::CacheVerification::NotRequired,
            pending_files: 0,
            indexed_defs: 0,
            project_defs: 0,
            source_defs: 0,
            errors: 0,
            message: None,
            updated_at_unix_ms: 0,
            total_files: None,
            processed_files: None,
            current_stage: None,
            current_location_name: None,
            index_built_at_unix_ms: None,
            generation,
        }
    }

    #[test]
    fn a_matching_state_entry_suppresses_initialization() {
        let current = status(IndexingPhase::Idle, None, 0);
        let effective_id = Some("project".to_string());
        assert!(!should_attempt_initialization(
            &current,
            &effective_id,
            true,
            0
        ));
    }

    #[test]
    fn no_matching_state_and_no_in_flight_job_attempts_initialization() {
        let current = status(IndexingPhase::Idle, None, 0);
        let effective_id = Some("project".to_string());
        assert!(should_attempt_initialization(
            &current,
            &effective_id,
            false,
            0
        ));
    }

    #[test]
    fn a_settings_mismatched_complete_status_remains_initialization_eligible() {
        // `Complete` alone (state_has_matching_entry = false, e.g. a stale index whose settings
        // fingerprint no longer matches) must not suppress initialization.
        let current = status(IndexingPhase::Complete, Some("project"), 0);
        let effective_id = Some("project".to_string());
        assert!(should_attempt_initialization(
            &current,
            &effective_id,
            false,
            0
        ));
    }

    #[test]
    fn a_pending_or_running_same_project_job_is_deduplicated() {
        let effective_id = Some("project".to_string());
        for phase in [IndexingPhase::Pending, IndexingPhase::Running] {
            let current = status(phase, Some("project"), 0);
            assert!(!should_attempt_initialization(
                &current,
                &effective_id,
                false,
                0
            ));
        }
    }

    #[test]
    fn a_pending_or_running_status_from_a_superseded_generation_does_not_suppress_initialization() {
        // Regression: a job abandoned mid-setup after a generation bump (e.g.
        // `execute_full_rebuild`'s early-return checks run *after* it already published
        // `Pending`/`Running`) never gets to publish a terminating status for that attempt --
        // `current_status.generation` staying behind the live generation is exactly how this is
        // told apart from a genuinely live in-flight job for the same project.
        let effective_id = Some("project".to_string());
        for phase in [IndexingPhase::Pending, IndexingPhase::Running] {
            let stale = status(phase, Some("project"), 0);
            assert!(
                should_attempt_initialization(&stale, &effective_id, false, 1),
                "a status stamped with a superseded generation must not be trusted as in-flight"
            );
        }
    }

    #[test]
    fn a_pending_job_for_a_different_project_does_not_suppress_initialization() {
        let current = status(IndexingPhase::Pending, Some("other"), 0);
        let effective_id = Some("project".to_string());
        assert!(should_attempt_initialization(
            &current,
            &effective_id,
            false,
            0
        ));
    }
}
