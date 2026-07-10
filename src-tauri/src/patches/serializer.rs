use super::model::{
    AttributeOperation, AttributeValueOperation, PatchFile, PatchOperationKind, PatchOperationNode,
    PatchSuccessMode, PathedOperation, PathedValueOperation, PathedValueOrderOperation,
    SetNameOperation,
};

pub(super) const INDENT_UNIT: &str = "  ";

/// Shared with `patches::custom_metadata` so metadata-driven field serialization matches the
/// built-in operation serializer's escaping and indentation conventions exactly.
pub(super) fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Shared with `patches::custom_metadata` so metadata fields declared with `xml: "attribute"`
/// escape the same way built-in attributes (e.g. `MayRequire`) do.
pub(super) fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

pub(super) fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str(INDENT_UNIT);
    }
}

/// Dedents a captured XML fragment (stripping common leading whitespace and surrounding blank
/// lines, mirroring the frontend's `dedentXmlFragment` in `xmlDedent.ts`) and re-indents every
/// remaining line to `level`. `value_xml`/custom-field XML payloads are captured verbatim from
/// their original source span (see `patches::parser::element_inner_xml`) or typed freehand by the
/// user, so their original indentation almost never matches the depth they're re-embedded at after
/// an edit -- this normalizes it so the whole file stays consistently formatted on every save.
pub(super) fn reindent_fragment(raw: &str, level: usize) -> String {
    let normalized = raw.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    let mut start = 0;
    let mut end = lines.len();
    while start < end && lines[start].trim().is_empty() {
        start += 1;
    }
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let core = &lines[start..end];
    if core.is_empty() {
        return String::new();
    }

    let min_indent = core
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);

    let prefix = INDENT_UNIT.repeat(level);
    let mut out = String::new();
    for (i, line) in core.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.trim().is_empty() {
            continue;
        }
        out.push_str(&prefix);
        out.push_str(&line[min_indent.min(line.len())..]);
    }
    out
}

fn write_text_field(out: &mut String, level: usize, tag: &str, text: &str) {
    indent(out, level);
    out.push('<');
    out.push_str(tag);
    out.push('>');
    out.push_str(&escape_text(text));
    out.push_str("</");
    out.push_str(tag);
    out.push_str(">\n");
}

fn write_optional_text_field(out: &mut String, level: usize, tag: &str, text: &Option<String>) {
    if let Some(t) = text {
        write_text_field(out, level, tag, t);
    }
}

fn write_value_field(out: &mut String, level: usize, value_xml: &Option<String>) {
    if let Some(v) = value_xml {
        indent(out, level);
        out.push_str("<value>");
        let body = reindent_fragment(v, level + 1);
        if body.is_empty() {
            out.push_str("</value>\n");
        } else {
            out.push('\n');
            out.push_str(&body);
            out.push('\n');
            indent(out, level);
            out.push_str("</value>\n");
        }
    }
}

fn write_pathed(out: &mut String, level: usize, op: &PathedOperation) {
    write_optional_text_field(out, level, "xpath", &op.xpath);
}

fn write_pathed_value(out: &mut String, level: usize, op: &PathedValueOperation) {
    write_optional_text_field(out, level, "xpath", &op.xpath);
    write_value_field(out, level, &op.value_xml);
}

fn write_pathed_value_order(out: &mut String, level: usize, op: &PathedValueOrderOperation) {
    write_optional_text_field(out, level, "xpath", &op.xpath);
    write_value_field(out, level, &op.value_xml);
    if let Some(order) = op.order {
        write_text_field(out, level, "order", order.as_xml_str());
    }
}

fn write_attribute_value(out: &mut String, level: usize, op: &AttributeValueOperation) {
    write_optional_text_field(out, level, "xpath", &op.xpath);
    write_optional_text_field(out, level, "attribute", &op.attribute);
    write_optional_text_field(out, level, "value", &op.value);
}

fn write_attribute(out: &mut String, level: usize, op: &AttributeOperation) {
    write_optional_text_field(out, level, "xpath", &op.xpath);
    write_optional_text_field(out, level, "attribute", &op.attribute);
}

fn write_set_name(out: &mut String, level: usize, op: &SetNameOperation) {
    write_optional_text_field(out, level, "xpath", &op.xpath);
    write_optional_text_field(out, level, "name", &op.name);
}

fn write_sequence(out: &mut String, level: usize, ops: &[PatchOperationNode]) {
    indent(out, level);
    out.push_str("<operations>\n");
    for op in ops {
        write_operation(out, op, level + 1, "li");
    }
    indent(out, level);
    out.push_str("</operations>\n");
}

fn write_mods(out: &mut String, level: usize, mods: &[String]) {
    indent(out, level);
    out.push_str("<mods>\n");
    for m in mods {
        write_text_field(out, level + 1, "li", m);
    }
    indent(out, level);
    out.push_str("</mods>\n");
}

fn write_match_nomatch(
    out: &mut String,
    level: usize,
    match_op: &Option<Box<PatchOperationNode>>,
    nomatch_op: &Option<Box<PatchOperationNode>>,
) {
    if let Some(m) = match_op {
        write_operation(out, m, level, "match");
    }
    if let Some(n) = nomatch_op {
        write_operation(out, n, level, "nomatch");
    }
}

fn write_operation(out: &mut String, node: &PatchOperationNode, level: usize, tag: &str) {
    if let PatchOperationKind::Unknown(unknown) = &node.kind {
        indent(out, level);
        out.push_str(unknown.raw_xml.trim_end());
        out.push('\n');
        return;
    }

    indent(out, level);
    out.push('<');
    out.push_str(tag);
    out.push_str(" Class=\"");
    out.push_str(&escape_attr(&node.class_name));
    out.push('"');
    for attr in &node.attributes {
        out.push(' ');
        out.push_str(&attr.name);
        out.push_str("=\"");
        out.push_str(&escape_attr(&attr.value));
        out.push('"');
    }
    out.push_str(">\n");

    if node.success != PatchSuccessMode::Normal {
        write_text_field(out, level + 1, "success", node.success.as_xml_str());
    }

    match &node.kind {
        PatchOperationKind::Add(op) | PatchOperationKind::Insert(op) => {
            write_pathed_value_order(out, level + 1, op)
        }
        PatchOperationKind::Remove(op) | PatchOperationKind::Test(op) => {
            write_pathed(out, level + 1, op)
        }
        PatchOperationKind::Replace(op) | PatchOperationKind::AddModExtension(op) => {
            write_pathed_value(out, level + 1, op)
        }
        PatchOperationKind::AttributeAdd(op) | PatchOperationKind::AttributeSet(op) => {
            write_attribute_value(out, level + 1, op)
        }
        PatchOperationKind::AttributeRemove(op) => write_attribute(out, level + 1, op),
        PatchOperationKind::SetName(op) => write_set_name(out, level + 1, op),
        PatchOperationKind::Sequence(ops) => write_sequence(out, level + 1, ops),
        PatchOperationKind::FindMod {
            mods,
            match_op,
            nomatch_op,
        } => {
            write_mods(out, level + 1, mods);
            write_match_nomatch(out, level + 1, match_op, nomatch_op);
        }
        PatchOperationKind::Conditional {
            xpath,
            match_op,
            nomatch_op,
        } => {
            write_optional_text_field(out, level + 1, "xpath", xpath);
            write_match_nomatch(out, level + 1, match_op, nomatch_op);
        }
        PatchOperationKind::Unknown(_) => unreachable!("handled via early return above"),
    }

    indent(out, level);
    out.push_str("</");
    out.push_str(tag);
    out.push_str(">\n");
}

pub fn serialize_patch_file(file: &PatchFile) -> String {
    let mut out = String::new();
    if let Some(decl) = &file.xml_declaration {
        out.push_str(decl);
        out.push('\n');
    }
    out.push_str("<Patch>\n");
    for op in &file.operations {
        write_operation(&mut out, op, 1, "Operation");
    }
    out.push_str("</Patch>\n");
    out
}

#[cfg(test)]
mod reindent_tests {
    use super::*;

    #[test]
    fn reindents_deeply_indented_source_to_target_level() {
        let raw = "\n            <MoveSpeed>1</MoveSpeed>\n        ";
        assert_eq!(reindent_fragment(raw, 3), "      <MoveSpeed>1</MoveSpeed>");
    }

    #[test]
    fn reindents_flush_left_content_and_preserves_relative_nesting() {
        let raw = "<li Class=\"SomeMod.ThingExtension\">\n  <value>1</value>\n</li>";
        assert_eq!(
            reindent_fragment(raw, 3),
            "      <li Class=\"SomeMod.ThingExtension\">\n        <value>1</value>\n      </li>"
        );
    }

    #[test]
    fn normalizes_tabs_and_crlf_line_endings() {
        let raw = "\r\n\t\t<foo>1</foo>\r\n\t\t<bar>2</bar>\r\n\t";
        assert_eq!(
            reindent_fragment(raw, 2),
            "    <foo>1</foo>\n    <bar>2</bar>"
        );
    }

    #[test]
    fn collapses_internal_blank_lines_to_empty() {
        let raw = "\n  <a/>\n\n  <b/>\n";
        assert_eq!(reindent_fragment(raw, 1), "  <a/>\n\n  <b/>");
    }

    #[test]
    fn empty_or_whitespace_only_fragment_reindents_to_empty_string() {
        assert_eq!(reindent_fragment("", 2), "");
        assert_eq!(reindent_fragment("   \n  \n ", 2), "");
    }

    #[test]
    fn write_value_field_reformats_messy_source_indentation() {
        let mut out = String::new();
        write_value_field(
            &mut out,
            2,
            &Some("\n          <MoveSpeed>1</MoveSpeed>\n      ".to_string()),
        );
        assert_eq!(
            out,
            "    <value>\n      <MoveSpeed>1</MoveSpeed>\n    </value>\n"
        );
    }

    #[test]
    fn write_value_field_writes_self_closed_tag_for_empty_value() {
        let mut out = String::new();
        write_value_field(&mut out, 1, &Some(String::new()));
        assert_eq!(out, "  <value></value>\n");
    }

    #[test]
    fn write_value_field_omits_output_for_absent_value() {
        let mut out = String::new();
        write_value_field(&mut out, 1, &None);
        assert_eq!(out, "");
    }
}
