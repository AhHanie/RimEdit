use crate::project_model::AppError;

#[derive(Debug, thiserror::Error)]
pub enum DefTemplateError {
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    #[error("Failed to read template store: {0}")]
    ReadFailed(String),
    #[error("Failed to write template store: {0}")]
    WriteFailed(String),
}

impl From<DefTemplateError> for AppError {
    fn from(e: DefTemplateError) -> Self {
        let code = match &e {
            DefTemplateError::TemplateNotFound(_) => "def_template_not_found",
            DefTemplateError::ReadFailed(_) => "def_template_read_failed",
            DefTemplateError::WriteFailed(_) => "def_template_write_failed",
        };
        AppError {
            code: code.to_string(),
            message: e.to_string(),
            details: None,
        }
    }
}
