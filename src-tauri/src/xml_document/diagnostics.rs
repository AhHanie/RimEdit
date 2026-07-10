use serde::{Deserialize, Serialize};

use super::model::XmlNodeId;

#[derive(Clone, Debug, Default)]
pub(crate) struct XmlSpan {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseDiagnostic {
    pub relative_path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub byte_offset: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationDiagnostic {
    pub relative_path: String,
    pub node_id: Option<XmlNodeId>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub code: String,
    pub def_type: Option<String>,
    pub def_name: Option<String>,
    pub field_path: Option<String>,
    pub blocking: bool,
}

impl ValidationDiagnostic {
    pub(crate) fn error(
        relative_path: impl Into<String>,
        node_id: Option<XmlNodeId>,
        line: Option<usize>,
        column: Option<usize>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            node_id,
            line,
            column,
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            code: code.into(),
            def_type: None,
            def_name: None,
            field_path: None,
            blocking: true,
        }
    }

    pub(crate) fn warning(
        relative_path: impl Into<String>,
        node_id: Option<XmlNodeId>,
        line: Option<usize>,
        column: Option<usize>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            node_id,
            line,
            column,
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            code: code.into(),
            def_type: None,
            def_name: None,
            field_path: None,
            blocking: false,
        }
    }

    pub(crate) fn with_def(mut self, def_type: &str, def_name: Option<&str>) -> Self {
        self.def_type = Some(def_type.to_string());
        self.def_name = def_name.map(str::to_string);
        self
    }

    pub(crate) fn with_field_path(mut self, field_path: impl Into<String>) -> Self {
        self.field_path = Some(field_path.into());
        self
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    // Reserved for ValidationDiagnostic once schema-aware validation emits informational notes.
    #[allow(dead_code)]
    Info,
}

pub(crate) fn build_newline_index(source: &[u8]) -> Vec<usize> {
    source
        .iter()
        .enumerate()
        .filter_map(|(i, &b)| if b == b'\n' { Some(i) } else { None })
        .collect()
}

pub(crate) fn offset_to_line_col(newline_index: &[usize], offset: usize) -> (usize, usize) {
    let line_idx = newline_index.partition_point(|&nl| nl < offset);
    let line = line_idx + 1;
    let col_start = if line_idx == 0 {
        0
    } else {
        newline_index[line_idx - 1] + 1
    };
    let col = offset - col_start + 1;
    (line, col)
}

pub(crate) fn make_span(newline_index: &[usize], start: usize, end: usize) -> XmlSpan {
    let (line, column) = offset_to_line_col(newline_index, start);
    XmlSpan {
        start,
        end,
        line,
        column,
    }
}
