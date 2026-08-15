use crate::project_model::AppError;
use crate::services::app_paths;
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

const OPEN_APP_DATA_FOLDER_FAILED: &str = "open_app_data_folder_failed";

fn ensure_app_data_dir(path: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(path).map_err(|e| AppError {
        code: OPEN_APP_DATA_FOLDER_FAILED.to_string(),
        message: format!(
            "Cannot create app data directory at {}: {}",
            path.display(),
            e
        ),
        details: None,
        args: crate::diagnostics::DiagnosticArgs::new(),
    })
}

#[tauri::command]
pub fn open_app_data_folder(app: AppHandle) -> Result<(), AppError> {
    let path = app_paths::app_storage_dir(&app, OPEN_APP_DATA_FOLDER_FAILED)?;
    ensure_app_data_dir(&path)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| AppError {
            code: OPEN_APP_DATA_FOLDER_FAILED.to_string(),
            message: format!(
                "Cannot open app data directory at {}: {}",
                path.display(),
                e
            ),
            details: None,
            args: crate::diagnostics::DiagnosticArgs::new(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_missing_nested_data_directory() {
        let root = tempdir().expect("tempdir");
        let nested = root.path().join("nested").join("RimEdit");

        let result = ensure_app_data_dir(&nested);

        assert!(result.is_ok());
        assert!(nested.is_dir());
    }

    #[test]
    fn accepts_already_existing_directory() {
        let root = tempdir().expect("tempdir");
        let existing = root.path().join("RimEdit");
        std::fs::create_dir_all(&existing).expect("pre-create dir");

        let result = ensure_app_data_dir(&existing);

        assert!(result.is_ok());
        assert!(existing.is_dir());
    }

    #[test]
    fn returns_error_code_when_file_occupies_required_path() {
        let root = tempdir().expect("tempdir");
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, b"not a directory").expect("write file");
        let target = blocked.join("RimEdit");

        let result = ensure_app_data_dir(&target);

        let err = result.expect_err("expected failure when path segment is a file");
        assert_eq!(err.code, OPEN_APP_DATA_FOLDER_FAILED);
        assert!(err.message.contains(&target.display().to_string()));
    }
}
