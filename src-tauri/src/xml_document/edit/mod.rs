mod api;
mod context;
mod error;
mod format;
mod lists;
mod maps;
mod names;
mod object_lists;
mod paths;
mod scalar;
mod tree;
mod typed_references;

// NameValuePair and TypedReferenceListItem are only referenced by name in test
// path expressions (not `use` imports), so the lint fires even though they ARE used.
#[allow(unused_imports)]
pub use api::{InitialElement, KeyValuePair, NameValuePair, TypedReferenceListItem, XmlEdit};
pub use context::XmlEditContext;
pub use error::XmlEditError;

use super::model::XmlDocument;

pub fn apply_xml_edit(
    document: &mut XmlDocument,
    edit: XmlEdit,
    context: &XmlEditContext,
) -> Result<(), XmlEditError> {
    match edit {
        XmlEdit::SetChildElementText {
            parent_node_id,
            child_name,
            value,
        } => scalar::set_child_element_text(
            document,
            parent_node_id,
            child_name,
            value,
            &context.field_order,
        ),

        XmlEdit::SetElementAttribute {
            element_node_id,
            attribute_name,
            value,
        } => scalar::set_element_attribute(document, element_node_id, attribute_name, value),

        XmlEdit::RemoveChildElement {
            parent_node_id,
            child_name,
        } => scalar::remove_child_element(document, parent_node_id, child_name),

        XmlEdit::SetListItems {
            parent_node_id,
            child_name,
            items,
        } => lists::set_list_items(
            document,
            parent_node_id,
            child_name,
            items,
            &context.field_order,
        ),

        XmlEdit::SetNestedObjectFieldText {
            parent_node_id,
            object_name,
            field_name,
            value,
        } => {
            let path = vec![object_name];
            let order = context.field_order_for_path(&path);
            scalar::set_nested_element_text(
                document,
                parent_node_id,
                &path,
                field_name,
                value,
                order,
                context,
            )
        }

        XmlEdit::SetNestedElementText {
            parent_node_id,
            object_path,
            field_name,
            value,
            field_order,
        } => {
            let order: &[String] = if !field_order.is_empty() {
                &field_order
            } else {
                context.field_order_for_path(&object_path)
            };
            scalar::set_nested_element_text(
                document,
                parent_node_id,
                &object_path,
                field_name,
                value,
                order,
                context,
            )
        }

        XmlEdit::SetNestedListItems {
            parent_node_id,
            object_path,
            field_name,
            items,
            field_order,
        } => {
            let order: &[String] = if !field_order.is_empty() {
                &field_order
            } else {
                context.field_order_for_path(&object_path)
            };
            lists::set_nested_list_items(
                document,
                parent_node_id,
                &object_path,
                field_name,
                items,
                order,
                context,
            )
        }

        XmlEdit::SetNamedMapEntry {
            parent_node_id,
            object_path,
            map_name,
            key,
            value,
            field_order,
        } => {
            let order: &[String] = if !field_order.is_empty() {
                &field_order
            } else {
                context.field_order_for_path(&object_path)
            };
            maps::set_named_map_entry(
                document,
                parent_node_id,
                &object_path,
                map_name,
                key,
                value,
                order,
                context,
            )
        }

        XmlEdit::RemoveNamedMapEntry {
            parent_node_id,
            object_path,
            map_name,
            key,
        } => maps::remove_named_map_entry(
            document,
            parent_node_id,
            &object_path,
            map_name,
            key,
            context,
        ),

        XmlEdit::RenameNamedMapEntry {
            parent_node_id,
            object_path,
            map_name,
            old_key,
            new_key,
            field_order,
        } => maps::rename_named_map_entry(
            document,
            parent_node_id,
            &object_path,
            map_name,
            old_key,
            new_key,
            &field_order,
            context,
        ),

        XmlEdit::SetObjectListItemAttribute {
            list_item_node_id,
            attribute_name,
            value,
        } => scalar::set_element_attribute(document, list_item_node_id, attribute_name, value),

        XmlEdit::SetObjectListItemChildText {
            list_item_node_id,
            child_name,
            value,
            field_order,
        } => object_lists::set_object_list_item_child_text(
            document,
            list_item_node_id,
            child_name,
            value,
            field_order,
        ),

        XmlEdit::RemoveObjectListItemChild {
            list_item_node_id,
            child_name,
        } => object_lists::remove_object_list_item_child(document, list_item_node_id, child_name),

        XmlEdit::InsertObjectListItem {
            parent_node_id,
            object_path,
            list_name,
            class_attribute,
            after_item_node_id,
            initial_child_fields,
            field_order,
            initial_children,
        } => object_lists::insert_object_list_item(
            document,
            parent_node_id,
            &object_path,
            list_name,
            class_attribute,
            after_item_node_id,
            initial_child_fields,
            field_order,
            initial_children,
            context,
        ),

        XmlEdit::RemoveObjectListItem {
            list_item_node_id,
            prune_empty_ancestors,
        } => object_lists::remove_object_list_item(
            document,
            list_item_node_id,
            prune_empty_ancestors,
        ),

        XmlEdit::RemoveElementAttribute {
            element_node_id,
            attribute_name,
        } => scalar::remove_element_attribute(document, element_node_id, attribute_name),

        XmlEdit::RemoveNestedElement {
            parent_node_id,
            object_path,
            field_name,
            prune_empty_ancestors,
        } => scalar::remove_nested_element(
            document,
            parent_node_id,
            &object_path,
            field_name,
            prune_empty_ancestors,
        ),

        XmlEdit::SetTypedReferenceListItems {
            parent_node_id,
            object_path,
            field_name,
            items,
        } => typed_references::set_typed_reference_list_items(
            document,
            parent_node_id,
            &object_path,
            field_name,
            items,
            context,
        ),

        XmlEdit::ReplaceKeyedValueListEntries {
            parent_node_id,
            object_path,
            map_name,
            entries,
        } => maps::replace_keyed_value_list_entries(
            document,
            parent_node_id,
            &object_path,
            map_name,
            entries.into_iter().map(|e| (e.key, e.value)).collect(),
            context,
        ),

        XmlEdit::InsertKeyedObjectListItem {
            parent_node_id,
            object_path,
            list_name,
            key_name,
            after_item_node_id,
            initial_children,
            field_order,
        } => object_lists::insert_keyed_object_list_item(
            document,
            parent_node_id,
            &object_path,
            list_name,
            key_name,
            after_item_node_id,
            initial_children,
            field_order,
            context,
        ),

        XmlEdit::RenameKeyedObjectListItem {
            item_node_id,
            new_name,
        } => object_lists::rename_keyed_object_list_item(document, item_node_id, new_name),

        XmlEdit::SetKeyedObjectListItemText {
            item_node_id,
            value,
        } => object_lists::set_keyed_object_list_item_text(document, item_node_id, value),

        XmlEdit::SetNestedElementAttribute {
            parent_node_id,
            object_path,
            attribute_name,
            value,
        } => scalar::set_nested_element_attribute(
            document,
            parent_node_id,
            &object_path,
            attribute_name,
            value,
            context,
        ),
    }
}
