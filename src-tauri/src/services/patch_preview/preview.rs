use std::collections::HashSet;
use std::path::PathBuf;

use sxd_document::Package;

use crate::patches::dom::serialize_element_pretty;
use crate::patches::{
    apply_patch_operations, resolve_inheritance, OperationTraceStatus, PatchApplyOptions,
    PatchImpactGraph, PatchOperationKey, XPathTarget,
};
use crate::project_model::{AppError, ProjectSettings};
use crate::schema_pack::build_schema_catalog;
use crate::services::patch_index_cache;

use super::conflicts::detect_visible_conflicts;
use super::model::{
    PatchPreviewConflictDiagnostic, PatchPreviewImpactSummary, PatchPreviewOperationSummary,
    PatchPreviewRequest, PatchPreviewResult, PreviewInputs,
};
use super::operation_lookup::operation_class_name;
use super::reorder::{apply_reorder, flatten_top_level_operations};
use super::scan::{append_def_file_contents, included_locations, scan_def_files_in_load_order};
use super::selection::{
    def_name_of, matches_selected_def, pre_patch_ancestor_names, top_level_def_elements,
    xpath_touches_target,
};

fn active_mod_names(settings: &ProjectSettings) -> Vec<String> {
    settings
        .locations
        .iter()
        .map(|l| l.display_name.clone())
        .collect()
}

/// Computes the preview for one Def. Reads Def XML files from disk (via `inputs.settings`'
/// registered locations) but touches nothing else outside `inputs` -- no `AppHandle`, no caches.
pub fn compute_def_preview(
    inputs: &PreviewInputs<'_>,
    def_type: &str,
    def_name: &str,
    request: &PatchPreviewRequest,
) -> PatchPreviewResult {
    let package = Package::new();
    let document = package.as_document();
    let defs_root = document.create_element("Defs");
    document.root().append_child(defs_root);

    for location in included_locations(inputs.settings, inputs.project_id) {
        for path in scan_def_files_in_load_order(inputs.settings, location) {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                append_def_file_contents(document, defs_root, &raw);
            }
        }
    }

    // Resolved once, up front, against the pre-patch document: whether `def_name` identifies the
    // selected Def via a real `<defName>` or only via `matches_selected_def`'s `Name`-attribute
    // fallback (an `Abstract="True"` parent template with no `defName` of its own -- see that
    // function's doc comment). This distinction matters for the impact-graph lookup immediately
    // below: `PatchImpactGraph`'s `by_def` map is keyed by the literal `defName="..."` predicate
    // string found in patch XPaths, which has no relationship to a `Name` attribute value. Passing
    // a `Name` fallback straight into `operations_affecting_def` as if it were a `defName` would
    // spuriously match any unrelated patch whose XPath happens to say
    // `defName="<the abstract Def's Name>"`, and would never match the `@Name="..."` predicates
    // that could actually affect an abstract Def (those are `XPathTarget::Unsupported`, handled by
    // the separate runtime-correlation loop below regardless of this branch).
    let pre_patch_top_level_defs = top_level_def_elements(defs_root);
    let selected_has_real_def_name = pre_patch_top_level_defs
        .iter()
        .find(|&&el| matches_selected_def(el, def_type, def_name))
        .map(|&el| def_name_of(el).is_some())
        .unwrap_or(true);

    let graph = PatchImpactGraph::build(inputs.patch_index);
    let affecting = if selected_has_real_def_name {
        graph.operations_affecting_def(def_type, def_name)
    } else {
        graph.type_wide_operations(def_type)
    };

    // Needed here (moved up from just before the runtime-correlation loop below) so the
    // `DefType`-target verification immediately below can use it too.
    let ancestor_names = pre_patch_ancestor_names(&pre_patch_top_level_defs, def_type, def_name);

    let mut visible_keys: HashSet<PatchOperationKey> = HashSet::new();
    let mut eligible_for_reorder: HashSet<PatchOperationKey> = HashSet::new();
    let mut summaries: Vec<PatchPreviewOperationSummary> = Vec::new();
    for reference in &affecting {
        let Some(file) = inputs.patch_index.files.iter().find(|f| {
            f.source.location_id == reference.location_id
                && f.relative_path == reference.relative_path
        }) else {
            continue;
        };
        let Some(op) = file
            .operations
            .iter()
            .find(|o| o.id == reference.operation_id)
        else {
            continue;
        };
        // Both `XPathTarget::Def` and `XPathTarget::DefType` are deliberately conservative static
        // classifications -- `patches::impact_graph::infer_xpath_target` ignores XPath segments
        // after the def-type-plus-predicate prefix, so `Defs/ThingDef[defName="Wall"]/label` and
        // `Defs/ThingDef/label` classify identically to their predicate-less/no-further-segment
        // forms (`Defs/ThingDef[defName="Wall"]`, `Defs/ThingDef`) even though the former two only
        // actually match instances that physically have a `<label>` child. That's fine for the
        // *impact graph*'s conflict/summary purposes, but this issue's own "patches that do not
        // affect the selected Def are not shown in the normal preview control list" requirement
        // needs real precision -- so every affecting operation is re-verified against the actual
        // pre-patch document via `xpath_touches_target` (the same real-document evaluation already
        // used for the runtime-correlation loop below) before being trusted for *this* Def. A bare
        // `Defs/<DefType>[defName="..."]` or `Defs/<DefType>` (no further segment) always passes
        // this check -- the selected Def's own element is itself among the matched nodes -- so this
        // only narrows the existing over-inclusion, never inverts it.
        let touches = op
            .xpath
            .as_deref()
            .map(|xpath| xpath_touches_target(document, xpath, def_type, def_name, &ancestor_names))
            .unwrap_or(true);
        if !touches {
            continue;
        }
        let key = PatchOperationKey {
            location_id: reference.location_id.clone(),
            relative_path: reference.relative_path.clone(),
            operation_id: reference.operation_id,
        };
        let can_reorder = !op.tree_path.contains('.');
        if can_reorder {
            eligible_for_reorder.insert(key.clone());
        }
        visible_keys.insert(key.clone());
        summaries.push(PatchPreviewOperationSummary {
            key,
            class_name: op.class_name.clone(),
            classification: op.classification,
            preview_support: op.preview_support.clone(),
            status: None,
            status_message: None,
            can_reorder,
            default_order: file.file_order,
            file_order: file.file_order,
            relative_path: file.relative_path.clone(),
            location_id: file.source.location_id.clone(),
            location_name: file.source.location_name.clone(),
            xpath: op.xpath.clone(),
            target: op.target.clone(),
        });
    }

    // Runtime-correlate operations the impact graph could not statically resolve to a Def/DefType
    // (`XPathTarget::Unsupported`, e.g. an operation targeting an abstract parent by
    // `[@Name="..."]`) against the selected Def's pre-patch ancestor chain. Without this, a patch
    // that mutates an abstract parent before inheritance -- one of this issue's own required
    // fixtures -- would change the Def's final XML but never appear in its preview controls.
    // Disable-only (not reorder-eligible): the static impact graph didn't vouch for these, so
    // reorder's "only touches slots already known to affect this Def" guarantee shouldn't extend
    // to an approximated match.
    // NOTE: this must run unconditionally, even when the selected Def has no `Name`/`ParentName`
    // of its own (so `ancestor_names` is empty) -- `xpath_touches_target` also checks whether a
    // match (or one of its ancestors) is the selected Def *itself* by `defType`/`defName`, e.g. a
    // complex predicate like `Defs/ThingDef[defName="Wall" and label="wall"]/statBases` that
    // `infer_xpath_target` can't statically resolve but still genuinely targets `Wall`. Gating
    // this loop on a non-empty ancestor set (an earlier version of this code did) would silently
    // skip that direct-match case for every ordinary, non-inherited Def.
    for file in &inputs.patch_index.files {
        for op in &file.operations {
            if op.target != XPathTarget::Unsupported {
                continue;
            }
            let key = PatchOperationKey {
                location_id: file.source.location_id.clone(),
                relative_path: file.relative_path.clone(),
                operation_id: op.id,
            };
            if visible_keys.contains(&key) {
                continue;
            }
            let Some(xpath) = op.xpath.as_deref() else {
                continue;
            };
            if xpath_touches_target(document, xpath, def_type, def_name, &ancestor_names) {
                visible_keys.insert(key.clone());
                summaries.push(PatchPreviewOperationSummary {
                    key,
                    class_name: op.class_name.clone(),
                    classification: op.classification,
                    preview_support: op.preview_support.clone(),
                    status: None,
                    status_message: None,
                    can_reorder: false,
                    default_order: file.file_order,
                    file_order: file.file_order,
                    relative_path: file.relative_path.clone(),
                    location_id: file.source.location_id.clone(),
                    location_name: file.source.location_name.clone(),
                    xpath: op.xpath.clone(),
                    target: op.target.clone(),
                });
            }
        }
    }
    summaries.sort_by(|a, b| {
        a.file_order
            .cmp(&b.file_order)
            .then_with(|| a.key.operation_id.cmp(&b.key.operation_id))
    });

    let default_order = flatten_top_level_operations(inputs.patch_index, inputs.patch_files);
    let final_order = apply_reorder(default_order, &eligible_for_reorder, &request.order);

    // Only operations visible for this Def may be disabled -- a stale or crafted request must not
    // be able to silently disable an operation affecting a different Def.
    let disabled: HashSet<PatchOperationKey> = request
        .disabled
        .iter()
        .filter(|key| visible_keys.contains(key))
        .cloned()
        .collect();
    let mod_names = active_mod_names(inputs.settings);
    let apply_options = PatchApplyOptions {
        active_mod_names: &mod_names,
        custom_operations: inputs.custom_operations,
        disabled: &disabled,
    };
    let apply_result = apply_patch_operations(document, &final_order, &apply_options);

    for summary in &mut summaries {
        let trace_entry = apply_result.trace.iter().find(|t| t.key == summary.key);
        summary.status = trace_entry.map(|t| t.status);
        summary.status_message = trace_entry.and_then(|t| t.message.clone());
    }

    let top_level_defs = top_level_def_elements(defs_root);
    let inheritance = resolve_inheritance(document, &top_level_defs);

    let target = top_level_defs
        .iter()
        .copied()
        .find(|&el| matches_selected_def(el, def_type, def_name));

    let xml = target.map(|el| serialize_element_pretty(inheritance.resolve(el)));
    let def_found = target.is_some();

    let conflicting_refs = graph.conflicts_involving_def(def_type, def_name);
    let conflict_count = conflicting_refs.len();
    // Each conflicting operation gets its own diagnostic entry (keyed to itself, matching every
    // other conflict diagnostic's one-entry-per-implicated-operation shape below) -- but the
    // message names *that* operation's own class rather than repeating identical, unlabeled text
    // for every entry, which previously read as duplicate/confusing diagnostics in the UI.
    let mut conflict_diagnostics: Vec<PatchPreviewConflictDiagnostic> = conflicting_refs
        .into_iter()
        .map(|reference| {
            let class_name =
                operation_class_name(inputs.patch_index, &reference).unwrap_or("This operation");
            PatchPreviewConflictDiagnostic {
                code: "patch_conflict_multiple_operations".to_string(),
                key: PatchOperationKey {
                    location_id: reference.location_id.clone(),
                    relative_path: reference.relative_path.clone(),
                    operation_id: reference.operation_id,
                },
                message: format!(
                    "'{}' is one of {} patch operations that target {} {} -- verify ordering and \
                     success modes",
                    class_name, conflict_count, def_type, def_name
                ),
            }
        })
        .collect();
    conflict_diagnostics.extend(detect_visible_conflicts(
        &summaries,
        &apply_result.trace,
        inputs.patch_index,
        inputs.patch_files,
        document,
        def_type,
        def_name,
    ));

    let unsupported_operation_count = summaries
        .iter()
        .filter(|s| matches!(s.status, Some(OperationTraceStatus::Unsupported)))
        .count();
    let impact_summary = PatchPreviewImpactSummary {
        visible_operation_count: summaries.len(),
        reorderable_operation_count: eligible_for_reorder.len(),
        unsupported_operation_count,
        conflict_count: conflict_diagnostics.len(),
    };

    PatchPreviewResult {
        xml,
        def_found,
        is_partial: apply_result.is_partial,
        visible_operations: summaries,
        operation_trace: apply_result.trace,
        apply_diagnostics: apply_result.diagnostics,
        inheritance_diagnostics: inheritance.diagnostics,
        conflict_diagnostics,
        impact_summary,
    }
}

/// Tauri-aware wrapper: loads settings, the schema catalog (for custom operation metadata), and
/// the (fingerprint-cached) patch index with its full parsed ASTs, then delegates to
/// [`compute_def_preview`].
pub fn preview_def_for_project(
    app: &tauri::AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    def_type: &str,
    def_name: &str,
    request: &PatchPreviewRequest,
) -> Result<PatchPreviewResult, AppError> {
    let roots: Vec<PathBuf> = settings
        .locations
        .iter()
        .map(|l| PathBuf::from(&l.root_path))
        .collect();
    let catalog = build_schema_catalog(&roots, Some(&settings.game_version)).catalog;

    // Patches have no background file-watcher (see `patches::state`'s doc comment), so this must
    // go through the guaranteed-fresh (fingerprint-cached) load path, matching
    // `query_patch_operations_for_def`'s existing choice.
    let cached_index = patch_index_cache::load_for_project(app, settings, project_id, false)?;

    // The cached `PatchIndex` alone does not retain full operation ASTs (value XML, attribute
    // name/value, nested operation trees -- see `build_patch_index_with_files`'s doc comment).
    // `load_patch_files_for_project` covers that gap with its own in-memory, fingerprint-gated
    // cache (`PatchFilesState`), so repeated calls to this command -- e.g. every checkbox
    // toggle/reorder click inside an already-open preview dialog -- don't re-read and re-parse
    // every patch file from disk when nothing has changed.
    let patch_files = patch_index_cache::load_patch_files_for_project(app, settings, project_id);

    let inputs = PreviewInputs {
        settings,
        project_id,
        patch_index: cached_index.as_ref(),
        patch_files: patch_files.as_slice(),
        custom_operations: &catalog.patch_operations,
    };
    Ok(compute_def_preview(&inputs, def_type, def_name, request))
}
