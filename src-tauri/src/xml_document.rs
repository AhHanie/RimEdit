mod about;
mod def_summary;
mod diagnostics;
mod edit;
pub(crate) mod model;
mod parser;
mod serializer;
mod validation;

#[cfg(test)]
mod tests;

pub use diagnostics::{ParseDiagnostic, ValidationDiagnostic};
#[cfg(test)]
pub use edit::XmlEditError;
pub use edit::{apply_xml_edit, InitialElement, NameValuePair, XmlEdit, XmlEditContext};
pub use model::{
    build_editor_view, XmlDocument, XmlDocumentLoadResult, XmlDocumentProfile,
    XmlEditorDocumentLoadResult,
};
pub use parser::{parse_to_document, parse_xml_document};
pub use serializer::serialize_xml_document;
pub use validation::{validate_about_metadata_document, validate_document, ValidationContext};
