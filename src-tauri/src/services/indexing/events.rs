use crate::def_index::IndexingStatus;
use tauri::{AppHandle, Emitter};

pub(crate) const INDEXING_EVENT: &str = "rimedit://indexing-status";

pub(crate) fn emit_indexing_status(app: &AppHandle, status: &IndexingStatus) {
    let _ = app.emit(INDEXING_EVENT, status);
}
