//! Applies parsed patch operations (`patches::model::PatchOperationNode`) to a combined
//! `sxd_document` XML tree (`patches::dom`), mirroring the decompiled `Verse.PatchOperation*`
//! `ApplyWorker` implementations (see `docs/patches-editor/Plan.md`'s "Reference Behavior" for the
//! source file list). This module does not know about Def files, load order, or the impact graph
//! -- it only applies an already-ordered, already-filtered stream of top-level operations to
//! whatever document it is given; `services::patch_preview` owns combining Defs and scoping the
//! preview-only enable/disable/reorder controls to the selected Def.
//!
//! Implementation is split across submodules so this file can stay a stable public facade:
//! `engine` owns the per-run result accumulators and the top-level/recursive dispatch loop,
//! `diagnostics` builds the trace/diagnostic messages shared by mutation and control-flow code,
//! `mutations` holds the built-in XPath-backed DOM handlers, and `control_flow` holds
//! `Sequence`/`FindMod`/`Conditional` plus `success`-attribute adjustment.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::index::CustomOperationMetadataMap;
use super::model::{PatchOperationId, PatchOperationNode};

mod control_flow;
mod diagnostics;
mod engine;
mod mutations;

#[cfg(test)]
mod tests;

pub use engine::apply_patch_operations;

/// Identifies one operation node (at any nesting depth) across the whole preview: which patch
/// file it came from, and its id within that file's operation tree (`PatchOperationNode::id`,
/// unique per file, not globally). Used both to request preview-only disable, and to report
/// per-operation trace/diagnostic entries back to the caller.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationKey {
    pub location_id: String,
    pub relative_path: String,
    pub operation_id: PatchOperationId,
}

/// One top-level operation to apply, in final (preview-reordered) order. Nested child operations
/// (inside `Sequence`/`Conditional`/`FindMod`) are walked automatically and do not need their own
/// entry here.
pub struct TopLevelOperation<'a> {
    pub location_id: String,
    pub relative_path: String,
    pub node: &'a PatchOperationNode,
}

pub struct PatchApplyOptions<'a> {
    /// Mod display names considered "active" for `PatchOperationFindMod` (RimWorld matches by
    /// mod name, not package id -- see `Plan.md`'s "Reference Behavior").
    pub active_mod_names: &'a [String],
    pub custom_operations: &'a CustomOperationMetadataMap,
    /// Operations (any nesting depth) to skip entirely for this preview run. A skipped operation
    /// is treated as vacuously successful (see [`OperationTraceStatus::Skipped`]) so it doesn't
    /// spuriously break a containing `Sequence`.
    pub disabled: &'a HashSet<PatchOperationKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationTraceStatus {
    Applied,
    Failed,
    Skipped,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationTraceEntry {
    pub key: PatchOperationKey,
    pub class_name: String,
    pub status: OperationTraceStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplyDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDiagnostic {
    pub severity: ApplyDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub key: Option<PatchOperationKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchApplyResult {
    /// One entry per operation actually visited (top-level and nested), in visit order.
    pub trace: Vec<OperationTraceEntry>,
    pub diagnostics: Vec<ApplyDiagnostic>,
    /// True if any custom/unknown operation without safe declarative preview support was
    /// encountered -- the final document is a best-effort approximation, not a guaranteed-exact
    /// result, whenever this is true.
    pub is_partial: bool,
}
