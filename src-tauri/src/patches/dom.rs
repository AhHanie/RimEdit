//! Shared mutable-XML-tree helpers for the patch preview engine
//! (`patches::apply`, `patches::inheritance`, `services::patch_preview`).
//!
//! RimEdit's existing `xml_document` tree (see `xml_document::model`) is built around
//! incremental, span-preserving edits to a *single* already-open file and has no XPath evaluator,
//! no cross-node clone helper, and its node types are private to that module. The preview engine
//! instead needs to combine many files into one document, evaluate arbitrary XPath 1.0 (matching
//! RimWorld's own `System.Xml` `SelectNodes`/`SelectSingleNode`), and deep-clone subtrees for
//! inheritance resolution -- none of which fit that tree's design. Rather than retrofit it, this
//! module builds a throwaway `sxd_document::Package` (a mutable arena-based DOM) for each preview
//! computation and evaluates XPath against it via `sxd_xpath` (a real XPath 1.0 engine), then
//! throws the whole tree away once the final XML string has been extracted. Neither type ever
//! crosses the Tauri command boundary.
//!
//! XML is read into this tree with a small hand-rolled `quick-xml`-based walker (`parse_fragment`)
//! rather than `sxd_document`'s own parser, so parsing stays consistent with the diagnostics style
//! already used by `xml_document::parser` elsewhere in this crate.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use sxd_document::dom::{ChildOfElement, Document, Element};
use sxd_xpath::nodeset::Node as XPathNode;
use sxd_xpath::{Context, Factory, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentDiagnostic {
    pub message: String,
}

/// Result of parsing an XML fragment (zero or more sibling elements/text, not required to have a
/// single root) directly into `document`. `nodes` are freestanding -- not yet attached as a child
/// of anything -- so callers can append them wherever RimWorld would (as new children, or spliced
/// in as new siblings at a specific position).
pub struct FragmentParseResult<'d> {
    pub nodes: Vec<ChildOfElement<'d>>,
    pub diagnostics: Vec<FragmentDiagnostic>,
    pub had_fatal_error: bool,
}

fn apply_attributes<'d>(
    el: &Element<'d>,
    start: &BytesStart<'_>,
    diagnostics: &mut Vec<FragmentDiagnostic>,
) {
    for result in start.attributes() {
        match result {
            Ok(attr) => {
                let name = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                match attr.unescape_value() {
                    Ok(v) => {
                        el.set_attribute_value(name.as_str(), v.as_ref());
                    }
                    Err(err) => diagnostics.push(FragmentDiagnostic {
                        message: format!("Invalid value for attribute '{}': {}", name, err),
                    }),
                }
            }
            Err(err) => diagnostics.push(FragmentDiagnostic {
                message: format!("Malformed attribute: {}", err),
            }),
        }
    }
}

/// Parses `xml` into freestanding nodes owned by `document`. Tolerant of multiple top-level
/// siblings (elements and/or text) since patch `<value>` payloads and combined-file content are
/// not single-root fragments in general.
pub fn parse_fragment<'d>(document: Document<'d>, xml: &str) -> FragmentParseResult<'d> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut stack: Vec<Element<'d>> = Vec::new();
    let mut top_level: Vec<ChildOfElement<'d>> = Vec::new();
    let mut diagnostics: Vec<FragmentDiagnostic> = Vec::new();
    let mut had_fatal_error = false;
    let mut buf: Vec<u8> = Vec::new();

    loop {
        buf.clear();
        let event = match reader.read_event_into(&mut buf) {
            Ok(e) => e,
            Err(e) => {
                diagnostics.push(FragmentDiagnostic {
                    message: e.to_string(),
                });
                had_fatal_error = true;
                break;
            }
        };
        match event {
            Event::Eof => break,
            Event::Start(ref e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let el = document.create_element(name.as_str());
                apply_attributes(&el, e, &mut diagnostics);
                match stack.last() {
                    Some(parent) => parent.append_child(el),
                    None => top_level.push(ChildOfElement::Element(el)),
                }
                stack.push(el);
            }
            Event::Empty(ref e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let el = document.create_element(name.as_str());
                apply_attributes(&el, e, &mut diagnostics);
                match stack.last() {
                    Some(parent) => parent.append_child(el),
                    None => top_level.push(ChildOfElement::Element(el)),
                }
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Text(ref e) => {
                let text = match e.unescape() {
                    Ok(v) => v.into_owned(),
                    Err(err) => {
                        diagnostics.push(FragmentDiagnostic {
                            message: format!("Invalid XML entity in text: {}", err),
                        });
                        String::from_utf8_lossy(e.as_ref()).into_owned()
                    }
                };
                // RimWorld's `XmlDocument` parses with the .NET default `PreserveWhitespace =
                // false`, which does not materialize whitespace-only text between elements as
                // text nodes at all. Matching that here (rather than preserving pretty-printed
                // source indentation verbatim, as `xml_document`'s span-preserving tree does) is
                // load-bearing for `patches::inheritance`'s merge logic: a stray indentation-only
                // text child must not be mistaken for significant mixed text content that should
                // override an inherited element's children.
                if !text.trim().is_empty() {
                    let node = document.create_text(&text);
                    match stack.last() {
                        Some(parent) => parent.append_child(node),
                        None => top_level.push(ChildOfElement::Text(node)),
                    }
                }
            }
            Event::CData(e) => {
                let text = String::from_utf8_lossy(&e.into_inner()).into_owned();
                let node = document.create_text(&text);
                match stack.last() {
                    Some(parent) => parent.append_child(node),
                    None => top_level.push(ChildOfElement::Text(node)),
                }
            }
            Event::Comment(_) | Event::PI(_) | Event::Decl(_) | Event::DocType(_) => {}
        }
    }

    FragmentParseResult {
        nodes: top_level,
        diagnostics,
        had_fatal_error,
    }
}

/// Deep-clones `source` (and its whole subtree) into fresh nodes owned by `document`, which may
/// be the same document `source` already belongs to (RimWorld's own inheritance resolver clones a
/// resolved parent within the same document, see `XmlInheritance.ResolveXmlNodeFor`'s
/// `CloneNode(deep: true)`). Not attached to anything -- the caller decides where it goes.
pub fn clone_element<'d>(document: Document<'d>, source: Element<'d>) -> Element<'d> {
    let cloned = document.create_element(source.name());
    for attr in source.attributes() {
        cloned.set_attribute_value(attr.name(), attr.value());
    }
    let cloned_children: Vec<ChildOfElement<'d>> = source
        .children()
        .into_iter()
        .map(|child| clone_child_of_element(document, child))
        .collect();
    cloned.append_children(cloned_children);
    cloned
}

/// Deep-clones a single child node (of any kind) into fresh nodes owned by `document`.
pub fn clone_child_of_element<'d>(
    document: Document<'d>,
    child: ChildOfElement<'d>,
) -> ChildOfElement<'d> {
    match child {
        ChildOfElement::Element(el) => ChildOfElement::Element(clone_element(document, el)),
        ChildOfElement::Text(t) => ChildOfElement::Text(document.create_text(t.text())),
        ChildOfElement::Comment(c) => ChildOfElement::Comment(document.create_comment(c.text())),
        ChildOfElement::ProcessingInstruction(p) => ChildOfElement::ProcessingInstruction(
            document.create_processing_instruction(p.target(), p.value()),
        ),
    }
}

/// Evaluates `xpath_str` against `document`, matching RimWorld's own `XmlDocument.SelectNodes`
/// context (rooted at the whole document, not a specific node). Returns every matched node
/// (elements, attributes, text, etc. -- callers filter for the node kind their operation cares
/// about) in document order. `Err` covers both XPath parse failures and evaluation failures
/// (e.g. an unsupported axis/function); RimEdit surfaces both as the same
/// "unsupported/failed XPath" diagnostic since neither can be told apart meaningfully to a user.
pub fn select_nodes<'d>(
    document: Document<'d>,
    xpath_str: &str,
) -> Result<Vec<XPathNode<'d>>, String> {
    let factory = Factory::new();
    let compiled = factory.build(xpath_str).map_err(|e| e.to_string())?;
    let compiled = match compiled {
        Some(x) => x,
        None => return Ok(Vec::new()),
    };
    let context = Context::new();
    let value = compiled
        .evaluate(&context, document.root())
        .map_err(|e| e.to_string())?;
    match value {
        Value::Nodeset(nodes) => Ok(nodes.document_order()),
        _ => Ok(Vec::new()),
    }
}

/// [`select_nodes`], filtered to element matches only (used by every built-in operation except
/// none -- RimWorld patch xpaths only ever target elements in practice, but a user-authored xpath
/// could still resolve to attributes/text, which this silently drops rather than erroring, since
/// `PatchOperation.ApplyWorker` implementations all cast matches to `XmlNode` too).
pub fn select_elements<'d>(
    document: Document<'d>,
    xpath_str: &str,
) -> Result<Vec<Element<'d>>, String> {
    Ok(select_nodes(document, xpath_str)?
        .into_iter()
        .filter_map(|n| match n {
            XPathNode::Element(el) => Some(el),
            _ => None,
        })
        .collect())
}

/// Direct element children of `parent` named `name`, in document order.
pub fn child_elements_named<'d>(parent: Element<'d>, name: &str) -> Vec<Element<'d>> {
    parent
        .children()
        .into_iter()
        .filter_map(|c| match c {
            ChildOfElement::Element(el) if el.name().local_part() == name => Some(el),
            _ => None,
        })
        .collect()
}

/// First direct element child of `parent` named `name`.
pub fn first_child_element_named<'d>(parent: Element<'d>, name: &str) -> Option<Element<'d>> {
    child_elements_named(parent, name).into_iter().next()
}

/// Concatenated direct text content of `el` (mirrors `xml_document::def_summary`'s
/// `element_text_value`), trimmed. `None` if there is no text content.
pub fn element_text(el: Element<'_>) -> Option<String> {
    let mut buf = String::new();
    for child in el.children() {
        if let ChildOfElement::Text(t) = child {
            buf.push_str(t.text());
        }
    }
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Renders `el` (and its subtree) as indented, human-readable XML for preview display. Not
/// intended to byte-for-byte match any original source formatting (there may be none -- the
/// element may be a post-patch, post-inheritance synthetic clone with no source span at all).
pub fn serialize_element_pretty(el: Element<'_>) -> String {
    let mut out = String::new();
    write_element(el, 0, &mut out);
    out
}

fn write_element(el: Element<'_>, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    out.push_str(&indent);
    out.push('<');
    out.push_str(el.name().local_part());
    for attr in el.attributes() {
        out.push(' ');
        out.push_str(attr.name().local_part());
        out.push_str("=\"");
        out.push_str(&escape_attr(attr.value()));
        out.push('"');
    }

    let children = el.children();
    if children.is_empty() {
        out.push_str(" />\n");
        return;
    }

    if children.len() == 1 {
        if let ChildOfElement::Text(t) = children[0] {
            out.push('>');
            out.push_str(&escape_text(t.text()));
            out.push_str("</");
            out.push_str(el.name().local_part());
            out.push_str(">\n");
            return;
        }
    }

    out.push_str(">\n");
    for child in children {
        match child {
            ChildOfElement::Element(child_el) => write_element(child_el, depth + 1, out),
            ChildOfElement::Text(t) => {
                let text = t.text().trim();
                if !text.is_empty() {
                    out.push_str(&"  ".repeat(depth + 1));
                    out.push_str(&escape_text(text));
                    out.push('\n');
                }
            }
            ChildOfElement::Comment(c) => {
                out.push_str(&"  ".repeat(depth + 1));
                out.push_str("<!--");
                out.push_str(c.text());
                out.push_str("-->\n");
            }
            ChildOfElement::ProcessingInstruction(_) => {}
        }
    }
    out.push_str(&indent);
    out.push_str("</");
    out.push_str(el.name().local_part());
    out.push_str(">\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sxd_document::Package;

    #[test]
    fn parse_fragment_builds_multiple_top_level_siblings() {
        let package = Package::new();
        let doc = package.as_document();
        let result = parse_fragment(doc, "<li>1</li><li>2</li>");
        assert!(!result.had_fatal_error);
        assert_eq!(result.nodes.len(), 2);
    }

    #[test]
    fn clone_element_deep_copies_children_and_attributes() {
        let package = Package::new();
        let doc = package.as_document();
        let result = parse_fragment(doc, "<ThingDef Name=\"Base\"><a><b>1</b></a></ThingDef>");
        let ChildOfElement::Element(original) = result.nodes[0] else {
            panic!("expected element")
        };
        let cloned = clone_element(doc, original);
        assert_eq!(cloned.attribute_value("Name"), Some("Base"));
        assert_ne!(cloned, original);
        let a_children = child_elements_named(cloned, "a");
        assert_eq!(a_children.len(), 1);
        let b_children = child_elements_named(a_children[0], "b");
        assert_eq!(element_text(b_children[0]), Some("1".to_string()));
    }

    #[test]
    fn select_elements_evaluates_defname_predicate() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = doc.create_element("Defs");
        doc.root().append_child(defs);
        let result = parse_fragment(
            doc,
            "<ThingDef><defName>Wall</defName></ThingDef><ThingDef><defName>Door</defName></ThingDef>",
        );
        defs.append_children(result.nodes);

        let matched = select_elements(doc, "/Defs/ThingDef[defName=\"Wall\"]").unwrap();
        assert_eq!(matched.len(), 1);
        assert_eq!(
            element_text(first_child_element_named(matched[0], "defName").unwrap()),
            Some("Wall".to_string())
        );
    }

    #[test]
    fn serialize_element_pretty_renders_scalar_and_nested_fields() {
        let package = Package::new();
        let doc = package.as_document();
        let result = parse_fragment(
            doc,
            "<ThingDef><defName>Wall</defName><statBases><MoveSpeed>0</MoveSpeed></statBases></ThingDef>",
        );
        let ChildOfElement::Element(el) = result.nodes[0] else {
            panic!("expected element")
        };
        let xml = serialize_element_pretty(el);
        assert!(xml.contains("<defName>Wall</defName>"));
        assert!(xml.contains("<statBases>"));
        assert!(xml.contains("<MoveSpeed>0</MoveSpeed>"));
    }
}
