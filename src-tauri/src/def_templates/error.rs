use crate::diagnostics::diagnostic_args;
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
        // `ReadFailed`/`WriteFailed` wrap arbitrary IO-error text; only `TemplateNotFound`'s
        // payload is a clean literal identifier.
        let args = match &e {
            DefTemplateError::TemplateNotFound(id) => {
                diagnostic_args([("templateId", id.as_str().into())])
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
    fn template_not_found_carries_template_id_arg() {
        let err: AppError = DefTemplateError::TemplateNotFound("tpl-1".to_string()).into();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "def_template_not_found");
        assert_eq!(json["args"]["templateId"], "tpl-1");
    }
}
