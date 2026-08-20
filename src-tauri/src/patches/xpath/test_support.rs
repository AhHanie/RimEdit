//! Minimal schema-catalog fixture builders shared by `xpath/`'s module-local unit tests. Kept
//! deliberately small (a bare scalar/object field and a one- or two-Def-type catalog) -- the full
//! synthetic catalog exercising every traversal shape lives in `patches::tests::xpath` as the
//! public behavior suite; duplicating it here would defeat the point of keeping these tests narrow.

use std::collections::BTreeMap;

use crate::schema_pack::{
    DefTypeSchema, FieldSchema, FieldType, FieldTypeKind, ObjectTypeSchema, SchemaCatalog,
    XmlFieldShape,
};

pub(super) fn scalar_field() -> FieldSchema {
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
        xml: XmlFieldShape::Element,
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

pub(super) fn object_field(schema_ref: &str) -> FieldSchema {
    let mut f = scalar_field();
    f.field_type = FieldType {
        kind: FieldTypeKind::Object,
        schema_ref: Some(schema_ref.to_string()),
        reference: None,
    };
    f.xml = XmlFieldShape::Object;
    f
}

pub(super) fn def_type_schema(inherits: &[&str], fields: &[(&str, FieldSchema)]) -> DefTypeSchema {
    DefTypeSchema {
        label: None,
        description: None,
        inherits: inherits.iter().map(|s| s.to_string()).collect(),
        abstract_type: false,
        field_order: Vec::new(),
        fields: fields
            .iter()
            .cloned()
            .map(|(name, field)| (name.to_string(), field))
            .collect(),
        templates: BTreeMap::new(),
        validation_rules: BTreeMap::new(),
        form_views: BTreeMap::new(),
    }
}

pub(super) fn object_type_schema(fields: &[(&str, FieldSchema)]) -> ObjectTypeSchema {
    ObjectTypeSchema {
        label: None,
        description: None,
        inherits: Vec::new(),
        field_order: Vec::new(),
        fields: fields
            .iter()
            .cloned()
            .map(|(name, field)| (name.to_string(), field))
            .collect(),
        discriminator: None,
    }
}

pub(super) fn catalog(
    def_types: &[(&str, DefTypeSchema)],
    object_types: &[(&str, ObjectTypeSchema)],
) -> SchemaCatalog {
    SchemaCatalog {
        format_version: 1,
        packs: Vec::new(),
        def_types: def_types
            .iter()
            .cloned()
            .map(|(name, schema)| (name.to_string(), schema))
            .collect(),
        object_types: object_types
            .iter()
            .cloned()
            .map(|(name, schema)| (name.to_string(), schema))
            .collect(),
        patch_operations: BTreeMap::new(),
    }
}
