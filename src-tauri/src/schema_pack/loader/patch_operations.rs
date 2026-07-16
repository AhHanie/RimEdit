use crate::schema_pack::model::{FieldTypeKind, PatchOperationMetadataFile, SchemaLoadDiagnostic};

/// Parse a single patch operation metadata file. `pack_id` is used only for diagnostic context.
///
/// `formatVersion` is checked directly against the raw JSON (mirroring
/// `parse_schema_pack_manifest`) rather than via serde, so an unsupported version reports a
/// dedicated diagnostic instead of a generic deserialization error.
pub fn parse_patch_operation_metadata(
    path_label: &str,
    pack_id: &str,
    raw_json: &str,
) -> (
    Option<PatchOperationMetadataFile>,
    Vec<SchemaLoadDiagnostic>,
) {
    let mut diags = Vec::new();

    let value: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "patch_operation_metadata_json_invalid",
                    format!("JSON parse error in patch operation metadata file: {}", e),
                )
                .with_pack_id(pack_id)
                .with_path(path_label),
            );
            return (None, diags);
        }
    };

    let format_version = value
        .get("formatVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if format_version != 1 {
        diags.push(
            SchemaLoadDiagnostic::error(
                "patch_operation_metadata_format_unsupported",
                format!(
                    "Unsupported patch operation metadata formatVersion: {}. Only version 1 is supported.",
                    format_version
                ),
            )
            .with_pack_id(pack_id)
            .with_path(path_label)
            .with_args(crate::diagnostics::diagnostic_args([(
                "formatVersion",
                (format_version as i64).into(),
            )])),
        );
        return (None, diags);
    }

    let class_name = value
        .get("className")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if class_name.is_empty() {
        diags.push(
            SchemaLoadDiagnostic::error(
                "patch_operation_metadata_missing_class_name",
                "Patch operation metadata file is missing a non-empty className.",
            )
            .with_pack_id(pack_id)
            .with_path(path_label),
        );
        return (None, diags);
    }

    let mut file: PatchOperationMetadataFile = match serde_json::from_value(value) {
        Ok(f) => f,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "patch_operation_metadata_json_invalid",
                    format!("Failed to deserialize patch operation metadata file: {}", e),
                )
                .with_pack_id(pack_id)
                .with_path(path_label),
            );
            return (None, diags);
        }
    };

    for (field_name, field) in file.schema.fields.iter_mut() {
        if field.field_type.kind == FieldTypeKind::Unrecognized {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_invalid_field_type",
                    format!(
                        "Unrecognized field type kind in patch operation {}.{}. Field treated as unknown.",
                        file.class_name, field_name
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path_label)
                .with_field_path(format!("{}.fields.{}", file.class_name, field_name))
                .with_args(crate::diagnostics::diagnostic_args([
                    ("className", file.class_name.as_str().into()),
                    ("fieldName", field_name.as_str().into()),
                ])),
            );
            field.field_type.kind = FieldTypeKind::Unknown;
        }
    }

    (Some(file), diags)
}
