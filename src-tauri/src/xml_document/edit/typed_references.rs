use super::super::diagnostics::XmlSpan;
use super::super::model::{XmlDocument, XmlElement, XmlNode, XmlNodeId, XmlNodeKind, XmlText};
use super::api::TypedReferenceListItem;
use super::context::XmlEditContext;
use super::error::XmlEditError;
use super::format::detect_child_indent;
use super::names::is_valid_xml_name;
use super::paths::find_or_create_object_path;
use super::tree::{create_child_element, ensure_element_node, find_child_element, NewElementKind};

pub(crate) fn set_typed_reference_list_items(
    document: &mut XmlDocument,
    parent_node_id: XmlNodeId,
    object_path: &[String],
    field_name: String,
    items: Vec<TypedReferenceListItem>,
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    ensure_element_node(document, parent_node_id)?;

    // Validate all def_type strings before mutating anything.
    for item in &items {
        if !is_valid_xml_name(&item.def_type) {
            return Err(XmlEditError::InvalidElementName(item.def_type.clone()));
        }
    }

    // Resolve the direct parent node (may be the def element or a nested object).
    let direct_parent_id = if object_path.is_empty() {
        parent_node_id
    } else {
        find_or_create_object_path(document, parent_node_id, object_path, context)?
    };

    // Field order for inserting the container under its parent.
    let container_field_order = context.field_order_for_path(object_path);

    // Find or create the container element.
    let container_id = {
        let existing = find_child_element(document, direct_parent_id, &field_name);
        if let Some(id) = existing {
            id
        } else {
            create_child_element(
                document,
                direct_parent_id,
                field_name,
                NewElementKind::Object,
                container_field_order,
            )
        }
    };

    // Clear self_closing on the container.
    if let XmlNodeKind::Element(ref mut el) = document.nodes[container_id].kind {
        el.self_closing = false;
    }

    let container_indent = detect_child_indent(document, direct_parent_id);
    let item_indent = format!("{}    ", container_indent);
    let close_indent = container_indent;

    // Replace all children of the container.
    document.nodes[container_id].children.clear();

    for item in &items {
        let text_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: text_id,
            parent: None,
            kind: XmlNodeKind::Text(XmlText {
                value: item.def_name.clone(),
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });

        let elem_id = document.nodes.len();
        document.nodes[text_id].parent = Some(elem_id);
        document.nodes.push(XmlNode {
            id: elem_id,
            parent: Some(container_id),
            kind: XmlNodeKind::Element(XmlElement {
                name: item.def_type.clone(),
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
            parent: Some(container_id),
            kind: XmlNodeKind::Text(XmlText {
                value: item_indent.clone(),
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });

        let container_children = &mut document.nodes[container_id].children;
        container_children.push(indent_id);
        container_children.push(elem_id);
    }

    // Closing indent before </field_name>.
    if !items.is_empty() {
        let close_id = document.nodes.len();
        document.nodes.push(XmlNode {
            id: close_id,
            parent: Some(container_id),
            kind: XmlNodeKind::Text(XmlText {
                value: close_indent,
            }),
            children: Vec::new(),
            span: XmlSpan::default(),
            dirty: true,
        });
        document.nodes[container_id].children.push(close_id);
    }

    document.nodes[container_id].dirty = true;

    Ok(())
}
