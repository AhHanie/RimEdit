mod commands;
mod def_index;
mod def_templates;
mod instrumentation;
mod patches;
mod project_files;
mod project_model;
mod project_save;
mod project_validation;
mod rimworld_load_folders;
mod schema_pack;
mod services;
mod settings_store;
mod xml_document;

use commands::{
    apply_xml_editor_edit, apply_xml_editor_edits, complete_patch_operation_xpath,
    create_def_from_indexed_def, create_def_from_template, create_def_from_user_template,
    create_project_file_cmd, create_project_folder_cmd, delete_project_path_cmd,
    delete_user_def_template, get_def_index_facets, get_indexing_status,
    get_instrumentation_config, get_project_settings, list_installed_schema_game_versions_cmd,
    list_user_def_templates, load_schema_catalog, parse_patch_operations, parse_patch_value_xml,
    parse_xml_editor_buffer, preview_def_patches, preview_project_xml_save, query_def_duplicates,
    query_patch_operations_for_def, read_indexed_def_xml, read_location_xml_editor_document,
    read_project_xml_document, read_project_xml_editor_document, read_project_xml_file,
    rebuild_def_index, rebuild_patch_index, remove_location, rename_project_path_cmd,
    resolve_def_reference_cmd, resolve_graphic_preview_assets, save_project_xml_file,
    save_user_def_template, scan_project_files, search_defs, serialize_patch_operations,
    serialize_patch_value_fragment, set_active_project, set_instrumentation_enabled,
    start_background_indexing, suggest_def_references_cmd, update_location,
    update_project_game_version, upsert_location, validate_project,
};
use def_index::DefIndexState;
use instrumentation::InstrumentationState;
use patches::{PatchFilesState, PatchIndexState};
use project_save::SaveValidationSecret;
use schema_pack::SchemaCatalogCacheState;
use services::graphic_preview::{self, AssetTokenCache};
use services::indexing::{IndexWatcherState, IndexingServiceState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let asset_cache = AssetTokenCache::default();
    tauri::Builder::default()
        .manage(asset_cache.clone())
        .manage(DefIndexState::default())
        .manage(PatchIndexState::default())
        .manage(PatchFilesState::default())
        .manage(InstrumentationState::from_env())
        .manage(SaveValidationSecret::default())
        .manage(IndexingServiceState::new())
        .manage(IndexWatcherState::default())
        .manage(SchemaCatalogCacheState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("rimedit-asset", move |_ctx, request| {
            let token = {
                let uri = request.uri();
                graphic_preview::extract_asset_token(uri.host().unwrap_or(""), uri.path())
            };

            let Some(token) = token else {
                return tauri::http::Response::builder()
                    .status(404)
                    .header("cache-control", "no-store")
                    .body(b"Not Found".to_vec())
                    .unwrap();
            };

            match graphic_preview::read_preview_asset(&asset_cache, &token) {
                Ok((bytes, content_type)) => tauri::http::Response::builder()
                    .status(200)
                    .header("content-type", content_type)
                    .header("cache-control", "no-store")
                    .body(bytes)
                    .unwrap(),
                Err(ref e) if e.code == "UNSUPPORTED_FORMAT" => tauri::http::Response::builder()
                    .status(415)
                    .header("cache-control", "no-store")
                    .body(b"Unsupported texture format".to_vec())
                    .unwrap(),
                Err(ref e) if e.code == "TOKEN_NOT_FOUND" => tauri::http::Response::builder()
                    .status(404)
                    .header("cache-control", "no-store")
                    .body(b"Not Found".to_vec())
                    .unwrap(),
                Err(_) => tauri::http::Response::builder()
                    .status(500)
                    .header("cache-control", "no-store")
                    .body(b"Internal Server Error".to_vec())
                    .unwrap(),
            }
        })
        .setup(|app| {
            let handle = app.handle();
            if let Err(e) = services::indexing::start_worker(handle) {
                eprintln!("[rimedit] Failed to start indexing worker: {}", e.message);
            }
            if let Ok(settings) = settings_store::load_settings(handle) {
                // Skip watcher/index startup for an active project whose folder no
                // longer exists, without persisting the deactivation here -- that
                // stays owned by `get_project_settings`, which the frontend calls
                // on load and which surfaces the "folder not found" notice to the
                // user. Mutating and saving here would silently clear the stale id
                // before the frontend ever learns about it.
                let mut indexing_settings = settings.clone();
                services::project_settings::deactivate_missing_active_project(
                    &mut indexing_settings,
                );
                let _ = services::indexing::restart_for_settings(handle, &indexing_settings);
                if let Some(ref pid) = indexing_settings.active_project_id {
                    services::indexing::enqueue_full_rebuild(
                        handle,
                        Some(pid.clone()),
                        services::indexing::IndexJobReason::InitialProjectOpen,
                    );
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_project_settings,
            upsert_location,
            remove_location,
            set_active_project,
            update_location,
            update_project_game_version,
            list_installed_schema_game_versions_cmd,
            scan_project_files,
            read_project_xml_file,
            create_project_file_cmd,
            create_project_folder_cmd,
            rename_project_path_cmd,
            delete_project_path_cmd,
            read_project_xml_document,
            read_project_xml_editor_document,
            read_location_xml_editor_document,
            parse_xml_editor_buffer,
            apply_xml_editor_edit,
            apply_xml_editor_edits,
            preview_project_xml_save,
            save_project_xml_file,
            load_schema_catalog,
            validate_project,
            rebuild_def_index,
            query_def_duplicates,
            search_defs,
            get_def_index_facets,
            suggest_def_references_cmd,
            resolve_def_reference_cmd,
            read_indexed_def_xml,
            create_def_from_template,
            get_indexing_status,
            start_background_indexing,
            resolve_graphic_preview_assets,
            get_instrumentation_config,
            set_instrumentation_enabled,
            list_user_def_templates,
            delete_user_def_template,
            save_user_def_template,
            create_def_from_user_template,
            create_def_from_indexed_def,
            rebuild_patch_index,
            query_patch_operations_for_def,
            parse_patch_operations,
            serialize_patch_operations,
            complete_patch_operation_xpath,
            parse_patch_value_xml,
            serialize_patch_value_fragment,
            preview_def_patches,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
