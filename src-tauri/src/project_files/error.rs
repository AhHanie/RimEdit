use crate::diagnostics::diagnostic_args;
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
        // Only variants whose payload is a clean literal identifier (a project id or a relative
        // path) get typed args; `ScanFailed` wraps arbitrary IO-error text, which Plan.md says to
        // keep as unbounded English technical detail rather than force into a translation key.
        let args = match &e {
            ProjectFileError::ProjectNotFound(id) | ProjectFileError::ProjectNotEditable(id) => {
                diagnostic_args([("projectId", id.as_str().into())])
            }
            ProjectFileError::FileNotFound(path)
            | ProjectFileError::InvalidFileName(path)
            | ProjectFileError::PathAlreadyExists(path)
            | ProjectFileError::KindMismatch(path) => {
                diagnostic_args([("path", path.as_str().into())])
            }
            _ => crate::diagnostics::DiagnosticArgs::new(),
        };
        AppError {
            code: code.to_string(),
            message: e.to_string(),
            details: None,
            args,
        }
    }
}

#[cfg(test)]
mod diagnostic_ref_wire_tests {
    use super::*;

    #[test]
    fn file_not_found_carries_path_arg() {
        let err: AppError = ProjectFileError::FileNotFound("Defs/Foo.xml".to_string()).into();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "project_file_not_found");
        assert_eq!(json["args"]["path"], "Defs/Foo.xml");
    }

    #[test]
    fn scan_failed_omits_args() {
        let err: AppError = ProjectFileError::ScanFailed("boom".to_string()).into();
        let json = serde_json::to_value(&err).unwrap();
        assert!(json.get("args").is_none());
    }
}
