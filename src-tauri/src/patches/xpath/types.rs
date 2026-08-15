//! Serialized IPC types for XPath completion: completion items/kinds, diagnostics, resolved-field
//! reporting, and the top-level [`XPathCompletionResult`] envelope. Kept independent of scanning,
//! predicate parsing, schema navigation, and completion presentation -- everything else in
//! `xpath/` builds these types rather than the other way around.

use serde::{Deserialize, Serialize};

use crate::patches::impact_graph::XPathTarget;
use crate::schema_pack::FieldSchema;

pub(super) const DEF_NAME_SUGGESTION_LIMIT: usize = 20;

/// Conservative server-side cap on Def-type, direct-field, object-field, and field-alias
/// suggestions, applied after deterministic sort and before serialization -- a short or empty
/// prefix must never send hundreds of items over IPC just to render a dropdown. Chosen to
/// comfortably exceed any single Def type's field count while staying small for keyboard
/// navigation/DOM cost; revisit once profiled against real schema-pack cardinalities.
pub(crate) const COMPLETION_ITEM_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum XPathCompletionItemKind {
    /// The `Defs` document root.
    Root,
    /// A Def type name from the schema catalog.
    DefType,
    /// A predicate key template (`defName="`, `@Name="`, `@ParentName="`).
    PredicateKey,
    /// A `defName` value from the Def index.
    DefName,
    /// An `or`/`and` boolean connective continuing a Def-predicate expression, offered either
    /// right after a completed clause while still typing the predicate, or in place of an
    /// already-closed predicate's `]` (see [`super::completion::predicate_close_continuation`]).
    BooleanOperator,
    /// A field declared directly on the target Def type, or -- once the cursor has descended
    /// into an object schema -- a field of that object type (own or inherited).
    Field,
    /// An XML alias for a `Field` suggestion.
    FieldAlias,
    /// The literal `li` element that opens a `listOfLi` or `keyedObjectMap` entry.
    ListItem,
    /// The literal `key` or `value` element inside a `keyedObjectMap` entry (`/li/key`,
    /// `/li/value`). Kept distinct from `Field`/`FieldAlias` since these are structural XML
    /// container names, not schema fields.
    MapEntry,
}

/// One completion suggestion. `insert_text` replaces the input from the result's `replace_from`
/// byte offset up to (not including) its `replace_to` byte offset -- the frontend never needs its
/// own XPath parsing to apply a suggestion, only string splicing of
/// `xpath[..replaceFrom] + item.insertText + xpath[replaceTo..]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathCompletionItem {
    pub insert_text: String,
    pub label: String,
    pub detail: Option<String>,
    pub kind: XPathCompletionItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum XPathDiagnosticSeverity {
    Error,
    Warning,
}

// Not `Eq`: `args` can carry a `DiagnosticArgValue::Float`, and `f64` has no `Eq` impl.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathDiagnostic {
    pub severity: XPathDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(
        default,
        skip_serializing_if = "crate::diagnostics::DiagnosticArgs::is_empty"
    )]
    pub args: crate::diagnostics::DiagnosticArgs,
}

impl XPathDiagnostic {
    pub(super) fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: XPathDiagnosticSeverity::Error,
            code: code.to_string(),
            message: message.into(),
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    pub(super) fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: XPathDiagnosticSeverity::Warning,
            code: code.to_string(),
            message: message.into(),
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    /// Attaches typed args for `code`. Additive on top of the still-English `message`.
    pub(super) fn with_args(mut self, args: crate::diagnostics::DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
    }
}

/// The field a fully- (or mostly-) typed XPath resolves to, for `PatchValueEditor`'s structured
/// subform. This is the *terminal* field on the path -- for a direct Def field it's a field
/// declared directly on `def_type` (an inherited-only match instead produces an
/// `xpath_autocomplete_inherited_field` diagnostic, see module docs); at any deeper, schema-cursor
/// resolved level (e.g. `graphicData/texPath`) it's the nested object-type field the path actually
/// resolved to. `def_type` always stays the *root* Def type for wire compatibility, regardless of
/// how deep `field_name`/`field` themselves are nested.
// `FieldSchema` (from `schema_pack`) is a Serialize-only catalog output type, so anything that
// embeds it -- this struct and `XPathCompletionResult` below -- can only derive `Serialize`, not
// `PartialEq`/`Deserialize`; these values only ever cross the IPC boundary outbound.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathResolvedField {
    pub def_type: String,
    pub field_name: String,
    pub field: FieldSchema,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathCompletionResult {
    /// Byte offset into the input XPath string that completion items replace from -- i.e. the
    /// frontend applies a suggestion via `xpath[..replaceFrom] + item.insertText + xpath[replaceTo..]`.
    pub replace_from: usize,
    /// Byte offset into the input XPath string that completion items replace up to (exclusive).
    /// Bounds the *active token*'s own text -- e.g. a Def type/field identifier run, or a
    /// predicate value up to (not including) its closing quote -- never anything past it, so a
    /// suffix already typed after the caret is preserved. Equals `replace_from` whenever `items`
    /// is empty (there is nothing to apply). When the caret sits at the end of the string this
    /// always equals `xpath.len()`, matching the pre-caret-aware contract exactly.
    pub replace_to: usize,
    /// The bounded, display-ready suggestion list (see [`COMPLETION_ITEM_LIMIT`]).
    pub items: Vec<XPathCompletionItem>,
    /// How many suggestions matched before truncation -- always `>= items.len()`. The frontend
    /// uses this (with `is_truncated`) to render a "showing first N matches; type more to
    /// narrow" status without needing its own count.
    pub total_matches: usize,
    /// Whether `items` was truncated to `total_matches`.
    pub is_truncated: bool,
    pub diagnostics: Vec<XPathDiagnostic>,
    /// The statically-inferred target of the XPath as typed so far (see module docs for how this
    /// differs from `impact_graph::infer_xpath_target`).
    pub target: XPathTarget,
    pub resolved_field: Option<XPathResolvedField>,
}

impl XPathCompletionResult {
    /// Construct a result whose `items` is already within any applicable cap (or inherently
    /// small/unbounded, e.g. structural `li`/`key`/`value` suggestions) -- `total_matches` is
    /// simply `items.len()` and `is_truncated` is always `false`.
    pub(super) fn new(
        replace_from: usize,
        replace_to: usize,
        items: Vec<XPathCompletionItem>,
        diagnostics: Vec<XPathDiagnostic>,
        target: XPathTarget,
        resolved_field: Option<XPathResolvedField>,
    ) -> Self {
        let total_matches = items.len();
        Self {
            replace_from,
            replace_to,
            items,
            total_matches,
            is_truncated: false,
            diagnostics,
            target,
            resolved_field,
        }
    }

    /// Same as [`Self::new`], but truncates `items` (already deterministically sorted by the
    /// caller) to `limit`, recording the true match count and truncation flag.
    pub(super) fn capped(
        replace_from: usize,
        replace_to: usize,
        mut items: Vec<XPathCompletionItem>,
        limit: usize,
        diagnostics: Vec<XPathDiagnostic>,
        target: XPathTarget,
        resolved_field: Option<XPathResolvedField>,
    ) -> Self {
        let total_matches = items.len();
        let is_truncated = total_matches > limit;
        if is_truncated {
            items.truncate(limit);
        }
        Self {
            replace_from,
            replace_to,
            items,
            total_matches,
            is_truncated,
            diagnostics,
            target,
            resolved_field,
        }
    }
}

/// A terminal result: no completion items, a single diagnostic, and an unsupported target.
pub(super) fn terminal(replace_from: usize, diagnostic: XPathDiagnostic) -> XPathCompletionResult {
    XPathCompletionResult::new(
        replace_from,
        replace_from,
        Vec::new(),
        vec![diagnostic],
        XPathTarget::Unsupported,
        None,
    )
}

/// A result with no completion items and no diagnostics -- traversal that silently stops (e.g.
/// past a scalar field) rather than reporting a problem.
pub(super) fn empty(replace_from: usize, target: XPathTarget) -> XPathCompletionResult {
    XPathCompletionResult::new(
        replace_from,
        replace_from,
        Vec::new(),
        Vec::new(),
        target,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::diagnostic_args;

    #[test]
    fn xpath_diagnostic_wire_shape_carries_code_and_args() {
        let diag = XPathDiagnostic::warning(
            "xpath_autocomplete_inherited_field",
            "'label' is declared on a schema parent of 'Building', not on 'Building' itself.",
        )
        .with_args(diagnostic_args([
            ("fieldName", "label".into()),
            ("defType", "Building".into()),
        ]));
        let json = serde_json::to_value(&diag).unwrap();
        assert_eq!(json["code"], "xpath_autocomplete_inherited_field");
        assert_eq!(json["args"]["fieldName"], "label");
    }

    #[test]
    fn xpath_diagnostic_without_args_omits_the_field() {
        let diag = XPathDiagnostic::error("xpath_invalid_syntax", "Unexpected ']'.");
        let json = serde_json::to_value(&diag).unwrap();
        assert!(json.get("args").is_none());
    }
}
