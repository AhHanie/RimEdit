use crate::patches::{
    complete_patch_xpath, parse_patch_file, parse_value_fragment, serialize_initial_elements,
    serialize_patch_file, IndexedPatchOperation, PatchFile, PatchImpactGraph, PatchIndexSummary,
    XPathCompletionResult,
};
use crate::project_model::AppError;
use crate::schema_pack::SchemaCatalogCacheState;
use crate::services::patch_preview::{
    self, PatchPreviewRequest, PatchPreviewResult, PatchPreviewTarget,
};
use crate::services::{def_index_cache, patch_index_cache};
use crate::settings_store::load_settings;
use crate::xml_document::model::XmlChildView;
use crate::xml_document::InitialElement;
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn rebuild_patch_index(
    app: AppHandle,
    project_id: Option<String>,
) -> Result<PatchIndexSummary, AppError> {
    let settings = load_settings(&app)?;
    patch_index_cache::rebuild_for_project(&app, &settings, project_id.as_deref())
}

/// Parse a `<Patch>` file's raw XML text into its editable operation AST. Stateless (no
/// filesystem/project access) -- the patches editor UI owns the raw XML buffer (via the same
/// `useXmlEditorSession` load/save/undo flow every other XML file uses) and calls this to derive
/// a structured operation tree from it, and [`serialize_patch_operations`] to go back to text.
#[tauri::command]
pub fn parse_patch_operations(relative_path: String, raw_xml: String) -> PatchFile {
    parse_patch_file(&relative_path, &raw_xml)
}

/// Serialize an edited patch operation AST back to RimWorld-compatible `<Patch>` XML text.
/// Stateless; the caller is responsible for feeding the result back through the normal raw-XML
/// save/save-preview commands.
#[tauri::command]
pub fn serialize_patch_operations(patch_file: PatchFile) -> String {
    serialize_patch_file(&patch_file)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationForDef {
    pub location_id: String,
    pub location_name: String,
    pub relative_path: String,
    pub file_order: usize,
    pub operation: IndexedPatchOperation,
}

/// Operations statically known to affect the given Def, in stable preview order (location
/// order, then load folder order, then file order, then operation order within a file).
#[tauri::command]
pub fn query_patch_operations_for_def(
    app: AppHandle,
    project_id: String,
    def_type: String,
    def_name: String,
) -> Result<Vec<PatchOperationForDef>, AppError> {
    let settings = load_settings(&app)?;
    // Unlike Defs, patches have no background file-watcher keeping the cache warm yet, so use
    // the guaranteed-fresh (but fingerprint-cached) load path rather than the non-blocking query
    // path -- otherwise a project that never called `rebuild_patch_index` would silently see an
    // empty index instead of one computed on first use.
    let index = patch_index_cache::load_for_project(&app, &settings, &project_id, false)?;
    let graph = PatchImpactGraph::build(&index);

    let mut results: Vec<PatchOperationForDef> = Vec::new();
    for reference in graph.operations_affecting_def(&def_type, &def_name) {
        let Some(file) = index.files.iter().find(|f| {
            f.source.location_id == reference.location_id
                && f.relative_path == reference.relative_path
        }) else {
            continue;
        };
        let Some(op) = file
            .operations
            .iter()
            .find(|op| op.id == reference.operation_id)
        else {
            continue;
        };
        results.push(PatchOperationForDef {
            location_id: file.source.location_id.clone(),
            location_name: file.source.location_name.clone(),
            relative_path: file.relative_path.clone(),
            file_order: file.file_order,
            operation: op.clone(),
        });
    }

    results.sort_by(|a, b| {
        a.file_order
            .cmp(&b.file_order)
            .then_with(|| a.operation.id.cmp(&b.operation.id))
    });

    Ok(results)
}

/// Schema- and Def-index-aware XPath completions/target inference for `PatchPathInput` (issue 05).
/// Builds the schema catalog the same way the project's already-loaded catalog is built (no extra
/// schema roots, filtered by the project's game version) so completions match the fields the
/// patch editor's forms already render, and reads the Def index via the same non-blocking cached
/// query path `suggest_def_references_cmd` uses.
///
/// This command fires on a per-keystroke (debounced) cadence from `PatchPathInput`, unlike other
/// `build_schema_catalog` callers, so the catalog is served from `SchemaCatalogCacheState` instead
/// of being rebuilt (re-parsing ~1,300 embedded schema JSON files) on every call.
#[tauri::command]
pub fn complete_patch_operation_xpath(
    app: AppHandle,
    project_id: String,
    xpath: String,
) -> Result<XPathCompletionResult, AppError> {
    let settings = load_settings(&app)?;
    let catalog = app
        .state::<SchemaCatalogCacheState>()
        .get_or_build(Some(&settings.game_version));
    let def_index = def_index_cache::load_for_project_query(&app, &settings, &project_id)?;
    Ok(complete_patch_xpath(&catalog, &def_index, &xpath))
}

/// Parse a patch operation's raw `<value>` inner XML into shape-classified child views for
/// `PatchValueEditor` (issue 06). Stateless -- no filesystem/project access -- since the value
/// fragment is never a real file, just a string held in the patch operation AST. Rejects when the
/// fragment isn't well-formed XML at all, so the frontend can't silently offer (and later rewrite)
/// structured editing over malformed content.
#[tauri::command]
pub fn parse_patch_value_xml(value_xml: String) -> Result<Vec<XmlChildView>, AppError> {
    parse_value_fragment(&value_xml).map_err(|message| AppError {
        code: "patch_value_xml_malformed".to_string(),
        message,
        details: None,
    })
}

/// Serialize a structured value edit (built client-side from an edited `ObjectFieldValue`) back
/// into XML text for `PatchValueEditor`. Stateless; reuses the same `InitialElement` tree shape
/// `xml-editor`'s object-list item insertion already sends over IPC.
#[tauri::command]
pub fn serialize_patch_value_fragment(elements: Vec<InitialElement>) -> String {
    serialize_initial_elements(&elements)
}

/// Preview a single Def's final, post-patch, post-inheritance XML (issue 07). Combines every
/// indexable Def XML file into one document in RimWorld load order, applies every patch operation
/// from every patch file (preview-only `request.disabled`/`request.order` scope only the
/// operations that affect this Def -- see `services::patch_preview`'s module doc for why
/// application itself always runs the full patch stream), resolves XML inheritance, and returns
/// the Def's final XML alongside diagnostics and the visible-operation list for preview controls.
///
/// `project_id` is only the active editable project used as preview context (registered
/// locations, load folders, patch files) -- it does not have to be where `target` was opened
/// from. `target` identifies the exact Def the caller opened (file origin + in-file ordinal,
/// required for exact source/project selection when another location has a same-named Def); see
/// `PatchPreviewTarget`'s doc comment.
#[tauri::command]
pub fn preview_def_patches(
    app: AppHandle,
    project_id: String,
    target: PatchPreviewTarget,
    request: PatchPreviewRequest,
) -> Result<PatchPreviewResult, AppError> {
    let settings = load_settings(&app)?;
    patch_preview::preview_def_for_project(&app, &settings, &project_id, &target, &request)
}
