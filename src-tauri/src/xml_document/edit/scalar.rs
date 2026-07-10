use super::super::diagnostics::XmlSpan;
use super::super::model::{XmlAttribute, XmlDocument, XmlNode, XmlNodeId, XmlNodeKind, XmlText};
use super::context::XmlEditContext;
use super::error::XmlEditError;
use super::paths::find_or_create_object_path;
use super::tree::{create_child_element, ensure_element_node, find_child_element, NewElementKind};

pub(crate) fn set_child_element_text(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    child_name: String,
    value: String,
    field_order: &[String],
) -> Result<(), XmlEditError> {
    ensure_element_node(document, parent_node_id)?;

    let existing_child = find_child_element(document, parent_node_id, &child_name);

    if let Some(child_elem_id) = existing_child {
        // Match both plain Text and CData - the edit always writes plain Text.
        let existing_text = document.nodes[child_elem_id]
            .children
            .iter()
            .copied()
            .find(|&c| {
                matches!(
                    document.nodes[c].kind,
                    XmlNodeKind::Text(_) | XmlNodeKind::CData(_)
                )
            });

        if let Some(text_id) = existing_text {
            // Replace CData nodes with plain Text so special characters
            // entered through the form are serialized as entities, not CDATA.
            document.nodes[text_id].kind = XmlNodeKind::Text(XmlText { value });
            document.nodes[text_id].dirty = true;
        } else {
            let text_id = document.nodes.len();
            document.nodes.push(XmlNode {
                id: text_id,
                parent: Some(child_elem_id),
                kind: XmlNodeKind::Text(XmlText { value }),
                children: Vec::new(),
                span: XmlSpan::default(),
                dirty: true,
            });
            document.nodes[child_elem_id].children.push(text_id);
            if let XmlNodeKind::Element(ref mut el) = document.nodes[child_elem_id].kind {
                el.self_closing = false;
            }
            document.nodes[child_elem_id].dirty = true;
        }
    } else {
        create_child_element(
            document,
            parent_node_id,
            child_name,
            NewElementKind::ScalarText(value),
            field_order,
        );
    }

    Ok(())
}

pub(crate) fn set_element_attribute(
    document: &mut XmlDocument,
    element_node_id: XmlNodeId,
    attribute_name: String,
    value: String,
) -> Result<(), XmlEditError> {
    if element_node_id >= document.nodes.len() {
        return Err(XmlEditError::NodeNotFound(element_node_id));
    }

    let node = &mut document.nodes[element_node_id];
    if let XmlNodeKind::Element(ref mut el) = node.kind {
        if let Some(attr) = el.attributes.iter_mut().find(|a| a.name == attribute_name) {
            attr.value = value;
        } else {
            el.attributes.push(XmlAttribute {
                name: attribute_name,
                value,
                span: XmlSpan::default(),
            });
        }
        node.dirty = true;
    } else {
        return Err(XmlEditError::NotAnElement(element_node_id));
    }

    Ok(())
}

pub(crate) fn remove_child_element(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    child_name: String,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, parent_node_id)?;

    let child_pos = document.nodes[parent_node_id]
        .children
        .iter()
        .position(|&c| {
            if let XmlNodeKind::Element(ref el) = document.nodes[c].kind {
                el.name == child_name
            } else {
                false
            }
        });

    if let Some(pos) = child_pos {
        let remove_preceding = pos > 0 && {
            let prev_id = document.nodes[parent_node_id].children[pos - 1];
            matches!(
                &document.nodes[prev_id].kind,
                XmlNodeKind::Text(ref t) if t.value.chars().all(char::is_whitespace)
            )
        };

        let children = &mut document.nodes[parent_node_id].children;
        children.remove(pos);
        if remove_preceding {
            children.remove(pos - 1);
        }
        document.nodes[parent_node_id].dirty = true;
    }
    // Not found is idempotent - no error.

    Ok(())
}

pub(crate) fn remove_element_attribute(
    document: &mut XmlDocument,
    element_node_id: XmlNodeId,
    attribute_name: String,
) -> Result<(), XmlEditError> {
    if element_node_id >= document.nodes.len() {
        return Err(XmlEditError::NodeNotFound(element_node_id));
    }
    let node = &mut document.nodes[element_node_id];
    if let XmlNodeKind::Element(ref mut el) = node.kind {
        let before = el.attributes.len();
        el.attributes.retain(|a| a.name != attribute_name);
        if el.attributes.len() < before {
            node.dirty = true;
        }
        // Not found is idempotent - no error.
    } else {
        return Err(XmlEditError::NotAnElement(element_node_id));
    }
    Ok(())
}

/// Remove a field element inside a nested object path, without creating the path if absent.
/// Idempotent when the path or field is missing.
/// When `prune_empty_ancestors` is true, walks up through `object_path` in reverse and removes
/// any ancestor element that has no meaningful content left after the field removal.
pub(crate) fn remove_nested_element(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    field_name: String,
    prune_empty_ancestors: bool,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, parent_node_id)?;

    // Resolve all object path node ids before mutating the tree so later steps can
    // find parents even after earlier steps remove children.
    let mut path_node_ids: Vec<XmlNodeId> = Vec::with_capacity(object_path.len());
    {
        let mut current_id = parent_node_id;
        for segment in object_path {
            match find_child_element(document, current_id, segment) {
                Some(id) => {
                    path_node_ids.push(id);
                    current_id = id;
                }
                None => return Ok(()), // Path absent - idempotent.
            }
        }
    }

    let object_node_id = path_node_ids.last().copied().unwrap_or(parent_node_id);

    // Only proceed (and prune) when the target field actually exists.
    // Clearing a missing field is idempotent; pruning must not fire in that case.
    if find_child_element(document, object_node_id, &field_name).is_none() {
        return Ok(());
    }
    remove_child_element(document, object_node_id, field_name)?;

    if !prune_empty_ancestors {
        return Ok(());
    }

    // Walk the resolved object path in reverse. For each ancestor that is now empty,
    // remove it from its parent. Stop when an ancestor still has meaningful content.
    let mut child_id_to_remove: Option<XmlNodeId> = None;
    for (depth, &ancestor_id) in path_node_ids.iter().enumerate().rev() {
        if let Some(child_id) = child_id_to_remove {
            // The previous (deeper) iteration decided this child should be pruned.
            // Remove it using the same whitespace-aware logic as remove_child_element.
            remove_child_node_by_id(document, ancestor_id, child_id);
            document.nodes[ancestor_id].dirty = true;
        }
        child_id_to_remove = None;

        if element_has_meaningful_content(document, ancestor_id) {
            break;
        }

        // This ancestor is now empty - mark it for removal from its parent.
        // Its parent is either the previous path entry or parent_node_id.
        let _ = depth; // depth tracked implicitly through path_node_ids index
        child_id_to_remove = Some(ancestor_id);
    }

    // Handle pruning the shallowest remaining candidate against parent_node_id.
    if let Some(child_id) = child_id_to_remove {
        remove_child_node_by_id(document, parent_node_id, child_id);
        document.nodes[parent_node_id].dirty = true;
    }

    Ok(())
}

/// Returns true if an element node has any content that should prevent ancestor pruning:
/// element children, non-whitespace text, CDATA, comments, processing instructions, or attributes.
fn element_has_meaningful_content(document: &XmlDocument, node_id: XmlNodeId) -> bool {
    if let XmlNodeKind::Element(ref el) = document.nodes[node_id].kind {
        if !el.attributes.is_empty() {
            return true;
        }
    }
    document.nodes[node_id].children.iter().any(|&child_id| {
        match &document.nodes[child_id].kind {
            XmlNodeKind::Text(ref t) => !t.value.chars().all(char::is_whitespace),
            _ => true, // Element, CData, Comment, PI - all meaningful
        }
    })
}

/// Removes a child node by id from its parent, also removing the preceding whitespace-indent
/// text node when present (same convention as `remove_child_element`).
fn remove_child_node_by_id(document: &mut XmlDocument, parent_id: XmlNodeId, child_id: XmlNodeId) {
    let pos = document.nodes[parent_id]
        .children
        .iter()
        .position(|&c| c == child_id);

    if let Some(pos) = pos {
        let remove_preceding = pos > 0 && {
            let prev_id = document.nodes[parent_id].children[pos - 1];
            matches!(
                &document.nodes[prev_id].kind,
                XmlNodeKind::Text(ref t) if t.value.chars().all(char::is_whitespace)
            )
        };

        let children = &mut document.nodes[parent_id].children;
        children.remove(pos);
        if remove_preceding {
            children.remove(pos - 1);
        }
    }
}

pub(crate) fn set_nested_element_attribute(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    attribute_name: String,
    value: String,
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    if object_path.is_empty() {
        return Err(XmlEditError::EmptyObjectPath);
    }
    ensure_element_node(document, parent_node_id)?;

    let object_node_id =
        find_or_create_object_path(document, parent_node_id, object_path, context)?;

    set_element_attribute(document, object_node_id, attribute_name, value)
}

pub(crate) fn set_nested_element_text(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    field_name: String,
    value: String,
    leaf_field_order: &[String],
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    if object_path.is_empty() {
        return Err(XmlEditError::EmptyObjectPath);
    }
    ensure_element_node(document, parent_node_id)?;

    let object_node_id =
        find_or_create_object_path(document, parent_node_id, object_path, context)?;

    set_child_element_text(
        document,
        object_node_id,
        field_name,
        value,
        leaf_field_order,
    )
}
