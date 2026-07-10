use crate::def_index::IndexedDef;
use crate::xml_document::diagnostics::ValidationDiagnostic;
use crate::xml_document::model::{XmlDocument, XmlNodeId};

pub(super) fn warning_at_node(
    doc: &XmlDocument,
    node_id: XmlNodeId,
    def_type: &str,
    def_name: Option<&str>,
    code: &str,
    message: String,
) -> ValidationDiagnostic {
    ValidationDiagnostic::warning(
        doc.relative_path.clone(),
        Some(node_id),
        Some(doc.nodes[node_id].span.line),
        Some(doc.nodes[node_id].span.column),
        code,
        message,
    )
    .with_def(def_type, def_name)
}

pub(super) fn error_at_node(
    doc: &XmlDocument,
    node_id: XmlNodeId,
    def_type: &str,
    def_name: Option<&str>,
    code: &str,
    message: String,
) -> ValidationDiagnostic {
    ValidationDiagnostic::error(
        doc.relative_path.clone(),
        Some(node_id),
        Some(doc.nodes[node_id].span.line),
        Some(doc.nodes[node_id].span.column),
        code,
        message,
    )
    .with_def(def_type, def_name)
}

pub(super) fn format_index_occurrences(occurrences: &[&IndexedDef]) -> String {
    occurrences
        .iter()
        .map(|o| match (o.line, o.column) {
            (Some(line), Some(column)) => {
                format!(
                    "{}:{}:{}:{}",
                    o.source.location_name, o.relative_path, line, column
                )
            }
            (Some(line), None) => {
                format!("{}:{}:{}", o.source.location_name, o.relative_path, line)
            }
            _ if o.node_id.is_some() => format!(
                "{}:{}#node{}",
                o.source.location_name,
                o.relative_path,
                o.node_id.unwrap()
            ),
            _ => format!("{}:{}", o.source.location_name, o.relative_path),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
