mod debounce;
pub(crate) mod events;
pub(crate) mod jobs;
pub(crate) mod watchers;

pub(crate) use jobs::{
    enqueue_file_change, enqueue_file_delete, enqueue_folder_delete, enqueue_full_rebuild,
    get_indexing_status, start_worker, IndexJobReason, IndexingServiceState,
};
pub(crate) use watchers::{restart_watchers_for_settings, IndexWatcherState};

use crate::def_index::DefIndexState;
use crate::project_model::{AppError, ProjectSettings};
use tauri::{AppHandle, Manager};

pub(crate) fn restart_for_settings(
    app: &AppHandle,
    settings: &ProjectSettings,
) -> Result<(), AppError> {
    app.state::<DefIndexState>().increment_generation();
    // Watcher restart failures for non-active locations are non-fatal;
    // the returned error only fires for the active project location.
    restart_watchers_for_settings(app, settings)
}
