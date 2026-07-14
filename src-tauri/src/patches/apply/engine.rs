//! Owns the per-run mutable result accumulators (`trace`, `diagnostics`, the partial flag) and
//! the top-level/recursive dispatch loop that walks an ordered stream of patch operations,
//! delegating XPath-backed DOM mutation to [`super::mutations`] and container/control-flow
//! behavior to [`super::control_flow`].

use std::collections::HashSet;

use sxd_document::dom::Document;

use super::{control_flow, mutations};
use super::{
    ApplyDiagnostic, ApplyDiagnosticSeverity, OperationTraceEntry, OperationTraceStatus,
    PatchApplyOptions, PatchApplyResult, PatchOperationKey, TopLevelOperation,
};
use crate::patches::index::CustomOperationMetadataMap;
use crate::patches::model::{PatchOperationKind, PatchOperationNode, PatchSuccessMode};

/// Scopes one top-level patch-file operation (and everything nested inside it): which patch file
/// it came from (for building [`PatchOperationKey`]s), plus the run-wide mod-activity/custom-op/
/// disabled-operation options.
pub(super) struct ApplyContext<'a> {
    pub(super) location_id: &'a str,
    pub(super) relative_path: &'a str,
    pub(super) active_mod_names: &'a [String],
    pub(super) custom_operations: &'a CustomOperationMetadataMap,
    pub(super) disabled: &'a HashSet<PatchOperationKey>,
}

/// Applies every operation in `operations`, in order, to `document`. Mutates `document` in place.
pub fn apply_patch_operations<'d>(
    document: Document<'d>,
    operations: &[TopLevelOperation<'_>],
    options: &PatchApplyOptions<'_>,
) -> PatchApplyResult {
    let mut engine = ApplyEngine::new();
    for entry in operations {
        let ctx = ApplyContext {
            location_id: &entry.location_id,
            relative_path: &entry.relative_path,
            active_mod_names: options.active_mod_names,
            custom_operations: options.custom_operations,
            disabled: options.disabled,
        };
        engine.apply_node(document, entry.node, &ctx);
    }
    engine.finish()
}

/// Owns the accumulators for one [`apply_patch_operations`] run. `trace` and `diagnostics` are
/// `pub(super)` so `control_flow`'s container operations can append to them directly when they
/// need to report something about the container itself (e.g. a `Sequence` short-circuit, or a
/// `FindMod` dependency skip) rather than about the child operation they delegate to.
pub(super) struct ApplyEngine {
    pub(super) trace: Vec<OperationTraceEntry>,
    pub(super) diagnostics: Vec<ApplyDiagnostic>,
    is_partial: bool,
}

impl ApplyEngine {
    fn new() -> Self {
        Self {
            trace: Vec::new(),
            diagnostics: Vec::new(),
            is_partial: false,
        }
    }

    fn finish(self) -> PatchApplyResult {
        PatchApplyResult {
            trace: self.trace,
            diagnostics: self.diagnostics,
            is_partial: self.is_partial,
        }
    }

    /// Port of `PatchOperation.Apply`/`ApplyWorker`'s split: computes the operation's raw result
    /// (`ApplyWorker`), then applies its own `success` attribute on top (`Apply`). Returns the
    /// fully adjusted boolean, which is what a containing `Sequence`/`Conditional`/`FindMod` sees
    /// as this operation's result.
    pub(super) fn apply_node<'d>(
        &mut self,
        document: Document<'d>,
        node: &PatchOperationNode,
        ctx: &ApplyContext<'_>,
    ) -> bool {
        let key = PatchOperationKey {
            location_id: ctx.location_id.to_string(),
            relative_path: ctx.relative_path.to_string(),
            operation_id: node.id,
        };

        if ctx.disabled.contains(&key) {
            self.trace.push(OperationTraceEntry {
                key,
                class_name: node.class_name.clone(),
                status: OperationTraceStatus::Skipped,
                message: None,
            });
            return true;
        }

        if !node.is_known_class() {
            let is_custom = ctx.custom_operations.contains_key(&node.class_name);
            let message = if is_custom {
                format!(
                    "'{}' is a custom operation without declared preview support",
                    node.class_name
                )
            } else {
                format!(
                    "'{}' is not a recognized built-in patch operation class",
                    node.class_name
                )
            };
            self.trace.push(OperationTraceEntry {
                key: key.clone(),
                class_name: node.class_name.clone(),
                status: OperationTraceStatus::Unsupported,
                message: Some(message.clone()),
            });
            self.diagnostics.push(ApplyDiagnostic {
                severity: ApplyDiagnosticSeverity::Warning,
                code: "patch_apply_unsupported_operation".to_string(),
                message,
                key: Some(key),
            });
            self.is_partial = true;
            // Assumed to have "worked" (see struct docs) so `success="Never"`/`"Invert"` on an
            // unsupported operation still visibly fails rather than silently no-op-succeeding.
            return control_flow::apply_success_mode(node.success, true);
        }

        let raw = self.apply_worker(document, node, ctx, &key);
        let adjusted = control_flow::apply_success_mode(node.success, raw);
        if node.success == PatchSuccessMode::Always && !raw {
            self.diagnostics.push(ApplyDiagnostic {
                severity: ApplyDiagnosticSeverity::Warning,
                code: "patch_apply_success_always_masks_failure".to_string(),
                message: "Operation would have failed, but success=\"Always\" forces it to succeed"
                    .to_string(),
                key: Some(key.clone()),
            });
        }
        self.trace.push(OperationTraceEntry {
            key,
            class_name: node.class_name.clone(),
            status: if adjusted {
                OperationTraceStatus::Applied
            } else {
                OperationTraceStatus::Failed
            },
            message: None,
        });
        adjusted
    }

    fn apply_worker<'d>(
        &mut self,
        document: Document<'d>,
        node: &PatchOperationNode,
        ctx: &ApplyContext<'_>,
        key: &PatchOperationKey,
    ) -> bool {
        match &node.kind {
            PatchOperationKind::Add(op) => {
                mutations::add_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::Insert(op) => {
                mutations::insert_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::Remove(op) => {
                mutations::remove_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::Replace(op) => {
                mutations::replace_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::AttributeAdd(op) => {
                mutations::attribute_add_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::AttributeSet(op) => {
                mutations::attribute_set_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::AttributeRemove(op) => {
                mutations::attribute_remove_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::AddModExtension(op) => {
                mutations::add_mod_extension_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::SetName(op) => {
                mutations::set_name_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::Sequence(children) => {
                control_flow::sequence_apply(document, children, ctx, self, key)
            }
            PatchOperationKind::FindMod {
                mods,
                match_op,
                nomatch_op,
            } => control_flow::find_mod_apply(document, mods, match_op, nomatch_op, ctx, self),
            PatchOperationKind::Conditional {
                xpath,
                match_op,
                nomatch_op,
            } => control_flow::conditional_apply(
                document,
                xpath.as_deref(),
                match_op,
                nomatch_op,
                ctx,
                self,
                key,
            ),
            PatchOperationKind::Test(op) => {
                mutations::test_op(document, op, &mut self.diagnostics, key)
            }
            PatchOperationKind::Unknown(_) => {
                unreachable!("apply_worker is only reached for known classes (see apply_node)")
            }
        }
    }
}
