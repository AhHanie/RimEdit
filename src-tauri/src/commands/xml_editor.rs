use crate::project_files::read_xml_file;
use crate::project_model::AppError;
use crate::services::{validation, xml_editor as xml_editor_service};
use crate::settings_store::load_settings;
use crate::xml_document::{
    parse_to_document, parse_xml_document, XmlDocumentLoadResult, XmlEdit, XmlEditContext,
    XmlEditorDocumentLoadResult,
};
use tauri::AppHandle;

// Every command below is `async` because each one (transitively) calls into
// `services::xml_editor`/`services::validation`'s interactive document paths
// (`validate_doc_for_project`, `read_editor_document`, etc.), which load the def index under
// `IndexLoadPolicy::Interactive` -- never blocking on a project-wide file scan, but still async
// because a cache-miss hydration runs via `spawn_blocking`. Tauri runs non-`async` commands on the
// main WebView thread, so leaving any of these synchronous would freeze the UI for that duration.
// Save/commit and explicit-validation paths use `IndexLoadPolicy::RequireFresh` directly (see
// `commands::xml_save`, `commands::project_validation`) and do not go through this file.

#[tauri::command]
pub async fn read_project_xml_document(
    app: AppHandle,
    project_id: String,
    relative_path: String,
) -> Result<XmlDocumentLoadResult, AppError> {
    let settings = load_settings(&app)?;
    let content = read_xml_file(&settings, &project_id, &relative_path).map_err(AppError::from)?;
    let mut result = parse_xml_document(&relative_path, &content.contents);
    if result.document.is_some() {
        let mut doc = parse_to_document(&relative_path, &content.contents);
        validation::validate_doc_for_project(
            &app,
            &settings,
            &project_id,
            &relative_path,
            &mut doc,
        )
        .await?;
        result.validation_diagnostics = doc.validation_diagnostics;
    }
    result.project_id = project_id;
    Ok(result)
}

#[tauri::command]
pub async fn read_project_xml_editor_document(
    app: AppHandle,
    project_id: String,
    relative_path: String,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let _span = crate::instrumentation::span_with_tags(
        &app,
        "commands.readProjectXmlEditorDocument",
        [
            ("relativePath".to_string(), relative_path.clone()),
            ("sourceKind".to_string(), "project".to_string()),
        ],
    );
    xml_editor_service::read_editor_document(&app, project_id, relative_path).await
}

#[tauri::command]
pub async fn read_location_xml_editor_document(
    app: AppHandle,
    project_id: String,
    location_id: String,
    relative_path: String,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let _span = crate::instrumentation::span_with_tags(
        &app,
        "commands.readLocationXmlEditorDocument",
        [
            ("relativePath".to_string(), relative_path.clone()),
            ("sourceKind".to_string(), "source".to_string()),
        ],
    );
    xml_editor_service::read_location_editor_document(&app, project_id, location_id, relative_path)
        .await
}

#[tauri::command]
pub async fn parse_xml_editor_buffer(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let _span = crate::instrumentation::span_with_tags(
        &app,
        "commands.parseXmlEditorBuffer",
        [("relativePath".to_string(), relative_path.clone())],
    );
    xml_editor_service::parse_editor_buffer(&app, project_id, relative_path, raw_xml).await
}

#[tauri::command]
pub async fn apply_xml_editor_edit(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
    edit: XmlEdit,
    edit_context: Option<XmlEditContext>,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let _span = crate::instrumentation::span_with_tags(
        &app,
        "commands.applyXmlEditorEdit",
        [("relativePath".to_string(), relative_path.clone())],
    );
    xml_editor_service::apply_editor_edits(
        &app,
        project_id,
        relative_path,
        raw_xml,
        vec![edit],
        edit_context,
    )
    .await
}

#[tauri::command]
pub async fn apply_xml_editor_edits(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
    edits: Vec<XmlEdit>,
    edit_context: Option<XmlEditContext>,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let _span = crate::instrumentation::span_with_tags(
        &app,
        "commands.applyXmlEditorEdits",
        [
            ("relativePath".to_string(), relative_path.clone()),
            ("batchSize".to_string(), edits.len().to_string()),
        ],
    );
    xml_editor_service::apply_editor_edits(
        &app,
        project_id,
        relative_path,
        raw_xml,
        edits,
        edit_context,
    )
    .await
}
