use serde::Deserialize;

use super::super::model::XmlNodeId;

#[derive(Clone, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum XmlEdit {
    SetChildElementText {
        parent_node_id: XmlNodeId,
        child_name: String,
        value: String,
    },
    SetElementAttribute {
        element_node_id: XmlNodeId,
        attribute_name: String,
        value: String,
    },
    RemoveChildElement {
        parent_node_id: XmlNodeId,
        child_name: String,
    },
    SetListItems {
        parent_node_id: XmlNodeId,
        child_name: String,
        items: Vec<String>,
    },
    SetNestedObjectFieldText {
        parent_node_id: XmlNodeId,
        object_name: String,
        field_name: String,
        value: String,
    },
    SetNestedElementText {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        field_name: String,
        value: String,
        #[serde(default)]
        field_order: Vec<String>,
    },
    /// Replace all `<li>` children of `<field_name>` inside the object at `object_path`.
    SetNestedListItems {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        field_name: String,
        items: Vec<String>,
        #[serde(default)]
        field_order: Vec<String>,
    },
    /// Set or create a named child element inside a named-children-map field.
    SetNamedMapEntry {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        map_name: String,
        key: String,
        value: String,
        #[serde(default)]
        field_order: Vec<String>,
    },
    /// Remove a named child element from a named-children-map field.
    RemoveNamedMapEntry {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        map_name: String,
        key: String,
    },
    /// Rename a named child element inside a named-children-map field, preserving its value.
    RenameNamedMapEntry {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        map_name: String,
        old_key: String,
        new_key: String,
        #[serde(default)]
        field_order: Vec<String>,
    },
    /// Set or add an attribute on an object-list `<li>` item (e.g. `Class`).
    SetObjectListItemAttribute {
        list_item_node_id: XmlNodeId,
        attribute_name: String,
        value: String,
    },
    /// Set or create a scalar child element of an object-list `<li>` item.
    ///
    /// Converts a self-closing `<li/>` to a normal open/close element when needed.
    SetObjectListItemChildText {
        list_item_node_id: XmlNodeId,
        child_name: String,
        value: String,
        #[serde(default)]
        field_order: Vec<String>,
    },
    /// Remove a scalar child element from an object-list `<li>` item.
    RemoveObjectListItemChild {
        list_item_node_id: XmlNodeId,
        child_name: String,
    },
    /// Insert a new `<li>` item into an object list, creating the container if absent.
    ///
    /// `object_path` is an optional sequence of element names to navigate from
    /// `parent_node_id` before looking up `list_name`. Empty for top-level lists.
    /// `initial_child_fields` are set as child element text nodes on the new item
    /// in the same operation, so the caller does not need a second round-trip.
    InsertObjectListItem {
        parent_node_id: XmlNodeId,
        #[serde(default)]
        object_path: Vec<String>,
        list_name: String,
        class_attribute: Option<String>,
        after_item_node_id: Option<XmlNodeId>,
        #[serde(default)]
        initial_child_fields: Vec<NameValuePair>,
        #[serde(default)]
        field_order: Vec<String>,
        /// Recursive element tree; applied after `initial_child_fields` when present.
        #[serde(default)]
        initial_children: Vec<InitialElement>,
    },
    /// Remove a `<li>` item from an object list, including its preceding indent.
    /// When `prune_empty_ancestors` is true and the list container becomes empty after
    /// removal, the empty container and any empty ancestor object elements are also removed.
    RemoveObjectListItem {
        list_item_node_id: XmlNodeId,
        #[serde(default)]
        prune_empty_ancestors: bool,
    },
    /// Remove an attribute from an element node. Idempotent if the attribute is absent.
    RemoveElementAttribute {
        element_node_id: XmlNodeId,
        attribute_name: String,
    },
    /// Remove a field element inside a nested object path without creating the path if absent.
    /// Idempotent if the path or field is missing.
    /// When `prune_empty_ancestors` is true, any ancestor object elements that become empty
    /// after the removal are also removed, walking up through `object_path`.
    RemoveNestedElement {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        field_name: String,
        #[serde(default)]
        prune_empty_ancestors: bool,
    },
    /// Replace all element children of a typed-reference-list container with
    /// ordered children named by `def_type` containing `def_name` as text.
    SetTypedReferenceListItems {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        field_name: String,
        items: Vec<TypedReferenceListItem>,
    },
    /// Replace all keyed child elements of a `keyedValueList` container with
    /// an ordered list of (key, value) pairs. Used for `repeatable = true` fields
    /// that allow duplicate keys (e.g. `nullifyingTraitDegrees`).
    ReplaceKeyedValueListEntries {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        map_name: String,
        entries: Vec<KeyValuePair>,
    },
    /// Insert a new named element into a keyed-object-list container, creating the container if absent.
    ///
    /// Unlike `InsertObjectListItem`, the new item is a named element (not `<li>`); its name is
    /// `key_name` (the item key, typically a Def defName). `after_item_node_id` controls insertion
    /// order; `initial_children` populates field elements on the new item immediately.
    InsertKeyedObjectListItem {
        parent_node_id: XmlNodeId,
        #[serde(default)]
        object_path: Vec<String>,
        list_name: String,
        key_name: String,
        after_item_node_id: Option<XmlNodeId>,
        #[serde(default)]
        initial_children: Vec<InitialElement>,
        #[serde(default)]
        field_order: Vec<String>,
    },
    /// Rename a keyed element in a keyed-object-list in place. Used when the item key (def reference)
    /// is changed by the user. The element name IS the key, so renaming it changes the def reference.
    RenameKeyedObjectListItem {
        item_node_id: XmlNodeId,
        new_name: String,
    },
    /// Set the scalar text content of a keyed object-list item element directly.
    /// Used when the item was loaded via `defaultValueField` shorthand (e.g. `<Corpse>0.25</Corpse>`),
    /// so editing the field must update the element's own text rather than creating a child element.
    SetKeyedObjectListItemText {
        item_node_id: XmlNodeId,
        value: String,
    },
    /// Set an XML attribute on an element found by navigating `object_path` from `parent_node_id`.
    /// Creates the object path if any segment is absent.
    SetNestedElementAttribute {
        parent_node_id: XmlNodeId,
        object_path: Vec<String>,
        attribute_name: String,
        value: String,
    },
}

/// A recursive element to create when inserting a new object-list item.
/// Mirrors the TypeScript `XmlInitialElement` type.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialElement {
    pub name: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub attributes: Vec<NameValuePair>,
    #[serde(default)]
    pub children: Vec<InitialElement>,
    #[serde(default)]
    pub li_items: Vec<InitialElement>,
}

/// A (key, value) pair used by `ReplaceKeyedValueListEntries`.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedReferenceListItem {
    pub def_type: String,
    pub def_name: String,
}

/// A (name, value) pair used to set initial child fields on a new list item.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameValuePair {
    pub name: String,
    pub value: String,
}
