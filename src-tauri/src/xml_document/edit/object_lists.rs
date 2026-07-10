use super::super::diagnostics::XmlSpan;
use super::super::model::{
    XmlAttribute, XmlDocument, XmlElement, XmlNode, XmlNodeId, XmlNodeKind, XmlText,
};
use super::api::{InitialElement, NameValuePair};
use super::context::XmlEditContext;
use super::error::XmlEditError;
use super::format::{detect_child_indent, detect_closing_indent, index_before_trailing_whitespace};
use super::names::is_valid_xml_name;
use super::paths::find_or_create_object_path;
use super::scalar::{remove_child_element, set_child_element_text, set_element_attribute};
use super::tree::{create_child_element, ensure_element_node, find_child_element, NewElementKind};

pub(crate) fn set_object_list_item_child_text(
    document: &mut XmlDocument,
    list_item_node_id: XmlNodeId,
    child_name: String,
    value: String,
    field_order: Vec<String>,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, list_item_node_id)?;
    set_child_element_text(document, list_item_node_id, child_name, value, &field_order)
}

pub(crate) fn remove_object_list_item_child(
    document: &mut XmlDocument,
    list_item_node_id: XmlNodeId,
    child_name: String,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, list_item_node_id)?;
    remove_child_element(document, list_item_node_id, child_name)
}

/// Recursively creates XML elements described by `elem` as children of `parent_id`.
/// - `elem.value`    → scalar text element (only when children/li_items are empty)
/// - `elem.children` → named child sub-elements (object fields, map entries)
/// - `elem.li_items` → list item children (each becomes a direct child of this element)
/// - `elem.attributes` → attributes set on the created element after creation
fn apply_initial_element(
    document: &mut XmlDocument,
    parent_id: XmlNodeId,
    elem: &InitialElement,
) -> Result<(), XmlEditError> {
    let has_children = !elem.children.is_empty();
    let has_li_items = !elem.li_items.is_empty();
    let has_value = elem.value.as_ref().is_some_and(|v| !v.is_empty());

    // Skip entirely if no content and no attributes.
    if !has_children && !has_li_items && !has_value && elem.attributes.is_empty() {
        return Ok(());
    }

    let kind = if !has_children && !has_li_items && has_value {
        NewElementKind::ScalarText(elem.value.clone().unwrap_or_default())
    } else {
        NewElementKind::Object
    };

    let elem_id = create_child_element(document, parent_id, elem.name.clone(), kind, &[]);

    for attr in &elem.attributes {
        set_element_attribute(document, elem_id, attr.name.clone(), attr.value.clone())?;
    }

    for child in &elem.children {
        apply_initial_element(document, elem_id, child)?;
    }

    for li in &elem.li_items {
        apply_initial_element(document, elem_id, li)?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_object_list_item(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    list_name: String,
    class_attribute: Option<String>,
    after_item_node_id: Option<XmlNodeId>,
    initial_child_fields: Vec<NameValuePair>,
    field_order: Vec<String>,
    initial_children: Vec<InitialElement>,
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, parent_node_id)?;

    // Navigate through object_path to find the actual parent of the list container.
    let container_parent_id = if object_path.is_empty() {
        parent_node_id
    } else {
        find_or_create_object_path(document, parent_node_id, object_path, context)?
    };

    // Find or create the list container element.
    let list_elem_id = {
        let existing = find_child_element(document, container_parent_id, &list_name);
        if let Some(id) = existing {
            id
        } else {
            let list_field_order = context.field_order_for_path(object_path);
            create_child_element(
                document,
                container_parent_id,
                list_name,
                NewElementKind::Object,
                list_field_order,
            )
        }
    };

    // Determine indentation for the new <li>.
    let li_indent = detect_child_indent(document, list_elem_id);

    // Create the new <li> element (self-closing by default when no class,
    // or self-closing with Class attribute).
    let li_attrs = if let Some(ref class) = class_attribute {
        vec![XmlAttribute {
            name: "Class".to_string(),
            value: class.clone(),
            span: XmlSpan::default(),
        }]
    } else {
        vec![]
    };

    let li_elem_id = document.nodes.len();
    document.nodes.push(XmlNode {
        id: li_elem_id,
        parent: Some(list_elem_id),
        kind: XmlNodeKind::Element(XmlElement {
            name: "li".to_string(),
            attributes: li_attrs,
            start_tag_span: XmlSpan::default(),
            end_tag_span: None,
            self_closing: true,
        }),
        children: Vec::new(),
        span: XmlSpan::default(),
        dirty: true,
    });

    let indent_id = document.nodes.len();
    document.nodes.push(XmlNode {
        id: indent_id,
        parent: Some(list_elem_id),
        kind: XmlNodeKind::Text(XmlText { value: li_indent }),
        children: Vec::new(),
        span: XmlSpan::default(),
        dirty: true,
    });

    // Find insert position (after given item or before trailing whitespace).
    let insert_pos = if let Some(after_id) = after_item_node_id {
        let children = &document.nodes[list_elem_id].children;
        children
            .iter()
            .position(|&id| id == after_id)
            .map(|p| p + 1)
            .unwrap_or_else(|| index_before_trailing_whitespace(document, children))
    } else {
        index_before_trailing_whitespace(document, &document.nodes[list_elem_id].children)
    };

    {
        let children = &mut document.nodes[list_elem_id].children;
        children.insert(insert_pos, li_elem_id);
        children.insert(insert_pos, indent_id);
    }

    // Ensure the list container has a closing indent (needed when it was empty).
    let has_trailing_ws = document.nodes[list_elem_id]
        .children
        .last()
        .is_some_and(|&last_id| {
            matches!(
                &document.nodes[last_id].kind,
                XmlNodeKind::Text(ref t) if t.value.chars().all(char::is_whitespace)
            )
        });

    if !has_trailing_ws {
        let closing_indent = detect_closing_indent(document, list_elem_id);
        let close_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: close_id,
            parent: Some(list_elem_id),
            kind: XmlNodeKind::Text(XmlText {
                value: closing_indent,
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });
        document.nodes[list_elem_id].children.push(close_id);
    }

    // Clear self_closing on the list container.
    if let XmlNodeKind::Element(ref mut el) = document.nodes[list_elem_id].kind {
        el.self_closing = false;
    }

    document.nodes[list_elem_id].dirty = true;

    // Apply flat initial child fields (legacy path, scalar only).
    for pair in &initial_child_fields {
        set_child_element_text(
            document,
            li_elem_id,
            pair.name.clone(),
            pair.value.clone(),
            &field_order,
        )?;
    }

    // Apply recursive initial element tree (supersedes flat fields for structured content).
    for child in &initial_children {
        apply_initial_element(document, li_elem_id, child)?;
    }

    Ok(())
}

pub(crate) fn remove_object_list_item(
    document: &mut XmlDocument,
    list_item_node_id: XmlNodeId,
    prune_empty_ancestors: bool,
) -> Result<(), XmlEditError> {
    if list_item_node_id >= document.nodes.len() {
        return Err(XmlEditError::NodeNotFound(list_item_node_id));
    }

    // The <li>'s parent is the list container element.
    let list_container_id = match document.nodes[list_item_node_id].parent {
        Some(id) => id,
        None => return Ok(()), // orphan - idempotent
    };

    let pos = document.nodes[list_container_id]
        .children
        .iter()
        .position(|&id| id == list_item_node_id);

    if let Some(pos) = pos {
        let remove_preceding = pos > 0 && {
            let prev_id = document.nodes[list_container_id].children[pos - 1];
            matches!(
                &document.nodes[prev_id].kind,
                XmlNodeKind::Text(ref t) if t.value.chars().all(char::is_whitespace)
            )
        };
        let children = &mut document.nodes[list_container_id].children;
        children.remove(pos);
        if remove_preceding {
            children.remove(pos - 1);
        }
        document.nodes[list_container_id].dirty = true;
    }

    if !prune_empty_ancestors {
        return Ok(());
    }

    // If the list container now has no element children (only whitespace), walk up the parent
    // chain and remove each empty ancestor until we reach one that still has content.
    let mut current_id = list_container_id;
    loop {
        if list_element_has_content(document, current_id) {
            break;
        }
        let parent_id = match document.nodes[current_id].parent {
            Some(id) => id,
            None => break,
        };
        remove_node_from_parent(document, parent_id, current_id);
        current_id = parent_id;
    }

    Ok(())
}

/// Returns true when the element has any element children or non-whitespace text.
fn list_element_has_content(document: &XmlDocument, node_id: XmlNodeId) -> bool {
    document.nodes[node_id]
        .children
        .iter()
        .any(|&child_id| match &document.nodes[child_id].kind {
            XmlNodeKind::Text(ref t) => !t.value.chars().all(char::is_whitespace),
            _ => true,
        })
}

/// Insert a new named element (keyed object list item) into a container, creating the container
/// if absent. Unlike `insert_object_list_item`, the item element name is `key_name` (not `<li>`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_keyed_object_list_item(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    list_name: String,
    key_name: String,
    after_item_node_id: Option<XmlNodeId>,
    initial_children: Vec<InitialElement>,
    _field_order: Vec<String>,
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    if !is_valid_xml_name(&key_name) {
        return Err(XmlEditError::InvalidElementName(key_name));
    }
    ensure_element_node(document, parent_node_id)?;

    let container_parent_id = if object_path.is_empty() {
        parent_node_id
    } else {
        find_or_create_object_path(document, parent_node_id, object_path, context)?
    };

    let list_elem_id = {
        let existing = find_child_element(document, container_parent_id, &list_name);
        if let Some(id) = existing {
            id
        } else {
            let list_field_order = context.field_order_for_path(object_path);
            create_child_element(
                document,
                container_parent_id,
                list_name,
                NewElementKind::Object,
                list_field_order,
            )
        }
    };

    let item_indent = detect_child_indent(document, list_elem_id);

    let item_elem_id = document.nodes.len();
    document.nodes.push(XmlNode {
        id: item_elem_id,
        parent: Some(list_elem_id),
        kind: XmlNodeKind::Element(XmlElement {
            name: key_name,
            attributes: vec![],
            start_tag_span: XmlSpan::default(),
            end_tag_span: Some(XmlSpan::default()),
            self_closing: initial_children.is_empty(),
        }),
        children: Vec::new(),
        span: XmlSpan::default(),
        dirty: true,
    });

    let indent_id = document.nodes.len();
    document.nodes.push(XmlNode {
        id: indent_id,
        parent: Some(list_elem_id),
        kind: XmlNodeKind::Text(XmlText { value: item_indent }),
        children: Vec::new(),
        span: XmlSpan::default(),
        dirty: true,
    });

    let insert_pos = if let Some(after_id) = after_item_node_id {
        let children = &document.nodes[list_elem_id].children;
        children
            .iter()
            .position(|&id| id == after_id)
            .map(|p| p + 1)
            .unwrap_or_else(|| index_before_trailing_whitespace(document, children))
    } else {
        index_before_trailing_whitespace(document, &document.nodes[list_elem_id].children)
    };

    {
        let children = &mut document.nodes[list_elem_id].children;
        children.insert(insert_pos, item_elem_id);
        children.insert(insert_pos, indent_id);
    }

    // Ensure trailing whitespace in the container.
    let has_trailing_ws = document.nodes[list_elem_id]
        .children
        .last()
        .is_some_and(|&last_id| {
            matches!(
                &document.nodes[last_id].kind,
                XmlNodeKind::Text(ref t) if t.value.chars().all(char::is_whitespace)
            )
        });
    if !has_trailing_ws {
        let closing_indent = detect_closing_indent(document, list_elem_id);
        let close_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: close_id,
            parent: Some(list_elem_id),
            kind: XmlNodeKind::Text(XmlText {
                value: closing_indent,
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });
        document.nodes[list_elem_id].children.push(close_id);
    }

    if let XmlNodeKind::Element(ref mut el) = document.nodes[list_elem_id].kind {
        el.self_closing = false;
    }
    document.nodes[list_elem_id].dirty = true;

    for child in &initial_children {
        apply_initial_element(document, item_elem_id, child)?;
    }

    // Ensure the item element is not self-closing when it has children.
    if !initial_children.is_empty() {
        if let XmlNodeKind::Element(ref mut el) = document.nodes[item_elem_id].kind {
            el.self_closing = false;
        }
        // Add closing indent for the item element.
        let item_closing = detect_closing_indent(document, item_elem_id);
        let item_close_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: item_close_id,
            parent: Some(item_elem_id),
            kind: XmlNodeKind::Text(XmlText {
                value: item_closing,
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });
        document.nodes[item_elem_id].children.push(item_close_id);
        document.nodes[item_elem_id].dirty = true;
    }

    Ok(())
}

/// Set the scalar text content of a keyed object-list item element.
/// Used when the item was loaded via `defaultValueField` shorthand (e.g. `<Corpse>0.25</Corpse>`).
/// Finds and updates (or creates) the direct text node child of `item_node_id`.
pub(crate) fn set_keyed_object_list_item_text(
    document: &mut XmlDocument,
    item_node_id: XmlNodeId,
    value: String,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, item_node_id)?;

    let existing_text = document.nodes[item_node_id]
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
        document.nodes[text_id].kind = XmlNodeKind::Text(XmlText { value });
        document.nodes[text_id].dirty = true;
    } else {
        let text_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: text_id,
            parent: Some(item_node_id),
            kind: XmlNodeKind::Text(XmlText { value }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });
        document.nodes[item_node_id].children.push(text_id);
        if let XmlNodeKind::Element(ref mut el) = document.nodes[item_node_id].kind {
            el.self_closing = false;
        }
        document.nodes[item_node_id].dirty = true;
    }

    Ok(())
}

/// Rename a keyed object list item element in place.
/// `item_node_id` is the node id of the keyed element (not the container).
/// Returns `XmlEditError::InvalidElementName` if `new_name` is not a valid XML name.
pub(crate) fn rename_keyed_object_list_item(
    document: &mut XmlDocument,
    item_node_id: XmlNodeId,
    new_name: String,
) -> Result<(), XmlEditError> {
    if !is_valid_xml_name(&new_name) {
        return Err(XmlEditError::InvalidElementName(new_name));
    }
    ensure_element_node(document, item_node_id)?;
    if let XmlNodeKind::Element(ref mut el) = document.nodes[item_node_id].kind {
        el.name = new_name;
    }
    document.nodes[item_node_id].dirty = true;
    Ok(())
}

/// Removes `child_id` from `parent_id`'s children, also removing the preceding whitespace indent.
fn remove_node_from_parent(document: &mut XmlDocument, parent_id: XmlNodeId, child_id: XmlNodeId) {
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
        document.nodes[parent_id].dirty = true;
    }
}
