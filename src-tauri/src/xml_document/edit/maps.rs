use super::super::diagnostics::XmlSpan;
use super::super::model::{XmlDocument, XmlNode, XmlNodeId, XmlNodeKind, XmlText};
use super::context::XmlEditContext;
use super::error::XmlEditError;
use super::names::is_valid_xml_name;
use super::paths::find_or_create_object_path;
use super::tree::{create_child_element, ensure_element_node, find_child_element, NewElementKind};

#[allow(clippy::too_many_arguments)]
pub(crate) fn set_named_map_entry(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    map_name: String,
    key: String,
    value: String,
    map_field_order: &[String],
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    if !is_valid_xml_name(&key) {
        return Err(XmlEditError::InvalidElementName(key));
    }
    ensure_element_node(document, parent_node_id)?;

    // Resolve or create the object path (may be empty for top-level map fields).
    let container_id = if object_path.is_empty() {
        parent_node_id
    } else {
        find_or_create_object_path(document, parent_node_id, object_path, context)?
    };
    let map_elem_id = {
        let existing = find_child_element(document, container_id, &map_name);
        if let Some(id) = existing {
            id
        } else {
            create_child_element(
                document,
                container_id,
                map_name,
                NewElementKind::Object,
                map_field_order,
            )
        }
    };

    // Find or create the key element inside the map.
    let existing_key = find_child_element(document, map_elem_id, &key);
    if let Some(key_id) = existing_key {
        // Update existing entry value.
        let existing_text = document.nodes[key_id].children.iter().copied().find(|&c| {
            matches!(
                document.nodes[c].kind,
                XmlNodeKind::Text(_) | XmlNodeKind::CData(_)
            )
        });
        if let Some(text_id) = existing_text {
            document.nodes[text_id].kind = XmlNodeKind::Text(XmlText { value });
            document.nodes[text_id].dirty = true;
        } else {
            let text_id = document.nodes.len();
            document.nodes.push(XmlNode {
                id: text_id,
                parent: Some(key_id),
                kind: XmlNodeKind::Text(XmlText { value }),
                children: Vec::new(),
                span: XmlSpan::default(),
                dirty: true,
            });
            document.nodes[key_id].children.push(text_id);
            if let XmlNodeKind::Element(ref mut el) = document.nodes[key_id].kind {
                el.self_closing = false;
            }
            document.nodes[key_id].dirty = true;
        }
    } else {
        // Create new entry - insert before the map's closing indent.
        create_child_element(
            document,
            map_elem_id,
            key,
            NewElementKind::ScalarText(value),
            &[],
        );
    }
    document.nodes[map_elem_id].dirty = true;

    Ok(())
}

pub(crate) fn remove_named_map_entry(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    map_name: String,
    key: String,
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, parent_node_id)?;

    let container_id = if object_path.is_empty() {
        parent_node_id
    } else {
        find_or_create_object_path(document, parent_node_id, object_path, context)?
    };

    let map_elem_id = match find_child_element(document, container_id, &map_name) {
        Some(id) => id,
        None => return Ok(()), // map doesn't exist - idempotent
    };

    let key_pos = document.nodes[map_elem_id].children.iter().position(|&c| {
        if let XmlNodeKind::Element(ref el) = document.nodes[c].kind {
            el.name == key
        } else {
            false
        }
    });

    if let Some(pos) = key_pos {
        let remove_preceding = pos > 0 && {
            let prev_id = document.nodes[map_elem_id].children[pos - 1];
            matches!(
                &document.nodes[prev_id].kind,
                XmlNodeKind::Text(ref t) if t.value.chars().all(char::is_whitespace)
            )
        };
        let children = &mut document.nodes[map_elem_id].children;
        children.remove(pos);
        if remove_preceding {
            children.remove(pos - 1);
        }
        document.nodes[map_elem_id].dirty = true;
    }
    // Key not found is idempotent.

    Ok(())
}

/// Replace all element children of a keyed-value-list container with a fresh ordered list of
/// (key, value) pairs. Existing entries are removed and the provided list is written in order.
/// Used for `repeatable = true` fields that allow duplicate child element names.
pub(crate) fn replace_keyed_value_list_entries(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    map_name: String,
    entries: Vec<(String, String)>,
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, parent_node_id)?;

    let container_id = if object_path.is_empty() {
        parent_node_id
    } else {
        find_or_create_object_path(document, parent_node_id, object_path, context)?
    };

    let map_field_order = context.field_order_for_path(object_path);
    let map_elem_id = {
        let existing = find_child_element(document, container_id, &map_name);
        if let Some(id) = existing {
            id
        } else {
            create_child_element(
                document,
                container_id,
                map_name,
                NewElementKind::Object,
                map_field_order,
            )
        }
    };

    // Remove all existing children (element entries and their preceding indent nodes).
    document.nodes[map_elem_id].children.clear();

    // Reset self_closing based on whether entries are provided.
    if let XmlNodeKind::Element(ref mut el) = document.nodes[map_elem_id].kind {
        el.self_closing = entries.is_empty();
    }

    // Re-add the entries in order; create_child_element infers indentation automatically.
    for (key, value) in entries {
        create_child_element(
            document,
            map_elem_id,
            key,
            NewElementKind::ScalarText(value),
            &[],
        );
    }

    document.nodes[map_elem_id].dirty = true;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn rename_named_map_entry(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    map_name: String,
    old_key: String,
    new_key: String,
    _field_order: &[String],
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    if !is_valid_xml_name(&new_key) {
        return Err(XmlEditError::InvalidElementName(new_key));
    }
    ensure_element_node(document, parent_node_id)?;

    let container_id = if object_path.is_empty() {
        parent_node_id
    } else {
        find_or_create_object_path(document, parent_node_id, object_path, context)?
    };

    let map_elem_id = match find_child_element(document, container_id, &map_name) {
        Some(id) => id,
        None => return Ok(()), // map doesn't exist - idempotent
    };

    // Check that new_key doesn't already exist.
    if new_key != old_key && find_child_element(document, map_elem_id, &new_key).is_some() {
        return Err(XmlEditError::DuplicateMapKey(new_key));
    }

    // Find the old key element and rename it.
    let old_id = match find_child_element(document, map_elem_id, &old_key) {
        Some(id) => id,
        None => return Ok(()), // old key not found - idempotent
    };

    if let XmlNodeKind::Element(ref mut el) = document.nodes[old_id].kind {
        el.name = new_key;
    }
    document.nodes[old_id].dirty = true;
    document.nodes[map_elem_id].dirty = true;

    Ok(())
}
