use super::model::{XmlDocument, XmlNodeId, XmlNodeKind};

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn is_subtree_clean(doc: &XmlDocument, node_id: XmlNodeId) -> bool {
    let node = &doc.nodes[node_id];
    if node.dirty {
        return false;
    }
    node.children.iter().all(|&c| is_subtree_clean(doc, c))
}

fn serialize_node(doc: &XmlDocument, node_id: XmlNodeId, out: &mut String) {
    let node = &doc.nodes[node_id];

    if is_subtree_clean(doc, node_id) {
        out.push_str(&doc.source[node.span.start..node.span.end]);
        return;
    }

    match &node.kind {
        XmlNodeKind::Element(el) => {
            if el.self_closing {
                out.push('<');
                out.push_str(&el.name);
                for attr in &el.attributes {
                    out.push(' ');
                    out.push_str(&attr.name);
                    out.push_str("=\"");
                    out.push_str(&escape_attr(&attr.value));
                    out.push('"');
                }
                out.push_str("/>");
                return;
            }

            if node.dirty {
                out.push('<');
                out.push_str(&el.name);
                for attr in &el.attributes {
                    out.push(' ');
                    out.push_str(&attr.name);
                    out.push_str("=\"");
                    out.push_str(&escape_attr(&attr.value));
                    out.push('"');
                }
                out.push('>');
            } else {
                out.push_str(&doc.source[el.start_tag_span.start..el.start_tag_span.end]);
            }

            for &child_id in &node.children {
                serialize_node(doc, child_id, out);
            }

            if node.dirty {
                out.push_str("</");
                out.push_str(&el.name);
                out.push('>');
            } else if let Some(ref end_span) = el.end_tag_span {
                out.push_str(&doc.source[end_span.start..end_span.end]);
            }
        }
        XmlNodeKind::Text(t) => {
            if node.dirty {
                out.push_str(&escape_text(&t.value));
            } else {
                out.push_str(&doc.source[node.span.start..node.span.end]);
            }
        }
        XmlNodeKind::Comment(t) => {
            if node.dirty {
                out.push_str("<!--");
                out.push_str(&t.value);
                out.push_str("-->");
            } else {
                out.push_str(&doc.source[node.span.start..node.span.end]);
            }
        }
        XmlNodeKind::CData(t) => {
            if node.dirty {
                out.push_str("<![CDATA[");
                out.push_str(&t.value);
                out.push_str("]]>");
            } else {
                out.push_str(&doc.source[node.span.start..node.span.end]);
            }
        }
        XmlNodeKind::ProcessingInstruction(t) => {
            if node.dirty && !t.value.is_empty() {
                out.push_str("<?");
                out.push_str(&t.value);
                out.push_str("?>");
            } else {
                out.push_str(&doc.source[node.span.start..node.span.end]);
            }
        }
        XmlNodeKind::DocType(t) => {
            if node.dirty {
                out.push_str("<!DOCTYPE ");
                out.push_str(&t.value);
                out.push('>');
            } else {
                out.push_str(&doc.source[node.span.start..node.span.end]);
            }
        }
    }
}

pub fn serialize_xml_document(document: &XmlDocument) -> String {
    if document.nodes.iter().all(|n| !n.dirty) {
        return document.source.clone();
    }

    let mut out = String::with_capacity(document.source.len() + 64);
    for &node_id in &document.top_level_nodes {
        serialize_node(document, node_id, &mut out);
    }
    out
}
