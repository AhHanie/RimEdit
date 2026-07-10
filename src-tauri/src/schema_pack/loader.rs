use super::model::{
    DefTypeSchemaFile, FieldTypeKind, ObjectTypeSchemaFile, PatchOperationMetadataFile,
    SchemaLoadDiagnostic, SchemaPackManifest, SchemaPackManifestFile,
};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/built_in_schema_packs.rs"));

const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_DEF_FILE_BYTES: u64 = 1024 * 1024;
const MAX_DEF_FILES_PER_PACK: usize = 5000;

pub struct LoadedPack {
    pub manifest: SchemaPackManifest,
    pub is_builtin: bool,
    pub source_path: Option<String>,
}

pub fn load_built_in_packs() -> (Vec<LoadedPack>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();
    let mut packs = Vec::new();

    for (path_label, manifest_raw, def_files, obj_files, patch_op_files) in BUILT_IN_SCHEMA_PACKS {
        let (manifest_file_opt, mdiags) = parse_schema_pack_manifest(path_label, manifest_raw);
        diags.extend(mdiags);

        if let Some(manifest_file) = manifest_file_opt {
            let pack_id = manifest_file.pack_id.clone();
            let mut def_file_pairs: Vec<(&str, DefTypeSchemaFile)> = Vec::new();
            for (label, raw) in *def_files {
                let (def_opt, ddiags) = parse_def_type_schema(label, &pack_id, raw);
                diags.extend(ddiags);
                if let Some(def_file) = def_opt {
                    def_file_pairs.push((label, def_file));
                }
            }
            let mut obj_file_pairs: Vec<(&str, ObjectTypeSchemaFile)> = Vec::new();
            for (label, raw) in *obj_files {
                let (obj_opt, odiags) = parse_object_type_schema(label, &pack_id, raw);
                diags.extend(odiags);
                if let Some(obj_file) = obj_opt {
                    obj_file_pairs.push((label, obj_file));
                }
            }
            let mut patch_op_file_pairs: Vec<(&str, PatchOperationMetadataFile)> = Vec::new();
            for (label, raw) in *patch_op_files {
                let (op_opt, pdiags) = parse_patch_operation_metadata(label, &pack_id, raw);
                diags.extend(pdiags);
                if let Some(op_file) = op_opt {
                    patch_op_file_pairs.push((label, op_file));
                }
            }
            let def_refs: Vec<(&str, &DefTypeSchemaFile)> =
                def_file_pairs.iter().map(|(l, d)| (*l, d)).collect();
            let obj_refs: Vec<(&str, &ObjectTypeSchemaFile)> =
                obj_file_pairs.iter().map(|(l, o)| (*l, o)).collect();
            let patch_op_refs: Vec<(&str, &PatchOperationMetadataFile)> =
                patch_op_file_pairs.iter().map(|(l, o)| (*l, o)).collect();
            let (pack_opt, adiags) = assemble_schema_pack(
                path_label,
                manifest_file,
                &def_refs,
                &obj_refs,
                &patch_op_refs,
            );
            diags.extend(adiags);
            if let Some(manifest) = pack_opt {
                packs.push(LoadedPack {
                    manifest,
                    is_builtin: true,
                    source_path: None,
                });
            }
        }
    }

    (packs, diags)
}

pub fn load_external_packs(roots: &[PathBuf]) -> (Vec<LoadedPack>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();
    let mut packs = Vec::new();

    for root in roots {
        if !root.exists() {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_root_missing",
                    format!("Schema pack root does not exist: {}", root.display()),
                )
                .with_path(root.to_string_lossy()),
            );
            continue;
        }

        let manifest_paths = discover_manifest_paths_in_root(root);
        for manifest_path in manifest_paths {
            let (pack_opt, pack_diags) = load_pack_from_directory(&manifest_path);
            diags.extend(pack_diags);
            if let Some(pack) = pack_opt {
                packs.push(pack);
            }
        }
    }

    (packs, diags)
}

pub fn load_pack_from_directory(
    manifest_path: &Path,
) -> (Option<LoadedPack>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();
    let path_label = manifest_path.to_string_lossy().to_string();

    if let Ok(meta) = std::fs::metadata(manifest_path) {
        if meta.len() > MAX_MANIFEST_BYTES {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_manifest_too_large",
                    format!("Manifest file exceeds 256 KiB limit: {}", path_label),
                )
                .with_path(&path_label),
            );
            return (None, diags);
        }
    }

    let raw_manifest = match std::fs::read_to_string(manifest_path) {
        Ok(s) => s,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_manifest_read_failed",
                    format!("Cannot read manifest file: {}", e),
                )
                .with_path(&path_label),
            );
            return (None, diags);
        }
    };

    let (manifest_file_opt, mdiags) = parse_schema_pack_manifest(&path_label, &raw_manifest);
    diags.extend(mdiags);
    let manifest_file = match manifest_file_opt {
        Some(m) => m,
        None => return (None, diags),
    };

    let pack_id = manifest_file.pack_id.clone();
    let manifest_dir = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let mut def_file_pairs: Vec<(String, DefTypeSchemaFile)> = Vec::new();
    let mut total_def_files = 0usize;

    for dir_entry in &manifest_file.def_type_directories {
        let resolved = resolve_manifest_relative_dir(&manifest_dir, dir_entry);
        let resolved = match resolved {
            Some(p) => p,
            None => {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_def_type_directory_escape",
                        format!(
                            "defTypeDirectories entry '{}' escapes the pack root via '..' - skipping.",
                            dir_entry
                        ),
                    )
                    .with_pack_id(&pack_id)
                    .with_path(&path_label),
                );
                continue;
            }
        };

        if is_symlink(&resolved) {
            continue;
        }

        if !resolved.is_dir() {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_def_type_directory_missing",
                    format!("def type directory does not exist: {}", resolved.display()),
                )
                .with_pack_id(&pack_id)
                .with_path(resolved.to_string_lossy()),
            );
            continue;
        }

        let mut json_paths = collect_json_files(&resolved);
        json_paths.sort();

        for json_path in json_paths {
            if total_def_files >= MAX_DEF_FILES_PER_PACK {
                diags.push(
                    SchemaLoadDiagnostic::warning(
                        "schema_pack_too_many_def_files",
                        format!(
                            "Pack '{}' has more than {} def files - remaining files skipped.",
                            pack_id, MAX_DEF_FILES_PER_PACK
                        ),
                    )
                    .with_pack_id(&pack_id),
                );
                break;
            }

            let file_label = json_path.to_string_lossy().to_string();

            if let Ok(meta) = std::fs::metadata(&json_path) {
                if meta.len() > MAX_DEF_FILE_BYTES {
                    diags.push(
                        SchemaLoadDiagnostic::warning(
                            "schema_pack_def_file_too_large",
                            format!("Def file exceeds 1 MiB limit, skipping: {}", file_label),
                        )
                        .with_pack_id(&pack_id)
                        .with_path(&file_label),
                    );
                    continue;
                }
            }

            match std::fs::read_to_string(&json_path) {
                Ok(raw) => {
                    let (def_opt, ddiags) = parse_def_type_schema(&file_label, &pack_id, &raw);
                    diags.extend(ddiags);
                    if let Some(def_file) = def_opt {
                        def_file_pairs.push((file_label, def_file));
                        total_def_files += 1;
                    }
                }
                Err(e) => {
                    diags.push(
                        SchemaLoadDiagnostic::error(
                            "schema_pack_def_type_file_read_failed",
                            format!("Cannot read def file: {}", e),
                        )
                        .with_pack_id(&pack_id)
                        .with_path(&file_label),
                    );
                }
            }
        }
    }

    let mut obj_file_pairs: Vec<(String, ObjectTypeSchemaFile)> = Vec::new();
    let mut total_obj_files = 0usize;

    for dir_entry in &manifest_file.object_type_directories {
        let resolved = resolve_manifest_relative_dir(&manifest_dir, dir_entry);
        let resolved = match resolved {
            Some(p) => p,
            None => {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_object_type_directory_escape",
                        format!(
                            "objectTypeDirectories entry '{}' escapes the pack root via '..' - skipping.",
                            dir_entry
                        ),
                    )
                    .with_pack_id(&pack_id)
                    .with_path(&path_label),
                );
                continue;
            }
        };

        if is_symlink(&resolved) {
            continue;
        }

        if !resolved.is_dir() {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_object_type_directory_missing",
                    format!(
                        "object type directory does not exist: {}",
                        resolved.display()
                    ),
                )
                .with_pack_id(&pack_id)
                .with_path(resolved.to_string_lossy()),
            );
            continue;
        }

        let mut json_paths = collect_json_files(&resolved);
        json_paths.sort();

        for json_path in json_paths {
            if total_obj_files >= MAX_DEF_FILES_PER_PACK {
                diags.push(
                    SchemaLoadDiagnostic::warning(
                        "schema_pack_too_many_object_files",
                        format!(
                            "Pack '{}' has more than {} object files - remaining files skipped.",
                            pack_id, MAX_DEF_FILES_PER_PACK
                        ),
                    )
                    .with_pack_id(&pack_id),
                );
                break;
            }

            let file_label = json_path.to_string_lossy().to_string();

            if let Ok(meta) = std::fs::metadata(&json_path) {
                if meta.len() > MAX_DEF_FILE_BYTES {
                    diags.push(
                        SchemaLoadDiagnostic::warning(
                            "schema_pack_object_file_too_large",
                            format!("Object file exceeds 1 MiB limit, skipping: {}", file_label),
                        )
                        .with_pack_id(&pack_id)
                        .with_path(&file_label),
                    );
                    continue;
                }
            }

            match std::fs::read_to_string(&json_path) {
                Ok(raw) => {
                    let (obj_opt, odiags) = parse_object_type_schema(&file_label, &pack_id, &raw);
                    diags.extend(odiags);
                    if let Some(obj_file) = obj_opt {
                        obj_file_pairs.push((file_label, obj_file));
                        total_obj_files += 1;
                    }
                }
                Err(e) => {
                    diags.push(
                        SchemaLoadDiagnostic::error(
                            "schema_pack_object_type_file_read_failed",
                            format!("Cannot read object file: {}", e),
                        )
                        .with_pack_id(&pack_id)
                        .with_path(&file_label),
                    );
                }
            }
        }
    }

    let mut patch_op_file_pairs: Vec<(String, PatchOperationMetadataFile)> = Vec::new();
    let mut total_patch_op_files = 0usize;

    for dir_entry in &manifest_file.patch_operation_directories {
        let resolved = resolve_manifest_relative_dir(&manifest_dir, dir_entry);
        let resolved = match resolved {
            Some(p) => p,
            None => {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_patch_operation_directory_escape",
                        format!(
                            "patchOperationDirectories entry '{}' escapes the pack root via '..' - skipping.",
                            dir_entry
                        ),
                    )
                    .with_pack_id(&pack_id)
                    .with_path(&path_label),
                );
                continue;
            }
        };

        if is_symlink(&resolved) {
            continue;
        }

        if !resolved.is_dir() {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_patch_operation_directory_missing",
                    format!(
                        "patch operation directory does not exist: {}",
                        resolved.display()
                    ),
                )
                .with_pack_id(&pack_id)
                .with_path(resolved.to_string_lossy()),
            );
            continue;
        }

        let mut json_paths = collect_json_files(&resolved);
        json_paths.sort();

        for json_path in json_paths {
            if total_patch_op_files >= MAX_DEF_FILES_PER_PACK {
                diags.push(
                    SchemaLoadDiagnostic::warning(
                        "schema_pack_too_many_patch_operation_files",
                        format!(
                            "Pack '{}' has more than {} patch operation files - remaining files skipped.",
                            pack_id, MAX_DEF_FILES_PER_PACK
                        ),
                    )
                    .with_pack_id(&pack_id),
                );
                break;
            }

            let file_label = json_path.to_string_lossy().to_string();

            if let Ok(meta) = std::fs::metadata(&json_path) {
                if meta.len() > MAX_DEF_FILE_BYTES {
                    diags.push(
                        SchemaLoadDiagnostic::warning(
                            "schema_pack_patch_operation_file_too_large",
                            format!(
                                "Patch operation file exceeds 1 MiB limit, skipping: {}",
                                file_label
                            ),
                        )
                        .with_pack_id(&pack_id)
                        .with_path(&file_label),
                    );
                    continue;
                }
            }

            match std::fs::read_to_string(&json_path) {
                Ok(raw) => {
                    let (op_opt, pdiags) =
                        parse_patch_operation_metadata(&file_label, &pack_id, &raw);
                    diags.extend(pdiags);
                    if let Some(op_file) = op_opt {
                        patch_op_file_pairs.push((file_label, op_file));
                        total_patch_op_files += 1;
                    }
                }
                Err(e) => {
                    diags.push(
                        SchemaLoadDiagnostic::error(
                            "schema_pack_patch_operation_file_read_failed",
                            format!("Cannot read patch operation file: {}", e),
                        )
                        .with_pack_id(&pack_id)
                        .with_path(&file_label),
                    );
                }
            }
        }
    }

    let def_refs: Vec<(&str, &DefTypeSchemaFile)> = def_file_pairs
        .iter()
        .map(|(l, d)| (l.as_str(), d))
        .collect();
    let obj_refs: Vec<(&str, &ObjectTypeSchemaFile)> = obj_file_pairs
        .iter()
        .map(|(l, o)| (l.as_str(), o))
        .collect();
    let patch_op_refs: Vec<(&str, &PatchOperationMetadataFile)> = patch_op_file_pairs
        .iter()
        .map(|(l, o)| (l.as_str(), o))
        .collect();
    let (pack_opt, adiags) = assemble_schema_pack(
        &path_label,
        manifest_file,
        &def_refs,
        &obj_refs,
        &patch_op_refs,
    );
    diags.extend(adiags);

    let pack = pack_opt.map(|manifest| LoadedPack {
        source_path: Some(path_label),
        manifest,
        is_builtin: false,
    });

    (pack, diags)
}

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
    // packs written for version 1 keep working unchanged and don't need a bump.
    if format_version != 1 && format_version != 2 {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_manifest_format_unsupported",
                format!(
                    "Unsupported formatVersion: {}. Supported versions: 1, 2.",
                    format_version
                ),
            )
            .with_path(path_label),
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

/// Parse a single def type file. `pack_id` is used only for diagnostic context.
pub fn parse_def_type_schema(
    path_label: &str,
    pack_id: &str,
    raw_json: &str,
) -> (Option<DefTypeSchemaFile>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();

    let value: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_def_type_json_invalid",
                    format!("JSON parse error in def file: {}", e),
                )
                .with_pack_id(pack_id)
                .with_path(path_label),
            );
            return (None, diags);
        }
    };

    let def_type = value
        .get("defType")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if def_type.is_empty() {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_missing_def_type",
                "Def file is missing a non-empty defType field.",
            )
            .with_pack_id(pack_id)
            .with_path(path_label),
        );
        return (None, diags);
    }

    let mut def_file: DefTypeSchemaFile = match serde_json::from_value(value) {
        Ok(d) => d,
        Err(e) => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_def_type_json_invalid",
                    format!("Failed to deserialize def file: {}", e),
                )
                .with_pack_id(pack_id)
                .with_path(path_label),
            );
            return (None, diags);
        }
    };

    // Normalize Unrecognized field type kinds to Unknown, emitting warnings.
    for (field_name, field) in def_file.schema.fields.iter_mut() {
        if field.field_type.kind == FieldTypeKind::Unrecognized {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_invalid_field_type",
                    format!(
                        "Unrecognized field type kind in {}.{}. Field treated as unknown.",
                        def_file.def_type, field_name
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path_label)
                .with_field_path(format!("{}.fields.{}", def_file.def_type, field_name)),
            );
            field.field_type.kind = FieldTypeKind::Unknown;
        }
    }

    (Some(def_file), diags)
}

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
                .with_field_path(format!("{}.fields.{}", obj_file.object_type, field_name)),
            );
            field.field_type.kind = FieldTypeKind::Unknown;
        }
    }

    (Some(obj_file), diags)
}

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
            .with_path(path_label),
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
                .with_field_path(format!("{}.fields.{}", file.class_name, field_name)),
            );
            field.field_type.kind = FieldTypeKind::Unknown;
        }
    }

    (Some(file), diags)
}

/// Assemble a `SchemaPackManifest` from a parsed manifest file and lists of parsed def and object type files.
pub fn assemble_schema_pack(
    path_label: &str,
    manifest_file: SchemaPackManifestFile,
    def_files: &[(&str, &DefTypeSchemaFile)],
    object_files: &[(&str, &ObjectTypeSchemaFile)],
    patch_operation_files: &[(&str, &PatchOperationMetadataFile)],
) -> (Option<SchemaPackManifest>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();
    let pack_id = &manifest_file.pack_id;

    let mut def_types = BTreeMap::new();
    for (file_path, def_file) in def_files {
        let def_type = def_file.def_type.clone();
        if def_types.contains_key(&def_type) {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_duplicate_def_type",
                    format!(
                        "Duplicate defType '{}' found in pack '{}' at '{}'.",
                        def_type, pack_id, file_path
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(*file_path),
            );
            continue;
        }
        def_types.insert(def_type, def_file.schema.clone());
    }

    let mut object_types = BTreeMap::new();
    for (file_path, obj_file) in object_files {
        let object_type = obj_file.object_type.clone();
        if object_types.contains_key(&object_type) {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_duplicate_object_type",
                    format!(
                        "Duplicate objectType '{}' found in pack '{}' at '{}'.",
                        object_type, pack_id, file_path
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(*file_path),
            );
            continue;
        }
        object_types.insert(object_type, obj_file.schema.clone());
    }

    let mut patch_operations = BTreeMap::new();
    for (file_path, op_file) in patch_operation_files {
        let class_name = op_file.class_name.clone();
        if patch_operations.contains_key(&class_name) {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "patch_operation_metadata_duplicate_class_name_in_pack",
                    format!(
                        "Duplicate patch operation className '{}' found in pack '{}' at '{}'.",
                        class_name, pack_id, file_path
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(*file_path),
            );
            continue;
        }
        patch_operations.insert(class_name, op_file.schema.clone());
    }

    let manifest = SchemaPackManifest {
        format_version: manifest_file.format_version,
        pack_id: manifest_file.pack_id,
        name: manifest_file.name,
        version: manifest_file.version,
        game_version: manifest_file.game_version,
        rimedit_version: manifest_file.rimedit_version,
        author: manifest_file.author,
        priority: manifest_file.priority,
        dependencies: manifest_file.dependencies,
        def_types,
        object_types,
        patch_operations,
    };

    let _ = path_label;
    (Some(manifest), diags)
}

fn discover_manifest_paths_in_root(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // <root>/schema-pack.json
    let direct = root.join("schema-pack.json");
    if direct.is_file() && !is_symlink(&direct) {
        paths.push(direct);
    }

    // <root>/About/schema-pack.json
    let about = root.join("About").join("schema-pack.json");
    if about.is_file() && !is_symlink(&about) {
        paths.push(about);
    }

    // <root>/SchemaPacks/<name>/schema-pack.json
    let schema_packs_dir = root.join("SchemaPacks");
    if schema_packs_dir.is_dir() && !is_symlink(&schema_packs_dir) {
        if let Ok(entries) = std::fs::read_dir(&schema_packs_dir) {
            for entry in entries.flatten() {
                let sub = entry.path();
                if sub.is_dir() && !is_symlink(&sub) {
                    let candidate = sub.join("schema-pack.json");
                    if candidate.is_file() && !is_symlink(&candidate) {
                        paths.push(candidate);
                    }
                }
            }
        }
    }

    paths
}

fn resolve_manifest_relative_dir(manifest_dir: &Path, entry: &str) -> Option<PathBuf> {
    // Reject absolute entries - they would replace manifest_dir entirely when joined.
    if Path::new(entry).is_absolute() {
        return None;
    }
    let candidate = manifest_dir.join(entry);
    // Reject paths that contain '..' after joining - they might escape the pack root.
    for component in candidate.components() {
        if component == Component::ParentDir {
            return None;
        }
    }
    Some(candidate)
}

/// Walk `dir` recursively, collecting all `.json` files at any depth.
/// Symlinks are skipped at every level. Callers sort the result for determinism.
fn collect_json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_json_files_recursive(dir, &mut files);
    files
}

fn collect_json_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_symlink(&path) {
                continue;
            }
            if path.is_file() {
                if path.extension().and_then(|x| x.to_str()) == Some("json") {
                    files.push(path);
                }
            } else if path.is_dir() {
                collect_json_files_recursive(&path, files);
            }
        }
    }
}

fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}
