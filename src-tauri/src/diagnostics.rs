//! Shared, localization-ready diagnostic reference model used across every backend diagnostic
//! family (`AppError`, `ParseDiagnostic`, `ValidationDiagnostic`, `SchemaLoadDiagnostic`,
//! `PatchDiagnostic`, `XPathDiagnostic`, `ApplyDiagnostic`, `InheritanceDiagnostic`,
//! `PatchPreviewConflictDiagnostic`). See `docs/i18n/diagnostic-codes.md` for the code-ownership
//! registry and `docs/i18n/issues/03-structured-backend-diagnostics.md` for the issue this
//! implements.
//!
//! ## Wire shape
//!
//! Every diagnostic family keeps its existing `code: String` field unchanged and gains a sibling
//! `args: DiagnosticArgs` field, omitted from JSON entirely when empty
//! (`#[serde(skip_serializing_if = "DiagnosticArgs::is_empty")]`). `code` is a stable, namespaced
//! identifier; `args` carries the same literal values already used to assemble the (still
//! present, still English) `message`/`Display` text, so a future frontend renderer (issue 04) can
//! look up `code` in a translation catalog and interpolate `args` without parsing `message`.
//! `message`, `thiserror` strings, `Display` impls, and `eprintln!`/log output all stay English --
//! this module never localizes anything itself, and adding `args` to a family is never required
//! to also translate anything.
//!
//! ## Naming policy
//!
//! Every diagnostic code in this codebase already follows one flat, stable convention established
//! well before this module existed: `snake_case`, prefixed by an owning-domain word, e.g.
//! `validation_missing_required_field`, `about_invalid_root`, `settings_read_failed`,
//! `patch_apply_xpath_no_match`, `inheritance_missing_parent`,
//! `xpath_autocomplete_unsupported_pattern`. Many of these are already asserted on by exact string
//! in existing tests and documented as stable in doc comments, so this module keeps that
//! convention -- rather than introducing `Plan.md`'s illustrative dotted `domain.condition` form --
//! instead of renaming already-shipped, already-tested identifiers. See this issue's
//! "Implementation notes" for the full rationale. New codes should keep following the same
//! pattern: pick the owning domain prefix from `docs/i18n/diagnostic-codes.md` (or add a new one
//! there) and describe the specific condition after it.
//!
//! ## Argument values
//!
//! [`DiagnosticArgValue`] is intentionally a small, closed set of scalar/list shapes -- exactly
//! the literal, untranslated data every existing `message`/`Display` implementation already
//! formats into English text (field names, def types/names, file paths, xpath strings, counts).
//! It is never an assembled sentence fragment, and never itself translated.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A single typed, literal interpolation value for a [`DiagnosticArgs`] map. Untagged so the wire
/// form is the plain JSON scalar/array a frontend interpolation call expects (e.g. `"label"`,
/// `3`, `true`, `["A", "B"]`), never `{ "type": "text", "value": "label" }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DiagnosticArgValue {
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    /// A list of literal strings (e.g. multiple candidate mod names). Never a list of translated
    /// fragments.
    List(Vec<String>),
}

impl From<&str> for DiagnosticArgValue {
    fn from(v: &str) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<String> for DiagnosticArgValue {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<&String> for DiagnosticArgValue {
    fn from(v: &String) -> Self {
        Self::Text(v.clone())
    }
}

impl From<usize> for DiagnosticArgValue {
    fn from(v: usize) -> Self {
        Self::Int(v as i64)
    }
}

impl From<i64> for DiagnosticArgValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<i32> for DiagnosticArgValue {
    fn from(v: i32) -> Self {
        Self::Int(v as i64)
    }
}

impl From<f64> for DiagnosticArgValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<bool> for DiagnosticArgValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<Vec<String>> for DiagnosticArgValue {
    fn from(v: Vec<String>) -> Self {
        Self::List(v)
    }
}

impl From<&[String]> for DiagnosticArgValue {
    fn from(v: &[String]) -> Self {
        Self::List(v.to_vec())
    }
}

/// Typed interpolation arguments for a diagnostic `code`, keyed by argument name (e.g.
/// `"fieldName"`, `"defType"`, `"count"`). Deterministically ordered (`BTreeMap`) so serialized
/// fixtures and snapshot tests are stable across runs.
pub type DiagnosticArgs = BTreeMap<String, DiagnosticArgValue>;

/// Builds a [`DiagnosticArgs`] map from literal `(name, value)` pairs in one expression, e.g.
/// `diagnostic_args([("fieldName", field.into())])`. A thin convenience over collecting an array
/// into a `BTreeMap` -- kept as a named helper so call sites read as "these are diagnostic args",
/// not an anonymous map literal.
pub fn diagnostic_args<const N: usize>(pairs: [(&str, DiagnosticArgValue); N]) -> DiagnosticArgs {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// A stable diagnostic code paired with its typed literal arguments. Constructed only through
/// [`DiagnosticRef::code`] plus [`DiagnosticRef::with_arg`], so a producer can never end up
/// attaching args without a code. Diagnostic families do not embed
/// this as a nested wire object -- each keeps its own flat `code`/`args` fields (see module docs)
/// -- but their constructors build one of these internally, and less-common producers (custom
/// codes assembled ad hoc) can use it directly before destructuring into a family's fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiagnosticRef {
    pub code: String,
    pub args: DiagnosticArgs,
}

impl DiagnosticRef {
    pub fn code(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            args: DiagnosticArgs::new(),
        }
    }

    pub fn with_arg(
        mut self,
        key: impl Into<String>,
        value: impl Into<DiagnosticArgValue>,
    ) -> Self {
        self.args.insert(key.into(), value.into());
        self
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_arg_serializes_as_plain_string() {
        let value: DiagnosticArgValue = "label".into();
        assert_eq!(serde_json::to_string(&value).unwrap(), "\"label\"");
    }

    #[test]
    fn int_arg_serializes_as_plain_number() {
        let value: DiagnosticArgValue = 3usize.into();
        assert_eq!(serde_json::to_string(&value).unwrap(), "3");
    }

    #[test]
    fn bool_arg_serializes_as_plain_boolean() {
        let value: DiagnosticArgValue = true.into();
        assert_eq!(serde_json::to_string(&value).unwrap(), "true");
    }

    #[test]
    fn list_arg_serializes_as_plain_array() {
        let value: DiagnosticArgValue = vec!["A".to_string(), "B".to_string()].into();
        assert_eq!(serde_json::to_string(&value).unwrap(), "[\"A\",\"B\"]");
    }

    #[test]
    fn diagnostic_args_helper_builds_a_map() {
        let args = diagnostic_args([("fieldName", "label".into()), ("count", 2usize.into())]);
        assert_eq!(args.len(), 2);
        assert_eq!(
            args["fieldName"],
            DiagnosticArgValue::Text("label".to_string())
        );
        assert_eq!(args["count"], DiagnosticArgValue::Int(2));
    }

    #[test]
    fn diagnostic_ref_builder_accumulates_args() {
        let reference = DiagnosticRef::code("validation_missing_required_field")
            .with_arg("fieldName", "label")
            .with_arg("defType", "ThingDef");
        assert_eq!(reference.code, "validation_missing_required_field");
        assert_eq!(reference.args.len(), 2);
        assert_eq!(
            reference.args["fieldName"],
            DiagnosticArgValue::Text("label".to_string())
        );
    }

    #[test]
    fn args_map_is_deterministically_ordered() {
        let args = diagnostic_args([("zField", "z".into()), ("aField", "a".into())]);
        let keys: Vec<&String> = args.keys().collect();
        assert_eq!(keys, vec!["aField", "zField"]);
    }
}
