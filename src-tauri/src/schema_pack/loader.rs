use super::model::{
    DefTypeSchemaFile, FieldTypeKind, FormViewDef, ObjectTypeSchemaFile,
    PatchOperationMetadataFile, SchemaLoadDiagnostic, SchemaPackManifest, SchemaPackManifestFile,
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
                let (def_opt, ddiags) =
                    parse_def_type_schema(label, &pack_id, raw, manifest_file.format_version);
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
/// `manifest_format_version` is the owning pack's manifest `formatVersion`: it gates whether
/// `formViews` is even attempted on this Def type (see below) and must therefore be resolved by
/// the caller (from the pack's already-parsed manifest) before this function runs.
pub fn parse_def_type_schema(
    path_label: &str,
    pack_id: &str,
    raw_json: &str,
    manifest_format_version: u16,
) -> (Option<DefTypeSchemaFile>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();

    let mut value: serde_json::Value = match serde_json::from_str(raw_json) {
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

    // formViews requires manifest formatVersion 3 (Plan.md section 4). This gate MUST run before
    // any v3-only structural validation: a v1/v2 pack's formViews (whether well-formed, malformed
    // shape, or semantically invalid -- e.g. the reserved "default" id) is simply not a supported
    // feature on that pack version. It is a contract violation worth a diagnostic, but must not
    // sink the rest of the Def-type file (fields/templates/etc. still load), and the below
    // whole-file-fatal structural checks must never even run against content this pack version
    // doesn't support. Detect + report + strip the key here, before full struct deserialization,
    // so garbage formViews content on an old pack can't cause a spurious whole-file "malformed
    // JSON" failure either.
    if manifest_format_version < 3 && value_has_nonempty_form_views(&value) {
        diags.push(
            SchemaLoadDiagnostic::error(
                "schema_pack_form_views_requires_v3",
                format!(
                    "Def type '{}' declares formViews, but the pack manifest formatVersion is {}. formViews requires formatVersion 3; all formViews declarations for this Def type are ignored.",
                    def_type, manifest_format_version
                ),
            )
            .with_pack_id(pack_id)
            .with_path(path_label)
            .with_field_path(format!("{}.formViews", def_type)),
        );
        if let Some(obj) = value.as_object_mut() {
            obj.remove("formViews");
        }
    }

    let mut def_file: DefTypeSchemaFile = if manifest_format_version < 3 {
        // `formViews` has already been stripped above if it was present -- a v1/v2 pack never
        // reaches the v3-only duplicate-id detection below, so deserializing from the
        // already-built `value` (rather than raw text) loses nothing relevant here.
        match serde_json::from_value(value) {
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
        }
    } else {
        // Deserialize directly from the original JSON text, not from the already-built `value`
        // above: `serde_json::Value`'s `Map` construction silently collapses a duplicate object
        // key (e.g. two `"formViews": { "weapon": {...}, "weapon": {...} }` entries) before we
        // would ever see it. Deserializing straight from `raw_json` lets
        // `DefTypeSchemaDef.form_views`'s custom `deserialize_form_views` (see model.rs) observe
        // genuine duplicate keys via the flatten buffering's `MapAccess` and reject them as an
        // ordinary deserialize error here.
        //
        // A structured field path (e.g. via `serde_path_to_error`) was investigated for a
        // deserialize-level failure under `formViews` (e.g. `"label": 1` instead of a string),
        // but `serde_path_to_error` cannot see through `DefTypeSchemaFile`'s `#[serde(flatten)]`
        // field: flatten's derive-generated code buffers the whole object into an internal
        // `Content` tree via a plain (untracked) deserialize, then redistributes each field from
        // that buffer with a fresh, untracked deserializer -- so path tracking is lost the moment
        // it crosses the flatten boundary, for any field, not just `formViews` (confirmed
        // empirically: wrapping still produced `path() == "."`). Reworking `DefTypeSchemaFile`
        // away from `flatten` to fix this would touch every field on `DefTypeSchemaDef`, which is
        // disproportionate for this one diagnostic's precision. `serde_json::Error`'s `Display`
        // already includes a line/column ("at line N column M"), which is included in the message
        // below -- an acceptable fallback location hint without a structured field path.
        match serde_json::from_str(raw_json) {
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

    // Per Plan.md section 5, a malformed formViews shape, blank/reserved id, blank/missing
    // label, impossible `disabled` combination, or contradictory/duplicate field list is fatal
    // for the whole v3 Def schema file -- not a recoverable per-declaration skip. Mirror the same
    // whole-file-rejection mechanism used above for a genuinely malformed def file. Only reached
    // for a confirmed v3+ pack: a v1/v2 pack's formViews was already stripped to empty above, so
    // this loop body never runs for it.
    if manifest_format_version >= 3 && !def_file.schema.form_views.is_empty() {
        let form_view_diags = validate_form_view_declarations(
            &def_file.def_type,
            pack_id,
            path_label,
            &def_file.schema.form_views,
        );
        if !form_view_diags.is_empty() {
            diags.extend(form_view_diags);
            return (None, diags);
        }
    }

    (Some(def_file), diags)
}

/// Whether `value`'s top-level `formViews` key is present with meaningful content: only a wholly
/// absent key or an explicit empty object count as "no formViews" (nothing to gate or report,
/// since both declare zero views). Any other shape counts as present -- including an explicit
/// `null`: `null` is NOT equivalent to "key absent" here. `#[serde(default)]` on
/// `DefTypeSchemaDef.form_views` only substitutes a default when the key is missing entirely, not
/// when it is present-but-null, so an unstripped `formViews: null` would otherwise reach
/// `serde_json::from_value` and fail to deserialize into `BTreeMap<String, FormViewDef>`, taking
/// the whole v1/v2 Def-type file down with it (the same whole-file-loss bug a non-object shape
/// like an array would cause if left unstripped). A non-empty object also counts as present, same
/// as before.
fn value_has_nonempty_form_views(value: &serde_json::Value) -> bool {
    match value.get("formViews") {
        None => false,
        Some(serde_json::Value::Object(map)) => !map.is_empty(),
        Some(_) => true,
    }
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
    let mut def_type_source_paths: BTreeMap<String, String> = BTreeMap::new();
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

        // formViews manifest-version gating and v3 structural validation both already happened
        // in `parse_def_type_schema` (which needs the manifest's format version to decide whether
        // to even attempt the v3-only checks -- see that function's doc comment). By the time a
        // `DefTypeSchemaFile` reaches assembly, its `form_views` is already fully valid: empty for
        // a v1/v2 pack (stripped at parse time with a `schema_pack_form_views_requires_v3`
        // diagnostic already emitted), or fully validated for a v3+ pack.
        def_type_source_paths.insert(def_type.clone(), file_path.to_string());
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
        def_type_source_paths,
    };

    let _ = path_label;
    (Some(manifest), diags)
}

/// Return the first value in `items` that also appears earlier in `items` (a within-array
/// duplicate), or `None` if every entry is unique. Used to reject e.g. `["apparel", "apparel"]`
/// inside a single declaration's `hiddenFields`/`unhideFields`.
fn first_duplicate_in_list(items: &[String]) -> Option<&str> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for item in items {
        if !seen.insert(item.as_str()) {
            return Some(item.as_str());
        }
    }
    None
}

/// Validate one Def-type file's own `formViews` declarations for internal shape/consistency:
/// blank/reserved id, blank/missing label, impossible `disabled` combinations, and contradictory
/// or duplicate-within-array `hiddenFields`/`unhideFields`. Every one of these is listed in
/// Plan.md section 5 as fatal for the whole v3 Def schema file, so this returns diagnostics only
/// (not a sanitized map): `parse_def_type_schema` rejects the entire file -- the same mechanism
/// already used for a genuinely malformed def file -- the moment this returns anything non-empty,
/// rather than dropping just the offending declaration and keeping the rest.
///
/// A duplicate view id (two declarations sharing one JSON key) is caught earlier, during
/// deserialization itself, by `model::deserialize_form_views` -- by the time a `formViews` map
/// reaches this function, its keys are already known-unique.
///
/// Deliberately out of scope here (issue 03's job): resolving `hiddenFields`/`unhideFields`
/// deltas against an inherited base, validating field ids against the real known field universe,
/// and cross-pack/cross-type view precedence. This function only looks at one Def type's own
/// declarations in isolation.
fn validate_form_view_declarations(
    def_type: &str,
    pack_id: &str,
    file_path: &str,
    form_views: &BTreeMap<String, FormViewDef>,
) -> Vec<SchemaLoadDiagnostic> {
    let mut diags = Vec::new();

    for (id, view) in form_views {
        let field_path = format!("{}.formViews.{}", def_type, id);

        if id.trim().is_empty() {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_form_view_blank_id",
                    format!(
                        "Def type '{}' has a formViews entry with a blank/whitespace-only id.",
                        def_type
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(file_path)
                .with_field_path(field_path),
            );
            continue;
        }

        if id == "default" {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_form_view_reserved_id",
                    format!(
                        "Def type '{}' formViews id 'default' is reserved for the synthetic Default View and cannot be used as a schema-declared view id.",
                        def_type
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(file_path)
                .with_field_path(field_path),
            );
            continue;
        }

        // "View-defining" metadata beyond the delta-only controls (hiddenFields/unhideFields/
        // replace/disabled). A declaration carrying only delta controls (or `disabled: true`
        // alone) is a legitimate amendment to an inherited view and needs no label of its own --
        // issue 03 resolves it against the inherited base. A declaration carrying any of these is
        // treated as defining a view outright and must have a nonblank label.
        let has_view_metadata = view.description.is_some()
            || view.icon.is_some()
            || view.order.is_some()
            || view.recommended.is_some();
        let has_delta_content = view.hidden_fields.is_some()
            || view.unhide_fields.is_some()
            || view.replace.is_some()
            || view.disabled.is_some();

        // `disabled: true` combined with any other meaningful content is an impossible
        // declaration (Plan.md section 5). This check must run before the label checks below:
        // e.g. `{ "disabled": true, "description": "..." }` must be diagnosed as
        // disabled-with-content, not misdiagnosed as a missing label.
        if view.disabled == Some(true) {
            let other_content = view.label.is_some()
                || has_view_metadata
                || view.hidden_fields.is_some()
                || view.unhide_fields.is_some()
                || view.replace.is_some();
            if other_content {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_disabled_with_content",
                        format!(
                            "Def type '{}' formViews entry '{}' sets disabled: true but also declares other content; disabled must be the only meaningful field on the declaration.",
                            def_type, id
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path),
                );
                continue;
            }
        }

        if view.label.is_none() && !has_view_metadata && !has_delta_content {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_form_view_empty_declaration",
                    format!(
                        "Def type '{}' formViews entry '{}' has no label and no other content (hiddenFields/unhideFields/replace/disabled/description/icon/order/recommended); the declaration is meaningless.",
                        def_type, id
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(file_path)
                .with_field_path(field_path),
            );
            continue;
        }

        if let Some(label) = &view.label {
            if label.trim().is_empty() {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_blank_label",
                        format!(
                            "Def type '{}' formViews entry '{}' has a blank/whitespace-only label.",
                            def_type, id
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path),
                );
                continue;
            }
        } else if has_view_metadata {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_form_view_missing_label",
                    format!(
                        "Def type '{}' formViews entry '{}' declares description/icon/order/recommended (a new view) but has no label. A pure delta amendment (hiddenFields/unhideFields/replace/disabled only) may omit the label.",
                        def_type, id
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(file_path)
                .with_field_path(field_path),
            );
            continue;
        }

        if let Some(hidden) = &view.hidden_fields {
            if let Some(dup) = first_duplicate_in_list(hidden) {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_duplicate_hidden_field",
                        format!(
                            "Def type '{}' formViews entry '{}' lists field '{}' more than once in hiddenFields.",
                            def_type, id, dup
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path),
                );
                continue;
            }
        }

        if let Some(unhide) = &view.unhide_fields {
            if let Some(dup) = first_duplicate_in_list(unhide) {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_duplicate_unhide_field",
                        format!(
                            "Def type '{}' formViews entry '{}' lists field '{}' more than once in unhideFields.",
                            def_type, id, dup
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path),
                );
                continue;
            }
        }

        if let (Some(hidden), Some(unhide)) = (&view.hidden_fields, &view.unhide_fields) {
            let unhide_set: std::collections::BTreeSet<&str> =
                unhide.iter().map(String::as_str).collect();
            let conflicting: Vec<&str> = hidden
                .iter()
                .map(String::as_str)
                .filter(|f| unhide_set.contains(f))
                .collect();
            if !conflicting.is_empty() {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_form_view_conflicting_hidden_unhide",
                        format!(
                            "Def type '{}' formViews entry '{}' lists field(s) {:?} in both hiddenFields and unhideFields.",
                            def_type, id, conflicting
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(file_path)
                    .with_field_path(field_path),
                );
                continue;
            }
        }
    }

    diags
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
