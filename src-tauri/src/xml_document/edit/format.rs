use super::super::model::{XmlDocument, XmlNodeId, XmlNodeKind};

pub(crate) fn index_before_trailing_whitespace(doc: &XmlDocument, children: &[XmlNodeId]) -> usize {
    let mut end = children.len();
    while end > 0 {
        let id = children[end - 1];
        if let XmlNodeKind::Text(ref t) = doc.nodes[id].kind {
            if t.value.chars().all(char::is_whitespace) {
                end -= 1;
                continue;
            }
        }
        break;
    }
    end
}

pub(crate) fn find_insert_index(
    doc: &XmlDocument,
    parent_id: XmlNodeId,
    child_name: &str,
    field_order: &[String],
) -> usize {
    let target_order = field_order.iter().position(|n| n == child_name);
    let children = &doc.nodes[parent_id].children;

    if let Some(target_idx) = target_order {
        // Backward pass: insert after the last preceding known sibling.
        for (i, &child_id) in children.iter().enumerate().rev() {
            if let XmlNodeKind::Element(ref el) = doc.nodes[child_id].kind {
                if let Some(sibling_idx) = field_order.iter().position(|n| n == el.name.as_str()) {
                    if sibling_idx < target_idx {
                        return i + 1;
                    }
                }
            }
        }

        // Forward pass: insert before the first later known sibling (handles
        // fields that belong before all existing known siblings).
        for (i, &child_id) in children.iter().enumerate() {
            if let XmlNodeKind::Element(ref el) = doc.nodes[child_id].kind {
                if let Some(sibling_idx) = field_order.iter().position(|n| n == el.name.as_str()) {
                    if sibling_idx > target_idx {
                        // Step back over any preceding whitespace indent.
                        let insert_before_indent = i > 0
                            && matches!(
                                &doc.nodes[children[i - 1]].kind,
                                XmlNodeKind::Text(ref t) if t.value.chars().all(char::is_whitespace)
                            );
                        return if insert_before_indent { i - 1 } else { i };
                    }
                }
            }
        }
    }

    index_before_trailing_whitespace(doc, children)
}

/// Returns the appropriate indent string for a new child element under `parent_id`.
/// When the parent already has indented children, their indentation is reused.
/// For newly-created empty parents, the indentation is derived from the whitespace
/// preceding `parent_id` in its own parent's children, plus two extra spaces.
pub(crate) fn detect_child_indent(doc: &XmlDocument, parent_id: XmlNodeId) -> String {
    // First: reuse existing child indentation (matches current detect_indent behavior).
    for &child_id in doc.nodes[parent_id].children.iter() {
        if let XmlNodeKind::Text(ref t) = doc.nodes[child_id].kind {
            if let Some(last_line) = t.value.split('\n').next_back() {
                if !last_line.is_empty() && last_line.chars().all(char::is_whitespace) {
                    return format!("\n{}", last_line);
                }
            }
        }
    }

    // Second: infer from the whitespace text node preceding parent_id.
    if let Some(grandparent_id) = doc.nodes[parent_id].parent {
        let siblings = &doc.nodes[grandparent_id].children;
        if let Some(pos) = siblings.iter().position(|&id| id == parent_id) {
            if pos > 0 {
                let prev_id = siblings[pos - 1];
                if let XmlNodeKind::Text(ref t) = doc.nodes[prev_id].kind {
                    if let Some(last_line) = t.value.split('\n').next_back() {
                        if !last_line.is_empty() && last_line.chars().all(char::is_whitespace) {
                            return format!("\n{}  ", last_line);
                        }
                    }
                }
            }
        }
    }

    "\n    ".to_string()
}

/// Returns the closing-indent string that should appear immediately before
/// `</parent_tag>`. Derived from the whitespace text node preceding `parent_id`
/// in its own parent's children (i.e. the same indentation level as the opening tag).
pub(crate) fn detect_closing_indent(doc: &XmlDocument, parent_id: XmlNodeId) -> String {
    if let Some(grandparent_id) = doc.nodes[parent_id].parent {
        let siblings = &doc.nodes[grandparent_id].children;
        if let Some(pos) = siblings.iter().position(|&id| id == parent_id) {
            if pos > 0 {
                let prev_id = siblings[pos - 1];
                if let XmlNodeKind::Text(ref t) = doc.nodes[prev_id].kind {
                    if let Some(last_line) = t.value.split('\n').next_back() {
                        if !last_line.is_empty() && last_line.chars().all(char::is_whitespace) {
                            return format!("\n{}", last_line);
                        }
                    }
                }
            }
        }
    }
    "\n".to_string()
}
