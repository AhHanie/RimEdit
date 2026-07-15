use crate::diagnostics::diagnostic_args;
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
        // `ReadFailed`/`WriteFailed` wrap arbitrary IO-error text; the remaining variants carry a
        // clean literal identifier.
        let args = match &e {
            FormViewStoreError::ViewNotFound(id) => {
                diagnostic_args([("viewId", id.as_str().into())])
            }
            FormViewStoreError::DuplicateName(name) => {
                diagnostic_args([("viewName", name.as_str().into())])
            }
            FormViewStoreError::ProjectIdMismatch(id) => {
                diagnostic_args([("projectId", id.as_str().into())])
            }
            FormViewStoreError::UnsupportedNewerVersion(version) => {
                diagnostic_args([("schemaVersion", (*version as i64).into())])
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
    fn duplicate_name_carries_view_name_arg() {
        let err: AppError = FormViewStoreError::DuplicateName("My View".to_string()).into();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "form_view_duplicate_name");
        assert_eq!(json["args"]["viewName"], "My View");
    }

    #[test]
    fn unsupported_newer_version_carries_schema_version_arg() {
        let err: AppError = FormViewStoreError::UnsupportedNewerVersion(7).into();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["args"]["schemaVersion"], 7);
    }
}
