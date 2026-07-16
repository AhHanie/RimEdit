mod assemble;
mod def_types;
mod external_pack;
mod form_views;
mod fs_utils;
mod locales;
mod manifest;
mod object_types;
mod patch_operations;

use super::locale::SchemaLocaleOverlay;
use super::model::{
    DefTypeSchemaFile, ObjectTypeSchemaFile, PatchOperationMetadataFile, SchemaLoadDiagnostic,
    SchemaPackManifest,
};
use fs_utils::discover_manifest_paths_in_root;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub use assemble::assemble_schema_pack;
pub use def_types::parse_def_type_schema;
pub use external_pack::load_pack_from_directory;
pub(crate) use locales::parse_locale_bundle;
pub use manifest::parse_schema_pack_manifest;
pub use object_types::parse_object_type_schema;
pub use patch_operations::parse_patch_operation_metadata;

include!(concat!(env!("OUT_DIR"), "/built_in_schema_packs.rs"));

const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_DEF_FILE_BYTES: u64 = 1024 * 1024;
const MAX_DEF_FILES_PER_PACK: usize = 5000;
const MAX_LOCALE_FILE_BYTES: u64 = 256 * 1024;

pub struct LoadedPack {
    pub manifest: SchemaPackManifest,
    pub is_builtin: bool,
    pub source_path: Option<String>,
    /// This pack's own parsed locale sidecars, keyed by locale tag (e.g. `"en"`), each holding a
    /// flat `{resourceId -> override text}` map. Empty when the pack declares no
    /// `localesDirectory` or ships no sidecar files. See `schema_pack::locale`.
    pub locales: BTreeMap<String, SchemaLocaleOverlay>,
}

pub fn load_built_in_packs() -> (Vec<LoadedPack>, Vec<SchemaLoadDiagnostic>) {
    let mut diags = Vec::new();
    let mut packs = Vec::new();

    for (path_label, manifest_raw, def_files, obj_files, patch_op_files, locale_files) in
        BUILT_IN_SCHEMA_PACKS
    {
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
                let locales = parse_locale_bundle(&pack_id, locale_files, &mut diags);
                packs.push(LoadedPack {
                    manifest,
                    is_builtin: true,
                    source_path: None,
                    locales,
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
                .with_path(root.to_string_lossy())
                .with_args(crate::diagnostics::diagnostic_args([(
                    "rootPath",
                    root.to_string_lossy().into_owned().into(),
                )])),
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
