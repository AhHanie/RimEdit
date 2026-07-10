use crate::project_files::{
    create_project_file, create_project_folder, delete_project_path, read_xml_file,
    rename_project_path, scan_all_project_files, ProjectFileContent, ProjectFileEntry,
    ProjectFileScan, ProjectFolderEntry, ProjectPathMutationResult,
};
use crate::project_model::AppError;
use crate::services::indexing::{self, IndexJobReason};
use crate::settings_store::load_settings;
use tauri::AppHandle;

#[tauri::command]
pub fn scan_project_files(app: AppHandle, project_id: String) -> Result<ProjectFileScan, AppError> {
    let _span = crate::instrumentation::span(&app, "commands.scanProjectFiles");
    let settings = load_settings(&app)?;
    scan_all_project_files(&settings, &project_id).map_err(Into::into)
}

#[tauri::command]
pub fn read_project_xml_file(
    app: AppHandle,
    project_id: String,
    relative_path: String,
) -> Result<ProjectFileContent, AppError> {
    let settings = load_settings(&app)?;
    read_xml_file(&settings, &project_id, &relative_path).map_err(Into::into)
}

#[tauri::command]
pub fn create_project_file_cmd(
    app: AppHandle,
    project_id: String,
    parent_path: String,
    file_name: String,
    contents: Option<String>,
) -> Result<ProjectFileEntry, AppError> {
    let settings = load_settings(&app)?;
    let entry = create_project_file(
        &settings,
        &project_id,
        &parent_path,
        &file_name,
        contents.as_deref(),
    )
    .map_err(AppError::from)?;
    if file_name.to_ascii_lowercase().ends_with(".xml") {
        indexing::enqueue_file_change(
            &app,
            project_id,
            entry.relative_path.clone(),
            IndexJobReason::ProjectFileMutation,
        );
    }
    Ok(entry)
}

#[tauri::command]
pub fn create_project_folder_cmd(
    app: AppHandle,
    project_id: String,
    parent_path: String,
    folder_name: String,
) -> Result<ProjectFolderEntry, AppError> {
    let settings = load_settings(&app)?;
    create_project_folder(&settings, &project_id, &parent_path, &folder_name).map_err(Into::into)
}

#[tauri::command]
pub fn rename_project_path_cmd(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    new_name: String,
    kind: String,
) -> Result<ProjectPathMutationResult, AppError> {
    let settings = load_settings(&app)?;
    let result = rename_project_path(&settings, &project_id, &relative_path, &new_name, &kind)
        .map_err(AppError::from)?;
    let is_xml = |p: &str| p.to_ascii_lowercase().ends_with(".xml");
    if kind == "file" {
        if is_xml(&relative_path) {
            indexing::enqueue_file_delete(
                &app,
                project_id.clone(),
                relative_path.clone(),
                IndexJobReason::ProjectFileMutation,
            );
        }
        if is_xml(&result.new_path) {
            indexing::enqueue_file_change(
                &app,
                project_id,
                result.new_path.clone(),
                IndexJobReason::ProjectFileMutation,
            );
        }
    } else {
        // Folder rename: full rebuild (simpler than per-file rename tracking)
        indexing::enqueue_full_rebuild(&app, Some(project_id), IndexJobReason::ProjectFileMutation);
    }
    Ok(result)
}

#[tauri::command]
pub fn delete_project_path_cmd(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    kind: String,
) -> Result<ProjectPathMutationResult, AppError> {
    let settings = load_settings(&app)?;
    let result = delete_project_path(&settings, &project_id, &relative_path, &kind)
        .map_err(AppError::from)?;
    if kind == "file" && relative_path.to_ascii_lowercase().ends_with(".xml") {
        indexing::enqueue_file_delete(
            &app,
            project_id,
            relative_path,
            IndexJobReason::ProjectFileMutation,
        );
    } else if kind == "folder" {
        indexing::enqueue_folder_delete(
            &app,
            project_id,
            relative_path,
            IndexJobReason::ProjectFileMutation,
        );
    }
    Ok(result)
}
