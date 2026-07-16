use crate::schema_pack::model::{SchemaLoadDiagnostic, SchemaPackManifestFile};

/// Parse the manifest-only JSON file. Does not load def type files.
pub fn parse_schema_pack_manifest(
    path_label: &str,
    raw_json: &str,
) -> (Option<SchemaPackManifestFile>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();

    let value: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_manifest_json_invalid",
                    format!("JSON parse error: {}", e),
                )
                .with_path(path_label),
            );
            return (None, diags);
        }
    };

    let format_version = value
        .get("formatVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    // Version 2 is additive: it only adds the optional `patchOperationDirectories` field, so
    // packs written for version 1 keep working unchanged and don't need a bump. Version 3 is
    // also additive: it only permits Def-type files to declare `formViews` (see
    // `parse_def_type_schema`); packs that don't use `formViews` need no bump to 3 either.
    if format_version != 1 && format_version != 2 && format_version != 3 {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_manifest_format_unsupported",
                format!(
                    "Unsupported formatVersion: {}. Supported versions: 1, 2, 3.",
                    format_version
                ),
            )
            .with_path(path_label)
            .with_args(crate::diagnostics::diagnostic_args([(
                "formatVersion",
                (format_version as i64).into(),
            )])),
        );
        return (None, diags);
    }

    let pack_id = value
        .get("packId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if pack_id.is_empty() {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_missing_pack_id",
                "Schema pack is missing a non-empty packId.",
            )
            .with_path(path_label),
        );
        return (None, diags);
    }

    let manifest_file: SchemaPackManifestFile = match serde_json::from_value(value) {
        Ok(m) => m,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_manifest_json_invalid",
                    format!("Failed to deserialize manifest: {}", e),
                )
                .with_path(path_label)
                .with_pack_id(&pack_id),
            );
            return (None, diags);
        }
    };

    if manifest_file.def_type_directories.is_empty() {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_def_type_directory_missing",
                "Manifest is missing a non-empty defTypeDirectories list.",
            )
            .with_path(path_label)
            .with_pack_id(&pack_id),
        );
        return (None, diags);
    }

    (Some(manifest_file), diags)
}
