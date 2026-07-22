use crate::schema_pack::model::{FieldTypeKind, ObjectTypeSchemaFile, SchemaLoadDiagnostic};

/// Parse a single object type file. `pack_id` is used only for diagnostic context.
pub fn parse_object_type_schema(
    path_label: &str,
    pack_id: &str,
    raw_json: &str,
) -> (Option<ObjectTypeSchemaFile>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();

    let value: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_object_type_json_invalid",
                    format!("JSON parse error in object file: {}", e),
                )
                .with_pack_id(pack_id)
                .with_path(path_label),
            );
            return (None, diags);
        }
    };

    let object_type = value
        .get("objectType")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if object_type.is_empty() {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_missing_object_type",
                "Object file is missing a non-empty objectType field.",
            )
            .with_pack_id(pack_id)
            .with_path(path_label),
        );
        return (None, diags);
    }

    let mut obj_file: ObjectTypeSchemaFile = match serde_json::from_value(value) {
        Ok(o) => o,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_object_type_json_invalid",
                    format!("Failed to deserialize object file: {}", e),
                )
                .with_pack_id(pack_id)
                .with_path(path_label),
            );
            return (None, diags);
        }
    };

    for (field_name, field) in obj_file.schema.fields.iter_mut() {
        if field.field_type.kind == FieldTypeKind::Unrecognized {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_invalid_field_type",
                    format!(
                        "Unrecognized field type kind in {}.{}. Field treated as unknown.",
                        obj_file.object_type, field_name
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path_label)
                .with_field_path(format!("{}.fields.{}", obj_file.object_type, field_name))
                .with_args(crate::diagnostics::diagnostic_args([
                    ("objectType", obj_file.object_type.as_str().into()),
                    ("fieldName", field_name.as_str().into()),
                ])),
            );
            field.field_type.kind = FieldTypeKind::Unknown;
        }
    }

    (Some(obj_file), diags)
}
