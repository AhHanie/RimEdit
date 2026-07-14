use crate::project_model::AppError;

#[derive(Debug, thiserror::Error)]
pub enum FormViewStoreError {
    #[error("Custom Form View not found: {0}")]
    ViewNotFound(String),
    #[error("Failed to read Form View store: {0}")]
    ReadFailed(String),
    #[error("Failed to write Form View store: {0}")]
    WriteFailed(String),
    #[error(
        "A custom Form View named '{0}' already exists for this project/game version/Def type."
    )]
    DuplicateName(String),
    #[error("Form View name must not be blank.")]
    BlankName,
    #[error("Form View store project id mismatch: {0}")]
    ProjectIdMismatch(String),
    #[error(
        "The Form View store was saved by a newer version of RimEdit (schema version {0}); \
         opening read-only with no custom views until the app is upgraded."
    )]
    UnsupportedNewerVersion(u32),
}

impl From<FormViewStoreError> for AppError {
    fn from(e: FormViewStoreError) -> Self {
        let code = match &e {
            FormViewStoreError::ViewNotFound(_) => "form_view_not_found",
            FormViewStoreError::ReadFailed(_) => "form_view_read_failed",
            FormViewStoreError::WriteFailed(_) => "form_view_write_failed",
            FormViewStoreError::DuplicateName(_) => "form_view_duplicate_name",
            FormViewStoreError::BlankName => "form_view_invalid_name",
            FormViewStoreError::ProjectIdMismatch(_) => "form_view_project_id_mismatch",
            FormViewStoreError::UnsupportedNewerVersion(_) => "form_view_unsupported_version",
        };
        AppError {
            code: code.to_string(),
            message: e.to_string(),
            details: None,
        }
    }
}
