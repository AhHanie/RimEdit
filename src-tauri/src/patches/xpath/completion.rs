//! Completion presentation: turns explicit context (a partial token, a resolved schema cursor, a
//! predicate parse state) into a serialized [`super::types::XPathCompletionResult`]. This is the
//! only module that constructs completion items and result envelopes -- callers elsewhere in
//! `xpath/` pass in already-classified state rather than re-deriving it.

use crate::def_index::{suggest_def_references, DefIndex};
use crate::patches::impact_graph::{is_valid_identifier, XPathTarget};
use crate::schema_pack::{
    collect_object_fields_ordered, lookup_object_field_with_alias, FieldSchema, ReferenceScope,
    SchemaCatalog,
};

use super::predicate::{classify_tail, parse_single_clause, split_top_level_bool_terms, TailState};
use super::scan::{identifier_token_end, predicate_key_token_end, quoted_value_end};
use super::schema::{
    parse_list_item_step, resolve_field, unsupported_child_segment, ListItemStep, SchemaCursor,
};
use super::types::{
    terminal, XPathCompletionItem, XPathCompletionItemKind, XPathCompletionResult, XPathDiagnostic,
    XPathResolvedField, COMPLETION_ITEM_LIMIT, DEF_NAME_SUGGESTION_LIMIT,
};

pub(super) fn unsupported_def_type_diagnostic() -> XPathDiagnostic {
    XPathDiagnostic::warning(
        "xpath_autocomplete_unsupported_pattern",
        "Def type segment must be a plain element name; wildcards and functions are not supported for autocomplete.",
    )
}

pub(super) fn root_completion(xpath: &str, replace_from: usize) -> XPathCompletionResult {
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

pub(super) fn def_type_completion(
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
/// [`object_field_completion`] (nested object-type fields via `collect_object_fields_ordered`),
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

pub(super) fn field_completion(
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

/// Build the completion result for the segment currently being typed (in progress, or an empty
/// segment right after a trailing slash), dispatching on the cursor's current position.
#[allow(clippy::too_many_arguments)]
pub(super) fn child_completion(
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
/// schema traversal, so invalid syntax reads the same whether or not a `/` follows it.
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
// Predicate key/value completion (open predicate: `[defName="Wa`, `[@Nam`, `[`, ...)
// ---------------------------------------------------------------------------

/// Completion for a still-open (unclosed `[`) Def predicate, up to the caret. Splits `partial`
/// (the predicate content typed so far) on top-level `or`/`and` the same way
/// [`split_top_level_bool_terms`] does for a finished predicate; every term but the last must
/// already be a complete, valid clause (else the whole thing is an unsupported pattern, matching
/// `predicate::parse_boolean_chain`'s conservatism), and the last term -- still being typed -- is
/// classified by [`classify_tail`] into either "offer `or`/`and` continuation" or "fall through to
/// the existing single-clause key/value completion" ([`key_or_value_completion`]).
pub(super) fn predicate_completion(
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
pub(super) fn predicate_close_continuation(
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

pub(super) fn is_ident_prefix(s: &str) -> bool {
    s.is_empty() || is_valid_identifier(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_ident_prefix_accepts_empty_and_valid_identifiers_only() {
        assert!(is_ident_prefix(""));
        assert!(is_ident_prefix("ThingDef"));
        assert!(!is_ident_prefix("Thing/Def"));
        assert!(!is_ident_prefix("@Name"));
    }

    #[test]
    fn is_operator_prefix_matches_or_and_and_case_insensitively() {
        assert!(is_operator_prefix(""));
        assert!(is_operator_prefix("O"));
        assert!(is_operator_prefix("AN"));
        assert!(!is_operator_prefix("xor"));
    }

    #[test]
    fn boolean_operator_items_adds_leading_space_only_when_needed() {
        let items = boolean_operator_items(r#"defName="Wall""#, 14, "");
        assert_eq!(items[0].insert_text, " or ");
        let items = boolean_operator_items(r#"defName="Wall" "#, 15, "");
        assert_eq!(items[0].insert_text, "or ");
    }

    #[test]
    fn boolean_operator_items_filters_by_prefix() {
        let items = boolean_operator_items("", 0, "a");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "and");
    }
}
