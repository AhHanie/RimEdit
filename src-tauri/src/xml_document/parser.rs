use quick_xml::{events::Event, Reader};

use super::def_summary::extract_def_summaries;
use super::diagnostics::{
    build_newline_index, make_span, offset_to_line_col, ParseDiagnostic, XmlSpan,
};
use super::model::{
    XmlAttribute, XmlDocument, XmlDocumentLoadResult, XmlDocumentView, XmlElement, XmlNode,
    XmlNodeId, XmlNodeKind, XmlText,
};

fn collect_attributes(
    e: &quick_xml::events::BytesStart<'_>,
    placeholder_span: &XmlSpan,
    attr_errors: &mut Vec<String>,
) -> Vec<XmlAttribute> {
    let mut attrs = Vec::new();
    for result in e.attributes() {
        match result {
            Ok(attr) => {
                let name = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                match attr.unescape_value() {
                    Ok(v) => {
                        attrs.push(XmlAttribute {
                            name,
                            value: v.into_owned(),
                            span: placeholder_span.clone(),
                        });
                    }
                    Err(err) => {
                        attr_errors
                            .push(format!("Invalid value for attribute '{}': {}", name, err));
                    }
                }
            }
            Err(err) => {
                attr_errors.push(format!("Malformed attribute: {}", err));
            }
        }
    }
    attrs
}

fn push_leaf(
    nodes: &mut Vec<XmlNode>,
    top_level_nodes: &mut Vec<XmlNodeId>,
    open_stack: &[XmlNodeId],
    kind: XmlNodeKind,
    span: XmlSpan,
) {
    let node_id = nodes.len();
    let parent = open_stack.last().copied();
    nodes.push(XmlNode {
        id: node_id,
        parent,
        kind,
        children: Vec::new(),
        span,
        dirty: false,
    });
    if let Some(p) = parent {
        nodes[p].children.push(node_id);
    } else {
        top_level_nodes.push(node_id);
    }
}

fn make_error_result(
    relative_path: &str,
    source: &str,
    diagnostics: Vec<ParseDiagnostic>,
) -> XmlDocumentLoadResult {
    XmlDocumentLoadResult {
        project_id: String::new(),
        relative_path: relative_path.to_string(),
        raw_xml: source.to_string(),
        document: None,
        parse_diagnostics: diagnostics,
        validation_diagnostics: Vec::new(),
    }
}

pub fn parse_to_document(relative_path: &str, source: &str) -> XmlDocument {
    let newline_index = build_newline_index(source.as_bytes());
    let mut reader = Reader::from_str(source);

    // `Reader::from_str` silently consumes a leading UTF-8 BOM without counting
    // its bytes in `buffer_position()` - every position it reports is relative
    // to the content *after* the BOM, while `source` (and every span we store)
    // still includes it. Left uncorrected, every span in a BOM-prefixed document
    // is short by the BOM's byte length, which breaks any exact-slice use of
    // `doc.source[span.start..span.end]` (e.g. `extract_indexed_def_xml`).
    const UTF8_BOM: &str = "\u{feff}";
    let bom_offset = if source.starts_with(UTF8_BOM) {
        UTF8_BOM.len()
    } else {
        0
    };

    let mut nodes: Vec<XmlNode> = Vec::new();
    let mut top_level_nodes: Vec<XmlNodeId> = Vec::new();
    let mut open_stack: Vec<XmlNodeId> = Vec::new();
    let mut diagnostics: Vec<ParseDiagnostic> = Vec::new();
    let mut buf = Vec::new();
    let mut had_fatal_error = false;

    loop {
        let event_start = reader.buffer_position() as usize + bom_offset;
        buf.clear();
        let event = match reader.read_event_into(&mut buf) {
            Ok(e) => e,
            Err(e) => {
                let offset = reader.buffer_position() as usize + bom_offset;
                let (line, col) = offset_to_line_col(&newline_index, offset);
                diagnostics.push(ParseDiagnostic::new(
                    relative_path,
                    Some(line),
                    Some(col),
                    Some(offset),
                    "parse_xml_syntax_error",
                    e.to_string(),
                ));
                had_fatal_error = true;
                break;
            }
        };
        let event_end = reader.buffer_position() as usize + bom_offset;
        let span = make_span(&newline_index, event_start, event_end);

        match event {
            Event::Start(ref e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attr_errors = Vec::new();
                let attributes = collect_attributes(e, &span, &mut attr_errors);
                for msg in attr_errors {
                    let (line, col) = offset_to_line_col(&newline_index, event_start);
                    diagnostics.push(ParseDiagnostic::new(
                        relative_path,
                        Some(line),
                        Some(col),
                        Some(event_start),
                        "parse_invalid_attribute",
                        msg,
                    ));
                }
                let node_id = nodes.len();
                let parent = open_stack.last().copied();
                nodes.push(XmlNode {
                    id: node_id,
                    parent,
                    kind: XmlNodeKind::Element(XmlElement {
                        name,
                        attributes,
                        start_tag_span: span.clone(),
                        end_tag_span: None,
                        self_closing: false,
                    }),
                    children: Vec::new(),
                    span: span.clone(),
                    dirty: false,
                });
                if let Some(p) = parent {
                    nodes[p].children.push(node_id);
                } else {
                    top_level_nodes.push(node_id);
                }
                open_stack.push(node_id);
            }
            Event::End(_) => {
                if let Some(elem_id) = open_stack.pop() {
                    nodes[elem_id].span.end = event_end;
                    if let XmlNodeKind::Element(ref mut el) = nodes[elem_id].kind {
                        el.end_tag_span = Some(make_span(&newline_index, event_start, event_end));
                    }
                }
            }
            Event::Empty(ref e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attr_errors = Vec::new();
                let attributes = collect_attributes(e, &span, &mut attr_errors);
                for msg in attr_errors {
                    let (line, col) = offset_to_line_col(&newline_index, event_start);
                    diagnostics.push(ParseDiagnostic::new(
                        relative_path,
                        Some(line),
                        Some(col),
                        Some(event_start),
                        "parse_invalid_attribute",
                        msg,
                    ));
                }
                let node_id = nodes.len();
                let parent = open_stack.last().copied();
                nodes.push(XmlNode {
                    id: node_id,
                    parent,
                    kind: XmlNodeKind::Element(XmlElement {
                        name,
                        attributes,
                        start_tag_span: span.clone(),
                        end_tag_span: None,
                        self_closing: true,
                    }),
                    children: Vec::new(),
                    span: span.clone(),
                    dirty: false,
                });
                if let Some(p) = parent {
                    nodes[p].children.push(node_id);
                } else {
                    top_level_nodes.push(node_id);
                }
            }
            Event::Text(ref e) => {
                let value = match e.unescape() {
                    Ok(v) => v.into_owned(),
                    Err(err) => {
                        let (line, col) = offset_to_line_col(&newline_index, event_start);
                        diagnostics.push(ParseDiagnostic::new(
                            relative_path,
                            Some(line),
                            Some(col),
                            Some(event_start),
                            "parse_invalid_text_entity",
                            format!("Invalid XML entity in text: {}", err),
                        ));
                        String::from_utf8_lossy(e.as_ref()).into_owned()
                    }
                };
                push_leaf(
                    &mut nodes,
                    &mut top_level_nodes,
                    &open_stack,
                    XmlNodeKind::Text(XmlText { value }),
                    span,
                );
            }
            Event::Comment(ref e) => {
                let value = String::from_utf8_lossy(e.as_ref()).to_string();
                push_leaf(
                    &mut nodes,
                    &mut top_level_nodes,
                    &open_stack,
                    XmlNodeKind::Comment(XmlText { value }),
                    span,
                );
            }
            Event::CData(ref e) => {
                let value = String::from_utf8_lossy(e.as_ref()).to_string();
                push_leaf(
                    &mut nodes,
                    &mut top_level_nodes,
                    &open_stack,
                    XmlNodeKind::CData(XmlText { value }),
                    span,
                );
            }
            Event::PI(_) => {
                push_leaf(
                    &mut nodes,
                    &mut top_level_nodes,
                    &open_stack,
                    XmlNodeKind::ProcessingInstruction(XmlText {
                        value: String::new(),
                    }),
                    span,
                );
            }
            Event::Decl(_) => {
                push_leaf(
                    &mut nodes,
                    &mut top_level_nodes,
                    &open_stack,
                    XmlNodeKind::ProcessingInstruction(XmlText {
                        value: String::new(),
                    }),
                    span,
                );
            }
            Event::DocType(ref e) => {
                let value = String::from_utf8_lossy(e.as_ref()).to_string();
                push_leaf(
                    &mut nodes,
                    &mut top_level_nodes,
                    &open_stack,
                    XmlNodeKind::DocType(XmlText { value }),
                    span,
                );
            }
            Event::Eof => {
                if !open_stack.is_empty() {
                    let offset = reader.buffer_position() as usize + bom_offset;
                    let (line, col) = offset_to_line_col(&newline_index, offset);
                    diagnostics.push(
                        ParseDiagnostic::new(
                            relative_path,
                            Some(line),
                            Some(col),
                            Some(offset),
                            "parse_unexpected_eof",
                            format!(
                                "Unexpected end of file: {} unclosed element(s)",
                                open_stack.len()
                            ),
                        )
                        .with_args(crate::diagnostics::diagnostic_args([
                            ("unclosedCount", open_stack.len().into()),
                        ])),
                    );
                    had_fatal_error = true;
                }
                break;
            }
        }
    }

    let mut doc = XmlDocument {
        source: source.to_string(),
        relative_path: relative_path.to_string(),
        nodes,
        top_level_nodes,
        def_summaries: Vec::new(),
        profile: super::model::XmlDocumentProfile::GenericXml,
        parse_diagnostics: diagnostics,
        validation_diagnostics: Vec::new(),
        had_fatal_parse_error: had_fatal_error,
    };

    if !had_fatal_error {
        doc.profile = super::model::detect_profile(&doc);
        doc.def_summaries = extract_def_summaries(&doc);
    }

    doc
}

#[cfg(test)]
mod bom_tests {
    use super::*;

    /// `quick_xml::Reader::from_str` consumes a leading UTF-8 BOM without
    /// counting its bytes toward `buffer_position()` - proven by comparing a
    /// BOM-prefixed source against the same source without one. Left
    /// uncorrected, every node span in a BOM-prefixed document lands 3 bytes
    /// short of where it needs to be in `source`, corrupting any exact
    /// `doc.source[span.start..span.end]` slice (e.g. indexed-def cloning).
    #[test]
    fn element_span_is_correct_in_a_bom_prefixed_document() {
        let without_bom =
            "<Defs>\n  <ThingDef>\n    <defName>Foo</defName>\n  </ThingDef>\n</Defs>";
        let with_bom = format!("\u{feff}{without_bom}");

        let doc = parse_to_document("Defs/Foo.xml", &with_bom);
        assert!(!doc.had_fatal_parse_error);
        let def_id = doc.def_summaries[0].node_id;
        let span = &doc.nodes[def_id].span;

        assert_eq!(
            &with_bom[span.start..span.end],
            "<ThingDef>\n    <defName>Foo</defName>\n  </ThingDef>"
        );
    }

    #[test]
    fn element_span_is_unaffected_without_a_bom() {
        let raw = "<Defs>\n  <ThingDef>\n    <defName>Foo</defName>\n  </ThingDef>\n</Defs>";
        let doc = parse_to_document("Defs/Foo.xml", raw);
        let def_id = doc.def_summaries[0].node_id;
        let span = &doc.nodes[def_id].span;

        assert_eq!(
            &raw[span.start..span.end],
            "<ThingDef>\n    <defName>Foo</defName>\n  </ThingDef>"
        );
    }
}

pub fn parse_xml_document(relative_path: &str, source: &str) -> XmlDocumentLoadResult {
    let doc = parse_to_document(relative_path, source);

    if doc.had_fatal_parse_error {
        return make_error_result(relative_path, source, doc.parse_diagnostics);
    }

    let element_count = doc
        .top_level_nodes
        .iter()
        .filter(|&&id| matches!(doc.nodes[id].kind, XmlNodeKind::Element(_)))
        .count();

    if element_count != 1 {
        let msg = if element_count == 0 {
            "document has no root element".to_string()
        } else {
            format!(
                "document has {} root elements; exactly one is required",
                element_count
            )
        };
        let parsed_relative_path = doc.relative_path.clone();
        let mut diags = doc.parse_diagnostics;
        diags.push(
            ParseDiagnostic::new(
                parsed_relative_path,
                None,
                None,
                None,
                "parse_invalid_root_element_count",
                msg,
            )
            .with_args(crate::diagnostics::diagnostic_args([(
                "elementCount",
                element_count.into(),
            )])),
        );
        return make_error_result(relative_path, source, diags);
    }

    let root_element = doc.top_level_nodes.iter().find_map(|&id| {
        if let XmlNodeKind::Element(ref el) = doc.nodes[id].kind {
            Some(el.name.clone())
        } else {
            None
        }
    });

    let view = XmlDocumentView {
        node_count: doc.nodes.len(),
        root_element,
        profile: doc.profile,
        defs: doc.def_summaries.clone(),
    };

    XmlDocumentLoadResult {
        project_id: String::new(),
        relative_path: doc.relative_path.clone(),
        raw_xml: source.to_string(),
        document: Some(view),
        parse_diagnostics: doc.parse_diagnostics,
        validation_diagnostics: doc.validation_diagnostics,
    }
}
