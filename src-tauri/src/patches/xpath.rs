//! Schema-aware XPath completion and target inference for `PatchPathInput`, per
//! `docs/patches-editor/05-xpath-autocomplete-and-target-inference.md`.
//!
//! This is a *different, more permissive* conservative subset than [`super::impact_graph`]'s
//! static target inference: `impact_graph::infer_xpath_target` only trusts `Defs/<DefType>` and
//! `Defs/<DefType>[defName="<Name>"]` because it feeds patch-conflict/impact analysis, where an
//! `@Name`/`@ParentName` predicate can't be resolved to one concrete `defName`-keyed Def. Here,
//! `@Name="..."`/`@ParentName="..."` predicates are recognized as *supported autocomplete syntax*
//! (per the issue's "Supported First-Pass Patterns") because knowing the Def *type* alone is
//! enough to keep offering field completions and a value-subform target -- we just can't narrow
//! `XPathTarget` down to a specific `defName` from them, so they resolve to
//! `XPathTarget::DefType` rather than `XPathTarget::Def`.
//!
//! The input is assumed to be a plain, no-cursor-tracking text field: every function here treats
//! the *end* of the string as the position being completed (mirrors how `ReferencePicker` treats
//! its whole current value as the live query). Completion is only offered for the conservative
//! path shapes the issue documents; anything else -- axes, functions, wildcards, multiple
//! predicates, `//`, attribute-node path segments, deeper-than-one-field nesting -- is reported as
//! [`XPathDiagnostic`] with code `xpath_autocomplete_unsupported_pattern` and an empty completion
//! list, but is never rejected outright: the XPath stays editable and (per the Plan's XPath
//! evaluation/autocomplete boundary) previewable by a fuller backend XML library later.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::def_index::{suggest_def_references, DefIndex};
use crate::schema_pack::{DefTypeSchema, FieldSchema, ReferenceScope, SchemaCatalog};

use super::impact_graph::{is_valid_identifier, XPathTarget};

const DEF_NAME_SUGGESTION_LIMIT: usize = 20;

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
    /// A field declared directly on the target Def type.
    Field,
    /// An XML alias for a field declared directly on the target Def type.
    FieldAlias,
}

/// One completion suggestion. `insert_text` replaces the input from the result's `replace_from`
/// byte offset to the end of the string -- the frontend never needs its own XPath parsing to
/// apply a suggestion, only string splicing.
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
    fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: XPathDiagnosticSeverity::Error,
            code: code.to_string(),
            message: message.into(),
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: XPathDiagnosticSeverity::Warning,
            code: code.to_string(),
            message: message.into(),
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    /// Attaches typed args for `code`. Additive on top of the still-English `message`.
    fn with_args(mut self, args: crate::diagnostics::DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
    }
}

/// The field a fully- (or mostly-) typed XPath resolves to, for `PatchValueEditor`'s structured
/// subform (a later patches-editor issue). Only ever set for a field declared *directly* on the
/// resolved Def type -- an inherited-only match instead produces an
/// `xpath_autocomplete_inherited_field` diagnostic (see module docs).
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
    /// frontend applies a suggestion via `xpath[..replaceFrom] + item.insertText`.
    pub replace_from: usize,
    pub items: Vec<XPathCompletionItem>,
    pub diagnostics: Vec<XPathDiagnostic>,
    /// The statically-inferred target of the XPath as typed so far (see module docs for how this
    /// differs from `impact_graph::infer_xpath_target`).
    pub target: XPathTarget,
    pub resolved_field: Option<XPathResolvedField>,
}

/// Compute completions, diagnostics, and target/field inference for a `PatchPathInput` value.
pub fn complete_patch_xpath(
    catalog: &SchemaCatalog,
    def_index: &DefIndex,
    xpath: &str,
) -> XPathCompletionResult {
    let mut segments = match split_segments(xpath) {
        Ok(segments) => segments,
        Err(diagnostic) => return terminal(xpath.len(), diagnostic),
    };

    // A single leading '/' is just absolute-path notation, not a `//` (descendant) step.
    if segments.first().is_some_and(|s| s.start == s.end) {
        segments.remove(0);
    }
    let trailing_slash = segments.last().is_some_and(|s| s.start == s.end);
    if trailing_slash {
        segments.pop();
    }
    if segments.iter().any(|s| s.start == s.end) {
        return terminal(
            xpath.len(),
            XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_pattern",
                "'//' (descendant axis) is not supported for autocomplete or target inference.",
            ),
        );
    }
    if segments.is_empty() {
        return root_completion(xpath.len());
    }

    let root_text = segments[0].text(xpath);
    if root_text != "Defs" {
        if segments.len() == 1
            && !trailing_slash
            && is_ident_prefix(root_text)
            && "Defs".starts_with(root_text)
        {
            return XPathCompletionResult {
                replace_from: segments[0].start,
                items: vec![XPathCompletionItem {
                    insert_text: "Defs".to_string(),
                    label: "Defs".to_string(),
                    detail: Some("Patch document root".to_string()),
                    kind: XPathCompletionItemKind::Root,
                }],
                diagnostics: Vec::new(),
                target: XPathTarget::Unsupported,
                resolved_field: None,
            };
        }
        return terminal(
            xpath.len(),
            XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_root",
                "XPath must be rooted at 'Defs' to be understood for autocomplete.",
            ),
        );
    }

    if segments.len() == 1 {
        return if trailing_slash {
            def_type_completion(catalog, xpath.len(), "")
        } else {
            empty(xpath.len(), XPathTarget::Unsupported)
        };
    }

    let is_last_segment = segments.len() == 2;
    let (def_type, target) = match resolve_def_segment(
        catalog,
        def_index,
        xpath,
        &segments[1],
        is_last_segment,
        trailing_slash,
    ) {
        DefSegmentOutcome::Result(result) => return *result,
        DefSegmentOutcome::Resolved { def_type, target } => (def_type, target),
    };

    if is_last_segment {
        return if trailing_slash {
            field_completion(catalog, &def_type, target, xpath.len(), "")
        } else {
            empty(xpath.len(), target)
        };
    }

    // segments.len() >= 3: a field segment follows the Def type/predicate.
    let field_seg = &segments[2];
    if segments.len() == 3 && !trailing_slash {
        return field_completion(
            catalog,
            &def_type,
            target,
            field_seg.start,
            field_seg.text(xpath),
        );
    }

    // Either typing beyond the first field segment (segments.len() > 3) or a trailing slash right
    // after the first field segment (e.g. ".../statBases/") -- both are beyond issue 05's
    // documented completion depth. Still resolve the first field segment on a best-effort basis so
    // a value subform can use it, matching this module's "stay transparent, don't just go silent"
    // philosophy.
    let (resolved_field, mut diagnostics) =
        resolve_field(catalog, &def_type, field_seg.text(xpath));
    diagnostics.push(XPathDiagnostic::warning(
        "xpath_autocomplete_unsupported_pattern",
        "Only one field segment after the Def type is supported for autocomplete.",
    ));
    XPathCompletionResult {
        replace_from: xpath.len(),
        items: Vec::new(),
        diagnostics,
        target,
        resolved_field,
    }
}

// ---------------------------------------------------------------------------
// Segment splitting
// ---------------------------------------------------------------------------

struct Segment {
    start: usize,
    end: usize,
}

impl Segment {
    fn text<'a>(&self, xpath: &'a str) -> &'a str {
        &xpath[self.start..self.end]
    }
}

/// Split `xpath` on `/` at bracket-depth 0 into byte ranges (possibly empty, e.g. for a leading,
/// trailing, or doubled `/`). Brackets and `/` inside a quoted string literal (`"..."`/`'...'`) --
/// e.g. a `contains(defName, "]")` predicate, which is valid XPath even though it's outside our
/// conservative subset -- don't count, so a literal bracket char never trips the balance check
/// below. The only case reported as an error rather than "still typing" is a stray `]` with no
/// matching `[` outside any quote -- an unclosed `[` at the end of the string is a normal
/// in-progress predicate for a live-typing input, not a malformed XPath.
fn split_segments(xpath: &str) -> Result<Vec<Segment>, XPathDiagnostic> {
    let mut segments = Vec::new();
    let mut seg_start = 0usize;
    let mut depth: i32 = 0;
    let mut quote: Option<char> = None;
    for (i, ch) in xpath.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth < 0 {
                    return Err(XPathDiagnostic::error(
                        "xpath_invalid_syntax",
                        "Unexpected ']' with no matching '['.",
                    ));
                }
            }
            '/' if depth == 0 => {
                segments.push(Segment {
                    start: seg_start,
                    end: i,
                });
                seg_start = i + 1;
            }
            _ => {}
        }
    }
    segments.push(Segment {
        start: seg_start,
        end: xpath.len(),
    });
    Ok(segments)
}

/// Find the byte offset of the `]` that closes the `[` at `open_pos` within `text`, tracking
/// nesting depth so a second, adjacent predicate (`[a][b]`) is not mistaken for the first
/// predicate's own close, and ignoring brackets inside quoted string literals for the same reason
/// as `split_segments` above. Returns `None` if no matching close exists (predicate still open).
fn find_matching_close(text: &str, open_pos: usize) -> Option<usize> {
    let mut depth = 1i32;
    let mut quote: Option<char> = None;
    for (i, ch) in text[open_pos + 1..].char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_pos + 1 + i);
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Def type / predicate segment resolution
// ---------------------------------------------------------------------------

enum DefSegmentOutcome {
    /// A terminal completion result (still typing the Def type, or an unsupported pattern).
    Result(Box<XPathCompletionResult>),
    /// The Def type (and predicate, if present) segment is fully resolved; the caller continues
    /// on to check for a following field segment.
    Resolved {
        def_type: String,
        target: XPathTarget,
    },
}

enum PredicateMatch {
    DefName(String),
    NameAttr,
    ParentNameAttr,
}

fn resolve_def_segment(
    catalog: &SchemaCatalog,
    def_index: &DefIndex,
    xpath: &str,
    def_seg: &Segment,
    is_last_segment: bool,
    trailing_slash: bool,
) -> DefSegmentOutcome {
    let text = def_seg.text(xpath);
    let bracket_pos = text.find('[');

    let Some(bracket_pos) = bracket_pos else {
        // Still typing the def type name itself: nothing else has been typed yet, so keep
        // offering completions against the partial text (see module docs on the "cursor is
        // always at the end" assumption).
        if is_last_segment && !trailing_slash {
            return DefSegmentOutcome::Result(Box::new(def_type_completion(
                catalog,
                def_seg.start,
                text,
            )));
        }
        if !is_valid_identifier(text) {
            return DefSegmentOutcome::Result(Box::new(terminal(
                xpath.len(),
                unsupported_def_type_diagnostic(),
            )));
        }
        return DefSegmentOutcome::Resolved {
            def_type: text.to_string(),
            target: XPathTarget::DefType {
                def_type: text.to_string(),
            },
        };
    };

    let def_type_str = &text[..bracket_pos];
    if !is_valid_identifier(def_type_str) {
        return DefSegmentOutcome::Result(Box::new(terminal(
            xpath.len(),
            unsupported_def_type_diagnostic(),
        )));
    }

    // Find the matching close for *this* predicate's own '[' by tracking depth, rather than just
    // the last ']' in the segment -- two adjacent predicates (`[a][b]`) both end exactly at the
    // segment's end, so a naive `rfind(']')` would swallow both into one (malformed-looking)
    // predicate instead of correctly reporting "multiple predicates are unsupported".
    let matching_close = find_matching_close(text, bracket_pos);

    match matching_close {
        None => {
            // Open predicate, still being typed. Structurally guaranteed to be the last segment
            // with no trailing slash: an unclosed '[' keeps bracket depth > 0 for the rest of the
            // string, so `split_segments` can't have split any further '/' after this point.
            let partial = &text[bracket_pos + 1..];
            DefSegmentOutcome::Result(Box::new(predicate_completion(
                def_index,
                def_type_str,
                partial,
                def_seg.start + bracket_pos + 1,
            )))
        }
        Some(close_pos) if close_pos != text.len() - 1 => {
            DefSegmentOutcome::Result(Box::new(terminal(
                xpath.len(),
                XPathDiagnostic::warning(
                    "xpath_autocomplete_unsupported_pattern",
                    "Multiple predicates or trailing content after ']' are not supported for autocomplete.",
                ),
            )))
        }
        Some(close_pos) => {
            let content = &text[bracket_pos + 1..close_pos];
            match parse_predicate_content(content) {
                Some(PredicateMatch::DefName(def_name)) => DefSegmentOutcome::Resolved {
                    def_type: def_type_str.to_string(),
                    target: XPathTarget::Def {
                        def_type: def_type_str.to_string(),
                        def_name,
                    },
                },
                // `@Name`/`@ParentName` identify an inheritance-template node rather than a
                // concrete `defName`-keyed Def, so we can't narrow to `XPathTarget::Def` -- but the
                // Def *type* (and therefore its fields) is still known.
                Some(PredicateMatch::NameAttr) | Some(PredicateMatch::ParentNameAttr) => {
                    DefSegmentOutcome::Resolved {
                        def_type: def_type_str.to_string(),
                        target: XPathTarget::DefType {
                            def_type: def_type_str.to_string(),
                        },
                    }
                }
                None => DefSegmentOutcome::Result(Box::new(terminal(
                    xpath.len(),
                    XPathDiagnostic::warning(
                        "xpath_autocomplete_unsupported_pattern",
                        "Predicate must be defName=\"...\", @Name=\"...\", or @ParentName=\"...\".",
                    ),
                ))),
            }
        }
    }
}

fn unsupported_def_type_diagnostic() -> XPathDiagnostic {
    XPathDiagnostic::warning(
        "xpath_autocomplete_unsupported_pattern",
        "Def type segment must be a plain element name; wildcards and functions are not supported for autocomplete.",
    )
}

fn parse_predicate_content(content: &str) -> Option<PredicateMatch> {
    let trimmed = content.trim();
    if let Some(rest) = trimmed.strip_prefix("defName") {
        let value = parse_eq_quoted_value(rest)?;
        return (!value.is_empty()).then(|| PredicateMatch::DefName(value.to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix("@Name") {
        let value = parse_eq_quoted_value(rest)?;
        return (!value.is_empty()).then_some(PredicateMatch::NameAttr);
    }
    if let Some(rest) = trimmed.strip_prefix("@ParentName") {
        let value = parse_eq_quoted_value(rest)?;
        return (!value.is_empty()).then_some(PredicateMatch::ParentNameAttr);
    }
    None
}

fn parse_eq_quoted_value(rest: &str) -> Option<&str> {
    let rest = rest.trim_start().strip_prefix('=')?;
    let rest = rest.trim();
    let value = rest
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| rest.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))?;
    if value.contains(['"', '\'']) {
        return None;
    }
    Some(value)
}

// ---------------------------------------------------------------------------
// Predicate key/value completion (open predicate: `[defName="Wa`, `[@Nam`, `[`, ...)
// ---------------------------------------------------------------------------

fn predicate_completion(
    def_index: &DefIndex,
    def_type: &str,
    partial: &str,
    base_offset: usize,
) -> XPathCompletionResult {
    let trimmed_start = partial.len() - partial.trim_start().len();
    let trimmed = &partial[trimmed_start..];

    for key in ["defName", "@Name", "@ParentName"] {
        let Some(after_key) = trimmed.strip_prefix(key) else {
            continue;
        };
        let after_key_trimmed_start = after_key.len() - after_key.trim_start().len();
        let after_key_trimmed = &after_key[after_key_trimmed_start..];
        let Some(after_eq) = after_key_trimmed.strip_prefix('=') else {
            return XPathCompletionResult {
                replace_from: base_offset + trimmed_start,
                items: vec![predicate_key_item(key)],
                diagnostics: Vec::new(),
                target: XPathTarget::DefType {
                    def_type: def_type.to_string(),
                },
                resolved_field: None,
            };
        };
        let after_eq_trimmed_start = after_eq.len() - after_eq.trim_start().len();
        let after_eq_trimmed = &after_eq[after_eq_trimmed_start..];
        let quote_char = after_eq_trimmed
            .chars()
            .next()
            .filter(|c| *c == '"' || *c == '\'');
        let Some(quote_char) = quote_char else {
            return terminal(
                base_offset + partial.len(),
                XPathDiagnostic::warning(
                    "xpath_autocomplete_unsupported_pattern",
                    format!("'{key}' predicate values must be quoted, e.g. {key}=\"Wall\"."),
                )
                .with_args(crate::diagnostics::diagnostic_args([(
                    "predicateKey",
                    key.into(),
                )])),
            );
        };
        let after_quote = &after_eq_trimmed[1..];
        let value_partial = after_quote.split(quote_char).next().unwrap_or("");
        let value_start = base_offset
            + trimmed_start
            + key.len()
            + after_key_trimmed_start
            + 1
            + after_eq_trimmed_start
            + 1;

        if key == "defName" {
            let suggestions = suggest_def_references(
                def_index,
                &[def_type],
                value_partial,
                &ReferenceScope::AllSources,
                DEF_NAME_SUGGESTION_LIMIT,
            );
            let items = suggestions
                .into_iter()
                .map(|s| XPathCompletionItem {
                    insert_text: format!("{}\"]", s.def_name),
                    label: s.def_name.clone(),
                    detail: Some(s.location_name),
                    kind: XPathCompletionItemKind::DefName,
                })
                .collect();
            return XPathCompletionResult {
                replace_from: value_start,
                items,
                diagnostics: Vec::new(),
                target: XPathTarget::DefType {
                    def_type: def_type.to_string(),
                },
                resolved_field: None,
            };
        }
        // `@Name`/`@ParentName` are recognized, supported predicate syntax, but RimEdit has no
        // index of inheritance-template `Name=`/`ParentName=` identifiers to suggest values from.
        return XPathCompletionResult {
            replace_from: value_start,
            items: Vec::new(),
            diagnostics: Vec::new(),
            target: XPathTarget::DefType {
                def_type: def_type.to_string(),
            },
            resolved_field: None,
        };
    }

    let needle = trimmed.to_lowercase();
    let items: Vec<_> = ["defName", "@Name", "@ParentName"]
        .into_iter()
        .filter(|k| k.to_lowercase().starts_with(&needle))
        .map(predicate_key_item)
        .collect();
    if items.is_empty() && !trimmed.is_empty() {
        return terminal(
            base_offset + partial.len(),
            XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_pattern",
                "Predicate key is not one of defName, @Name, or @ParentName.",
            ),
        );
    }
    XPathCompletionResult {
        replace_from: base_offset + trimmed_start,
        items,
        diagnostics: Vec::new(),
        target: XPathTarget::DefType {
            def_type: def_type.to_string(),
        },
        resolved_field: None,
    }
}

fn predicate_key_item(key: &str) -> XPathCompletionItem {
    XPathCompletionItem {
        insert_text: format!("{key}=\""),
        label: format!("{key}=\"...\""),
        detail: Some(match key {
            "defName" => "Match a specific Def by defName".to_string(),
            "@Name" => "Match by inheritance template Name attribute".to_string(),
            _ => "Match by inheritance template ParentName attribute".to_string(),
        }),
        kind: XPathCompletionItemKind::PredicateKey,
    }
}

// ---------------------------------------------------------------------------
// Def type / field completion
// ---------------------------------------------------------------------------

fn root_completion(replace_from: usize) -> XPathCompletionResult {
    XPathCompletionResult {
        replace_from,
        items: vec![XPathCompletionItem {
            insert_text: "Defs".to_string(),
            label: "Defs".to_string(),
            detail: Some("Patch document root".to_string()),
            kind: XPathCompletionItemKind::Root,
        }],
        diagnostics: Vec::new(),
        target: XPathTarget::Unsupported,
        resolved_field: None,
    }
}

fn def_type_completion(
    catalog: &SchemaCatalog,
    replace_from: usize,
    partial: &str,
) -> XPathCompletionResult {
    if !is_ident_prefix(partial) {
        return terminal(replace_from, unsupported_def_type_diagnostic());
    }
    let needle = partial.to_lowercase();
    let mut items: Vec<XPathCompletionItem> = catalog
        .def_types
        .iter()
        .filter(|(name, _)| needle.is_empty() || name.to_lowercase().starts_with(&needle))
        .map(|(name, schema)| XPathCompletionItem {
            insert_text: name.clone(),
            label: name.clone(),
            detail: schema.label.clone(),
            kind: XPathCompletionItemKind::DefType,
        })
        .collect();
    items.sort_by(|a, b| a.label.cmp(&b.label));
    XPathCompletionResult {
        replace_from,
        items,
        diagnostics: Vec::new(),
        target: XPathTarget::Unsupported,
        resolved_field: None,
    }
}

fn field_completion(
    catalog: &SchemaCatalog,
    def_type: &str,
    target: XPathTarget,
    replace_from: usize,
    partial: &str,
) -> XPathCompletionResult {
    if !is_ident_prefix(partial) {
        return XPathCompletionResult {
            replace_from,
            items: Vec::new(),
            diagnostics: vec![XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_pattern",
                "Field segment must be a plain element name; attribute-node targeting and functions are not supported here.",
            )],
            target,
            resolved_field: None,
        };
    }

    let mut items = Vec::new();
    if let Some(schema) = catalog.def_types.get(def_type) {
        let needle = partial.to_lowercase();
        for (name, field) in &schema.fields {
            if needle.is_empty() || name.to_lowercase().starts_with(&needle) {
                items.push(XPathCompletionItem {
                    insert_text: name.clone(),
                    label: name.clone(),
                    detail: field.label.clone(),
                    kind: XPathCompletionItemKind::Field,
                });
            }
            for alias in &field.xml_aliases {
                if needle.is_empty() || alias.to_lowercase().starts_with(&needle) {
                    items.push(XPathCompletionItem {
                        insert_text: alias.clone(),
                        label: alias.clone(),
                        detail: Some(format!("XML alias for '{name}'")),
                        kind: XPathCompletionItemKind::FieldAlias,
                    });
                }
            }
        }
    }
    items.sort_by(|a, b| a.label.cmp(&b.label));

    let (resolved_field, diagnostics) = resolve_field(catalog, def_type, partial);
    XPathCompletionResult {
        replace_from,
        items,
        diagnostics,
        target,
        resolved_field,
    }
}

/// Resolve a fully-typed field name (or alias) against a Def type's *direct* fields only.
/// Returns an `xpath_autocomplete_inherited_field` diagnostic when `name` is a real field but only
/// reachable through the schema's `inherits` chain -- RimWorld applies patches before XML
/// inheritance, so such a field cannot be targeted unless it is physically present in the XML
/// being patched (which this module, having no access to the live combined document, can't check;
/// see `docs/patches-editor/05-xpath-autocomplete-and-target-inference.md`).
fn resolve_field(
    catalog: &SchemaCatalog,
    def_type: &str,
    name: &str,
) -> (Option<XPathResolvedField>, Vec<XPathDiagnostic>) {
    if name.is_empty() {
        return (None, Vec::new());
    }
    if let Some((canonical, field)) = direct_field(catalog, def_type, name) {
        return (
            Some(XPathResolvedField {
                def_type: def_type.to_string(),
                field_name: canonical.to_string(),
                field: field.clone(),
            }),
            Vec::new(),
        );
    }
    if is_inherited_only_field(catalog, def_type, name) {
        return (
            None,
            vec![XPathDiagnostic::warning(
                "xpath_autocomplete_inherited_field",
                format!(
                    "'{name}' is declared on a schema parent of '{def_type}', not on '{def_type}' itself. RimWorld applies patches before XML inheritance, so this field can only be targeted if it is physically present in the XML being patched."
                ),
            )
            .with_args(crate::diagnostics::diagnostic_args([
                ("fieldName", name.into()),
                ("defType", def_type.into()),
            ]))],
        );
    }
    (None, Vec::new())
}

fn field_or_alias(schema: &DefTypeSchema, name: &str) -> bool {
    schema.fields.contains_key(name)
        || schema
            .fields
            .values()
            .any(|f| f.xml_aliases.iter().any(|a| a == name))
}

fn direct_field<'a>(
    catalog: &'a SchemaCatalog,
    def_type: &str,
    name: &str,
) -> Option<(&'a str, &'a FieldSchema)> {
    let schema = catalog.def_types.get(def_type)?;
    if let Some((canonical, field)) = schema.fields.get_key_value(name) {
        return Some((canonical.as_str(), field));
    }
    schema.fields.iter().find_map(|(canonical, field)| {
        field
            .xml_aliases
            .iter()
            .any(|a| a == name)
            .then_some((canonical.as_str(), field))
    })
}

/// Whether `name` is a field known to `def_type` only via its schema `inherits` chain (i.e. absent
/// from `def_type`'s own direct fields). Walks parents depth-first with a cycle guard, mirroring
/// `schema_pack::lookup::collect_all_object_inherited_fields`'s object-type equivalent -- there is
/// no Def-type version of that helper today because nothing needed "is this field inherited-only"
/// as a yes/no question before this module.
fn is_inherited_only_field(catalog: &SchemaCatalog, def_type: &str, name: &str) -> bool {
    let Some(schema) = catalog.def_types.get(def_type) else {
        return false;
    };
    if field_or_alias(schema, name) {
        return false;
    }
    let mut visited: HashSet<String> = HashSet::from([def_type.to_string()]);
    let mut stack: Vec<String> = schema.inherits.clone();
    while let Some(parent) = stack.pop() {
        if !visited.insert(parent.clone()) {
            continue;
        }
        if let Some(parent_schema) = catalog.def_types.get(&parent) {
            if field_or_alias(parent_schema, name) {
                return true;
            }
            stack.extend(parent_schema.inherits.iter().cloned());
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Shared small helpers
// ---------------------------------------------------------------------------

fn is_ident_prefix(s: &str) -> bool {
    s.is_empty() || is_valid_identifier(s)
}

fn terminal(replace_from: usize, diagnostic: XPathDiagnostic) -> XPathCompletionResult {
    XPathCompletionResult {
        replace_from,
        items: Vec::new(),
        diagnostics: vec![diagnostic],
        target: XPathTarget::Unsupported,
        resolved_field: None,
    }
}

fn empty(replace_from: usize, target: XPathTarget) -> XPathCompletionResult {
    XPathCompletionResult {
        replace_from,
        items: Vec::new(),
        diagnostics: Vec::new(),
        target,
        resolved_field: None,
    }
}

#[cfg(test)]
mod diagnostic_ref_wire_tests {
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
