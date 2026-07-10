use super::super::diagnostics::XmlSpan;
use super::super::model::{XmlDocument, XmlElement, XmlNode, XmlNodeId, XmlNodeKind, XmlText};
use super::error::XmlEditError;
use super::format::{detect_child_indent, detect_closing_indent, find_insert_index};

pub(crate) enum NewElementKind {
    ScalarText(String),
    Object,
}

pub(crate) fn ensure_element_node(
    document: &XmlDocument,
    node_id: XmlNodeId,
) -> Result<(), XmlEditError> {
    if node_id >= document.nodes.len() {
        return Err(XmlEditError::NodeNotFound(node_id));
    }
    if !matches!(document.nodes[node_id].kind, XmlNodeKind::Element(_)) {
        return Err(XmlEditError::NotAnElement(node_id));
    }
    Ok(())
}

pub(crate) fn find_child_element(
    document: &XmlDocument,
    parent_id: XmlNodeId,
    child_name: &str,
) -> Option<XmlNodeId> {
    document.nodes[parent_id]
        .children
        .iter()
        .copied()
        .find(|&c| {
            if let XmlNodeKind::Element(ref el) = document.nodes[c].kind {
                el.name == child_name
            } else {
                false
            }
        })
}

/// Creates a new child element under `parent_id` with correct indentation and
/// ordering. For `Object` elements the new element starts empty; children must
/// be added afterwards. For `ScalarText` elements the text content is set
/// immediately.
///
/// Also ensures:
/// - A closing-indent whitespace node is appended to `parent_id` if one is not
///   already present (needed when `parent_id` was previously empty).
/// - `parent_id`'s `self_closing` flag is cleared.
pub(crate) fn create_child_element(
    document: &mut XmlDocument,
    parent_id: XmlNodeId,
    child_name: String,
    child_kind: NewElementKind,
    field_order: &[String],
) -> XmlNodeId {
    let indent = detect_child_indent(document, parent_id);
    let insert_pos = find_insert_index(document, parent_id, &child_name, field_order);

    let elem_id = match child_kind {
        NewElementKind::ScalarText(value) => {
            let text_id = document.nodes.len();
            document.nodes.push(XmlNode {
                id: text_id,
                parent: None,
                kind: XmlNodeKind::Text(XmlText { value }),
                children: Vec::new(),
                span: XmlSpan::default(),
                dirty: true,
            });

            let elem_id = document.nodes.len();
            document.nodes[text_id].parent = Some(elem_id);
            document.nodes.push(XmlNode {
                id: elem_id,
                parent: Some(parent_id),
                kind: XmlNodeKind::Element(XmlElement {
                    name: child_name,
                    attributes: Vec::new(),
                    start_tag_span: XmlSpan::default(),
                    end_tag_span: Some(XmlSpan::default()),
                    self_closing: false,
                }),
                children: vec![text_id],
                span: XmlSpan::default(),
                dirty: true,
            });
            elem_id
        }
        NewElementKind::Object => {
            let elem_id = document.nodes.len();
            document.nodes.push(XmlNode {
                id: elem_id,
                parent: Some(parent_id),
                kind: XmlNodeKind::Element(XmlElement {
                    name: child_name,
                    attributes: Vec::new(),
                    start_tag_span: XmlSpan::default(),
                    end_tag_span: Some(XmlSpan::default()),
                    self_closing: false,
                }),
                children: Vec::new(),
                span: XmlSpan::default(),
                dirty: true,
            });
            elem_id
        }
    };

    let indent_id = document.nodes.len();
    document.nodes.push(XmlNode {
        id: indent_id,
        parent: Some(parent_id),
        kind: XmlNodeKind::Text(XmlText { value: indent }),
        children: Vec::new(),
        span: XmlSpan::default(),
        dirty: true,
    });

    {
        let children = &mut document.nodes[parent_id].children;
        children.insert(insert_pos, elem_id);
        children.insert(insert_pos, indent_id);
    }

    // Clear self_closing on the parent - it now has a child element.
    if let XmlNodeKind::Element(ref mut el) = document.nodes[parent_id].kind {
        el.self_closing = false;
    }

    // Add a closing-indent whitespace node to the parent if one is not already
    // present (the case when the parent was previously empty or self-closing).
    let parent_has_trailing_ws =
        document.nodes[parent_id]
            .children
            .last()
            .is_some_and(|&last_id| {
                matches!(
                    &document.nodes[last_id].kind,
                    XmlNodeKind::Text(ref t) if t.value.chars().all(char::is_whitespace)
                )
            });

    if !parent_has_trailing_ws {
        let closing_indent = detect_closing_indent(document, parent_id);
        let close_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: close_id,
            parent: Some(parent_id),
            kind: XmlNodeKind::Text(XmlText {
                value: closing_indent,
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });
        document.nodes[parent_id].children.push(close_id);
    }

    document.nodes[parent_id].dirty = true;
    elem_id
}
