//! Def predicate grammar: parsing a completed (already `]`-closed) boolean chain of equality
//! clauses, classifying it into an [`XPathTarget`], and classifying a still-being-typed predicate's
//! last term for live completion. Pure parsing/classification -- this module returns structured
//! parse state, never a serialized [`super::types::XPathCompletionResult`]; predicate-specific
//! completion *presentation* driven by that state lives in [`super::completion`].

use super::scan::is_identifier_byte;
use crate::patches::impact_graph::XPathTarget;

/// One equality clause recognized inside a Def predicate (`defName="..."`, `@Name="..."`, or
/// `@ParentName="..."`), as parsed by [`parse_single_clause`].
#[derive(Debug, Clone)]
pub(super) struct ParsedClause {
    pub(super) key: PredicateKey,
    pub(super) value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PredicateKey {
    DefName,
    NameAttr,
    ParentNameAttr,
}

/// A boolean connective joining two predicate clauses, recognized only as a standalone lowercase
/// token outside quoted values (see [`split_top_level_bool_terms`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoolOp {
    Or,
    And,
}

/// Resolves a fully-typed `defName="..."` / `@Name="..."` / `@ParentName="..."` equality clause,
/// same conservatism as the old single-clause `parse_predicate_content`: an unquoted, empty, or
/// otherwise malformed value fails the whole clause to `None` rather than guessing.
pub(super) fn parse_single_clause(content: &str) -> Option<ParsedClause> {
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
pub(super) fn parse_boolean_chain(content: &str) -> Option<(Vec<ParsedClause>, Vec<BoolOp>)> {
    let (terms, ops) = split_top_level_bool_terms(content);
    let clauses = terms
        .iter()
        .map(|t| parse_single_clause(content[t.start..t.end].trim()))
        .collect::<Option<Vec<_>>>()?;
    Some((clauses, ops))
}

/// Classifies a successfully-parsed boolean predicate chain into an [`XPathTarget`], per the
/// `xpath` module docs' target-inference guarantees: a single `defName="A"` targets exactly one
/// Def; a 2+-term OR-only chain of `defName="..."` equalities targets exactly those Defs (matching
/// `impact_graph::infer_xpath_target`'s own OR-only-`defName` subset, see module docs); anything
/// else supported by the grammar (an `and`, an `@Name`/`@ParentName` clause, or a mixture of
/// operators/keys) still pins down the Def *type* but not a specific Def or Def set.
pub(super) fn classify_boolean_chain(
    def_type: &str,
    clauses: &[ParsedClause],
    ops: &[BoolOp],
) -> XPathTarget {
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
pub(super) struct BoolTerm {
    pub(super) start: usize,
    pub(super) end: usize,
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
/// per the `xpath` module docs (autocomplete's conservative subset is intentionally more permissive
/// than impact inference's).
pub(super) fn split_top_level_bool_terms(content: &str) -> (Vec<BoolTerm>, Vec<BoolOp>) {
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

/// How a predicate's last (still-being-typed) term reads, classified by [`classify_tail`].
pub(super) enum TailState<'a> {
    /// Still typing this clause's key or value (no `=` yet, no opening quote yet, or an
    /// unterminated value) -- the caller falls through to key/value completion, which is exactly
    /// the old single-clause `predicate_completion` behavior.
    Incomplete,
    /// A complete, valid clause (its value's closing quote is present) followed only by
    /// whitespace and/or a partial `or`/`and` word. `prefix` is that trailing word (trailing
    /// whitespace of its own already stripped) and `start` its byte offset within the tail.
    Continuation { prefix: &'a str, start: usize },
    /// A complete clause followed by content that is neither whitespace nor an `or`/`and` prefix.
    Invalid,
}

/// Classifies a predicate's last, still-being-typed term (see [`TailState`]). Reuses the same
/// key/`=`/quote scanning shape as `completion::key_or_value_completion` just to detect whether the
/// value's closing quote has *already* been typed -- once it has, what matters here is only what
/// comes after it, not re-deriving completion items (that stays the incomplete-tail caller's job).
pub(super) fn classify_tail(tail: &str) -> TailState<'_> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn clause_value(content: &str) -> Option<(PredicateKey, String)> {
        parse_single_clause(content).map(|c| (c.key, c.value))
    }

    #[test]
    fn parse_single_clause_accepts_each_supported_key() {
        assert_eq!(
            clause_value(r#"defName="Wall""#),
            Some((PredicateKey::DefName, "Wall".to_string()))
        );
        assert_eq!(
            clause_value(r#"@Name="WallBase""#),
            Some((PredicateKey::NameAttr, "WallBase".to_string()))
        );
        assert_eq!(
            clause_value(r#"@ParentName="WallBase""#),
            Some((PredicateKey::ParentNameAttr, "WallBase".to_string()))
        );
    }

    #[test]
    fn parse_single_clause_rejects_unquoted_empty_or_mixed_quotes() {
        assert!(parse_single_clause(r#"defName=Wall"#).is_none());
        assert!(parse_single_clause(r#"defName="""#).is_none());
        assert!(parse_single_clause("defName=\"Wall'\"").is_none());
        assert!(parse_single_clause("unknownKey=\"Wall\"").is_none());
    }

    #[test]
    fn split_top_level_bool_terms_ignores_or_and_inside_identifiers_and_quotes() {
        let (terms, ops) =
            split_top_level_bool_terms(r#"defName="MN_NetworkController" or defName="A or B""#);
        assert_eq!(terms.len(), 2);
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], BoolOp::Or));
    }

    #[test]
    fn split_top_level_bool_terms_recognizes_and() {
        let (terms, ops) = split_top_level_bool_terms(r#"defName="A" and @Name="B""#);
        assert_eq!(terms.len(), 2);
        assert!(matches!(ops[0], BoolOp::And));
    }

    #[test]
    fn split_top_level_bool_terms_leading_operator_yields_empty_first_term() {
        let (terms, ops) = split_top_level_bool_terms(r#" or defName="A""#);
        assert_eq!(ops.len(), 1);
        assert_eq!(terms[0].start, terms[0].end.min(terms[0].start));
    }

    #[test]
    fn classify_boolean_chain_single_def_name_targets_one_def() {
        let clauses = vec![ParsedClause {
            key: PredicateKey::DefName,
            value: "Wall".to_string(),
        }];
        let target = classify_boolean_chain("ThingDef", &clauses, &[]);
        assert!(matches!(target, XPathTarget::Def { .. }));
    }

    #[test]
    fn classify_boolean_chain_or_only_def_names_targets_def_set() {
        let clauses = vec![
            ParsedClause {
                key: PredicateKey::DefName,
                value: "A".to_string(),
            },
            ParsedClause {
                key: PredicateKey::DefName,
                value: "B".to_string(),
            },
        ];
        let target = classify_boolean_chain("ThingDef", &clauses, &[BoolOp::Or]);
        assert!(matches!(target, XPathTarget::Defs { .. }));
    }

    #[test]
    fn classify_boolean_chain_mixed_operators_targets_def_type_only() {
        let clauses = vec![
            ParsedClause {
                key: PredicateKey::DefName,
                value: "A".to_string(),
            },
            ParsedClause {
                key: PredicateKey::DefName,
                value: "B".to_string(),
            },
        ];
        let target = classify_boolean_chain("ThingDef", &clauses, &[BoolOp::And]);
        assert!(matches!(target, XPathTarget::DefType { .. }));
    }

    #[test]
    fn classify_tail_incomplete_before_equals_or_quote() {
        assert!(matches!(classify_tail("defName"), TailState::Incomplete));
        assert!(matches!(classify_tail("defName="), TailState::Incomplete));
        assert!(matches!(
            classify_tail(r#"defName="Wa"#),
            TailState::Incomplete
        ));
    }

    #[test]
    fn classify_tail_continuation_after_closed_clause() {
        match classify_tail(r#"defName="Wall" o"#) {
            TailState::Continuation { prefix, .. } => assert_eq!(prefix, "o"),
            _ => panic!("expected continuation"),
        }
    }

    #[test]
    fn classify_tail_invalid_after_closed_clause_with_unrelated_trailing_text() {
        assert!(matches!(
            classify_tail(r#"defName="Wall" xyz"#),
            TailState::Invalid
        ));
    }

    #[test]
    fn classify_tail_invalid_for_empty_value() {
        assert!(matches!(classify_tail(r#"defName="""#), TailState::Invalid));
    }
}
