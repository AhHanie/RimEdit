//! Raw input scanning: caret clamping, path-segment splitting, bracket/quote-aware matching, and
//! token-end lexical helpers. This is reusable syntax infrastructure over the raw `xpath` string --
//! it knows nothing about schemas, Def indexes, or completion presentation. All offsets returned
//! here are byte offsets into the original `xpath` string passed to [`split_segments`] et al.

use super::types::XPathDiagnostic;

#[derive(Debug)]
pub(super) struct Segment {
    pub(super) start: usize,
    pub(super) end: usize,
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
    pub(super) fn trimmed<'a>(&self, xpath: &'a str) -> (&'a str, usize) {
        let raw = self.text(xpath);
        let leading = raw.len() - raw.trim_start().len();
        (raw.trim(), self.start + leading)
    }
}

/// Clamp `offset` to `xpath.len()` and then to the nearest char boundary at or before it, so
/// slicing `xpath` at the result never panics regardless of what a caller passes in. The Tauri
/// command layer independently rejects an out-of-range or mid-character `cursor_byte_offset`
/// outright with an `AppError` before ever reaching here (see `commands::patches`); this is
/// defense-in-depth for every other/direct caller of [`super::complete_patch_xpath_at`].
pub(super) fn clamp_to_char_boundary(xpath: &str, offset: usize) -> usize {
    let offset = offset.min(xpath.len());
    let mut offset = offset;
    while offset > 0 && !xpath.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The end of an identifier-like token starting at `from` (a Def type, field, root, or structural
/// `li`/`key`/`value` literal): the first byte offset `>= from` whose character is not a valid
/// identifier character (matching `impact_graph::is_valid_identifier`'s alphabet) -- naturally a
/// `/`, `[`, whitespace, or end of string, so trailing indentation before the next path segment is
/// excluded from the replacement span even though it is included when scanning past the caret into
/// a suffix the user already typed.
pub(super) fn identifier_token_end(xpath: &str, from: usize) -> usize {
    xpath[from..]
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map(|(i, _)| from + i)
        .unwrap_or(xpath.len())
}

/// The end of a predicate-key token (`defName`, `@Name`, `@ParentName`) starting at `from`: an
/// optional leading `@` followed by identifier characters, matching [`identifier_token_end`]'s
/// alphabet plus the one leading `@` these three keys can start with.
pub(super) fn predicate_key_token_end(xpath: &str, from: usize) -> usize {
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
pub(super) fn quoted_value_end(xpath: &str, from: usize, quote_char: char) -> usize {
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
pub(super) fn split_segments(xpath: &str) -> Result<Vec<Segment>, XPathDiagnostic> {
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
pub(super) fn find_matching_close(text: &str, open_pos: usize) -> Option<usize> {
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

pub(super) fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_segments_ignores_slash_and_brackets_inside_quotes() {
        let segments = split_segments(r#"Defs/ThingDef[defName="A/B[C]"]/statBases"#).unwrap();
        let texts: Vec<&str> = segments
            .iter()
            .map(|s| &r#"Defs/ThingDef[defName="A/B[C]"]/statBases"#[s.start..s.end])
            .collect();
        assert_eq!(
            texts,
            vec!["Defs", r#"ThingDef[defName="A/B[C]"]"#, "statBases"]
        );
    }

    #[test]
    fn split_segments_reports_unmatched_close_bracket() {
        let err = split_segments("Defs/ThingDef]").unwrap_err();
        assert_eq!(err.code, "xpath_invalid_syntax");
    }

    #[test]
    fn split_segments_allows_unclosed_open_bracket_for_live_typing() {
        let segments = split_segments(r#"Defs/ThingDef[defName="Wa"#).unwrap();
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn split_segments_yields_empty_segment_for_leading_and_trailing_slash() {
        let segments = split_segments("/Defs/").unwrap();
        assert!(segments[0].start == segments[0].end);
        assert!(segments.last().unwrap().start == segments.last().unwrap().end);
    }

    #[test]
    fn trimmed_strips_leading_and_trailing_whitespace_and_reports_absolute_offset() {
        let xpath = "Defs/  ThingDef  /statBases";
        let seg = Segment { start: 5, end: 17 };
        let (text, start) = seg.trimmed(xpath);
        assert_eq!(text, "ThingDef");
        assert_eq!(start, 7);
        assert_eq!(&xpath[start..start + text.len()], "ThingDef");
    }

    #[test]
    fn clamp_to_char_boundary_steps_back_from_mid_character_offset() {
        let xpath = "Defs/Thing\u{00e9}Def";
        let mid = xpath.find('\u{00e9}').unwrap() + 1;
        assert!(!xpath.is_char_boundary(mid));
        let clamped = clamp_to_char_boundary(xpath, mid);
        assert!(xpath.is_char_boundary(clamped));
        assert!(clamped <= mid);
    }

    #[test]
    fn clamp_to_char_boundary_clamps_out_of_range_offset_to_len() {
        assert_eq!(clamp_to_char_boundary("Defs", 999), 4);
    }

    #[test]
    fn identifier_token_end_stops_at_slash_bracket_and_whitespace() {
        assert_eq!(identifier_token_end("ThingDef/statBases", 0), 8);
        assert_eq!(identifier_token_end("ThingDef[defName", 0), 8);
        assert_eq!(identifier_token_end("ThingDef statBases", 0), 8);
        assert_eq!(identifier_token_end("ThingDef", 0), 8);
    }

    #[test]
    fn find_matching_close_tracks_nesting_and_ignores_quoted_brackets() {
        let text = r#"ThingDef[defName="A[B]C"]"#;
        let open = text.find('[').unwrap();
        assert_eq!(find_matching_close(text, open), Some(text.len() - 1));
    }

    #[test]
    fn find_matching_close_distinguishes_adjacent_predicates() {
        let text = "ThingDef[1][2]";
        let first_open = text.find('[').unwrap();
        let first_close = find_matching_close(text, first_open).unwrap();
        assert_eq!(&text[first_open..=first_close], "[1]");
    }

    #[test]
    fn find_matching_close_returns_none_when_unclosed() {
        let text = "ThingDef[defName=\"Wa";
        let open = text.find('[').unwrap();
        assert_eq!(find_matching_close(text, open), None);
    }
}
