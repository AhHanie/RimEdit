use super::super::diagnostics::XmlSpan;
use super::super::model::{XmlDocument, XmlElement, XmlNode, XmlNodeId, XmlNodeKind, XmlText};
use super::context::XmlEditContext;
use super::error::XmlEditError;
use super::format::detect_child_indent;
use super::paths::find_or_create_object_path;
use super::tree::{create_child_element, ensure_element_node, find_child_element, NewElementKind};

/// Replace all `<li>` children of the `container_name` element under `parent_id`.
/// Creates the container element if it does not exist.
pub(crate) fn set_list_items_under_parent(
    document: &mut XmlDocument,
    parent_id: XmlNodeId,
    container_name: String,
    items: Vec<String>,
    field_order: &[String],
) -> Result<(), XmlEditError> {
    let list_elem_id = {
        let existing = find_child_element(document, parent_id, &container_name);
        if let Some(id) = existing {
            id
        } else {
            create_child_element(
                document,
                parent_id,
                container_name,
                NewElementKind::Object,
                field_order,
            )
        }
    };

    // An existing element may have been parsed as self-closing (e.g. `<tradeTags Inherit="False" />`).
    // Clear that flag so the serializer emits open/close tags and includes the new <li> children.
    if let XmlNodeKind::Element(ref mut el) = document.nodes[list_elem_id].kind {
        el.self_closing = false;
    }

    // One extra indent level for <li> items inside the container.
    let container_indent = detect_child_indent(document, parent_id);
    let li_indent = format!("{}    ", container_indent);
    let close_indent = container_indent;

    // Replace all children of the list container.
    document.nodes[list_elem_id].children.clear();

    for item_value in &items {
        let text_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: text_id,
            parent: None,
            kind: XmlNodeKind::Text(XmlText {
                value: item_value.clone(),
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });

        let li_id = document.nodes.len();
        document.nodes[text_id].parent = Some(li_id);
        document.nodes.push(XmlNode {
            id: li_id,
            parent: Some(list_elem_id),
            kind: XmlNodeKind::Element(XmlElement {
                name: "li".to_string(),
                attributes: Vec::new(),
                start_tag_span: XmlSpan::default(),
                end_tag_span: Some(XmlSpan::default()),
                self_closing: false,
            }),
            children: vec![text_id],
            span: XmlSpan::default(),
            dirty: true,
        });

        let indent_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: indent_id,
            parent: Some(list_elem_id),
            kind: XmlNodeKind::Text(XmlText {
                value: li_indent.clone(),
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });

        let list_children = &mut document.nodes[list_elem_id].children;
        list_children.push(indent_id);
        list_children.push(li_id);
    }

    // Closing indent before </container_name>.
    if !items.is_empty() {
        let close_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: close_id,
            parent: Some(list_elem_id),
            kind: XmlNodeKind::Text(XmlText {
                value: close_indent,
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });
        document.nodes[list_elem_id].children.push(close_id);
    }

    document.nodes[list_elem_id].dirty = true;
    Ok(())
}

pub(crate) fn set_list_items(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    child_name: String,
    items: Vec<String>,
    field_order: &[String],
) -> Result<(), XmlEditError> {
    ensure_element_node(document, parent_node_id)?;
    set_list_items_under_parent(document, parent_node_id, child_name, items, field_order)
}

pub(crate) fn set_nested_list_items(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    field_name: String,
    items: Vec<String>,
    leaf_field_order: &[String],
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    if object_path.is_empty() {
        return Err(XmlEditError::EmptyObjectPath);
    }
    ensure_element_node(document, parent_node_id)?;

    let object_node_id =
        find_or_create_object_path(document, parent_node_id, object_path, context)?;

    set_list_items_under_parent(
        document,
        object_node_id,
        field_name,
        items,
        leaf_field_order,
    )
}
