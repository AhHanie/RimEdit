use crate::def_index::{
    get_facet_summary, resolve_def_reference, search_def_results, suggest_def_references,
    DefDuplicateQueryResult, DefIndexFacetSummary, DefIndexSearchQuery, DefIndexSummary,
    DefReferenceResolution, DefReferenceSuggestion, IndexedDefSearchResult, IndexedSourceKind,
    IndexingPhase, IndexingStatus,
};
use crate::project_files::validate_and_resolve_location;
use crate::project_model::AppError;
use crate::schema_pack::ReferenceScope;
use crate::services::def_index_cache;
use crate::services::indexing::{self, IndexJobReason};
use crate::settings_store::load_settings;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefXmlPreview {
    pub raw_xml: String,
    pub def_line: Option<usize>,
}

#[tauri::command]
pub fn rebuild_def_index(
    app: AppHandle,
    project_id: Option<String>,
) -> Result<DefIndexSummary, AppError> {
    let _span = crate::instrumentation::span_with_tags(
        &app,
        "commands.rebuildDefIndex",
        [(
            "projectPresent".to_string(),
            project_id.is_some().to_string(),
        )],
    );
    let settings = load_settings(&app)?;
    def_index_cache::rebuild_for_project(&app, &settings, project_id.as_deref())
}

#[tauri::command]
pub fn get_indexing_status(app: AppHandle) -> IndexingStatus {
    indexing::get_indexing_status(&app)
}

#[tauri::command]
pub fn start_background_indexing(
    app: AppHandle,
    project_id: Option<String>,
) -> Result<IndexingStatus, AppError> {
    let settings = load_settings(&app)?;
    let effective_id = project_id.or_else(|| settings.active_project_id.clone());
    let current = indexing::get_indexing_status(&app);
    // Skip if already pending or running for the same project - avoids a double scan
    // when the backend setup and the frontend hook both call this at startup.
    let already_in_flight = (current.phase == IndexingPhase::Pending
        || current.phase == IndexingPhase::Running)
        && current.project_id == effective_id;
    if !already_in_flight {
        indexing::enqueue_full_rebuild(&app, effective_id, IndexJobReason::InitialProjectOpen);
    }
    Ok(indexing::get_indexing_status(&app))
}

#[tauri::command]
pub fn query_def_duplicates(
    app: AppHandle,
    project_id: String,
    def_type: String,
    def_name: String,
) -> Result<DefDuplicateQueryResult, AppError> {
    let settings = load_settings(&app)?;
    let index = def_index_cache::load_for_project(&app, &settings, &project_id, false)?;
    let matches = index.find_all_duplicates(&def_type, &def_name);
    let project_occurrences = index
        .find_project_duplicates(&def_type, &def_name)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let source_occurrences = matches
        .iter()
        .filter(|d| d.source.source_kind == IndexedSourceKind::Source)
        .map(|d| (*d).clone())
        .collect::<Vec<_>>();
    Ok(DefDuplicateQueryResult {
        blocking_project_duplicate: project_occurrences.len() > 1,
        source_duplicate_warning: !source_occurrences.is_empty(),
        project_occurrences,
        source_occurrences,
    })
}

#[tauri::command]
pub fn get_def_index_facets(
    app: AppHandle,
    project_id: String,
    include_sources: Option<bool>,
) -> Result<DefIndexFacetSummary, AppError> {
    let settings = load_settings(&app)?;
    let index = def_index_cache::load_for_project_query(&app, &settings, &project_id)?;
    Ok(get_facet_summary(&index, include_sources.unwrap_or(true)))
}

#[tauri::command]
pub fn search_defs(
    app: AppHandle,
    project_id: String,
    query: String,
    def_type: Option<String>,
    include_sources: Option<bool>,
    limit: Option<usize>,
) -> Result<Vec<IndexedDefSearchResult>, AppError> {
    let _span = crate::instrumentation::span_with_tags(
        &app,
        "commands.searchDefs",
        [("queryLength".to_string(), query.len().to_string())],
    );
    let settings = load_settings(&app)?;
    let index = def_index_cache::load_for_project_query(&app, &settings, &project_id)?;
    Ok(search_def_results(
        &index,
        &DefIndexSearchQuery {
            query,
            def_type,
            include_sources: include_sources.unwrap_or(true),
            limit,
        },
    ))
}

#[tauri::command]
pub fn suggest_def_references_cmd(
    app: AppHandle,
    project_id: String,
    target_def_types: Vec<String>,
    query: String,
    scope: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<DefReferenceSuggestion>, AppError> {
    let settings = load_settings(&app)?;
    let index = def_index_cache::load_for_project_query(&app, &settings, &project_id)?;
    let scope = match scope.as_deref() {
        Some("projectOnly") => ReferenceScope::ProjectOnly,
        Some("samePack") => ReferenceScope::SamePack,
        _ => ReferenceScope::AllSources,
    };
    let types: Vec<&str> = target_def_types.iter().map(String::as_str).collect();
    Ok(suggest_def_references(
        &index,
        &types,
        &query,
        &scope,
        limit.unwrap_or(25),
    ))
}

#[tauri::command]
pub fn resolve_def_reference_cmd(
    app: AppHandle,
    project_id: String,
    target_def_types: Vec<String>,
    def_name: String,
    scope: Option<String>,
) -> Result<DefReferenceResolution, AppError> {
    let settings = load_settings(&app)?;
    let index = def_index_cache::load_for_project_query(&app, &settings, &project_id)?;
    let scope = match scope.as_deref() {
        Some("projectOnly") => ReferenceScope::ProjectOnly,
        Some("samePack") => ReferenceScope::SamePack,
        _ => ReferenceScope::AllSources,
    };
    let types: Vec<&str> = target_def_types.iter().map(String::as_str).collect();
    Ok(resolve_def_reference(&index, &types, &def_name, &scope))
}

#[tauri::command]
pub fn read_indexed_def_xml(
    app: AppHandle,
    project_id: String,
    location_id: String,
    relative_path: String,
    def_type: String,
    def_name: String,
) -> Result<DefXmlPreview, AppError> {
    let settings = load_settings(&app)?;
    // Require project_id to be a known location so callers cannot probe arbitrary paths.
    if !settings.locations.iter().any(|l| l.id == project_id) {
        return Err(AppError {
            code: "project_not_found".to_string(),
            message: format!("No registered project with id '{}'.", project_id),
            details: None,
            args: crate::diagnostics::diagnostic_args([("projectId", project_id.into())]),
        });
    }
    // Validate relative_path is inside the registered location root (rejects traversal, absolutes,
    // non-XML extensions, and files outside the root after canonicalization).
    let canonical = validate_and_resolve_location(&settings, &location_id, &relative_path)
        .map_err(AppError::from)?;
    // Verify the requested Def is actually indexed at this location and path.
    let index = def_index_cache::load_for_project_query(&app, &settings, &project_id)?;
    let def_entry = index
        .defs
        .iter()
        .find(|d| {
            d.source.location_id == location_id
                && d.relative_path == relative_path
                && d.def_type == def_type
                && d.def_name == def_name
        })
        .ok_or_else(|| {
            AppError {
                code: "def_not_indexed".to_string(),
                message: format!(
                    "'{}' ({}) was not found in the index at '{}'.",
                    def_name, def_type, relative_path
                ),
                details: None,
                args: crate::diagnostics::DiagnosticArgs::new(),
            }
            .with_args(crate::diagnostics::diagnostic_args([
                ("defName", def_name.as_str().into()),
                ("defType", def_type.as_str().into()),
                ("relativePath", relative_path.as_str().into()),
            ]))
        })?;
    let def_line = def_entry.line;
    let raw_xml = std::fs::read_to_string(&canonical).map_err(|e| AppError {
        code: "file_read_error".to_string(),
        message: format!("Failed to read '{}': {}", canonical.display(), e),
        details: None,
        args: crate::diagnostics::diagnostic_args([(
            "path",
            canonical.to_string_lossy().into_owned().into(),
        )]),
    })?;
    Ok(DefXmlPreview { raw_xml, def_line })
}
