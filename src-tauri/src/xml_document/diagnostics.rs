use serde::{Deserialize, Serialize};

use crate::diagnostics::DiagnosticArgs;

use super::model::XmlNodeId;

#[derive(Clone, Debug, Default)]
pub(crate) struct XmlSpan {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

/// A parse-time failure. `code` normalizes the underlying `quick-xml` failure into a stable,
/// documented identifier (see `docs/i18n/diagnostic-codes.md`); `message` keeps the raw
/// parser-library text as an English technical detail (Plan.md: "raw parser text stays as
/// optional English technical detail for logs/support, not normal UI rendering").
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseDiagnostic {
    pub relative_path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub byte_offset: Option<usize>,
    pub message: String,
    pub code: String,
    #[serde(default, skip_serializing_if = "DiagnosticArgs::is_empty")]
    pub args: DiagnosticArgs,
}

impl ParseDiagnostic {
    pub(crate) fn new(
        relative_path: impl Into<String>,
        line: Option<usize>,
        column: Option<usize>,
        byte_offset: Option<usize>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            line,
            column,
            byte_offset,
            message: message.into(),
            code: code.into(),
            args: DiagnosticArgs::new(),
        }
    }

    pub(crate) fn with_args(mut self, args: DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
    }
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
    #[serde(default, skip_serializing_if = "DiagnosticArgs::is_empty")]
    pub args: DiagnosticArgs,
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
            args: DiagnosticArgs::new(),
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
            args: DiagnosticArgs::new(),
        }
    }

    /// Attaches typed args for `code` (see `crate::diagnostics` module docs). Additive on top of
    /// the still-English `message`.
    pub(crate) fn with_args(mut self, args: DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
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

#[cfg(test)]
mod diagnostic_ref_wire_tests {
    use super::*;
    use crate::diagnostics::diagnostic_args;

    #[test]
    fn parse_diagnostic_wire_shape_carries_code_and_args() {
        let diag = ParseDiagnostic::new(
            "About/About.xml",
            None,
            None,
            None,
            "parse_invalid_root_element_count",
            "document has 2 root elements; exactly one is required",
        )
        .with_args(diagnostic_args([("elementCount", 2usize.into())]));
        let json = serde_json::to_value(&diag).unwrap();
        assert_eq!(json["code"], "parse_invalid_root_element_count");
        assert_eq!(json["args"]["elementCount"], 2);
    }

    #[test]
    fn parse_diagnostic_without_args_omits_the_field() {
        let diag = ParseDiagnostic::new("x.xml", None, None, None, "parse_xml_syntax_error", "bad");
        let json = serde_json::to_value(&diag).unwrap();
        assert!(json.get("args").is_none());
    }

    #[test]
    fn validation_diagnostic_wire_shape_carries_code_and_args() {
        let diag = ValidationDiagnostic::warning(
            "Defs/Things.xml",
            None,
            None,
            None,
            "validation_missing_required_field",
            "Required field 'label' is missing from ThingDef.",
        )
        .with_args(diagnostic_args([
            ("fieldName", "label".into()),
            ("defType", "ThingDef".into()),
        ]));
        let json = serde_json::to_value(&diag).unwrap();
        assert_eq!(json["code"], "validation_missing_required_field");
        assert_eq!(json["args"]["fieldName"], "label");
        assert_eq!(json["args"]["defType"], "ThingDef");
    }

    #[test]
    fn validation_diagnostic_without_args_omits_the_field() {
        let diag = ValidationDiagnostic::error("x.xml", None, None, None, "some_code", "message");
        let json = serde_json::to_value(&diag).unwrap();
        assert!(json.get("args").is_none());
    }
}
