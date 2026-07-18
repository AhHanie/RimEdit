use crate::patches::{
    complete_patch_xpath, parse_patch_file, parse_value_fragment, serialize_initial_elements,
    serialize_patch_file, IndexedPatchOperation, PatchFile, PatchImpactGraph, PatchIndexSummary,
    XPathCompletionItemKind, XPathCompletionResult,
};
use crate::project_model::AppError;
use crate::schema_pack::{schema_pack_roots, SchemaCatalog, SchemaCatalogCacheState};
use crate::services::patch_preview::{
    self, PatchPreviewRequest, PatchPreviewResult, PatchPreviewTarget,
};
use crate::services::{def_index_cache, patch_index_cache};
use crate::settings_store::load_settings;
use crate::xml_document::model::XmlChildView;
use crate::xml_document::InitialElement;
use serde::Serialize;
use std::sync::Arc;
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
/// Builds the schema catalog the same way the project's already-loaded display catalog is built --
/// including every registered location's root as a candidate external-schema-pack search root
/// (`schema_pack_roots`, the same helper `services::validation`/`patch_preview` use), filtered by
/// the project's game version -- so completions match the fields the patch editor's forms already
/// render, including fields contributed by an external pack. Reads the Def index via the same
/// non-blocking cached query path `suggest_def_references_cmd` uses.
///
/// `locale` is the frontend's active UI locale (`useLocale()`'s current value), passed explicitly
/// rather than read from persisted `settings.locale` -- see issue 06's "commands needing localized
/// schema metadata receive an explicit, validated `locale` argument ... makes concurrent/background
/// work deterministic" -- so a runtime locale switch that has not yet finished persisting can never
/// race a completion request into serving a stale locale's labels.
///
/// This command fires on a per-keystroke (debounced) cadence from `PatchPathInput`, so -- unlike
/// other `build_schema_catalog` callers -- it always serves the catalog from
/// `SchemaCatalogCacheState`, keyed by `(gameVersion, locale, externalRootsSignature)`, rather than
/// rebuilding (re-parsing every embedded/external schema JSON file) on every call. A project with
/// registered locations gets its own cache entry per distinct root set, so external-pack fields
/// still appear in completion exactly as they do in the display catalog (see `schema_pack::cache`'s
/// module docs for the cache's invalidation policy).
#[tauri::command]
pub fn complete_patch_operation_xpath(
    app: AppHandle,
    project_id: String,
    xpath: String,
    locale: Option<String>,
) -> Result<XPathCompletionResult, AppError> {
    let settings = load_settings(&app)?;
    let roots = schema_pack_roots(&settings);
    let mut span = crate::instrumentation::span_with_tags(
        &app,
        "patches.xpathCompletion",
        [
            ("xpathLen".to_string(), xpath.len().to_string()),
            (
                "hasExternalRoots".to_string(),
                (!roots.is_empty()).to_string(),
            ),
        ],
    );

    let (catalog, cache_hit): (Arc<SchemaCatalog>, bool) = app
        .state::<SchemaCatalogCacheState>()
        .get_or_build_with_roots(&roots, Some(&settings.game_version), locale.as_deref());
    span.set_tag("catalogCacheHit", cache_hit.to_string());

    let def_index = def_index_cache::load_for_project_query(&app, &settings, &project_id)?;
    let result = complete_patch_xpath(&catalog, &def_index, &xpath);

    span.set_tag("resultCount", result.items.len().to_string());
    span.set_tag("isTruncated", result.is_truncated.to_string());
    span.set_tag("completionContext", completion_context_tag(&result));

    Ok(result)
}

/// Classify a completion result into a small, safe-to-log context tag -- never the XPath text,
/// path, Def name, or XML itself -- for `complete_patch_operation_xpath`'s instrumentation span.
fn completion_context_tag(result: &XPathCompletionResult) -> &'static str {
    match result.items.first().map(|item| item.kind) {
        Some(XPathCompletionItemKind::Root) => "root",
        Some(XPathCompletionItemKind::DefType) => "defType",
        Some(XPathCompletionItemKind::PredicateKey) | Some(XPathCompletionItemKind::DefName) => {
            "defName"
        }
        Some(XPathCompletionItemKind::Field) | Some(XPathCompletionItemKind::FieldAlias) => "field",
        Some(XPathCompletionItemKind::ListItem) | Some(XPathCompletionItemKind::MapEntry) => {
            "structural"
        }
        None => "none",
    }
}

/// Parse a patch operation's raw `<value>` inner XML into shape-classified child views for
/// `PatchValueEditor` (issue 06). Stateless -- no filesystem/project access -- since the value
/// fragment is never a real file, just a string held in the patch operation AST. Rejects when the
/// fragment isn't well-formed XML at all, so the frontend can't silently offer (and later rewrite)
/// structured editing over malformed content.
#[tauri::command]
pub fn parse_patch_value_xml(value_xml: String) -> Result<Vec<XmlChildView>, AppError> {
    // Propagate the shared XML parser's own diagnostic (`code` + `args`, e.g.
    // `parse_xml_syntax_error`/`parse_unexpected_eof`) rather than collapsing it to raw parser
    // text -- `message` is kept only as compatibility/technical detail, never the sole rendered
    // string (see `renderDiagnostic`'s priority: catalog-backed `code` wins over `message`).
    parse_value_fragment(&value_xml).map_err(|diagnostic| AppError {
        code: diagnostic.code,
        message: diagnostic.message,
        details: None,
        args: diagnostic.args,
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
