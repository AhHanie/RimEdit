use crate::project_model::AppError;

#[derive(Debug, thiserror::Error)]
pub enum ProjectFileError {
    #[error("Project not found: {0}")]
    ProjectNotFound(String),
    #[error("Project is not editable: {0}")]
    ProjectNotEditable(String),
    #[error("File scan failed: {0}")]
    ScanFailed(String),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("File path is outside project root")]
    FileOutsideRoot,
    #[error("File type is not supported")]
    UnsupportedFile,
    #[error("Invalid file name: {0}")]
    InvalidFileName(String),
    #[error("Path already exists: {0}")]
    PathAlreadyExists(String),
    #[error("Kind mismatch for path: {0}")]
    KindMismatch(String),
    #[error("Cannot modify the project root")]
    CannotModifyRoot,
}

impl From<ProjectFileError> for AppError {
    fn from(e: ProjectFileError) -> Self {
        let code = match &e {
            ProjectFileError::ProjectNotFound(_) => "project_not_found",
            ProjectFileError::ProjectNotEditable(_) => "project_not_editable",
            ProjectFileError::ScanFailed(_) => "project_file_scan_failed",
            ProjectFileError::FileNotFound(_) => "project_file_not_found",
            ProjectFileError::FileOutsideRoot => "project_file_outside_root",
            ProjectFileError::UnsupportedFile => "unsupported_project_file",
            ProjectFileError::InvalidFileName(_) => "invalid_file_name",
            ProjectFileError::PathAlreadyExists(_) => "path_already_exists",
            ProjectFileError::KindMismatch(_) => "kind_mismatch",
            ProjectFileError::CannotModifyRoot => "cannot_modify_root",
        };
        AppError {
            code: code.to_string(),
            message: e.to_string(),
            details: None,
        }
    }
}
