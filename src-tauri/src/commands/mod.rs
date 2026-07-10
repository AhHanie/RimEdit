mod create_def;
mod def_index;
mod def_templates;
mod graphic_preview;
mod instrumentation;
mod patches;
mod project_files;
mod project_settings;
mod project_validation;
mod schema_catalog;
mod xml_editor;
mod xml_save;

pub use create_def::create_def_from_template;
pub use def_index::{
    get_def_index_facets, get_indexing_status, query_def_duplicates, read_indexed_def_xml,
    rebuild_def_index, resolve_def_reference_cmd, search_defs, start_background_indexing,
    suggest_def_references_cmd,
};
pub use def_templates::{
    create_def_from_indexed_def, create_def_from_user_template, delete_user_def_template,
    list_user_def_templates, save_user_def_template,
};
pub use graphic_preview::resolve_graphic_preview_assets;
pub use instrumentation::{get_instrumentation_config, set_instrumentation_enabled};
pub use patches::{
    complete_patch_operation_xpath, parse_patch_operations, parse_patch_value_xml,
    preview_def_patches, query_patch_operations_for_def, rebuild_patch_index,
    serialize_patch_operations, serialize_patch_value_fragment,
};
pub use project_files::{
    create_project_file_cmd, create_project_folder_cmd, delete_project_path_cmd,
    read_project_xml_file, rename_project_path_cmd, scan_project_files,
};
pub use project_settings::{
    get_project_settings, remove_location, set_active_project, update_location,
    update_project_game_version, upsert_location,
};
pub use project_validation::validate_project;
pub use schema_catalog::{list_installed_schema_game_versions_cmd, load_schema_catalog};
pub use xml_editor::{
    apply_xml_editor_edit, apply_xml_editor_edits, parse_xml_editor_buffer,
    read_location_xml_editor_document, read_project_xml_document, read_project_xml_editor_document,
};
pub use xml_save::{preview_project_xml_save, save_project_xml_file};
