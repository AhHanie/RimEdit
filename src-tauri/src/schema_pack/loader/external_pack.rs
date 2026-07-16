use super::assemble::assemble_schema_pack;
use super::def_types::parse_def_type_schema;
use super::fs_utils::{collect_json_files, is_symlink, resolve_manifest_relative_dir};
use super::locales::{parse_locale_bundle, read_locale_directory_files};
use super::manifest::parse_schema_pack_manifest;
use super::object_types::parse_object_type_schema;
use super::patch_operations::parse_patch_operation_metadata;
use super::{LoadedPack, MAX_DEF_FILES_PER_PACK, MAX_DEF_FILE_BYTES, MAX_MANIFEST_BYTES};
use crate::schema_pack::model::{
    DefTypeSchemaFile, ObjectTypeSchemaFile, PatchOperationMetadataFile, SchemaLoadDiagnostic,
};
use std::path::Path;

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
                            "defTypeDirectories entry '{}' escapes the pack root - skipping.",
                            dir_entry
                        ),
                    )
                    .with_pack_id(&pack_id)
                    .with_path(&path_label)
                    .with_args(crate::diagnostics::diagnostic_args([(
                        "directoryEntry",
                        dir_entry.as_str().into(),
                    )])),
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
                .with_path(resolved.to_string_lossy())
                .with_args(crate::diagnostics::diagnostic_args([(
                    "directory",
                    resolved.to_string_lossy().into_owned().into(),
                )])),
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
                    .with_pack_id(&pack_id)
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("packId", pack_id.as_str().into()),
                        ("maxFiles", MAX_DEF_FILES_PER_PACK.into()),
                    ])),
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
                    let (def_opt, ddiags) = parse_def_type_schema(
                        &file_label,
                        &pack_id,
                        &raw,
                        manifest_file.format_version,
                    );
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
                            "objectTypeDirectories entry '{}' escapes the pack root - skipping.",
                            dir_entry
                        ),
                    )
                    .with_pack_id(&pack_id)
                    .with_path(&path_label)
                    .with_args(crate::diagnostics::diagnostic_args([(
                        "directoryEntry",
                        dir_entry.as_str().into(),
                    )])),
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
                .with_path(resolved.to_string_lossy())
                .with_args(crate::diagnostics::diagnostic_args([(
                    "directory",
                    resolved.to_string_lossy().into_owned().into(),
                )])),
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
                    .with_pack_id(&pack_id)
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("packId", pack_id.as_str().into()),
                        ("maxFiles", MAX_DEF_FILES_PER_PACK.into()),
                    ])),
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
                            "patchOperationDirectories entry '{}' escapes the pack root - skipping.",
                            dir_entry
                        ),
                    )
                    .with_pack_id(&pack_id)
                    .with_path(&path_label)
                    .with_args(crate::diagnostics::diagnostic_args([(
                        "directoryEntry",
                        dir_entry.as_str().into(),
                    )])),
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
                .with_path(resolved.to_string_lossy())
                .with_args(crate::diagnostics::diagnostic_args([(
                    "directory",
                    resolved.to_string_lossy().into_owned().into(),
                )])),
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
                    .with_pack_id(&pack_id)
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("packId", pack_id.as_str().into()),
                        ("maxFiles", MAX_DEF_FILES_PER_PACK.into()),
                    ])),
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

    let locale_file_contents = read_locale_directory_files(
        &manifest_dir,
        &pack_id,
        &path_label,
        manifest_file.locales_directory.as_deref(),
        &mut diags,
    );
    let locale_refs: Vec<(&str, &str)> = locale_file_contents
        .iter()
        .map(|(l, r)| (l.as_str(), r.as_str()))
        .collect();
    let locales = parse_locale_bundle(&pack_id, &locale_refs, &mut diags);

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
        locales,
    });

    (pack, diags)
}
