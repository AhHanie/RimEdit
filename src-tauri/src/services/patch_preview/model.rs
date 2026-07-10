use serde::{Deserialize, Serialize};

use crate::patches::{
    ApplyDiagnostic, CustomOperationMetadataMap, InheritanceDiagnostic, OperationTraceEntry,
    OperationTraceStatus, PatchFile, PatchIndex, PatchOperationClassification, PatchOperationKey,
    PatchPreviewSupport, XPathTarget,
};
use crate::project_model::ProjectSettings;

/// Preview-only per-request overrides. Never persisted to any patch XML file.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPreviewRequest {
    /// Operations (any nesting depth) to skip for this preview run.
    #[serde(default)]
    pub disabled: Vec<PatchOperationKey>,
    /// Desired order for the selected Def's visible, *top-level* operations (see
    /// [`PatchPreviewOperationSummary::can_reorder`]). Keys outside that eligible set are
    /// ignored; eligible keys not mentioned here keep their default relative order, appended
    /// after any explicitly ordered ones.
    #[serde(default)]
    pub order: Vec<PatchOperationKey>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPreviewOperationSummary {
    pub key: PatchOperationKey,
    pub class_name: String,
    pub classification: PatchOperationClassification,
    pub preview_support: PatchPreviewSupport,
    pub status: Option<OperationTraceStatus>,
    /// Explains `status`, when the apply engine has something more specific to say than the
    /// status alone (e.g. a `PatchOperationFindMod`-wrapped operation skipped because its
    /// required mod isn't registered as active in this project -- see
    /// `patches::apply::find_mod_apply`). `None` for the common case where `status` alone is
    /// self-explanatory (e.g. a plain `Applied`).
    pub status_message: Option<String>,
    /// Whether preview-only reorder controls apply to this operation (top-level operations only
    /// -- see `docs/patches-editor/07-preview-engine.md`'s "Implementation Notes" for why nested
    /// reorder is out of scope for this issue).
    pub can_reorder: bool,
    pub default_order: usize,
    pub file_order: usize,
    pub relative_path: String,
    pub location_id: String,
    pub location_name: String,
    /// The operation's raw XPath, if it has one -- lets the UI show *what* it targets alongside
    /// `target` below.
    pub xpath: Option<String>,
    /// The statically inferred target of `xpath` (see `patches::impact_graph::XPathTarget`).
    /// `Def`/`DefType` means the impact graph vouches for this operation directly; `Unsupported`
    /// means it was included only via the pre-patch ancestor-chain runtime correlation in
    /// `compute_def_preview` (see that function's comments) -- issue 08's UI uses this to show
    /// such operations in a separate "unknown impact" group rather than the normal control list.
    pub target: XPathTarget,
}

/// An operation that targets a Def the selected Def transitively inherits from, or another
/// operation touching the same Def -- surfaced so the UI can explain *why* a listed operation is
/// considered relevant, and to flag operations that may race each other.
///
/// `code` distinguishes the specific conflict cause (see `detect_visible_conflicts`'s doc comment
/// for the full list of codes) so the UI/tests can act on *why* two operations conflict, not just
/// that they do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPreviewConflictDiagnostic {
    pub code: String,
    pub key: PatchOperationKey,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPreviewImpactSummary {
    pub visible_operation_count: usize,
    pub reorderable_operation_count: usize,
    pub unsupported_operation_count: usize,
    pub conflict_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPreviewResult {
    /// `None` if no Def matching `def_type`/`def_name` was found in the combined document.
    pub xml: Option<String>,
    pub def_found: bool,
    pub is_partial: bool,
    /// Operations affecting the selected Def (any nesting depth), in default preview order.
    pub visible_operations: Vec<PatchPreviewOperationSummary>,
    /// Every operation actually visited while applying the full patch stream (not just
    /// `visible_operations`), in visit order -- the full "operation trace" the issue's
    /// Requirements section calls for.
    pub operation_trace: Vec<OperationTraceEntry>,
    pub apply_diagnostics: Vec<ApplyDiagnostic>,
    pub inheritance_diagnostics: Vec<InheritanceDiagnostic>,
    pub conflict_diagnostics: Vec<PatchPreviewConflictDiagnostic>,
    pub impact_summary: PatchPreviewImpactSummary,
}

/// Everything [`crate::services::patch_preview::compute_def_preview`] needs that isn't
/// Tauri/filesystem state -- already-loaded project settings and an already-built patch index
/// (with its full parsed ASTs, see `build_patch_index_with_files`).
pub struct PreviewInputs<'a> {
    pub settings: &'a ProjectSettings,
    pub project_id: &'a str,
    pub patch_index: &'a PatchIndex,
    /// Parallel to `patch_index.files` (same order, same length) -- see
    /// `build_patch_index_with_files`.
    pub patch_files: &'a [PatchFile],
    pub custom_operations: &'a CustomOperationMetadataMap,
}
