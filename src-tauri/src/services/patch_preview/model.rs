use serde::{Deserialize, Serialize};

use crate::patches::{
    ApplyDiagnostic, CustomOperationMetadataMap, InheritanceDiagnostic, OperationTraceEntry,
    OperationTraceStatus, PatchFile, PatchIndex, PatchOperationClassification, PatchOperationKey,
    PatchPreviewSupport, XPathTarget,
};
use crate::project_model::ProjectSettings;

/// Identifies the exact Def an open editor tab is showing, independent of the active editable
/// project used as preview context. `location_id` + `relative_path` name the file the Def was
/// opened from (which may be a read-only source, not the editable project), and `ordinal` is the
/// Def's zero-based position among that single file's own top-level Def elements -- the same
/// position `xml_document::def_summary::extract_def_summaries` assigns when building the editor's
/// `defs` list, so the frontend can compute it directly from the already-parsed document without
/// any extra backend round-trip.
///
/// `def_type`/`identity` (the real `defName`, or the `Name` attribute for an `Abstract="True"`
/// template with no `defName` of its own) are carried along as validation data, not as the primary
/// lookup key: the backend re-verifies them against whatever element `location_id` +
/// `relative_path` + `ordinal` resolves to in the combined document, and refuses to substitute a
/// same-named Def elsewhere if they don't match (see `services::patch_preview::preview`'s
/// provenance resolution).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPreviewTarget {
    pub location_id: String,
    pub relative_path: String,
    pub def_type: String,
    pub identity: String,
    pub ordinal: usize,
}

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
    /// Compatibility English text mirroring `status_code`/`status_args` below -- see
    /// `OperationTraceEntry::message`'s doc comment for the same pattern. Explains `status` when
    /// the apply engine has something more specific to say than the status alone (e.g. a
    /// `PatchOperationFindMod`-wrapped operation skipped because its required mod isn't
    /// registered as active in this project -- see `patches::apply::find_mod_apply`). `None` for
    /// the common case where `status` alone is self-explanatory (e.g. a plain `Applied`).
    pub status_message: Option<String>,
    /// Stable diagnostic code mirroring `status_message`, for the frontend's shared diagnostic
    /// renderer (`renderDiagnostic`) to look up and translate instead of showing raw English.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::diagnostics::DiagnosticArgs::is_empty"
    )]
    pub status_args: crate::diagnostics::DiagnosticArgs,
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
    #[serde(
        default,
        skip_serializing_if = "crate::diagnostics::DiagnosticArgs::is_empty"
    )]
    pub args: crate::diagnostics::DiagnosticArgs,
}

impl PatchPreviewConflictDiagnostic {
    pub(super) fn new(
        code: impl Into<String>,
        key: PatchOperationKey,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            key,
            message: message.into(),
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    /// Attaches typed args for `code`. Additive on top of the still-English `message`.
    pub(super) fn with_args(mut self, args: crate::diagnostics::DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
    }
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
    /// `None` if the requested [`PatchPreviewTarget`] could not be resolved in the combined
    /// document (stale/mismatched origin) or a patch removed the resolved element.
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

/// Built when a [`PatchPreviewTarget`] cannot be resolved against the combined document's
/// provenance table before patch application -- the opened file no longer contains a Def at that
/// origin/ordinal, or the element found there no longer has the stated Def type/identity. Distinct
/// from a normal "Def not found" (which the old loose `def_type`/`def_name` lookup used for both a
/// missing Def *and* transport failures): this always carries an explicit diagnostic so the dialog
/// can tell the user their tab is stale rather than silently showing another Def's preview.
pub fn target_not_found_result(target: &PatchPreviewTarget) -> PatchPreviewResult {
    PatchPreviewResult {
        xml: None,
        def_found: false,
        is_partial: false,
        visible_operations: Vec::new(),
        operation_trace: Vec::new(),
        apply_diagnostics: vec![ApplyDiagnostic::error(
            "patch_preview_target_not_found",
            format!(
                "Could not find {} \"{}\" at {} #{} in location \"{}\" -- the file may have \
                 changed, or this tab is no longer showing an active Def.",
                target.def_type,
                target.identity,
                target.relative_path,
                target.ordinal,
                target.location_id
            ),
            None,
        )
        .with_args(crate::diagnostics::diagnostic_args([
            ("defType", target.def_type.as_str().into()),
            ("identity", target.identity.as_str().into()),
            ("relativePath", target.relative_path.as_str().into()),
            ("ordinal", target.ordinal.into()),
            ("locationId", target.location_id.as_str().into()),
        ]))],
        inheritance_diagnostics: Vec::new(),
        conflict_diagnostics: Vec::new(),
        impact_summary: PatchPreviewImpactSummary {
            visible_operation_count: 0,
            reorderable_operation_count: 0,
            unsupported_operation_count: 0,
            conflict_count: 0,
        },
    }
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

#[cfg(test)]
mod diagnostic_ref_wire_tests {
    use super::*;
    use crate::diagnostics::diagnostic_args;

    fn key() -> PatchOperationKey {
        PatchOperationKey {
            location_id: "loc-1".to_string(),
            relative_path: "Patches/Foo.xml".to_string(),
            operation_id: 0,
        }
    }

    #[test]
    fn conflict_diagnostic_wire_shape_carries_code_and_args() {
        let diag = PatchPreviewConflictDiagnostic::new(
            "patch_conflict_duplicate_add_child",
            key(),
            "2 Add operations add a <label> child",
        )
        .with_args(diagnostic_args([
            ("count", 2usize.into()),
            ("childName", "label".into()),
        ]));
        let json = serde_json::to_value(&diag).unwrap();
        assert_eq!(json["code"], "patch_conflict_duplicate_add_child");
        assert_eq!(json["args"]["count"], 2);
    }

    #[test]
    fn conflict_diagnostic_without_args_omits_the_field() {
        let diag =
            PatchPreviewConflictDiagnostic::new("patch_conflict_multiple_operations", key(), "x");
        let json = serde_json::to_value(&diag).unwrap();
        assert!(json.get("args").is_none());
    }

    #[test]
    fn target_not_found_result_carries_typed_args() {
        let target = PatchPreviewTarget {
            location_id: "loc-1".to_string(),
            relative_path: "Defs/Things.xml".to_string(),
            def_type: "ThingDef".to_string(),
            identity: "Wall".to_string(),
            ordinal: 0,
        };
        let result = target_not_found_result(&target);
        let diag = &result.apply_diagnostics[0];
        assert_eq!(diag.code, "patch_preview_target_not_found");
        assert_eq!(
            diag.args["defType"],
            crate::diagnostics::DiagnosticArgValue::Text("ThingDef".to_string())
        );
    }
}
