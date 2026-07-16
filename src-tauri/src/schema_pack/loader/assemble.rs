use crate::schema_pack::model::{
    DefTypeSchemaFile, ObjectTypeSchemaFile, PatchOperationMetadataFile, SchemaLoadDiagnostic,
    SchemaPackManifest, SchemaPackManifestFile,
};
use std::collections::BTreeMap;

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
                .with_path(*file_path)
                .with_args(crate::diagnostics::diagnostic_args([(
                    "defType",
                    def_type.as_str().into(),
                )])),
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
                .with_path(*file_path)
                .with_args(crate::diagnostics::diagnostic_args([(
                    "objectType",
                    object_type.as_str().into(),
                )])),
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
                .with_path(*file_path)
                .with_args(crate::diagnostics::diagnostic_args([(
                    "className",
                    class_name.as_str().into(),
                )])),
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
