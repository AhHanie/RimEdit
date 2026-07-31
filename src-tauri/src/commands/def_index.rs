use crate::def_index::{
    get_facet_summary, resolve_def_reference, search_def_results, settings_fingerprint,
    suggest_def_references, DefDuplicateQueryResult, DefIndexBuildOptions, DefIndexError,
    DefIndexFacetSummary, DefIndexSearchQuery, DefIndexState, DefIndexSummary,
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
use tauri::{AppHandle, Manager};

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

/// Pure scheduling decision for `start_background_indexing`, split out so it's testable without
/// an `AppHandle`. A full rebuild is skipped when a same-project job is already pending/running,
/// or when `DefIndexState` already holds an entry whose settings fingerprint matches the current
/// settings/project options -- covering both a hydrated startup cache and an index that was just
/// rebuilt or incrementally updated in this process. A `Complete` status alone is never
/// sufficient; a stale index (settings-mismatched) must remain eligible for rebuild.
fn should_enqueue_full_rebuild(
    current: &IndexingStatus,
    effective_id: &Option<String>,
    state_has_matching_entry: bool,
) -> bool {
    let already_in_flight = (current.phase == IndexingPhase::Pending
        || current.phase == IndexingPhase::Running)
        && &current.project_id == effective_id;
    !already_in_flight && !state_has_matching_entry
}

// Tauri runs non-`async` commands synchronously on the main thread. Cache-only hydration walks
// every indexed file to verify content fingerprints, which can take a while for a large
// collection (e.g. Steam Workshop) -- so this command must be `async` and hand that work to a
// blocking thread via `spawn_blocking`, exactly like the `.setup()` hydration path in lib.rs.
#[tauri::command]
pub async fn start_background_indexing(
    app: AppHandle,
    project_id: Option<String>,
) -> Result<IndexingStatus, AppError> {
    let settings = load_settings(&app)?;
    let effective_id = project_id.or_else(|| settings.active_project_id.clone());
    let current = indexing::get_indexing_status(&app);

    let state_has_matching_entry = match effective_id.as_deref() {
        Some(id) => {
            let options = DefIndexBuildOptions {
                project_id: Some(id),
                include_sources: true,
                replacement: None,
                force_rebuild: false,
            };
            let fp = settings_fingerprint(&settings, &options);
            app.state::<DefIndexState>()
                .get_if_settings_match(&fp)
                .is_some()
        }
        None => false,
    };

    if should_enqueue_full_rebuild(&current, &effective_id, state_has_matching_entry) {
        // Attempt cache-only hydration before falling back to a full rebuild, in case setup
        // hasn't hydrated this project yet (or an alternate startup route reaches this command
        // first). A miss here never blocks on a synchronous rebuild.
        let hydrated = match effective_id.clone() {
            Some(id) => {
                let blocking_app = app.clone();
                let blocking_settings = settings.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    def_index_cache::hydrate_for_project(&blocking_app, &blocking_settings, &id)
                })
                .await
                .ok()
                .and_then(Result::ok)
                .flatten()
            }
            None => None,
        };
        match hydrated {
            Some(index) => {
                app.state::<DefIndexState>()
                    .set_status_complete_for_project(effective_id.clone(), &index);
            }
            None => {
                indexing::enqueue_full_rebuild(
                    &app,
                    effective_id,
                    IndexJobReason::InitialProjectOpen,
                );
            }
        }
    }
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
    let index = def_index_cache::load_for_project(&app, &settings, &project_id, false).await?;
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

    fn status(phase: IndexingPhase, project_id: Option<&str>) -> IndexingStatus {
        IndexingStatus {
            project_id: project_id.map(str::to_string),
            phase,
            pending_files: 0,
            indexed_defs: 0,
            project_defs: 0,
            source_defs: 0,
            errors: 0,
            message: None,
            updated_at_unix_ms: 0,
            total_files: None,
            processed_files: None,
            current_stage: None,
            current_location_name: None,
        }
    }

    #[test]
    fn a_matching_state_entry_suppresses_the_full_rebuild() {
        let current = status(IndexingPhase::Idle, None);
        let effective_id = Some("project".to_string());
        assert!(!should_enqueue_full_rebuild(&current, &effective_id, true));
    }

    #[test]
    fn no_matching_state_and_no_in_flight_job_enqueues_exactly_one_rebuild() {
        let current = status(IndexingPhase::Idle, None);
        let effective_id = Some("project".to_string());
        assert!(should_enqueue_full_rebuild(&current, &effective_id, false));
    }

    #[test]
    fn a_settings_mismatched_complete_status_remains_rebuild_eligible() {
        // `Complete` alone (state_has_matching_entry = false, e.g. a stale index whose
        // settings fingerprint no longer matches) must not suppress the rebuild.
        let current = status(IndexingPhase::Complete, Some("project"));
        let effective_id = Some("project".to_string());
        assert!(should_enqueue_full_rebuild(&current, &effective_id, false));
    }

    #[test]
    fn a_pending_or_running_same_project_job_is_deduplicated() {
        let effective_id = Some("project".to_string());
        for phase in [IndexingPhase::Pending, IndexingPhase::Running] {
            let current = status(phase, Some("project"));
            assert!(!should_enqueue_full_rebuild(&current, &effective_id, false));
        }
    }

    #[test]
    fn a_pending_job_for_a_different_project_does_not_suppress_the_rebuild() {
        let current = status(IndexingPhase::Pending, Some("other"));
        let effective_id = Some("project".to_string());
        assert!(should_enqueue_full_rebuild(&current, &effective_id, false));
    }

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
