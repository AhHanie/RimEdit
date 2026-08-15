use crate::def_index::{
    get_facet_summary, resolve_def_reference, search_def_results, suggest_def_references,
    DefDuplicateQueryResult, DefIndexError, DefIndexFacetSummary, DefIndexSearchQuery,
    DefIndexSummary, DefReferenceResolution, DefReferenceSuggestion, IndexedDefSearchResult,
    IndexedSourceKind, IndexingStatus,
};
use crate::project_files::validate_and_resolve_location;
use crate::project_model::AppError;
use crate::schema_pack::ReferenceScope;
use crate::services::def_index_cache;
use crate::services::indexing;
use crate::settings_store::load_settings;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefXmlPreview {
    pub raw_xml: String,
    pub def_line: Option<usize>,
}

/// Maximum number of `DefIndexError` records returned by `get_def_index_errors` in a single
/// response, so a badly broken collection cannot create an unbounded frontend payload.
const DEF_INDEX_ERRORS_LIMIT: usize = 200;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefIndexErrorsResponse {
    pub errors: Vec<DefIndexError>,
    pub total: usize,
    pub truncated: bool,
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

/// Idempotent "ensure indexing is scheduled" command: if a matching index is already in
/// `DefIndexState` (including an unverified hydrated cache from `.setup()`), or a matching
/// initialization/rebuild is already pending/running/in flight, this does nothing beyond
/// returning the current status. Otherwise it delegates to the same single-flight
/// `schedule_initialization` helper `.setup()` and `IndexLoadPolicy::Interactive` loads use, so
/// setup, this command, and interactive editor opens can never race each other into starting
/// duplicate hydrations or full rebuilds for the same project/generation.
///
/// `schedule_initialization` never reads the disk-cache file itself -- it hands that off to a
/// background thread and returns immediately -- so this command returns the just-published status
/// (`HydratingCache`/`Pending`/whatever was already current) without waiting for that thread.
#[tauri::command]
pub async fn start_background_indexing(
    app: AppHandle,
    project_id: Option<String>,
) -> Result<IndexingStatus, AppError> {
    let settings = load_settings(&app)?;
    let effective_id = project_id.or_else(|| settings.active_project_id.clone());
    def_index_cache::schedule_initialization(&app, &settings, effective_id);
    Ok(indexing::get_indexing_status(&app))
}

#[tauri::command]
pub async fn query_def_duplicates(
    app: AppHandle,
    project_id: String,
    def_type: String,
    def_name: String,
) -> Result<DefDuplicateQueryResult, AppError> {
    let settings = load_settings(&app)?;
    let index = def_index_cache::load_fresh_for_project(&app, &settings, &project_id).await?;
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

/// Sorts (deterministically, by location name / relative path / line / column / code) and caps
/// `errors` to `DEF_INDEX_ERRORS_LIMIT`, reporting the true total and whether it was truncated.
/// A pure mapper so it is directly unit-testable without an `AppHandle`.
fn build_def_index_errors_response(mut errors: Vec<DefIndexError>) -> DefIndexErrorsResponse {
    errors.sort_by(|a, b| {
        a.location_name
            .cmp(&b.location_name)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.column.cmp(&b.column))
            .then_with(|| a.code.cmp(&b.code))
    });
    let total = errors.len();
    let truncated = total > DEF_INDEX_ERRORS_LIMIT;
    errors.truncate(DEF_INDEX_ERRORS_LIMIT);
    DefIndexErrorsResponse {
        errors,
        total,
        truncated,
    }
}

#[tauri::command]
pub fn get_def_index_errors(
    app: AppHandle,
    project_id: String,
) -> Result<DefIndexErrorsResponse, AppError> {
    let settings = load_settings(&app)?;
    // Data-only read: never forces a scan or rebuild, and serves the prior completed index
    // (possibly stale) while a rebuild is in progress, same as other query-only commands.
    let index = def_index_cache::load_for_project_query(&app, &settings, &project_id)?;
    Ok(build_def_index_errors_response(index.errors.clone()))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn error(
        location_name: &str,
        relative_path: Option<&str>,
        line: Option<usize>,
        column: Option<usize>,
        code: &str,
    ) -> DefIndexError {
        DefIndexError {
            location_id: location_name.to_lowercase(),
            location_name: location_name.to_string(),
            source_kind: IndexedSourceKind::Source,
            relative_path: relative_path.map(str::to_string),
            code: code.to_string(),
            message: format!("{} message", code),
            line,
            column,
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    #[test]
    fn sorts_deterministically_by_location_path_line_column_then_code() {
        let response = build_def_index_errors_response(vec![
            error("Zeta", Some("b.xml"), None, None, "code_z"),
            error("Alpha", Some("b.xml"), Some(2), None, "code_a"),
            error("Alpha", Some("a.xml"), None, None, "code_b"),
            error("Alpha", Some("b.xml"), Some(1), None, "code_c"),
            error("Alpha", Some("b.xml"), Some(1), Some(5), "code_a"),
            error("Alpha", Some("b.xml"), Some(1), Some(5), "code_b"),
        ]);
        let ordering: Vec<(&str, Option<&str>, Option<usize>, Option<usize>, &str)> = response
            .errors
            .iter()
            .map(|e| {
                (
                    e.location_name.as_str(),
                    e.relative_path.as_deref(),
                    e.line,
                    e.column,
                    e.code.as_str(),
                )
            })
            .collect();
        assert_eq!(
            ordering,
            vec![
                ("Alpha", Some("a.xml"), None, None, "code_b"),
                ("Alpha", Some("b.xml"), Some(1), None, "code_c"),
                ("Alpha", Some("b.xml"), Some(1), Some(5), "code_a"),
                ("Alpha", Some("b.xml"), Some(1), Some(5), "code_b"),
                ("Alpha", Some("b.xml"), Some(2), None, "code_a"),
                ("Zeta", Some("b.xml"), None, None, "code_z"),
            ]
        );
        assert_eq!(response.total, 6);
        assert!(!response.truncated);
    }

    #[test]
    fn caps_returned_errors_but_reports_true_total_and_truncated_flag() {
        let errors: Vec<DefIndexError> = (0..(DEF_INDEX_ERRORS_LIMIT + 37))
            .map(|i| error("Loc", Some("f.xml"), Some(i), None, "some_code"))
            .collect();
        let response = build_def_index_errors_response(errors);
        assert_eq!(response.errors.len(), DEF_INDEX_ERRORS_LIMIT);
        assert_eq!(response.total, DEF_INDEX_ERRORS_LIMIT + 37);
        assert!(response.truncated);
    }

    #[test]
    fn does_not_truncate_when_exactly_at_the_limit() {
        let errors: Vec<DefIndexError> = (0..DEF_INDEX_ERRORS_LIMIT)
            .map(|i| error("Loc", Some("f.xml"), Some(i), None, "some_code"))
            .collect();
        let response = build_def_index_errors_response(errors);
        assert_eq!(response.errors.len(), DEF_INDEX_ERRORS_LIMIT);
        assert_eq!(response.total, DEF_INDEX_ERRORS_LIMIT);
        assert!(!response.truncated);
    }

    #[test]
    fn empty_errors_produce_an_empty_non_truncated_response() {
        let response = build_def_index_errors_response(Vec::new());
        assert!(response.errors.is_empty());
        assert_eq!(response.total, 0);
        assert!(!response.truncated);
    }
}
