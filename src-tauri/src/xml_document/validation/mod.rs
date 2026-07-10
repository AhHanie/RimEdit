mod about;
mod context;
mod diagnostics;
mod document;
mod fields;
mod references;
mod scalar;
mod shapes;
mod xml;

pub use about::validate_about_metadata_document;
pub use context::ValidationContext;

use crate::xml_document::diagnostics::ValidationDiagnostic;
use crate::xml_document::model::XmlDocument;

pub fn validate_document(
    doc: &XmlDocument,
    context: &ValidationContext<'_>,
) -> Vec<ValidationDiagnostic> {
    document::validate_document(doc, context)
}
