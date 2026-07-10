use std::path::Path;
use std::sync::Mutex;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Manager};

use crate::project_model::{AppError, ProjectSettings};
use crate::services::app_paths;

use super::jobs::{IndexingServiceState, WatcherRawEvent};

pub(crate) struct IndexWatcherState {
    watchers: Mutex<Vec<RecommendedWatcher>>,
}

impl Default for IndexWatcherState {
    fn default() -> Self {
        Self {
            watchers: Mutex::new(Vec::new()),
        }
    }
}

pub(crate) fn restart_watchers_for_settings(
    app: &AppHandle,
    settings: &ProjectSettings,
) -> Result<(), AppError> {
    let watcher_state = app.state::<IndexWatcherState>();
    let mut guard = watcher_state.watchers.lock().unwrap();

    // Drop all existing watchers
    guard.clear();

    let app_data_dir = app_paths::app_storage_dir(app, "watcher_setup").ok();

    let mut active_project_watch_error: Option<AppError> = None;

    for location in &settings.locations {
        let root = match std::fs::canonicalize(&location.root_path) {
            Ok(p) => p,
            Err(e) => {
                let err = AppError {
                    code: "watcher_path_invalid".into(),
                    message: format!("Cannot watch '{}': {}", location.root_path, e),
                    details: None,
                };
                // Only fail hard for the active project location
                let is_active = settings
                    .active_project_id
                    .as_deref()
                    .map(|id| id == location.id)
                    .unwrap_or(false);
                if is_active {
                    active_project_watch_error = Some(err);
                }
                continue;
            }
        };

        let watcher_sender = app.state::<IndexingServiceState>().watcher_sender.clone();
        let root_clone = root.clone();
        let location_id = location.id.clone();
        let app_data_dir_clone = app_data_dir.clone();

        let watcher_result =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                handle_fs_event(
                    &res,
                    &root_clone,
                    &location_id,
                    app_data_dir_clone.as_deref(),
                    &watcher_sender,
                );
            });

        let mut watcher = match watcher_result {
            Ok(w) => w,
            Err(e) => {
                let err = AppError {
                    code: "watcher_setup_failed".into(),
                    message: format!("Failed to create watcher for '{}': {}", root.display(), e),
                    details: None,
                };
                let is_active = settings
                    .active_project_id
                    .as_deref()
                    .map(|id| id == location.id)
                    .unwrap_or(false);
                if is_active {
                    active_project_watch_error = Some(err);
                }
                continue;
            }
        };

        if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
            let err = AppError {
                code: "watcher_watch_failed".into(),
                message: format!("Failed to watch '{}': {}", root.display(), e),
                details: None,
            };
            let is_active = settings
                .active_project_id
                .as_deref()
                .map(|id| id == location.id)
                .unwrap_or(false);
            if is_active {
                active_project_watch_error = Some(err);
            }
            continue;
        }

        guard.push(watcher);
    }

    if let Some(err) = active_project_watch_error {
        return Err(err);
    }

    Ok(())
}

fn handle_fs_event(
    res: &notify::Result<notify::Event>,
    root: &Path,
    location_id: &str,
    app_data_dir: Option<&Path>,
    sender: &tokio::sync::mpsc::UnboundedSender<WatcherRawEvent>,
) {
    let event = match res {
        Ok(e) => e,
        Err(_) => return,
    };

    for path in &event.paths {
        // Skip app data directory events
        if let Some(data_dir) = app_data_dir {
            if path.starts_with(data_dir) {
                continue;
            }
        }

        // Try to compute the relative path from the watch root
        // On Windows, canonicalize may fail for newly-created or deleted files, so
        // we try a best-effort strip without canonicalization if needed.
        let relative = relative_path_from_root(path, root);
        let Some(rel_str) = relative else { continue };

        let is_xml = rel_str.to_ascii_lowercase().ends_with(".xml");
        let is_rename_from = matches!(
            event.kind,
            EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::From
            ))
        );

        match event.kind {
            // Rename(From) means the old path disappeared - treat as deletion
            EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::From)) => {
                if is_xml {
                    let _ = sender.send(WatcherRawEvent::Deleted {
                        location_id: location_id.to_string(),
                        relative_path: rel_str,
                    });
                } else {
                    let _ = sender.send(WatcherRawEvent::FolderDeleted {
                        location_id: location_id.to_string(),
                        folder_prefix: rel_str,
                    });
                }
            }
            EventKind::Create(_) | EventKind::Modify(_) => {
                // Only XML file creates/modifies matter; non-XML and Rename(To) of XML will
                // produce a Changed event, while Rename(To) of a directory is ignored.
                if is_xml && !is_rename_from {
                    let _ = sender.send(WatcherRawEvent::Changed {
                        location_id: location_id.to_string(),
                        relative_path: rel_str,
                    });
                }
            }
            EventKind::Remove(_) => {
                if is_xml {
                    let _ = sender.send(WatcherRawEvent::Deleted {
                        location_id: location_id.to_string(),
                        relative_path: rel_str,
                    });
                } else {
                    // Non-XML remove is likely a directory removal
                    let _ = sender.send(WatcherRawEvent::FolderDeleted {
                        location_id: location_id.to_string(),
                        folder_prefix: rel_str,
                    });
                }
            }
            EventKind::Other | EventKind::Access(_) | EventKind::Any => {}
        }
    }
}

fn relative_path_from_root(path: &Path, root: &Path) -> Option<String> {
    // First try with canonicalize for existing files
    let relative = if let Ok(canonical) = std::fs::canonicalize(path) {
        canonical.strip_prefix(root).ok()?.to_path_buf()
    } else {
        // For deleted/new files, try direct strip
        path.strip_prefix(root).ok()?.to_path_buf()
    };

    // Convert to forward-slash string
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|c| {
            if let std::path::Component::Normal(s) = c {
                s.to_str()
            } else {
                None
            }
        })
        .collect();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}
