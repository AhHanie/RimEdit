use super::super::model::{XmlDocument, XmlNodeId};
use super::context::XmlEditContext;
use super::error::XmlEditError;
use super::tree::{create_child_element, find_child_element, NewElementKind};

/// Walks `object_path` under `parent_id`, finding or creating each element.
/// Field order for each segment is resolved through `context.field_order_for_path`
/// using the path of all preceding segments as the key.
pub(crate) fn find_or_create_object_path(
    document: &mut XmlDocument,
    parent_id: XmlNodeId,
    object_path: &[String],
    context: &XmlEditContext,
) -> Result<XmlNodeId, XmlEditError> {
    let mut current_id = parent_id;

    for (segment_idx, segment) in object_path.iter().enumerate() {
        let existing = find_child_element(document, current_id, segment);

        if let Some(child_id) = existing {
            current_id = child_id;
        } else {
            let field_order = context.field_order_for_path(&object_path[..segment_idx]);
            let new_id = create_child_element(
                document,
                current_id,
                segment.clone(),
                NewElementKind::Object,
                field_order,
            );
            current_id = new_id;
        }
    }

    Ok(current_id)
}
