//! Cross-references patch operation `class_name`s against schema-pack-defined patch operation
//! metadata (`schema_pack::PatchOperationMetadata`), and serializes metadata-described field
//! values back into RimWorld-compatible XML.
//!
//! Covers both custom/mod-defined operations and built-in operations shipped as metadata (see
//! `schema-packs/rimworld-core/1.6/patch-operations/` and
//! `docs/patches-editor/03-custom-operation-metadata.md`). This module never executes mod
//! assemblies: it only reads declarative metadata and produces or consumes plain XML
//! strings/text, never Rust/C# code.

use std::collections::BTreeMap;

use crate::schema_pack::{
    lookup_patch_operation_metadata, PatchOperationMetadata, SchemaCatalog, XmlFieldShape,
};

use super::serializer::{escape_attr, escape_text, indent, reindent_fragment};

/// Look up patch operation metadata by `class_name`. Returns metadata for built-in operation
/// classes too, since built-in operations are shipped as metadata so custom and built-in
/// operations can render through the same form path.
pub fn lookup_custom_operation_metadata<'a>(
    catalog: &'a SchemaCatalog,
    class_name: &str,
) -> Option<&'a PatchOperationMetadata> {
    lookup_patch_operation_metadata(catalog, class_name)
}

/// A resolved value for one metadata-described field, ready to serialize as XML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomFieldValue {
    /// Text content for a scalar field (e.g. an `xpath`-role field). Escaped on serialization.
    Text(String),
    /// Raw XML content for an `xmlValue`/`operation`/`operationList`-role field, inserted
    /// verbatim (not escaped), matching how `patches::serializer` writes `PatchOperationAdd`'s
    /// `<value>` element.
    Xml(String),
}

/// Result of serializing metadata field values: attributes belong on the operation's own
/// `<Operation Class="...">` opening tag (or a nested `<li>`/`<match>` tag for a nested
/// operation), while `body` is child-element content that sits between the opening and closing
/// tags. Split this way because a single inner-XML string cannot represent attribute-shaped
/// fields -- an XML attribute cannot be expressed as a child element without changing the
/// document's structure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SerializedCustomOperationFields {
    pub attributes: Vec<(String, String)>,
    pub body: String,
}

/// Serialize a set of resolved field values against a patch operation's metadata, using
/// `metadata.field_order` for order and each field's declared `xml` shape to decide whether the
/// value becomes an XML attribute or an XML child element.
///
/// Fields declared on `metadata` but absent from `values` are omitted. Keys in `values` that are
/// not declared on `metadata` are ignored; callers should validate field names against
/// `metadata.fields` before calling this (e.g. when applying user-entered form values).
///
/// `xml: "attribute"` fields are written to `SerializedCustomOperationFields::attributes`. Every
/// other declared shape (`element`, `object`, `text`, `listOfLi`, etc.) is written as a child
/// element in `body` -- patch operation fields in practice only use `element` (scalar text, e.g.
/// `xpath`) and `object` (raw nested XML, e.g. `value`/`match`/`operations`) today, so shapes
/// with a distinct RimWorld-side wire format beyond "child element" (e.g. `listOfLi`'s repeated
/// `<li>` items) rely on the caller supplying an already-shaped `CustomFieldValue::Xml` payload
/// rather than this function reconstructing shape-specific nesting itself.
pub fn serialize_custom_operation_fields(
    metadata: &PatchOperationMetadata,
    values: &BTreeMap<String, CustomFieldValue>,
) -> SerializedCustomOperationFields {
    let mut result = SerializedCustomOperationFields::default();
    for field_name in &metadata.field_order {
        let Some(field) = metadata.fields.get(field_name) else {
            continue;
        };
        let Some(value) = values.get(field_name) else {
            continue;
        };
        if field.xml == XmlFieldShape::Attribute {
            result
                .attributes
                .push((field_name.clone(), value_as_text(value)));
        } else {
            write_field(&mut result.body, field_name, value);
        }
    }
    result
}

fn value_as_text(value: &CustomFieldValue) -> String {
    match value {
        CustomFieldValue::Text(text) => text.clone(),
        CustomFieldValue::Xml(xml) => xml.clone(),
    }
}

fn write_field(out: &mut String, field_name: &str, value: &CustomFieldValue) {
    indent(out, 1);
    out.push('<');
    out.push_str(field_name);
    match value {
        CustomFieldValue::Text(text) => {
            out.push('>');
            out.push_str(&escape_text(text));
        }
        CustomFieldValue::Xml(xml) => {
            let body = reindent_fragment(xml, 2);
            if body.is_empty() {
                out.push('>');
            } else {
                out.push_str(">\n");
                out.push_str(&body);
                out.push('\n');
                indent(out, 1);
            }
        }
    }
    out.push_str("</");
    out.push_str(field_name);
    out.push_str(">\n");
}

/// Format a serialized attribute list as `Name="value"` pairs (space-separated, escaped),
/// suitable for appending after `Class="..."` on an operation's opening tag.
pub fn format_custom_operation_attributes(attributes: &[(String, String)]) -> String {
    let mut out = String::new();
    for (name, value) in attributes {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        out.push_str(&escape_attr(value));
        out.push('"');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_pack::{
        build_schema_catalog, FieldSchema, FieldType, FieldTypeKind, PatchOperationPreview,
        PatchOperationPreviewKind,
    };

    fn add_metadata(catalog: &SchemaCatalog) -> &PatchOperationMetadata {
        lookup_custom_operation_metadata(catalog, "PatchOperationAdd")
            .expect("PatchOperationAdd metadata should be shipped as a built-in")
    }

    fn test_field(xml: XmlFieldShape) -> FieldSchema {
        FieldSchema {
            label: None,
            description: None,
            field_type: FieldType {
                kind: FieldTypeKind::String,
                schema_ref: None,
                reference: None,
            },
            required: false,
            default_value: None,
            examples: Vec::new(),
            validation_hints: None,
            reference: None,
            key_reference: None,
            typed_reference: None,
            key_field: None,
            value_field: None,
            default_value_field: None,
            value_type: None,
            repeatable: false,
            xml,
            source_pack_id: None,
            label_source_pack_id: None,
            description_source_pack_id: None,
            items: None,
            flags: false,
            default_collapsed: None,
            xml_aliases: Vec::new(),
            role: None,
        }
    }

    #[test]
    fn serializes_text_and_xml_fields_in_declared_order() {
        let result = build_schema_catalog(&[], None);
        let metadata = add_metadata(&result.catalog);

        let mut values = BTreeMap::new();
        values.insert(
            "value".to_string(),
            CustomFieldValue::Xml("<statBases><MoveSpeed>1</MoveSpeed></statBases>".to_string()),
        );
        values.insert(
            "xpath".to_string(),
            CustomFieldValue::Text("Defs/ThingDef[defName=\"Wall\"]".to_string()),
        );

        let serialized = serialize_custom_operation_fields(metadata, &values);

        assert!(
            serialized.attributes.is_empty(),
            "xpath/value are element-shaped, not attribute-shaped"
        );
        let xml = serialized.body;
        let xpath_pos = xml.find("<xpath>").expect("xpath field present");
        let value_pos = xml.find("<value>").expect("value field present");
        assert!(
            xpath_pos < value_pos,
            "xpath should serialize before value per fieldOrder"
        );
        assert!(xml.contains("<xpath>Defs/ThingDef[defName=\"Wall\"]</xpath>"));
        assert!(xml
            .contains("<value>\n    <statBases><MoveSpeed>1</MoveSpeed></statBases>\n  </value>"));
    }

    #[test]
    fn escapes_text_field_values() {
        let result = build_schema_catalog(&[], None);
        let metadata = add_metadata(&result.catalog);

        let mut values = BTreeMap::new();
        values.insert(
            "xpath".to_string(),
            CustomFieldValue::Text("Defs/ThingDef[defName=\"A & B\"]".to_string()),
        );

        let serialized = serialize_custom_operation_fields(metadata, &values);
        assert!(serialized.body.contains("A &amp; B"));
        assert!(!serialized.body.contains("A & B"));
    }

    #[test]
    fn omits_fields_absent_from_values() {
        let result = build_schema_catalog(&[], None);
        let metadata = add_metadata(&result.catalog);

        let mut values = BTreeMap::new();
        values.insert(
            "xpath".to_string(),
            CustomFieldValue::Text("Defs/ThingDef".to_string()),
        );

        let serialized = serialize_custom_operation_fields(metadata, &values);
        assert!(serialized.body.contains("<xpath>"));
        assert!(!serialized.body.contains("<value>"));
        assert!(!serialized.body.contains("<order>"));
    }

    #[test]
    fn attribute_shaped_fields_serialize_as_attributes_not_child_elements() {
        // PatchOperationAttributeAdd's built-in metadata has no attribute-shaped fields (RimWorld
        // never uses XML attributes for operation-specific data), so build an inline metadata
        // entry with an attribute-shaped field to exercise the split.
        let metadata = PatchOperationMetadata {
            class_name: "MyMod.PatchOperationCustom".to_string(),
            label: None,
            description: None,
            field_order: vec!["xpath".to_string(), "MayRequireCustom".to_string()],
            fields: BTreeMap::from([
                ("xpath".to_string(), test_field(XmlFieldShape::Element)),
                (
                    "MayRequireCustom".to_string(),
                    test_field(XmlFieldShape::Attribute),
                ),
            ]),
            preview: PatchOperationPreview {
                kind: PatchOperationPreviewKind::Unsupported,
                message: None,
            },
            source_pack_id: None,
        };

        let mut values = BTreeMap::new();
        values.insert(
            "xpath".to_string(),
            CustomFieldValue::Text("Defs/ThingDef".to_string()),
        );
        values.insert(
            "MayRequireCustom".to_string(),
            CustomFieldValue::Text("SomeMod & Co".to_string()),
        );

        let serialized = serialize_custom_operation_fields(&metadata, &values);
        assert_eq!(
            serialized.attributes,
            vec![("MayRequireCustom".to_string(), "SomeMod & Co".to_string())],
            "attribute-shaped field must not appear in the child-element body"
        );
        assert!(serialized.body.contains("<xpath>"));
        assert!(!serialized.body.contains("MayRequireCustom"));

        let formatted = format_custom_operation_attributes(&serialized.attributes);
        assert_eq!(formatted, " MayRequireCustom=\"SomeMod &amp; Co\"");
    }
}
