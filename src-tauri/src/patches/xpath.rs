//! Schema-aware XPath completion and target inference for `PatchPathInput`.
//!
//! This is a *different, more permissive* conservative subset than [`super::impact_graph`]'s
//! static target inference: `impact_graph::infer_xpath_target` only trusts `Defs/<DefType>` and
//! `Defs/<DefType>[defName="<Name>"]` because it feeds patch-conflict/impact analysis, where an
//! `@Name`/`@ParentName` predicate can't be resolved to one concrete `defName`-keyed Def. Here,
//! `@Name="..."`/`@ParentName="..."` predicates are recognized as *supported autocomplete syntax*
//! because knowing the Def *type* alone is enough to keep offering field completions and a
//! value-subform target -- we just can't narrow `XPathTarget` down to a specific `defName` from
//! them, so they resolve to `XPathTarget::DefType` rather than `XPathTarget::Def`.
//!
//! A Def predicate may also chain 2+ equality clauses with lowercase `or`/`and` (see
//! [`parse_boolean_chain`]/[`split_top_level_bool_terms`] for the finished-predicate grammar and
//! [`predicate_completion`]/[`classify_tail`] for live completion while typing one). Target
//! inference here is again more permissive than impact analysis: a 2+-term OR-only chain of
//! `defName="..."` equalities resolves to `XPathTarget::Defs`, matching
//! `impact_graph::infer_xpath_target`'s own OR-only-`defName` subset, while any other supported
//! chain (an `and`, an `@Name`/`@ParentName` clause, or a mixture of operators/keys) still pins
//! down the Def *type* -- `XPathTarget::DefType` -- rather than a specific Def or Def set (see
//! [`classify_boolean_chain`]).
//!
//! Completion is computed at a caret position ([`complete_patch_xpath_at`]'s `cursor` byte
//! offset into `xpath`): only the text at or before the caret is analysed for structure (later
//! path segments are never treated as already typed just because they exist after the caret), and
//! a result's `replace_from`/`replace_to` describe the byte span of the *active* token the caret
//! sits in or just before -- never anything beyond it. [`complete_patch_xpath`] is a thin
//! convenience wrapper that completes at the end of the string, matching every existing caller
//! that has no caret information (mirrors how `ReferencePicker` treats its whole current value as
//! the live query). Completion is only offered for the conservative path shapes documented
//! above; anything else -- axes, functions, wildcards, multiple predicates, `//`,
//! attribute-node path segments -- is reported as [`XPathDiagnostic`] with code
//! `xpath_autocomplete_unsupported_pattern` and an empty completion list, but is never rejected
//! outright: the XPath stays editable and previewable by a fuller backend XML library later.
//!
//! Presentation whitespace is tolerated at path-segment boundaries: indentation right after a `/`
//! and trailing whitespace right before the next `/` are stripped from a segment before it is
//! inspected structurally (a Def type name, a field name, a structural `li`/`key`/`value` literal,
//! or a predicate's own `[...]` body), via [`Segment::trimmed`]. Whitespace *inside* a predicate
//! around `defName`/`=`/quote boundaries was already tolerated before caret-awareness landed (see
//! `parse_predicate_content`/`parse_eq_quoted_value`/`predicate_completion`'s own trimming) and is
//! unchanged here. Whitespace inside a quoted literal is never touched by either. A line break by
//! itself is just more whitespace to this trimming and never triggers
//! `xpath_autocomplete_unsupported_pattern` on its own.
//!
//! Field-segment depth is *not* capped: once the Def type/predicate segment resolves, a
//! [`SchemaCursor`] walks every remaining segment against the merged [`SchemaCatalog`], descending
//! through nested `object` fields, `listOfLi` object items (`/li[n]/...`), `keyedObjectMap` entries
//! (`/li[n]/key` or `/li[n]/value/...`), and `keyedObjectList` data-dependent keys (`/<key>/...`)
//! for as long as the schema keeps resolving concrete, statically-known children. A `listOfLi`/
//! `keyedObjectMap` entry step optionally carries a single one-based positional predicate (`li[1]`,
//! `li[2]`, ...); it only changes *which* XML list entry the XPath selects, never the selected
//! entry's schema, so `li` and `li[n]` transition the cursor identically (see
//! [`parse_list_item_step`]). Traversal stops -- silently, or with
//! `xpath_autocomplete_unsupported_pattern` when a *typed* segment doesn't match anything the
//! cursor understands -- at a scalar field, an XML shape with no statically known descendants
//! (scalar lists, `namedChildrenMap`, `keyedValueList`, `typedReferenceList`, attributes/text/
//! flags), an unresolvable `schemaRef`, or a discriminator-based list item's variant-specific
//! members (only the declared base `items.schemaRef` is traversed; narrowing to one `Class="..."`
//! variant remains unsupported since it would need predicate parsing beyond the plain positional
//! index, which stays outside this conservative grammar -- see [`SchemaCursor::ListItem`]).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::def_index::{suggest_def_references, DefIndex};
use crate::schema_pack::{
    collect_object_fields_ordered, lookup_object_field_with_alias, DefTypeSchema, FieldSchema,
    FieldTypeKind, ReferenceScope, SchemaCatalog, XmlFieldShape,
};

use super::impact_graph::{is_valid_identifier, XPathTarget};

const DEF_NAME_SUGGESTION_LIMIT: usize = 20;

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
    /// already-closed predicate's `]` (see [`predicate_close_continuation`]).
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
    fn new(
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
    fn capped(
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

/// Compute completions, diagnostics, and target/field inference for a `PatchPathInput` value,
/// treating the end of the string as the caret position. A thin convenience wrapper around
/// [`complete_patch_xpath_at`] for callers with no caret information (e.g. existing tests and any
/// future non-interactive use).
pub fn complete_patch_xpath(
    catalog: &SchemaCatalog,
    def_index: &DefIndex,
    xpath: &str,
) -> XPathCompletionResult {
    complete_patch_xpath_at(catalog, def_index, xpath, xpath.len())
}

/// Compute completions, diagnostics, and target/field inference for a `PatchPathInput` value at
/// `cursor`, a byte offset into `xpath` (clamped to `xpath.len()` and to the nearest preceding
/// char boundary if the caller passes an out-of-range or mid-character offset -- the Tauri command
/// layer is the authority that rejects a bad offset outright with an `AppError`; this function
/// stays panic-free for any input instead of duplicating that validation).
///
/// Only `xpath[..cursor]` is analysed for path/predicate structure: segments are split from this
/// prefix alone, so a later path segment that happens to exist after the caret is never treated as
/// already typed (see module docs). Every byte offset produced (`replace_from`, `replace_to`, and
/// every position threaded into the split/resolve helpers below) is still an absolute offset into
/// the original `xpath`, since `xpath[..cursor]` is a true prefix sharing the same byte indices.
pub fn complete_patch_xpath_at(
    catalog: &SchemaCatalog,
    def_index: &DefIndex,
    xpath: &str,
    cursor: usize,
) -> XPathCompletionResult {
    let cursor = clamp_to_char_boundary(xpath, cursor);
    let prefix = &xpath[..cursor];

    let mut segments = match split_segments(prefix) {
        Ok(segments) => segments,
        Err(diagnostic) => return terminal(cursor, diagnostic),
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
            cursor,
            XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_pattern",
                "'//' (descendant axis) is not supported for autocomplete or target inference.",
            ),
        );
    }
    if segments.is_empty() {
        return root_completion(xpath, cursor);
    }

    let (root_text, root_start) = segments[0].trimmed(xpath);
    if root_text != "Defs" {
        if segments.len() == 1
            && !trailing_slash
            && is_ident_prefix(root_text)
            && "Defs".starts_with(root_text)
        {
            return XPathCompletionResult::new(
                root_start,
                identifier_token_end(xpath, root_start),
                vec![XPathCompletionItem {
                    insert_text: "Defs".to_string(),
                    label: "Defs".to_string(),
                    detail: Some("Patch document root".to_string()),
                    kind: XPathCompletionItemKind::Root,
                }],
                Vec::new(),
                XPathTarget::Unsupported,
                None,
            );
        }
        return terminal(
            cursor,
            XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_root",
                "XPath must be rooted at 'Defs' to be understood for autocomplete.",
            ),
        );
    }

    if segments.len() == 1 {
        return if trailing_slash {
            def_type_completion(catalog, xpath, cursor, "")
        } else {
            empty(cursor, XPathTarget::Unsupported)
        };
    }

    let is_last_segment = segments.len() == 2;
    let (def_type, target, predicate_close) = match resolve_def_segment(
        catalog,
        def_index,
        xpath,
        cursor,
        &segments[1],
        is_last_segment,
        trailing_slash,
    ) {
        DefSegmentOutcome::Result(result) => return *result,
        DefSegmentOutcome::Resolved {
            def_type,
            target,
            predicate_close,
        } => (def_type, target, predicate_close),
    };

    if is_last_segment {
        return if trailing_slash {
            field_completion(catalog, xpath, &def_type, target, cursor, "")
        } else if let Some(close_pos) = predicate_close {
            predicate_close_continuation(xpath, close_pos, target)
        } else {
            empty(cursor, target)
        };
    }

    // segments.len() >= 3: one or more field/structural segments follow the Def type/predicate.
    // Walk them left-to-right against a schema cursor that starts at the Def's direct fields and
    // descends through object/list/map shapes with no fixed depth limit -- see module docs.
    let field_segments = &segments[2..];
    let mut cursor_state = SchemaCursor::DefFields {
        def_type: def_type.clone(),
    };
    let mut resolved_field: Option<XPathResolvedField> = None;

    // Every segment except the one being completed is fully typed (has a `/` after it, whether
    // that's an interior segment or -- when `trailing_slash` -- the last one too).
    let complete_count = if trailing_slash {
        field_segments.len()
    } else {
        field_segments.len() - 1
    };
    for seg in &field_segments[..complete_count] {
        let (text, _) = seg.trimmed(xpath);
        match resolve_and_transition(catalog, &mut cursor_state, &def_type, text) {
            Ok(Some(field)) => resolved_field = Some(field),
            Ok(None) => {}
            Err(diagnostic) => {
                return XPathCompletionResult::new(
                    cursor,
                    cursor,
                    Vec::new(),
                    vec![diagnostic],
                    target,
                    resolved_field,
                );
            }
        }
    }

    if trailing_slash {
        child_completion(
            catalog,
            xpath,
            &cursor_state,
            &def_type,
            target,
            cursor,
            "",
            resolved_field,
        )
    } else {
        let final_seg = &field_segments[complete_count];
        let (partial, partial_start) = final_seg.trimmed(xpath);
        child_completion(
            catalog,
            xpath,
            &cursor_state,
            &def_type,
            target,
            partial_start,
            partial,
            resolved_field,
        )
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

    /// This segment's token with symmetric leading/trailing whitespace trimmed, plus the absolute
    /// byte offset the trimmed text starts at. Presentation whitespace immediately after the
    /// preceding `/` (indentation) or immediately before the following `/` is never itself part
    /// of an XPath grammar token, so every caller that inspects a segment's content structurally
    /// (a Def type name, a field name, a structural `li`/`key`/`value` literal, or a predicate's
    /// own `[...]` body) uses this rather than `text`/`start` directly. `start`/`end` remain the
    /// raw byte range for anything keyed off segment boundaries themselves (e.g. detecting an
    /// empty segment for `//`/trailing-slash).
    fn trimmed<'a>(&self, xpath: &'a str) -> (&'a str, usize) {
        let raw = self.text(xpath);
        let leading = raw.len() - raw.trim_start().len();
        (raw.trim(), self.start + leading)
    }
}

/// Clamp `offset` to `xpath.len()` and then to the nearest char boundary at or before it, so
/// slicing `xpath` at the result never panics regardless of what a caller passes in. The Tauri
/// command layer independently rejects an out-of-range or mid-character `cursor_byte_offset`
/// outright with an `AppError` before ever reaching here (see `commands::patches`); this is
/// defense-in-depth for every other/direct caller of [`complete_patch_xpath_at`].
fn clamp_to_char_boundary(xpath: &str, offset: usize) -> usize {
    let offset = offset.min(xpath.len());
    let mut offset = offset;
    while offset > 0 && !xpath.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The end of an identifier-like token starting at `from` (a Def type, field, root, or structural
/// `li`/`key`/`value` literal): the first byte offset `>= from` whose character is not a valid
/// identifier character (matching [`is_valid_identifier`]'s alphabet) -- naturally a `/`, `[`,
/// whitespace, or end of string, so trailing indentation before the next path segment is excluded
/// from the replacement span even though it is included when scanning past the caret into a
/// suffix the user already typed.
fn identifier_token_end(xpath: &str, from: usize) -> usize {
    xpath[from..]
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map(|(i, _)| from + i)
        .unwrap_or(xpath.len())
}

/// The end of a predicate-key token (`defName`, `@Name`, `@ParentName`) starting at `from`: an
/// optional leading `@` followed by identifier characters, matching [`identifier_token_end`]'s
/// alphabet plus the one leading `@` these three keys can start with.
fn predicate_key_token_end(xpath: &str, from: usize) -> usize {
    let mut end = from;
    for (i, c) in xpath[from..].char_indices() {
        let allowed = c.is_ascii_alphanumeric() || c == '_' || (i == 0 && c == '@');
        if !allowed {
            break;
        }
        end = from + i + c.len_utf8();
    }
    end
}

/// The end of a quoted predicate value starting at `from` (just after its opening `quote_char`):
/// the byte offset of the matching closing quote, or end of string if unterminated (still being
/// typed past the caret too).
fn quoted_value_end(xpath: &str, from: usize, quote_char: char) -> usize {
    xpath[from..]
        .find(quote_char)
        .map(|i| from + i)
        .unwrap_or(xpath.len())
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
        /// The absolute byte offset of the `]` that closes a *valid* boolean predicate chain --
        /// `None` when there is no predicate at all (a bare `Defs/<DefType>` segment). Lets the
        /// caller offer `or`/`and` continuation completion in place of that `]` when the caret
        /// sits right after it with nothing typed beyond (see [`predicate_close_continuation`]).
        predicate_close: Option<usize>,
    },
}

/// One equality clause recognized inside a Def predicate (`defName="..."`, `@Name="..."`, or
/// `@ParentName="..."`), as parsed by [`parse_single_clause`].
#[derive(Debug, Clone)]
struct ParsedClause {
    key: PredicateKey,
    value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PredicateKey {
    DefName,
    NameAttr,
    ParentNameAttr,
}

/// A boolean connective joining two predicate clauses, recognized only as a standalone lowercase
/// token outside quoted values (see [`split_top_level_bool_terms`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoolOp {
    Or,
    And,
}

fn resolve_def_segment(
    catalog: &SchemaCatalog,
    def_index: &DefIndex,
    xpath: &str,
    cursor: usize,
    def_seg: &Segment,
    is_last_segment: bool,
    trailing_slash: bool,
) -> DefSegmentOutcome {
    let (text, trimmed_start) = def_seg.trimmed(xpath);
    let bracket_pos = text.find('[');

    let Some(bracket_pos) = bracket_pos else {
        // Still typing the def type name itself: nothing else has been typed yet, so keep
        // offering completions against the partial text typed so far (up to the caret).
        if is_last_segment && !trailing_slash {
            return DefSegmentOutcome::Result(Box::new(def_type_completion(
                catalog,
                xpath,
                trimmed_start,
                text,
            )));
        }
        if !is_valid_identifier(text) {
            return DefSegmentOutcome::Result(Box::new(terminal(
                cursor,
                unsupported_def_type_diagnostic(),
            )));
        }
        return DefSegmentOutcome::Resolved {
            def_type: text.to_string(),
            target: XPathTarget::DefType {
                def_type: text.to_string(),
            },
            predicate_close: None,
        };
    };

    let def_type_str = &text[..bracket_pos];
    if !is_valid_identifier(def_type_str) {
        return DefSegmentOutcome::Result(Box::new(terminal(
            cursor,
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
            // Open predicate, still being typed (as of the caret -- more of the predicate, or its
            // closing bracket, may already exist past it). Structurally guaranteed to be the last
            // segment with no trailing slash within the caret-truncated prefix: an unclosed '['
            // keeps bracket depth > 0 for the rest of the prefix, so `split_segments` can't have
            // split any further '/' after this point.
            let partial = &text[bracket_pos + 1..];
            DefSegmentOutcome::Result(Box::new(predicate_completion(
                def_index,
                xpath,
                def_type_str,
                partial,
                trimmed_start + bracket_pos + 1,
            )))
        }
        Some(close_pos) if close_pos != text.len() - 1 => {
            DefSegmentOutcome::Result(Box::new(terminal(
                cursor,
                XPathDiagnostic::warning(
                    "xpath_autocomplete_unsupported_pattern",
                    "Multiple predicates or trailing content after ']' are not supported for autocomplete.",
                ),
            )))
        }
        Some(close_pos) => {
            let content = &text[bracket_pos + 1..close_pos];
            match parse_boolean_chain(content) {
                // A single `defName="A"` continues to return `XPathTarget::Def`; a single
                // `@Name`/`@ParentName` clause identifies an inheritance-template node rather than
                // a concrete `defName`-keyed Def, so it resolves to `XPathTarget::DefType` instead
                // -- see [`classify_boolean_chain`] for the 2+-clause cases (OR-only `defName`
                // chains -> `XPathTarget::Defs`, everything else supported -> `XPathTarget::DefType`).
                Some((clauses, ops)) => DefSegmentOutcome::Resolved {
                    def_type: def_type_str.to_string(),
                    target: classify_boolean_chain(def_type_str, &clauses, &ops),
                    predicate_close: Some(trimmed_start + close_pos),
                },
                None => DefSegmentOutcome::Result(Box::new(terminal(
                    cursor,
                    XPathDiagnostic::warning(
                        "xpath_autocomplete_unsupported_pattern",
                        "Predicate must be defName=\"...\", @Name=\"...\", or @ParentName=\"...\", optionally joined by 'or'/'and'.",
                    ),
                ))),
            }
        }
    }
}

/// Resolves a fully-typed `defName="..."` / `@Name="..."` / `@ParentName="..."` equality clause,
/// same conservatism as the old single-clause `parse_predicate_content`: an unquoted, empty, or
/// otherwise malformed value fails the whole clause to `None` rather than guessing.
fn parse_single_clause(content: &str) -> Option<ParsedClause> {
    if let Some(rest) = content.strip_prefix("defName") {
        let value = parse_eq_quoted_value(rest)?;
        return (!value.is_empty()).then(|| ParsedClause {
            key: PredicateKey::DefName,
            value: value.to_string(),
        });
    }
    if let Some(rest) = content.strip_prefix("@Name") {
        let value = parse_eq_quoted_value(rest)?;
        return (!value.is_empty()).then(|| ParsedClause {
            key: PredicateKey::NameAttr,
            value: value.to_string(),
        });
    }
    if let Some(rest) = content.strip_prefix("@ParentName") {
        let value = parse_eq_quoted_value(rest)?;
        return (!value.is_empty()).then(|| ParsedClause {
            key: PredicateKey::ParentNameAttr,
            value: value.to_string(),
        });
    }
    None
}

/// Parses a *complete* (already `]`-closed) predicate body into a chain of [`parse_single_clause`]
/// clauses joined by top-level `or`/`and`. `None` for anything outside the conservative grammar --
/// an unquoted value, an unsupported key, a leading/trailing/doubled operator (which leaves an
/// empty term), nested grouping, or a function call -- same conservatism as the old
/// single-clause-only `parse_predicate_content`, just extended across every term.
fn parse_boolean_chain(content: &str) -> Option<(Vec<ParsedClause>, Vec<BoolOp>)> {
    let (terms, ops) = split_top_level_bool_terms(content);
    let clauses = terms
        .iter()
        .map(|t| parse_single_clause(content[t.start..t.end].trim()))
        .collect::<Option<Vec<_>>>()?;
    Some((clauses, ops))
}

/// Classifies a successfully-parsed boolean predicate chain into an [`XPathTarget`], per the
/// module docs' target-inference guarantees: a single `defName="A"` targets exactly one Def; a
/// 2+-term OR-only chain of `defName="..."` equalities targets exactly those Defs (matching
/// `impact_graph::infer_xpath_target`'s own OR-only-`defName` subset, see module docs); anything
/// else supported by the grammar (an `and`, an `@Name`/`@ParentName` clause, or a mixture of
/// operators/keys) still pins down the Def *type* but not a specific Def or Def set.
fn classify_boolean_chain(def_type: &str, clauses: &[ParsedClause], ops: &[BoolOp]) -> XPathTarget {
    if let [only] = clauses {
        return match only.key {
            PredicateKey::DefName => XPathTarget::Def {
                def_type: def_type.to_string(),
                def_name: only.value.clone(),
            },
            PredicateKey::NameAttr | PredicateKey::ParentNameAttr => XPathTarget::DefType {
                def_type: def_type.to_string(),
            },
        };
    }
    let all_or_def_names = ops.iter().all(|op| *op == BoolOp::Or)
        && clauses.iter().all(|c| c.key == PredicateKey::DefName);
    if all_or_def_names {
        XPathTarget::Defs {
            def_type: def_type.to_string(),
            def_names: clauses.iter().map(|c| c.value.clone()).collect(),
        }
    } else {
        XPathTarget::DefType {
            def_type: def_type.to_string(),
        }
    }
}

/// One clause's byte range within a boolean predicate's content, as produced by
/// [`split_top_level_bool_terms`].
struct BoolTerm {
    start: usize,
    end: usize,
}

/// Splits predicate `content` into clause byte-ranges separated by standalone top-level `or`/`and`
/// keyword tokens -- i.e. outside any quoted value and bounded by non-identifier characters on
/// both sides, so a `defName` value containing "or"/"and" as a substring (`MN_NetworkController`)
/// or a quoted value like `"A or B"` is never mistaken for a separator. `ops.len() ==
/// terms.len() - 1` always; `terms` is never empty (`content` with no separators yields one term
/// spanning it all, and a leading/trailing/doubled operator yields an empty term, which
/// [`parse_boolean_chain`]'s per-term [`parse_single_clause`] call naturally rejects). Mirrors
/// `impact_graph::split_top_level_or_terms`'s quote-/word-boundary scanning, generalized to
/// recognize `and` as well -- kept as an independent implementation rather than a shared helper
/// per the module docs (autocomplete's conservative subset is intentionally more permissive than
/// impact inference's).
fn split_top_level_bool_terms(content: &str) -> (Vec<BoolTerm>, Vec<BoolOp>) {
    let bytes = content.as_bytes();
    let mut terms = Vec::new();
    let mut ops = Vec::new();
    let mut term_start = 0usize;
    let mut in_quote: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match in_quote {
            Some(q) => {
                if b == q {
                    in_quote = None;
                }
                i += 1;
            }
            None => {
                if b == b'"' || b == b'\'' {
                    in_quote = Some(b);
                    i += 1;
                    continue;
                }
                // `b'o'`/`b'a'` are single-byte ASCII, so whenever they match, `i` is guaranteed to
                // already be a char boundary -- a UTF-8 continuation byte can never equal an ASCII
                // value -- so `content[i..]` below never panics.
                let candidate = if b == b'o' && content[i..].starts_with("or") {
                    Some((BoolOp::Or, 2usize))
                } else if b == b'a' && content[i..].starts_with("and") {
                    Some((BoolOp::And, 3usize))
                } else {
                    None
                };
                if let Some((op, len)) = candidate {
                    let before_ok = i == 0 || !is_identifier_byte(bytes[i - 1]);
                    let after_idx = i + len;
                    let after_ok =
                        after_idx >= bytes.len() || !is_identifier_byte(bytes[after_idx]);
                    if before_ok && after_ok {
                        terms.push(BoolTerm {
                            start: term_start,
                            end: i,
                        });
                        ops.push(op);
                        i = after_idx;
                        term_start = i;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
    terms.push(BoolTerm {
        start: term_start,
        end: content.len(),
    });
    (terms, ops)
}

fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn unsupported_def_type_diagnostic() -> XPathDiagnostic {
    XPathDiagnostic::warning(
        "xpath_autocomplete_unsupported_pattern",
        "Def type segment must be a plain element name; wildcards and functions are not supported for autocomplete.",
    )
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

/// Completion for a still-open (unclosed `[`) Def predicate, up to the caret. Splits `partial`
/// (the predicate content typed so far) on top-level `or`/`and` the same way
/// [`split_top_level_bool_terms`] does for a finished predicate; every term but the last must
/// already be a complete, valid clause (else the whole thing is an unsupported pattern, matching
/// [`parse_boolean_chain`]'s conservatism), and the last term -- still being typed -- is
/// classified by [`classify_tail`] into either "offer `or`/`and` continuation" or "fall through to
/// the existing single-clause key/value completion" ([`key_or_value_completion`]).
fn predicate_completion(
    def_index: &DefIndex,
    xpath: &str,
    def_type: &str,
    partial: &str,
    base_offset: usize,
) -> XPathCompletionResult {
    let (terms, _ops) = split_top_level_bool_terms(partial);
    let last_idx = terms.len() - 1;

    for term in &terms[..last_idx] {
        let text = partial[term.start..term.end].trim();
        if parse_single_clause(text).is_none() {
            return terminal(
                base_offset + partial.len(),
                XPathDiagnostic::warning(
                    "xpath_autocomplete_unsupported_pattern",
                    "Predicate must be defName=\"...\", @Name=\"...\", or @ParentName=\"...\", joined by 'or'/'and'.",
                ),
            );
        }
    }

    let tail_term = &terms[last_idx];
    let tail = &partial[tail_term.start..tail_term.end];
    let tail_base = base_offset + tail_term.start;
    let target = XPathTarget::DefType {
        def_type: def_type.to_string(),
    };

    match classify_tail(tail) {
        TailState::Continuation { prefix, start } => {
            let abs_start = tail_base + start;
            let replace_to = identifier_token_end(xpath, abs_start);
            let items = boolean_operator_items(xpath, abs_start, prefix);
            XPathCompletionResult::new(abs_start, replace_to, items, Vec::new(), target, None)
        }
        TailState::Invalid => terminal(
            base_offset + partial.len(),
            XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_pattern",
                "Predicate clause must be defName=\"...\", @Name=\"...\", or @ParentName=\"...\".",
            ),
        ),
        TailState::Incomplete => {
            key_or_value_completion(def_index, xpath, def_type, tail, tail_base)
        }
    }
}

/// How the predicate's last (still-being-typed) term reads, classified by [`classify_tail`].
enum TailState<'a> {
    /// Still typing this clause's key or value (no `=` yet, no opening quote yet, or an
    /// unterminated value) -- the caller falls through to [`key_or_value_completion`], which is
    /// exactly the old single-clause `predicate_completion` behavior.
    Incomplete,
    /// A complete, valid clause (its value's closing quote is present) followed only by
    /// whitespace and/or a partial `or`/`and` word. `prefix` is that trailing word (trailing
    /// whitespace of its own already stripped) and `start` its byte offset within the tail.
    Continuation { prefix: &'a str, start: usize },
    /// A complete clause followed by content that is neither whitespace nor an `or`/`and` prefix.
    Invalid,
}

/// Classifies a predicate's last, still-being-typed term (see [`TailState`]). Reuses the same
/// key/`=`/quote scanning shape as [`key_or_value_completion`] just to detect whether the value's
/// closing quote has *already* been typed -- once it has, what matters here is only what comes
/// after it, not re-deriving completion items (that stays the incomplete-tail caller's job).
fn classify_tail(tail: &str) -> TailState<'_> {
    let content_start = tail.len() - tail.trim_start().len();
    let content = &tail[content_start..];
    for key in ["defName", "@Name", "@ParentName"] {
        let Some(rest) = content.strip_prefix(key) else {
            continue;
        };
        let mut pos = key.len();
        let ws1 = rest.len() - rest.trim_start().len();
        let rest = &rest[ws1..];
        pos += ws1;
        let Some(rest) = rest.strip_prefix('=') else {
            return TailState::Incomplete;
        };
        pos += 1;
        let ws2 = rest.len() - rest.trim_start().len();
        let rest = &rest[ws2..];
        pos += ws2;
        let mut chars = rest.chars();
        let Some(quote_char) = chars.next().filter(|c| *c == '"' || *c == '\'') else {
            return TailState::Incomplete;
        };
        pos += quote_char.len_utf8();
        let after_quote = &content[pos..];
        let Some(close_rel) = after_quote.find(quote_char) else {
            return TailState::Incomplete; // value still open -- key/value completion continues it
        };
        let value = &after_quote[..close_rel];
        if value.is_empty() || value.contains(['"', '\'']) {
            return TailState::Invalid;
        }
        pos += close_rel + quote_char.len_utf8();
        let after_value = &content[pos..];
        let ws3 = after_value.len() - after_value.trim_start().len();
        let op_word = after_value[ws3..].trim_end();
        return if op_word.is_empty() || is_operator_prefix(op_word) {
            TailState::Continuation {
                prefix: op_word,
                start: content_start + pos + ws3,
            }
        } else {
            TailState::Invalid
        };
    }
    TailState::Incomplete
}

fn is_operator_prefix(s: &str) -> bool {
    let lower = s.to_lowercase();
    "or".starts_with(&lower) || "and".starts_with(&lower)
}

/// `or`/`and` completion items, filtered by `filter_prefix` (case-insensitive; empty matches
/// both). `replace_from` is where the items' replacement span starts -- right after a completed
/// clause's closing quote, right after any whitespace already typed past it, or at an
/// already-closed predicate's own `]` (see [`predicate_close_continuation`]) -- so a leading space
/// is added to `insert_text` only when the character immediately before it isn't already
/// whitespace, keeping `defName="Wall" or `/`defName="Wall"or ` both spliced to the same one
/// separating space either way.
fn boolean_operator_items(
    xpath: &str,
    replace_from: usize,
    filter_prefix: &str,
) -> Vec<XPathCompletionItem> {
    let needs_leading_space =
        replace_from == 0 || !xpath.as_bytes()[replace_from - 1].is_ascii_whitespace();
    let needle = filter_prefix.to_lowercase();
    ["or", "and"]
        .into_iter()
        .filter(|op| op.starts_with(&needle))
        .map(|op| {
            let insert_text = if needs_leading_space {
                format!(" {op} ")
            } else {
                format!("{op} ")
            };
            XPathCompletionItem {
                insert_text,
                label: op.to_string(),
                detail: Some("Continue the predicate with another clause".to_string()),
                kind: XPathCompletionItemKind::BooleanOperator,
            }
        })
        .collect()
}

/// Offers `or`/`and` continuation completion in place of the `]` that closes an already-valid
/// predicate, for a caret positioned immediately after that `]` with nothing else typed --
/// `complete_patch_xpath_at`'s `is_last_segment && !trailing_slash` boundary. Accepting a
/// suggestion here replaces exactly that one `]` character, per the plan's "preserve the simple-
/// predicate workflow" design: selecting a `defName` suggestion still finishes the predicate
/// (`defName="Wall"]`), and only from *there* -- caret right after the now-typed `]` -- does the
/// user get offered a way back into it via `or`/`and`.
fn predicate_close_continuation(
    xpath: &str,
    close_pos: usize,
    target: XPathTarget,
) -> XPathCompletionResult {
    let items = boolean_operator_items(xpath, close_pos, "");
    XPathCompletionResult::new(close_pos, close_pos + 1, items, Vec::new(), target, None)
}

/// Completion for a predicate clause still being typed -- its key, or its value up to an
/// unterminated quote. Unchanged from the pre-boolean-chain single-clause `predicate_completion`:
/// callers now reach this only for the *last* term of a (possibly multi-clause) predicate, via
/// [`predicate_completion`]/[`TailState::Incomplete`].
fn key_or_value_completion(
    def_index: &DefIndex,
    xpath: &str,
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
            let key_start = base_offset + trimmed_start;
            return XPathCompletionResult::new(
                key_start,
                predicate_key_token_end(xpath, key_start),
                vec![predicate_key_item(key)],
                Vec::new(),
                XPathTarget::DefType {
                    def_type: def_type.to_string(),
                },
                None,
            );
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
        let value_end = quoted_value_end(xpath, value_start, quote_char);

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
            return XPathCompletionResult::new(
                value_start,
                value_end,
                items,
                Vec::new(),
                XPathTarget::DefType {
                    def_type: def_type.to_string(),
                },
                None,
            );
        }
        // `@Name`/`@ParentName` are recognized, supported predicate syntax, but RimEdit has no
        // index of inheritance-template `Name=`/`ParentName=` identifiers to suggest values from.
        return XPathCompletionResult::new(
            value_start,
            value_end,
            Vec::new(),
            Vec::new(),
            XPathTarget::DefType {
                def_type: def_type.to_string(),
            },
            None,
        );
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
    let key_start = base_offset + trimmed_start;
    XPathCompletionResult::new(
        key_start,
        predicate_key_token_end(xpath, key_start),
        items,
        Vec::new(),
        XPathTarget::DefType {
            def_type: def_type.to_string(),
        },
        None,
    )
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

fn root_completion(xpath: &str, replace_from: usize) -> XPathCompletionResult {
    XPathCompletionResult::new(
        replace_from,
        identifier_token_end(xpath, replace_from),
        vec![XPathCompletionItem {
            insert_text: "Defs".to_string(),
            label: "Defs".to_string(),
            detail: Some("Patch document root".to_string()),
            kind: XPathCompletionItemKind::Root,
        }],
        Vec::new(),
        XPathTarget::Unsupported,
        None,
    )
}

fn def_type_completion(
    catalog: &SchemaCatalog,
    xpath: &str,
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
    XPathCompletionResult::capped(
        replace_from,
        identifier_token_end(xpath, replace_from),
        items,
        COMPLETION_ITEM_LIMIT,
        Vec::new(),
        XPathTarget::Unsupported,
        None,
    )
}

/// Append `Field`/`FieldAlias` items for every `(name, field)` pair whose name or XML alias
/// starts with `needle` (case-insensitive; an empty `needle` matches everything), preserving
/// `fields`' own iteration order. Shared by [`field_completion`] (direct Def fields) and
/// [`object_field_completion`] (nested object-type fields via [`collect_object_fields_ordered`]),
/// which otherwise duplicated this exact matching loop over two differently-shaped field
/// iterables.
fn push_matching_field_items<'a>(
    fields: impl IntoIterator<Item = (impl AsRef<str>, &'a FieldSchema)>,
    needle: &str,
    items: &mut Vec<XPathCompletionItem>,
) {
    for (name, field) in fields {
        let name = name.as_ref();
        if needle.is_empty() || name.to_lowercase().starts_with(needle) {
            items.push(XPathCompletionItem {
                insert_text: name.to_string(),
                label: name.to_string(),
                detail: field.label.clone(),
                kind: XPathCompletionItemKind::Field,
            });
        }
        for alias in &field.xml_aliases {
            if needle.is_empty() || alias.to_lowercase().starts_with(needle) {
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

fn field_completion(
    catalog: &SchemaCatalog,
    xpath: &str,
    def_type: &str,
    target: XPathTarget,
    replace_from: usize,
    partial: &str,
) -> XPathCompletionResult {
    if !is_ident_prefix(partial) {
        return XPathCompletionResult::new(
            replace_from,
            replace_from,
            Vec::new(),
            vec![XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_pattern",
                "Field segment must be a plain element name; attribute-node targeting and functions are not supported here.",
            )],
            target,
            None,
        );
    }

    let mut items = Vec::new();
    if let Some(schema) = catalog.def_types.get(def_type) {
        let needle = partial.to_lowercase();
        push_matching_field_items(&schema.fields, &needle, &mut items);
    }
    items.sort_by(|a, b| a.label.cmp(&b.label));

    let (resolved_field, diagnostics) = resolve_field(catalog, def_type, partial);
    XPathCompletionResult::capped(
        replace_from,
        identifier_token_end(xpath, replace_from),
        items,
        COMPLETION_ITEM_LIMIT,
        diagnostics,
        target,
        resolved_field,
    )
}

/// Resolve a fully-typed field name (or alias) against a Def type's *direct* fields only.
/// Returns an `xpath_autocomplete_inherited_field` diagnostic when `name` is a real field but only
/// reachable through the schema's `inherits` chain -- RimWorld applies patches before XML
/// inheritance, so such a field cannot be targeted unless it is physically present in the XML
/// being patched (which this module, having no access to the live combined document, can't check).
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
        return (None, vec![inherited_field_diagnostic(name, def_type)]);
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
// Schema cursor traversal (unlimited-depth field/structural descent)
// ---------------------------------------------------------------------------

/// Where the segment walk is positioned in the schema after the Def type/predicate segment.
/// Contains only catalog-derived identifiers and this small state enum -- never project XML --
/// so completion stays a pure, fast, deterministic function of the typed text and the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SchemaCursor {
    /// Direct XML children of a Def type. RimWorld applies patches before Def XML inheritance, so
    /// only fields declared directly on `def_type` are completed/resolved here -- an
    /// ancestor-only field surfaces the existing `xpath_autocomplete_inherited_field` diagnostic
    /// instead (see [`resolve_field`]/[`is_inherited_only_field`]).
    DefFields { def_type: String },
    /// Fields of an object-type schema, resolved through its `inherits` chain and XML aliases
    /// (`schema_pack::lookup_object_field_with_alias`/`collect_object_fields_ordered`). Unlike
    /// `DefFields`, inherited fields are completed/resolved directly: object-type inheritance is
    /// ordinary schema inheritance already resolved on one XML element, not RimWorld's
    /// before-patches Def inheritance.
    ObjectFields { schema_ref: String },
    /// A `listOfLi` field whose items are objects: the next segment must be `li` or `li[n]` (see
    /// [`parse_list_item_step`]) before entering `schema_ref`'s object fields. For a
    /// discriminator-based item type, only the declared base `schema_ref` is traversed --
    /// narrowing to one `Class="..."` variant's own members would require predicate parsing beyond
    /// the plain positional index, which stays outside this conservative grammar (see module
    /// docs).
    ListItem { schema_ref: String },
    /// A `keyedObjectMap` field: the next segment must be `li` or `li[n]` before entering a map
    /// entry (see [`SchemaCursor::KeyedMapEntry`]).
    KeyedMapLi { schema_ref: String },
    /// Inside a `keyedObjectMap` `<li>`: the next segment is either `key` (a scalar terminal) or
    /// `value` (enters `schema_ref`'s object fields).
    KeyedMapEntry { schema_ref: String },
    /// A `keyedObjectList` field: the next segment is a data-dependent key (the item's own XML
    /// element name, e.g. a `defName`) rather than a schema-known name. RimEdit has no index of
    /// these keys, so no suggestions are offered at an empty segment; any non-empty, well-formed
    /// segment is accepted as the key and transitions into `schema_ref`'s object fields.
    DynamicKey { schema_ref: String },
    /// A scalar field, or an XML shape with no statically known descendants (scalar lists,
    /// `namedChildrenMap`, `keyedValueList`, `typedReferenceList`, attributes/text/flags, or an
    /// object/list field whose `schemaRef` doesn't resolve in the catalog).
    Terminal,
}

/// Choose the next [`SchemaCursor`] after `field` resolves, from both its `field_type.kind` and
/// `xml` shape together. Falls back to [`SchemaCursor::Terminal`] for any shape with no
/// statically known descendants, including an
/// object/list field whose declared `schemaRef` isn't present in `catalog` -- an unknown ref stays
/// editable but uncompleted rather than guessed at.
fn cursor_after_field(catalog: &SchemaCatalog, field: &FieldSchema) -> SchemaCursor {
    match field.xml {
        // Some object fields use `xml: element` rather than `xml: object` (mirrors
        // `xml_document::validation::fields::validate_object_children`'s own
        // `matches!(.., XmlFieldShape::Object | XmlFieldShape::Element)` check).
        XmlFieldShape::Object | XmlFieldShape::Element
            if field.field_type.kind == FieldTypeKind::Object =>
        {
            object_schema_cursor(catalog, field.field_type.schema_ref.as_deref())
        }
        XmlFieldShape::ListOfLi if field.field_type.kind == FieldTypeKind::List => {
            match object_item_schema_ref(catalog, field) {
                Some(schema_ref) => SchemaCursor::ListItem {
                    schema_ref: schema_ref.to_string(),
                },
                None => SchemaCursor::Terminal,
            }
        }
        XmlFieldShape::KeyedObjectMap => match object_item_schema_ref(catalog, field) {
            Some(schema_ref) => SchemaCursor::KeyedMapLi {
                schema_ref: schema_ref.to_string(),
            },
            None => SchemaCursor::Terminal,
        },
        XmlFieldShape::KeyedObjectList => match object_item_schema_ref(catalog, field) {
            Some(schema_ref) => SchemaCursor::DynamicKey {
                schema_ref: schema_ref.to_string(),
            },
            None => SchemaCursor::Terminal,
        },
        // Scalar lists, NamedChildrenMap, KeyedValueList, TypedReferenceList, Attribute, Text,
        // FlagsText: no statically known child schema.
        _ => SchemaCursor::Terminal,
    }
}

fn object_schema_cursor(catalog: &SchemaCatalog, schema_ref: Option<&str>) -> SchemaCursor {
    match schema_ref {
        Some(schema_ref) if catalog.object_types.contains_key(schema_ref) => {
            SchemaCursor::ObjectFields {
                schema_ref: schema_ref.to_string(),
            }
        }
        _ => SchemaCursor::Terminal,
    }
}

/// `field.items.schemaRef` when `items.kind` is `object` and it resolves in `catalog`.
fn object_item_schema_ref<'a>(catalog: &SchemaCatalog, field: &'a FieldSchema) -> Option<&'a str> {
    let items = field.items.as_ref()?;
    if items.kind != FieldTypeKind::Object {
        return None;
    }
    let schema_ref = items.schema_ref.as_deref()?;
    catalog
        .object_types
        .contains_key(schema_ref)
        .then_some(schema_ref)
}

/// The three shapes a `listOfLi`/`keyedObjectMap` entry step (`li`, `li[n]`, ...) can take.
/// Deliberately independent of any RimWorld schema or project data -- see [`parse_list_item_step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListItemStep {
    /// Exactly `li`, or `li[n]` for a positive decimal `n` with no trailing content.
    Valid,
    /// A trailing, unclosed positional predicate (`li[`, `li[12`) -- still being typed.
    Incomplete,
    /// Anything else: a non-`li` step, a malformed/unsupported predicate (`li[0]`, `li[-1]`,
    /// `li[1.5]`, `li[last()]`, `li[@Class="..."]`), multiple predicates (`li[1][2]`), or trailing
    /// content after the closing bracket (`li[1]extra`).
    Invalid,
}

/// Classify a single fully-typed list-entry step's text against the grammar `"li" | "li"
/// "[" positive-integer "]"` (see module docs). Reuses [`find_matching_close`] so the closing
/// bracket must belong to the first (and only) opening bracket, which naturally rejects a second
/// adjacent predicate and any trailing text. The index is validated lexically (leading digit
/// 1-9, remaining digits 0-9) rather than parsed to an integer, so an arbitrarily long decimal
/// index isn't rejected by an arbitrary overflow limit.
fn parse_list_item_step(text: &str) -> ListItemStep {
    let Some(rest) = text.strip_prefix("li") else {
        return ListItemStep::Invalid;
    };
    if rest.is_empty() {
        return ListItemStep::Valid;
    }
    if !rest.starts_with('[') {
        return ListItemStep::Invalid;
    }
    match find_matching_close(rest, 0) {
        None => ListItemStep::Incomplete,
        Some(close_pos) if close_pos != rest.len() - 1 => ListItemStep::Invalid,
        Some(close_pos) => {
            let content = &rest[1..close_pos];
            if is_positive_integer(content) {
                ListItemStep::Valid
            } else {
                ListItemStep::Invalid
            }
        }
    }
}

/// A non-empty decimal literal with no leading zero, e.g. `"1"`, `"10"` -- rejects `"0"`, `"01"`,
/// `"-1"`, `"1.5"`, and non-numeric content alike.
fn is_positive_integer(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() && c != '0' => chars.all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

/// Resolve one fully-typed segment against `cursor`, mutating it in place to the next position.
/// Returns the field just resolved (for [`XPathResolvedField`] accumulation) when this segment
/// named a real field, `Ok(None)` for a structural transition (`li`/`value`) that doesn't itself
/// name a field, or `Err` with the existing `xpath_autocomplete_unsupported_pattern` diagnostic
/// when `cursor` cannot make sense of `text` at all -- callers stop the walk on `Err` rather than
/// guessing. A [`SchemaCursor::Terminal`] cursor always succeeds with `Ok(None)` and no diagnostic:
/// typing past a scalar field is simply not completed, not an error (see module docs).
fn resolve_and_transition(
    catalog: &SchemaCatalog,
    cursor: &mut SchemaCursor,
    def_type: &str,
    text: &str,
) -> Result<Option<XPathResolvedField>, XPathDiagnostic> {
    match cursor.clone() {
        SchemaCursor::DefFields { def_type: dt } => {
            if let Some((canonical, field)) = direct_field(catalog, &dt, text) {
                let resolved = XPathResolvedField {
                    def_type: dt.clone(),
                    field_name: canonical.to_string(),
                    field: field.clone(),
                };
                *cursor = cursor_after_field(catalog, field);
                Ok(Some(resolved))
            } else if is_inherited_only_field(catalog, &dt, text) {
                Err(inherited_field_diagnostic(text, &dt))
            } else {
                Err(unsupported_child_segment())
            }
        }
        SchemaCursor::ObjectFields { schema_ref } => {
            if let Some((canonical, field)) =
                lookup_object_field_with_alias(catalog, &schema_ref, text)
            {
                let resolved = XPathResolvedField {
                    def_type: def_type.to_string(),
                    field_name: canonical.to_string(),
                    field: field.clone(),
                };
                *cursor = cursor_after_field(catalog, field);
                Ok(Some(resolved))
            } else {
                Err(unsupported_child_segment())
            }
        }
        SchemaCursor::ListItem { schema_ref } => match parse_list_item_step(text) {
            ListItemStep::Valid => {
                *cursor = SchemaCursor::ObjectFields { schema_ref };
                Ok(None)
            }
            // Structurally can't happen for a fully-typed (non-final) segment -- an unclosed `[`
            // keeps bracket depth > 0, so `split_segments` can't have split a `/` after it.
            ListItemStep::Incomplete => Ok(None),
            ListItemStep::Invalid => Err(unsupported_child_segment()),
        },
        SchemaCursor::KeyedMapLi { schema_ref } => match parse_list_item_step(text) {
            ListItemStep::Valid => {
                *cursor = SchemaCursor::KeyedMapEntry { schema_ref };
                Ok(None)
            }
            ListItemStep::Incomplete => Ok(None),
            ListItemStep::Invalid => Err(unsupported_child_segment()),
        },
        SchemaCursor::KeyedMapEntry { schema_ref } => match text {
            "key" => {
                *cursor = SchemaCursor::Terminal;
                Ok(None)
            }
            "value" => {
                *cursor = SchemaCursor::ObjectFields { schema_ref };
                Ok(None)
            }
            _ => Err(unsupported_child_segment()),
        },
        SchemaCursor::DynamicKey { schema_ref } => {
            if is_valid_identifier(text) {
                *cursor = SchemaCursor::ObjectFields { schema_ref };
                Ok(None)
            } else {
                Err(unsupported_child_segment())
            }
        }
        // Nothing more to resolve past a scalar/unresolvable field -- stay silent, not an error.
        SchemaCursor::Terminal => Ok(None),
    }
}

fn unsupported_child_segment() -> XPathDiagnostic {
    XPathDiagnostic::warning(
        "xpath_autocomplete_unsupported_pattern",
        "This path segment does not match a known schema field or structural entry, so deeper autocomplete stops here.",
    )
}

fn inherited_field_diagnostic(name: &str, def_type: &str) -> XPathDiagnostic {
    XPathDiagnostic::warning(
        "xpath_autocomplete_inherited_field",
        format!(
            "'{name}' is declared on a schema parent of '{def_type}', not on '{def_type}' itself. RimWorld applies patches before XML inheritance, so this field can only be targeted if it is physically present in the XML being patched."
        ),
    )
    .with_args(crate::diagnostics::diagnostic_args([
        ("fieldName", name.into()),
        ("defType", def_type.into()),
    ]))
}

/// Build the completion result for the segment currently being typed (in progress, or an empty
/// segment right after a trailing slash), dispatching on the cursor's current position.
#[allow(clippy::too_many_arguments)]
fn child_completion(
    catalog: &SchemaCatalog,
    xpath: &str,
    cursor: &SchemaCursor,
    def_type: &str,
    target: XPathTarget,
    replace_from: usize,
    partial: &str,
    resolved_field: Option<XPathResolvedField>,
) -> XPathCompletionResult {
    match cursor {
        SchemaCursor::DefFields { def_type } => {
            field_completion(catalog, xpath, def_type, target, replace_from, partial)
        }
        SchemaCursor::ObjectFields { schema_ref } => object_field_completion(
            catalog,
            xpath,
            schema_ref,
            def_type,
            target,
            replace_from,
            partial,
            resolved_field,
        ),
        SchemaCursor::ListItem { .. } | SchemaCursor::KeyedMapLi { .. } => {
            list_item_step_completion(xpath, target, replace_from, partial, resolved_field)
        }
        SchemaCursor::KeyedMapEntry { .. } => literal_segment_completion(
            xpath,
            &["key", "value"],
            XPathCompletionItemKind::MapEntry,
            target,
            replace_from,
            partial,
            resolved_field,
        ),
        // DynamicKey: the key is data-dependent (e.g. a defName RimEdit has no index of) -- offer
        // no invented suggestions.
        SchemaCursor::DynamicKey { .. } | SchemaCursor::Terminal => XPathCompletionResult::new(
            replace_from,
            replace_from,
            Vec::new(),
            Vec::new(),
            target,
            resolved_field,
        ),
    }
}

/// Suggest fields (and XML aliases) of an object-type schema, resolved through its `inherits`
/// chain -- the `ObjectFields` cursor's counterpart to [`field_completion`]. Also resolves `partial`
/// as a terminal field for the value subform when it's a complete, exact field name or alias (no
/// "inherited-only" restriction here: see [`SchemaCursor::ObjectFields`]'s docs).
///
/// `container_field` is the field the walk already resolved to reach this cursor (e.g.
/// `graphicData` itself, for the `ObjectFields { schema_ref: "GraphicData" }` cursor) -- when
/// `partial` doesn't exactly resolve to one of `schema_ref`'s own fields (an empty segment right
/// after a trailing slash, or a still-being-typed prefix), this is kept as the best-effort
/// resolved field rather than discarded to `None`, matching every other cursor kind's
/// "in-progress typing still resolves to the last known-good field" behavior (see
/// `SchemaCursor::ListItem`/`KeyedMapEntry`'s `literal_segment_completion` callers).
#[allow(clippy::too_many_arguments)]
fn object_field_completion(
    catalog: &SchemaCatalog,
    xpath: &str,
    schema_ref: &str,
    def_type: &str,
    target: XPathTarget,
    replace_from: usize,
    partial: &str,
    container_field: Option<XPathResolvedField>,
) -> XPathCompletionResult {
    if !is_ident_prefix(partial) {
        return XPathCompletionResult::new(
            replace_from,
            replace_from,
            Vec::new(),
            vec![XPathDiagnostic::warning(
                "xpath_autocomplete_unsupported_pattern",
                "Field segment must be a plain element name; attribute-node targeting and functions are not supported here.",
            )],
            target,
            container_field,
        );
    }

    let needle = partial.to_lowercase();
    let mut items = Vec::new();
    push_matching_field_items(
        collect_object_fields_ordered(catalog, schema_ref),
        &needle,
        &mut items,
    );
    items.sort_by(|a, b| a.label.cmp(&b.label));

    let resolved_field = lookup_object_field_with_alias(catalog, schema_ref, partial)
        .map(|(canonical, field)| XPathResolvedField {
            def_type: def_type.to_string(),
            field_name: canonical.to_string(),
            field: field.clone(),
        })
        .or(container_field);

    XPathCompletionResult::capped(
        replace_from,
        identifier_token_end(xpath, replace_from),
        items,
        COMPLETION_ITEM_LIMIT,
        Vec::new(),
        target,
        resolved_field,
    )
}

/// Completion for the segment currently being typed at a `ListItem`/`KeyedMapLi` cursor: the
/// structural `li` suggestion while `partial` is still a plain identifier prefix (`""`, `"l"`,
/// `"li"`), and -- once brackets appear -- [`parse_list_item_step`]'s classification instead. A
/// complete, valid positional predicate (`li[1]`) or one still being typed (`li[`, `li[1`) yields
/// no items and no diagnostic; a completed invalid predicate yields the same
/// `xpath_autocomplete_unsupported_pattern` diagnostic a fully-typed invalid segment gets from
/// [`resolve_and_transition`], so invalid syntax reads the same whether or not a `/` follows it.
fn list_item_step_completion(
    xpath: &str,
    target: XPathTarget,
    replace_from: usize,
    partial: &str,
    resolved_field: Option<XPathResolvedField>,
) -> XPathCompletionResult {
    if is_ident_prefix(partial) {
        return literal_segment_completion(
            xpath,
            &["li"],
            XPathCompletionItemKind::ListItem,
            target,
            replace_from,
            partial,
            resolved_field,
        );
    }
    match parse_list_item_step(partial) {
        ListItemStep::Valid | ListItemStep::Incomplete => XPathCompletionResult::new(
            replace_from,
            replace_from,
            Vec::new(),
            Vec::new(),
            target,
            resolved_field,
        ),
        ListItemStep::Invalid => XPathCompletionResult::new(
            replace_from,
            replace_from,
            Vec::new(),
            vec![unsupported_child_segment()],
            target,
            resolved_field,
        ),
    }
}

/// Suggest a fixed set of structural literal segments (`li`, or `key`/`value`) matching `partial`
/// as a plain-text prefix. Shared by the `ListItem`/`KeyedMapLi` and `KeyedMapEntry` cursors --
/// only the candidate literals and item kind differ.
fn literal_segment_completion(
    xpath: &str,
    literals: &[&str],
    kind: XPathCompletionItemKind,
    target: XPathTarget,
    replace_from: usize,
    partial: &str,
    resolved_field: Option<XPathResolvedField>,
) -> XPathCompletionResult {
    if !is_ident_prefix(partial) {
        return XPathCompletionResult::new(
            replace_from,
            replace_from,
            Vec::new(),
            Vec::new(),
            target,
            resolved_field,
        );
    }
    let items = literals
        .iter()
        .filter(|l| l.starts_with(partial))
        .map(|l| XPathCompletionItem {
            insert_text: l.to_string(),
            label: l.to_string(),
            detail: None,
            kind,
        })
        .collect();
    XPathCompletionResult::new(
        replace_from,
        identifier_token_end(xpath, replace_from),
        items,
        Vec::new(),
        target,
        resolved_field,
    )
}

// ---------------------------------------------------------------------------
// Shared small helpers
// ---------------------------------------------------------------------------

fn is_ident_prefix(s: &str) -> bool {
    s.is_empty() || is_valid_identifier(s)
}

fn terminal(replace_from: usize, diagnostic: XPathDiagnostic) -> XPathCompletionResult {
    XPathCompletionResult::new(
        replace_from,
        replace_from,
        Vec::new(),
        vec![diagnostic],
        XPathTarget::Unsupported,
        None,
    )
}

fn empty(replace_from: usize, target: XPathTarget) -> XPathCompletionResult {
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
