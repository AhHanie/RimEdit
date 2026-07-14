//! Container/control-flow behavior for `Sequence`, `FindMod`, and `Conditional`, plus the
//! `success`-attribute Normal/Invert/Always/Never adjustment shared by every operation kind.
//! Each container delegates back into [`ApplyEngine::apply_node`] for its children so trace order
//! stays depth-first and each child's own `success` setting is already applied before its result
//! is seen here -- matching RimWorld calling `child.Apply(xml)`, never `child.ApplyWorker`, from
//! within a container's own `ApplyWorker`.

use sxd_document::dom::Document;

use super::diagnostics::{missing_field_diagnostic, xpath_error_diagnostic};
use super::engine::{ApplyContext, ApplyEngine};
use super::{
    ApplyDiagnostic, ApplyDiagnosticSeverity, OperationTraceEntry, OperationTraceStatus,
    PatchOperationKey,
};
use crate::patches::dom::select_nodes;
use crate::patches::model::{PatchOperationNode, PatchSuccessMode};

pub(super) fn apply_success_mode(mode: PatchSuccessMode, raw: bool) -> bool {
    match mode {
        PatchSuccessMode::Normal => raw,
        PatchSuccessMode::Invert => !raw,
        PatchSuccessMode::Always => true,
        PatchSuccessMode::Never => false,
    }
}

pub(super) fn sequence_apply<'d>(
    document: Document<'d>,
    children: &[PatchOperationNode],
    ctx: &ApplyContext<'_>,
    engine: &mut ApplyEngine,
    key: &PatchOperationKey,
) -> bool {
    for (i, child) in children.iter().enumerate() {
        if !engine.apply_node(document, child, ctx) {
            let skipped = children.len() - i - 1;
            if skipped > 0 {
                engine.diagnostics.push(ApplyDiagnostic {
                    severity: ApplyDiagnosticSeverity::Warning,
                    code: "patch_apply_sequence_short_circuited".to_string(),
                    message: format!(
                        "Sequence stopped after a failed operation; {} subsequent operation{} skipped",
                        skipped,
                        if skipped == 1 { "" } else { "s" }
                    ),
                    key: Some(key.clone()),
                });
            }
            return false;
        }
    }
    true
}

fn mod_is_active(active_mod_names: &[String], name: &str) -> bool {
    active_mod_names
        .iter()
        .any(|m| m.eq_ignore_ascii_case(name))
}

fn format_mod_list(mods: &[String]) -> String {
    match mods {
        [one] => format!("\"{}\"", one),
        many => format!(
            "one of [{}]",
            many.iter()
                .map(|m| format!("\"{}\"", m))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

pub(super) fn find_mod_apply<'d>(
    document: Document<'d>,
    mods: &[String],
    match_op: &Option<Box<PatchOperationNode>>,
    nomatch_op: &Option<Box<PatchOperationNode>>,
    ctx: &ApplyContext<'_>,
    engine: &mut ApplyEngine,
) -> bool {
    let matched = mods.iter().any(|m| mod_is_active(ctx.active_mod_names, m));
    if matched {
        if let Some(m) = match_op {
            return engine.apply_node(document, m, ctx);
        }
    } else if let Some(nm) = nomatch_op {
        return engine.apply_node(document, nm, ctx);
    } else if let Some(m) = match_op {
        // The mod this operation depends on isn't registered as active in this project, and
        // there's no `nomatch` branch to fall back to -- `m` (and everything nested inside it)
        // never runs, matching RimWorld's real "nothing to do" semantics. Silently returning
        // `true` here with no trace/diagnostic would leave `m`'s own preview row showing no
        // status at all, reading as an unexplained no-op. Report a `Skipped` trace entry (keyed
        // to `m` itself -- the operation actually shown as its own row in preview controls, see
        // `patches::index`'s indexing of nested FindMod/Sequence/Conditional children) plus a
        // diagnostic explaining why, so both the row and the diagnostics list surface the cause.
        let key = PatchOperationKey {
            location_id: ctx.location_id.to_string(),
            relative_path: ctx.relative_path.to_string(),
            operation_id: m.id,
        };
        let mod_list = format_mod_list(mods);
        engine.trace.push(OperationTraceEntry {
            key: key.clone(),
            class_name: m.class_name.clone(),
            status: OperationTraceStatus::Skipped,
            message: Some(format!("Requires mod {} to be active", mod_list)),
        });
        engine.diagnostics.push(ApplyDiagnostic {
            severity: ApplyDiagnosticSeverity::Warning,
            code: "patch_find_mod_dependency_not_active".to_string(),
            message: format!(
                "Requires mod {} to be active, which is not registered as a location in this \
                 project -- this operation did not apply",
                mod_list
            ),
            key: Some(key),
        });
    }
    true
}

/// Port of `PatchOperationConditional.ApplyWorker`'s exact (slightly non-obvious) truth table --
/// transcribed structurally from the decompiled source rather than simplified, since a matched
/// condition with no `match` operation still depends on whether a `nomatch` operation exists (it
/// is never run in that case, only its *presence* flips the result) -- see
/// `docs/patches-editor/07-preview-engine.md` for the full derivation.
pub(super) fn conditional_apply<'d>(
    document: Document<'d>,
    xpath: Option<&str>,
    match_op: &Option<Box<PatchOperationNode>>,
    nomatch_op: &Option<Box<PatchOperationNode>>,
    ctx: &ApplyContext<'_>,
    engine: &mut ApplyEngine,
    key: &PatchOperationKey,
) -> bool {
    let Some(xpath) = xpath else {
        missing_field_diagnostic(&mut engine.diagnostics, key, "xpath");
        return false;
    };
    let matched = match select_nodes(document, xpath) {
        Ok(nodes) => !nodes.is_empty(),
        Err(e) => {
            xpath_error_diagnostic(&mut engine.diagnostics, key, xpath, &e);
            false
        }
    };

    if matched {
        if let Some(m) = match_op {
            return engine.apply_node(document, m, ctx);
        }
    } else if let Some(nm) = nomatch_op {
        return engine.apply_node(document, nm, ctx);
    }
    if match_op.is_none() {
        return nomatch_op.is_some();
    }
    true
}
