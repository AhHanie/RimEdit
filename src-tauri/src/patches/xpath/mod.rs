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
//! [`predicate::parse_boolean_chain`]/[`predicate::split_top_level_bool_terms`] for the
//! finished-predicate grammar and [`completion::predicate_completion`]/[`predicate::classify_tail`]
//! for live completion while typing one). Target inference here is again more permissive than
//! impact analysis: a 2+-term OR-only chain of `defName="..."` equalities resolves to
//! `XPathTarget::Defs`, matching `impact_graph::infer_xpath_target`'s own OR-only-`defName` subset,
//! while any other supported chain (an `and`, an `@Name`/`@ParentName` clause, or a mixture of
//! operators/keys) still pins down the Def *type* -- `XPathTarget::DefType` -- rather than a
//! specific Def or Def set (see [`predicate::classify_boolean_chain`]).
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
//! or a predicate's own `[...]` body), via [`scan::Segment::trimmed`]. Whitespace *inside* a
//! predicate around `defName`/`=`/quote boundaries was already tolerated before caret-awareness
//! landed (see `predicate::parse_single_clause`/`predicate::parse_eq_quoted_value`/
//! `completion::predicate_completion`'s own trimming) and is unchanged here. Whitespace inside a
//! quoted literal is never touched by either. A line break by itself is just more whitespace to
//! this trimming and never triggers `xpath_autocomplete_unsupported_pattern` on its own.
//!
//! Field-segment depth is *not* capped: once the Def type/predicate segment resolves, a
//! [`schema::SchemaCursor`] (driven by [`schema::TraversalState`]) walks every remaining segment
//! against the merged `SchemaCatalog`, descending through nested `object` fields, `listOfLi` object
//! items (`/li[n]/...`), `keyedObjectMap` entries (`/li[n]/key` or `/li[n]/value/...`), and
//! `keyedObjectList` data-dependent keys (`/<key>/...`) for as long as the schema keeps resolving
//! concrete, statically-known children. A `listOfLi`/`keyedObjectMap` entry step optionally carries
//! a single one-based positional predicate (`li[1]`, `li[2]`, ...); it only changes *which* XML
//! list entry the XPath selects, never the selected entry's schema, so `li` and `li[n]` transition
//! the cursor identically (see [`schema::parse_list_item_step`]). Traversal stops -- silently, or
//! with `xpath_autocomplete_unsupported_pattern` when a *typed* segment doesn't match anything the
//! cursor understands -- at a scalar field, an XML shape with no statically known descendants
//! (scalar lists, `namedChildrenMap`, `keyedValueList`, `typedReferenceList`, attributes/text/
//! flags), an unresolvable `schemaRef`, or a discriminator-based list item's variant-specific
//! members (only the declared base `items.schemaRef` is traversed; narrowing to one `Class="..."`
//! variant remains unsupported since it would need predicate parsing beyond the plain positional
//! index, which stays outside this conservative grammar -- see [`schema::SchemaCursor::ListItem`]).

mod completion;
mod predicate;
mod scan;
mod schema;
#[cfg(test)]
mod test_support;
mod types;

use crate::def_index::DefIndex;
use crate::schema_pack::SchemaCatalog;

use super::impact_graph::{is_valid_identifier, XPathTarget};
use scan::{clamp_to_char_boundary, find_matching_close, split_segments, Segment};
use schema::TraversalState;
use types::{empty, terminal};

pub(crate) use types::COMPLETION_ITEM_LIMIT;
pub use types::{
    XPathCompletionItem, XPathCompletionItemKind, XPathCompletionResult, XPathDiagnostic,
    XPathDiagnosticSeverity, XPathResolvedField,
};

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
        return completion::root_completion(xpath, cursor);
    }

    let (root_text, root_start) = segments[0].trimmed(xpath);
    if root_text != "Defs" {
        if segments.len() == 1
            && !trailing_slash
            && completion::is_ident_prefix(root_text)
            && "Defs".starts_with(root_text)
        {
            return XPathCompletionResult::new(
                root_start,
                scan::identifier_token_end(xpath, root_start),
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
            completion::def_type_completion(catalog, xpath, cursor, "")
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
            completion::field_completion(catalog, xpath, &def_type, target, cursor, "")
        } else if let Some(close_pos) = predicate_close {
            completion::predicate_close_continuation(xpath, close_pos, target)
        } else {
            empty(cursor, target)
        };
    }

    // segments.len() >= 3: one or more field/structural segments follow the Def type/predicate.
    // Walk them left-to-right against a schema cursor that starts at the Def's direct fields and
    // descends through object/list/map shapes with no fixed depth limit -- see module docs.
    let field_segments = &segments[2..];
    let mut traversal = TraversalState::new(def_type.clone());

    // Every segment except the one being completed is fully typed (has a `/` after it, whether
    // that's an interior segment or -- when `trailing_slash` -- the last one too).
    let complete_count = if trailing_slash {
        field_segments.len()
    } else {
        field_segments.len() - 1
    };
    for seg in &field_segments[..complete_count] {
        let (text, _) = seg.trimmed(xpath);
        if let Err(diagnostic) = traversal.transition(catalog, text) {
            return XPathCompletionResult::new(
                cursor,
                cursor,
                Vec::new(),
                vec![diagnostic],
                target,
                traversal.into_last_resolved(),
            );
        }
    }

    if trailing_slash {
        let cursor_ref = traversal.cursor().clone();
        completion::child_completion(
            catalog,
            xpath,
            &cursor_ref,
            &def_type,
            target,
            cursor,
            "",
            traversal.into_last_resolved(),
        )
    } else {
        let final_seg = &field_segments[complete_count];
        let (partial, partial_start) = final_seg.trimmed(xpath);
        let cursor_ref = traversal.cursor().clone();
        completion::child_completion(
            catalog,
            xpath,
            &cursor_ref,
            &def_type,
            target,
            partial_start,
            partial,
            traversal.into_last_resolved(),
        )
    }
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
        /// sits right after it with nothing typed beyond (see
        /// [`completion::predicate_close_continuation`]).
        predicate_close: Option<usize>,
    },
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
            return DefSegmentOutcome::Result(Box::new(completion::def_type_completion(
                catalog,
                xpath,
                trimmed_start,
                text,
            )));
        }
        if !is_valid_identifier(text) {
            return DefSegmentOutcome::Result(Box::new(terminal(
                cursor,
                completion::unsupported_def_type_diagnostic(),
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
            completion::unsupported_def_type_diagnostic(),
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
            DefSegmentOutcome::Result(Box::new(completion::predicate_completion(
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
            match predicate::parse_boolean_chain(content) {
                // A single `defName="A"` continues to return `XPathTarget::Def`; a single
                // `@Name`/`@ParentName` clause identifies an inheritance-template node rather than
                // a concrete `defName`-keyed Def, so it resolves to `XPathTarget::DefType` instead
                // -- see [`predicate::classify_boolean_chain`] for the 2+-clause cases (OR-only
                // `defName` chains -> `XPathTarget::Defs`, everything else supported ->
                // `XPathTarget::DefType`).
                Some((clauses, ops)) => DefSegmentOutcome::Resolved {
                    def_type: def_type_str.to_string(),
                    target: predicate::classify_boolean_chain(def_type_str, &clauses, &ops),
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
