use crate::project_model::AppError;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub(crate) const APP_FOLDER_NAME: &str = "RimEdit";

pub(crate) fn app_storage_dir(app: &AppHandle, code: &str) -> Result<PathBuf, AppError> {
    app.path()
        .config_dir()
        .map(|d| d.join(APP_FOLDER_NAME))
        .map_err(|e| AppError {
            code: code.to_string(),
            message: format!("Cannot resolve app storage directory: {}", e),
            details: None,
        })
}

#[cfg(test)]
pub(crate) fn app_storage_dir_from_config_root(config_root: &std::path::Path) -> PathBuf {
    config_root.join(APP_FOLDER_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn storage_dir_joins_rimedit() {
        let root = Path::new("/home/user/.config");
        assert_eq!(
            app_storage_dir_from_config_root(root),
            PathBuf::from("/home/user/.config/RimEdit")
        );
    }
}
