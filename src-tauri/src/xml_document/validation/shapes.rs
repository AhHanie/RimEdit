use crate::schema_pack::{FieldSchema, XmlFieldShape};

pub(super) fn shape_mismatch_message(
    field_name: &str,
    field_schema: &FieldSchema,
    has_element_children: bool,
    has_scalar_text: bool,
    element_child_names: &[String],
) -> Option<String> {
    match &field_schema.xml {
        XmlFieldShape::Attribute => Some(format!(
            "Field '{}' is defined as an attribute but is present as an element.",
            field_name
        )),
        XmlFieldShape::Element | XmlFieldShape::Text => {
            if has_element_children {
                Some(format!("Field '{}' expects scalar text.", field_name))
            } else {
                None
            }
        }
        XmlFieldShape::ListOfLi => {
            if !has_scalar_text
                && (!has_element_children || element_child_names.iter().all(|n| n == "li"))
            {
                None
            } else {
                Some(format!(
                    "Field '{}' expects a list of <li> children.",
                    field_name
                ))
            }
        }
        XmlFieldShape::KeyedValueList => {
            if !has_scalar_text && element_child_names.iter().all(|n| n != "li") {
                None
            } else {
                Some(format!(
                    "Field '{}' expects keyed scalar child elements.",
                    field_name
                ))
            }
        }
        XmlFieldShape::Object | XmlFieldShape::NamedChildrenMap => None,
        XmlFieldShape::KeyedObjectList => {
            // Container must have element children only; no scalar text, no <li>.
            if has_scalar_text {
                Some(format!(
                    "Field '{}' expects keyed object children, not scalar text.",
                    field_name
                ))
            } else if has_element_children && element_child_names.iter().any(|n| n == "li") {
                Some(format!(
                    "Field '{}' expects keyed object children, not <li> children.",
                    field_name
                ))
            } else {
                None
            }
        }
        XmlFieldShape::TypedReferenceList => {
            // Container may be absent; when present it must have element children only (no <li>).
            if has_scalar_text {
                Some(format!(
                    "Field '{}' expects typed reference child elements, not scalar text.",
                    field_name
                ))
            } else if has_element_children && element_child_names.iter().any(|n| n == "li") {
                Some(format!(
                    "Field '{}' expects typed reference child elements, not <li> children.",
                    field_name
                ))
            } else {
                None
            }
        }
        XmlFieldShape::FlagsText => {
            if has_element_children {
                Some(format!(
                    "Field '{}' expects comma-separated text.",
                    field_name
                ))
            } else {
                None
            }
        }
        XmlFieldShape::KeyedObjectMap => {
            // Container must only have <li> children or be empty; no scalar text.
            if has_scalar_text {
                Some(format!(
                    "Field '{}' expects <li> map entries, not scalar text.",
                    field_name
                ))
            } else if has_element_children && element_child_names.iter().any(|n| n != "li") {
                Some(format!(
                    "Field '{}' expects only <li> children for keyed object map entries.",
                    field_name
                ))
            } else {
                None
            }
        }
    }
}
