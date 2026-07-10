//! Applies parsed patch operations (`patches::model::PatchOperationNode`) to a combined
//! `sxd_document` XML tree (`patches::dom`), mirroring the decompiled `Verse.PatchOperation*`
//! `ApplyWorker` implementations (see `docs/patches-editor/Plan.md`'s "Reference Behavior" for the
//! source file list). This module does not know about Def files, load order, or the impact graph
//! -- it only applies an already-ordered, already-filtered stream of top-level operations to
//! whatever document it is given; `services::patch_preview` owns combining Defs and scoping the
//! preview-only enable/disable/reorder controls to the selected Def.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sxd_document::dom::{ChildOfElement, Document, Element, ParentOfChild};

use super::dom::{
    child_elements_named, clone_child_of_element, first_child_element_named, parse_fragment,
    select_elements, select_nodes,
};
use super::index::CustomOperationMetadataMap;
use super::model::{
    AttributeOperation, AttributeValueOperation, PatchOperationId, PatchOperationKind,
    PatchOperationNode, PatchOrderMode, PatchSuccessMode, PathedOperation, PathedValueOperation,
    PathedValueOrderOperation, SetNameOperation,
};

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

struct ApplyContext<'a> {
    location_id: &'a str,
    relative_path: &'a str,
    active_mod_names: &'a [String],
    custom_operations: &'a CustomOperationMetadataMap,
    disabled: &'a HashSet<PatchOperationKey>,
}

/// Applies every operation in `operations`, in order, to `document`. Mutates `document` in place.
pub fn apply_patch_operations<'d>(
    document: Document<'d>,
    operations: &[TopLevelOperation<'_>],
    options: &PatchApplyOptions<'_>,
) -> PatchApplyResult {
    let mut trace = Vec::new();
    let mut diagnostics = Vec::new();
    let mut is_partial = false;

    for entry in operations {
        let ctx = ApplyContext {
            location_id: &entry.location_id,
            relative_path: &entry.relative_path,
            active_mod_names: options.active_mod_names,
            custom_operations: options.custom_operations,
            disabled: options.disabled,
        };
        apply_node(
            document,
            entry.node,
            &ctx,
            &mut trace,
            &mut diagnostics,
            &mut is_partial,
        );
    }

    PatchApplyResult {
        trace,
        diagnostics,
        is_partial,
    }
}

fn apply_success_mode(mode: PatchSuccessMode, raw: bool) -> bool {
    match mode {
        PatchSuccessMode::Normal => raw,
        PatchSuccessMode::Invert => !raw,
        PatchSuccessMode::Always => true,
        PatchSuccessMode::Never => false,
    }
}

/// Port of `PatchOperation.Apply`/`ApplyWorker`'s split: computes the operation's raw result
/// (`ApplyWorker`), then applies its own `success` attribute on top (`Apply`). Returns the fully
/// adjusted boolean, which is what a containing `Sequence`/`Conditional`/`FindMod` sees as this
/// operation's result -- matching RimWorld calling `child.Apply(xml)`, never `child.ApplyWorker`,
/// from within a container's own `ApplyWorker`.
fn apply_node<'d>(
    document: Document<'d>,
    node: &PatchOperationNode,
    ctx: &ApplyContext<'_>,
    trace: &mut Vec<OperationTraceEntry>,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    is_partial: &mut bool,
) -> bool {
    let key = PatchOperationKey {
        location_id: ctx.location_id.to_string(),
        relative_path: ctx.relative_path.to_string(),
        operation_id: node.id,
    };

    if ctx.disabled.contains(&key) {
        trace.push(OperationTraceEntry {
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
        trace.push(OperationTraceEntry {
            key: key.clone(),
            class_name: node.class_name.clone(),
            status: OperationTraceStatus::Unsupported,
            message: Some(message.clone()),
        });
        diagnostics.push(ApplyDiagnostic {
            severity: ApplyDiagnosticSeverity::Warning,
            code: "patch_apply_unsupported_operation".to_string(),
            message,
            key: Some(key),
        });
        *is_partial = true;
        // Assumed to have "worked" (see struct docs) so `success="Never"`/`"Invert"` on an
        // unsupported operation still visibly fails rather than silently no-op-succeeding.
        return apply_success_mode(node.success, true);
    }

    let raw = apply_worker(document, node, ctx, trace, diagnostics, is_partial, &key);
    let adjusted = apply_success_mode(node.success, raw);
    if node.success == PatchSuccessMode::Always && !raw {
        diagnostics.push(ApplyDiagnostic {
            severity: ApplyDiagnosticSeverity::Warning,
            code: "patch_apply_success_always_masks_failure".to_string(),
            message: "Operation would have failed, but success=\"Always\" forces it to succeed"
                .to_string(),
            key: Some(key.clone()),
        });
    }
    trace.push(OperationTraceEntry {
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

fn missing_field_diagnostic(
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
    field: &str,
) {
    diagnostics.push(ApplyDiagnostic {
        severity: ApplyDiagnosticSeverity::Error,
        code: "patch_apply_missing_field".to_string(),
        message: format!("Operation is missing its required '{}' field", field),
        key: Some(key.clone()),
    });
}

fn xpath_error_diagnostic(
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
    xpath: &str,
    error: &str,
) {
    diagnostics.push(ApplyDiagnostic {
        severity: ApplyDiagnosticSeverity::Warning,
        code: "patch_apply_xpath_error".to_string(),
        message: format!("XPath \"{}\" failed to evaluate: {}", xpath, error),
        key: Some(key.clone()),
    });
}

/// A well-formed, successfully-evaluated XPath matched zero nodes. Distinct from
/// `xpath_error_diagnostic` (a genuine evaluation failure) -- this is the common "nothing to act
/// on" case for a mutating operation (Add/Insert/Remove/Replace/attribute ops/AddModExtension/
/// SetName), which today silently no-ops with no explanation. Not raised for `Test`/`Conditional`,
/// where "zero matches" is itself the operation's intended outcome, not a failure to explain.
fn xpath_no_match_diagnostic(
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
    xpath: &str,
) {
    diagnostics.push(ApplyDiagnostic {
        severity: ApplyDiagnosticSeverity::Warning,
        code: "patch_apply_xpath_no_match".to_string(),
        message: format!("XPath \"{}\" did not match any node", xpath),
        key: Some(key.clone()),
    });
}

#[allow(clippy::too_many_arguments)]
fn apply_worker<'d>(
    document: Document<'d>,
    node: &PatchOperationNode,
    ctx: &ApplyContext<'_>,
    trace: &mut Vec<OperationTraceEntry>,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    is_partial: &mut bool,
    key: &PatchOperationKey,
) -> bool {
    match &node.kind {
        PatchOperationKind::Add(op) => add_op(document, op, diagnostics, key),
        PatchOperationKind::Insert(op) => insert_op(document, op, diagnostics, key),
        PatchOperationKind::Remove(op) => remove_op(document, op, diagnostics, key),
        PatchOperationKind::Replace(op) => replace_op(document, op, diagnostics, key),
        PatchOperationKind::AttributeAdd(op) => attribute_add_op(document, op, diagnostics, key),
        PatchOperationKind::AttributeSet(op) => attribute_set_op(document, op, diagnostics, key),
        PatchOperationKind::AttributeRemove(op) => {
            attribute_remove_op(document, op, diagnostics, key)
        }
        PatchOperationKind::AddModExtension(op) => {
            add_mod_extension_op(document, op, diagnostics, key)
        }
        PatchOperationKind::SetName(op) => set_name_op(document, op, diagnostics, key),
        PatchOperationKind::Sequence(children) => {
            sequence_apply(document, children, ctx, trace, diagnostics, is_partial, key)
        }
        PatchOperationKind::FindMod {
            mods,
            match_op,
            nomatch_op,
        } => find_mod_apply(
            document,
            mods,
            match_op,
            nomatch_op,
            ctx,
            trace,
            diagnostics,
            is_partial,
        ),
        PatchOperationKind::Conditional {
            xpath,
            match_op,
            nomatch_op,
        } => conditional_apply(
            document,
            xpath.as_deref(),
            match_op,
            nomatch_op,
            ctx,
            trace,
            diagnostics,
            is_partial,
            key,
        ),
        PatchOperationKind::Test(op) => test_op(document, op, diagnostics, key),
        PatchOperationKind::Unknown(_) => {
            unreachable!("apply_worker is only reached for known classes (see apply_node)")
        }
    }
}

/// Finds `target`'s position among its parent's children, and the parent element itself.
/// `None` if `target` has no element parent (e.g. it is a top-level `<Defs>` child) -- built-in
/// operations that need a sibling slot (`Insert`, `Replace`, `SetName`) cannot act on such a
/// target.
fn sibling_context<'d>(
    target: Element<'d>,
) -> Option<(Element<'d>, usize, Vec<ChildOfElement<'d>>)> {
    let ParentOfChild::Element(parent) = target.parent()? else {
        return None;
    };
    let siblings = parent.children();
    let idx = siblings
        .iter()
        .position(|c| matches!(c, ChildOfElement::Element(e) if *e == target))?;
    Some((parent, idx, siblings))
}

fn add_op<'d>(
    document: Document<'d>,
    op: &PathedValueOrderOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let Some(xpath) = op.xpath.as_deref() else {
        missing_field_diagnostic(diagnostics, key, "xpath");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    let order = op.order.unwrap_or(PatchOrderMode::Append);
    let value_xml = op.value_xml.as_deref().unwrap_or("");
    for target in matches {
        let fresh = parse_fragment(document, value_xml).nodes;
        match order {
            PatchOrderMode::Append => target.append_children(fresh),
            PatchOrderMode::Prepend => {
                let mut new_children = fresh;
                new_children.extend(target.children());
                target.replace_children(new_children);
            }
        }
    }
    true
}

/// `PatchOperationInsert.ApplyWorker` anchors every inserted value node to the *original* target
/// sibling rather than the previously inserted one, so multiple `<value>` children always end up
/// adjacent to the target in reverse order -- for both `Append` and `Prepend`. This is a real
/// upstream quirk (verified against the decompiled source), faithfully reproduced, not a RimEdit
/// bug: `docs/patches-editor/07-preview-engine.md` documents the derivation.
fn insert_op<'d>(
    document: Document<'d>,
    op: &PathedValueOrderOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let Some(xpath) = op.xpath.as_deref() else {
        missing_field_diagnostic(diagnostics, key, "xpath");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    let order = op.order.unwrap_or(PatchOrderMode::Prepend);
    let value_xml = op.value_xml.as_deref().unwrap_or("");
    for target in matches {
        let Some((parent, idx, mut siblings)) = sibling_context(target) else {
            continue;
        };
        let mut fresh = parse_fragment(document, value_xml).nodes;
        fresh.reverse();
        let insert_at = match order {
            PatchOrderMode::Append => idx + 1,
            PatchOrderMode::Prepend => idx,
        };
        siblings.splice(insert_at..insert_at, fresh);
        parent.replace_children(siblings);
    }
    true
}

fn remove_op<'d>(
    document: Document<'d>,
    op: &PathedOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let Some(xpath) = op.xpath.as_deref() else {
        missing_field_diagnostic(diagnostics, key, "xpath");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    for target in matches {
        if let Some(ParentOfChild::Element(parent)) = target.parent() {
            parent.remove_child(target);
        }
    }
    true
}

fn replace_op<'d>(
    document: Document<'d>,
    op: &PathedValueOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let Some(xpath) = op.xpath.as_deref() else {
        missing_field_diagnostic(diagnostics, key, "xpath");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    let value_xml = op.value_xml.as_deref().unwrap_or("");
    for target in matches {
        let Some((parent, idx, mut siblings)) = sibling_context(target) else {
            continue;
        };
        let fresh = parse_fragment(document, value_xml).nodes;
        siblings.splice(idx..idx + 1, fresh);
        parent.replace_children(siblings);
    }
    true
}

fn attribute_add_op<'d>(
    document: Document<'d>,
    op: &AttributeValueOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let (Some(xpath), Some(attribute), Some(value)) = (
        op.xpath.as_deref(),
        op.attribute.as_deref(),
        op.value.as_deref(),
    ) else {
        missing_field_diagnostic(diagnostics, key, "xpath/attribute/value");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    let mut any = false;
    for el in matches {
        if el.attribute_value(attribute).is_none() {
            el.set_attribute_value(attribute, value);
            any = true;
        }
    }
    let _ = document;
    any
}

fn attribute_set_op<'d>(
    document: Document<'d>,
    op: &AttributeValueOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let (Some(xpath), Some(attribute), Some(value)) = (
        op.xpath.as_deref(),
        op.attribute.as_deref(),
        op.value.as_deref(),
    ) else {
        missing_field_diagnostic(diagnostics, key, "xpath/attribute/value");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    let mut any = false;
    for el in matches {
        el.set_attribute_value(attribute, value);
        any = true;
    }
    any
}

fn attribute_remove_op<'d>(
    document: Document<'d>,
    op: &AttributeOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let (Some(xpath), Some(attribute)) = (op.xpath.as_deref(), op.attribute.as_deref()) else {
        missing_field_diagnostic(diagnostics, key, "xpath/attribute");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    let mut any = false;
    for el in matches {
        if el.attribute_value(attribute).is_some() {
            el.remove_attribute(attribute);
            any = true;
        }
    }
    any
}

fn add_mod_extension_op<'d>(
    document: Document<'d>,
    op: &PathedValueOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let Some(xpath) = op.xpath.as_deref() else {
        missing_field_diagnostic(diagnostics, key, "xpath");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    let value_xml = op.value_xml.as_deref().unwrap_or("");
    for target in matches {
        let mod_extensions =
            first_child_element_named(target, "modExtensions").unwrap_or_else(|| {
                let el = document.create_element("modExtensions");
                target.append_child(el);
                el
            });
        let fresh = parse_fragment(document, value_xml).nodes;
        mod_extensions.append_children(fresh);
    }
    true
}

fn set_name_op<'d>(
    document: Document<'d>,
    op: &SetNameOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let (Some(xpath), Some(name)) = (op.xpath.as_deref(), op.name.as_deref()) else {
        missing_field_diagnostic(diagnostics, key, "xpath/name");
        return false;
    };
    let matches = match select_elements(document, xpath) {
        Ok(m) => m,
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            return false;
        }
    };
    if matches.is_empty() {
        xpath_no_match_diagnostic(diagnostics, key, xpath);
        return false;
    }
    for target in matches {
        let Some((parent, idx, mut siblings)) = sibling_context(target) else {
            continue;
        };
        let new_el = document.create_element(name);
        // Only `target`'s children are copied (RimWorld's `InnerXml` copy) -- its own attributes
        // (including `Name`/`ParentName`) are intentionally dropped, matching real behavior.
        let cloned_children: Vec<ChildOfElement<'d>> = target
            .children()
            .into_iter()
            .map(|c| clone_child_of_element(document, c))
            .collect();
        new_el.append_children(cloned_children);
        siblings[idx] = ChildOfElement::Element(new_el);
        parent.replace_children(siblings);
    }
    true
}

fn test_op<'d>(
    document: Document<'d>,
    op: &PathedOperation,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
) -> bool {
    let Some(xpath) = op.xpath.as_deref() else {
        missing_field_diagnostic(diagnostics, key, "xpath");
        return false;
    };
    match select_nodes(document, xpath) {
        Ok(nodes) => !nodes.is_empty(),
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            false
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn sequence_apply<'d>(
    document: Document<'d>,
    children: &[PatchOperationNode],
    ctx: &ApplyContext<'_>,
    trace: &mut Vec<OperationTraceEntry>,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    is_partial: &mut bool,
    key: &PatchOperationKey,
) -> bool {
    for (i, child) in children.iter().enumerate() {
        if !apply_node(document, child, ctx, trace, diagnostics, is_partial) {
            let skipped = children.len() - i - 1;
            if skipped > 0 {
                diagnostics.push(ApplyDiagnostic {
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

#[allow(clippy::too_many_arguments)]
fn find_mod_apply<'d>(
    document: Document<'d>,
    mods: &[String],
    match_op: &Option<Box<PatchOperationNode>>,
    nomatch_op: &Option<Box<PatchOperationNode>>,
    ctx: &ApplyContext<'_>,
    trace: &mut Vec<OperationTraceEntry>,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    is_partial: &mut bool,
) -> bool {
    let matched = mods.iter().any(|m| mod_is_active(ctx.active_mod_names, m));
    if matched {
        if let Some(m) = match_op {
            return apply_node(document, m, ctx, trace, diagnostics, is_partial);
        }
    } else if let Some(nm) = nomatch_op {
        return apply_node(document, nm, ctx, trace, diagnostics, is_partial);
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
        trace.push(OperationTraceEntry {
            key: key.clone(),
            class_name: m.class_name.clone(),
            status: OperationTraceStatus::Skipped,
            message: Some(format!("Requires mod {} to be active", mod_list)),
        });
        diagnostics.push(ApplyDiagnostic {
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
#[allow(clippy::too_many_arguments)]
fn conditional_apply<'d>(
    document: Document<'d>,
    xpath: Option<&str>,
    match_op: &Option<Box<PatchOperationNode>>,
    nomatch_op: &Option<Box<PatchOperationNode>>,
    ctx: &ApplyContext<'_>,
    trace: &mut Vec<OperationTraceEntry>,
    diagnostics: &mut Vec<ApplyDiagnostic>,
    is_partial: &mut bool,
    key: &PatchOperationKey,
) -> bool {
    let Some(xpath) = xpath else {
        missing_field_diagnostic(diagnostics, key, "xpath");
        return false;
    };
    let matched = match select_nodes(document, xpath) {
        Ok(nodes) => !nodes.is_empty(),
        Err(e) => {
            xpath_error_diagnostic(diagnostics, key, xpath, &e);
            false
        }
    };

    if matched {
        if let Some(m) = match_op {
            return apply_node(document, m, ctx, trace, diagnostics, is_partial);
        }
    } else if let Some(nm) = nomatch_op {
        return apply_node(document, nm, ctx, trace, diagnostics, is_partial);
    }
    if match_op.is_none() {
        return nomatch_op.is_some();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patches::model::{
        AttributeOperation, AttributeValueOperation, PathedOperation, PathedValueOperation,
        PathedValueOrderOperation, SetNameOperation,
    };
    use crate::patches::parser::parse_patch_file;
    use std::collections::BTreeMap;
    use sxd_document::Package;

    fn combined_doc<'d>(document: Document<'d>, defs_xml: &str) -> Element<'d> {
        let defs = document.create_element("Defs");
        document.root().append_child(defs);
        let result = parse_fragment(document, defs_xml);
        assert!(!result.had_fatal_error, "{:?}", result.diagnostics);
        defs.append_children(result.nodes);
        defs
    }

    fn key(id: PatchOperationId) -> PatchOperationKey {
        PatchOperationKey {
            location_id: "loc".to_string(),
            relative_path: "Patches/Test.xml".to_string(),
            operation_id: id,
        }
    }

    fn no_op_options() -> (
        Vec<String>,
        CustomOperationMetadataMap,
        HashSet<PatchOperationKey>,
    ) {
        (Vec::new(), BTreeMap::new(), HashSet::new())
    }

    fn apply_one<'d>(document: Document<'d>, node: &PatchOperationNode) -> PatchApplyResult {
        let (active_mods, custom_ops, disabled) = no_op_options();
        let options = PatchApplyOptions {
            active_mod_names: &active_mods,
            custom_operations: &custom_ops,
            disabled: &disabled,
        };
        let ops = vec![TopLevelOperation {
            location_id: "loc".to_string(),
            relative_path: "Patches/Test.xml".to_string(),
            node,
        }];
        apply_patch_operations(document, &ops, &options)
    }

    #[test]
    fn add_appends_by_default() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef><defName>Wall</defName></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationAdd".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Add(PathedValueOrderOperation {
                xpath: Some("Defs/ThingDef".to_string()),
                value_xml: Some("<label>wall</label>".to_string()),
                order: None,
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert!(!result.is_partial);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert_eq!(
            first_child_element_named(thing_def, "label").map(|e| element_text(e)),
            Some(Some("wall".to_string()))
        );
    }

    #[test]
    fn add_prepend_preserves_value_order() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef><a>existing</a></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationAdd".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Add(PathedValueOrderOperation {
                xpath: Some("Defs/ThingDef".to_string()),
                value_xml: Some("<b>1</b><c>2</c>".to_string()),
                order: Some(PatchOrderMode::Prepend),
            }),
            span: None,
        };
        apply_one(doc, &node);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        let names: Vec<String> = thing_def
            .children()
            .into_iter()
            .filter_map(|c| match c {
                ChildOfElement::Element(e) => Some(e.name().local_part().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["b", "c", "a"]);
    }

    #[test]
    fn insert_reverses_multiple_value_children() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef><target /></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationInsert".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Insert(PathedValueOrderOperation {
                xpath: Some("Defs/ThingDef/target".to_string()),
                value_xml: Some("<a/><b/>".to_string()),
                order: None,
            }),
            span: None,
        };
        apply_one(doc, &node);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        let names: Vec<String> = thing_def
            .children()
            .into_iter()
            .filter_map(|c| match c {
                ChildOfElement::Element(e) => Some(e.name().local_part().to_string()),
                _ => None,
            })
            .collect();
        // Default order is Prepend (insert before target); RimWorld's real reversal quirk means
        // the two value children land as b, a (not a, b) immediately before `target`.
        assert_eq!(names, vec!["b", "a", "target"]);
    }

    #[test]
    fn replace_preserves_value_order() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef><target /></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationReplace".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Replace(PathedValueOperation {
                xpath: Some("Defs/ThingDef/target".to_string()),
                value_xml: Some("<a/><b/>".to_string()),
            }),
            span: None,
        };
        apply_one(doc, &node);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        let names: Vec<String> = thing_def
            .children()
            .into_iter()
            .filter_map(|c| match c {
                ChildOfElement::Element(e) => Some(e.name().local_part().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn remove_deletes_matched_node() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef><target /></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationRemove".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Remove(PathedOperation {
                xpath: Some("Defs/ThingDef/target".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert!(first_child_element_named(thing_def, "target").is_none());
    }

    #[test]
    fn attribute_add_skips_existing_attribute() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef Foo=\"1\"></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationAttributeAdd".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::AttributeAdd(AttributeValueOperation {
                xpath: Some("Defs/ThingDef".to_string()),
                attribute: Some("Foo".to_string()),
                value: Some("2".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert_eq!(thing_def.attribute_value("Foo"), Some("1"));
    }

    #[test]
    fn attribute_set_overwrites_or_creates() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef Foo=\"1\"></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationAttributeSet".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::AttributeSet(AttributeValueOperation {
                xpath: Some("Defs/ThingDef".to_string()),
                attribute: Some("Foo".to_string()),
                value: Some("2".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert_eq!(thing_def.attribute_value("Foo"), Some("2"));
    }

    #[test]
    fn attribute_remove_only_succeeds_if_present() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationAttributeRemove".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::AttributeRemove(AttributeOperation {
                xpath: Some("Defs/ThingDef".to_string()),
                attribute: Some("Foo".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
        let _ = defs;
    }

    #[test]
    fn add_mod_extension_creates_container_once() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationAddModExtension".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::AddModExtension(PathedValueOperation {
                xpath: Some("Defs/ThingDef".to_string()),
                value_xml: Some("<li Class=\"Foo\"/>".to_string()),
            }),
            span: None,
        };
        apply_one(doc, &node);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        let mod_ext = first_child_element_named(thing_def, "modExtensions").unwrap();
        assert_eq!(child_elements_named(mod_ext, "li").len(), 1);
    }

    #[test]
    fn set_name_renames_and_drops_attributes() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef Name=\"Old\"><a>1</a></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationSetName".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::SetName(SetNameOperation {
                xpath: Some("Defs/ThingDef".to_string()),
                name: Some("Renamed".to_string()),
            }),
            span: None,
        };
        apply_one(doc, &node);
        assert!(child_elements_named(defs, "ThingDef").is_empty());
        let renamed = child_elements_named(defs, "Renamed");
        assert_eq!(renamed.len(), 1);
        assert_eq!(renamed[0].attribute_value("Name"), None);
        assert_eq!(
            first_child_element_named(renamed[0], "a").map(element_text),
            Some(Some("1".to_string()))
        );
    }

    #[test]
    fn success_mode_always_forces_success() {
        let package = Package::new();
        let doc = package.as_document();
        combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationRemove".to_string(),
            success: PatchSuccessMode::Always,
            attributes: vec![],
            kind: PatchOperationKind::Remove(PathedOperation {
                xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
    }

    #[test]
    fn success_mode_never_forces_failure() {
        let package = Package::new();
        let doc = package.as_document();
        combined_doc(doc, "<ThingDef><target/></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationRemove".to_string(),
            success: PatchSuccessMode::Never,
            attributes: vec![],
            kind: PatchOperationKind::Remove(PathedOperation {
                xpath: Some("Defs/ThingDef/target".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
    }

    #[test]
    fn success_mode_invert_flips_result() {
        let package = Package::new();
        let doc = package.as_document();
        combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationRemove".to_string(),
            success: PatchSuccessMode::Invert,
            attributes: vec![],
            kind: PatchOperationKind::Remove(PathedOperation {
                xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        // Raw result is false (no match); Invert flips it to true.
        assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
    }

    #[test]
    fn sequence_stops_at_first_failed_child() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef><a/></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationSequence".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Sequence(vec![
                PatchOperationNode {
                    id: 1,
                    class_name: "PatchOperationRemove".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Remove(PathedOperation {
                        xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
                    }),
                    span: None,
                },
                PatchOperationNode {
                    id: 2,
                    class_name: "PatchOperationRemove".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Remove(PathedOperation {
                        xpath: Some("Defs/ThingDef/a".to_string()),
                    }),
                    span: None,
                },
            ]),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace.len(), 2); // sequence itself + first (failed) child only
        assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert!(
            first_child_element_named(thing_def, "a").is_some(),
            "second child must not run"
        );
    }

    #[test]
    fn conditional_runs_match_when_xpath_matches() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef><a/></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationConditional".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Conditional {
                xpath: Some("Defs/ThingDef/a".to_string()),
                match_op: Some(Box::new(PatchOperationNode {
                    id: 1,
                    class_name: "PatchOperationRemove".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Remove(PathedOperation {
                        xpath: Some("Defs/ThingDef/a".to_string()),
                    }),
                    span: None,
                })),
                nomatch_op: None,
            },
            span: None,
        };
        apply_one(doc, &node);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert!(first_child_element_named(thing_def, "a").is_none());
    }

    #[test]
    fn conditional_runs_nomatch_when_xpath_does_not_match() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationConditional".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Conditional {
                xpath: Some("Defs/ThingDef/missing".to_string()),
                match_op: None,
                nomatch_op: Some(Box::new(PatchOperationNode {
                    id: 1,
                    class_name: "PatchOperationAdd".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Add(PathedValueOrderOperation {
                        xpath: Some("Defs/ThingDef".to_string()),
                        value_xml: Some("<label>hi</label>".to_string()),
                        order: None,
                    }),
                    span: None,
                })),
            },
            span: None,
        };
        apply_one(doc, &node);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert!(first_child_element_named(thing_def, "label").is_some());
    }

    #[test]
    fn find_mod_matches_by_display_name() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationFindMod".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::FindMod {
                mods: vec!["Harmony".to_string()],
                match_op: Some(Box::new(PatchOperationNode {
                    id: 1,
                    class_name: "PatchOperationAdd".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Add(PathedValueOrderOperation {
                        xpath: Some("Defs/ThingDef".to_string()),
                        value_xml: Some("<label>hi</label>".to_string()),
                        order: None,
                    }),
                    span: None,
                })),
                nomatch_op: None,
            },
            span: None,
        };
        let options = PatchApplyOptions {
            active_mod_names: &["Harmony".to_string()],
            custom_operations: &BTreeMap::new(),
            disabled: &HashSet::new(),
        };
        let ops = vec![TopLevelOperation {
            location_id: "loc".to_string(),
            relative_path: "Patches/Test.xml".to_string(),
            node: &node,
        }];
        apply_patch_operations(doc, &ops, &options);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert!(first_child_element_named(thing_def, "label").is_some());
    }

    #[test]
    fn find_mod_with_no_nomatch_branch_and_inactive_mod_reports_skip_reason() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationFindMod".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::FindMod {
                mods: vec!["Harmony".to_string()],
                match_op: Some(Box::new(PatchOperationNode {
                    id: 1,
                    class_name: "PatchOperationAdd".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Add(PathedValueOrderOperation {
                        xpath: Some("Defs/ThingDef".to_string()),
                        value_xml: Some("<label>hi</label>".to_string()),
                        order: None,
                    }),
                    span: None,
                })),
                nomatch_op: None,
            },
            span: None,
        };
        let options = PatchApplyOptions {
            active_mod_names: &[],
            custom_operations: &BTreeMap::new(),
            disabled: &HashSet::new(),
        };
        let ops = vec![TopLevelOperation {
            location_id: "loc".to_string(),
            relative_path: "Patches/Test.xml".to_string(),
            node: &node,
        }];
        let result = apply_patch_operations(doc, &ops, &options);

        // The wrapped Add never ran, so nothing changed in the document.
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert!(first_child_element_named(thing_def, "label").is_none());

        // Its own trace entry (keyed to the inner Add, not the FindMod wrapper) reports Skipped
        // with an explanatory message, rather than being entirely absent from the trace.
        let inner_key = key(1);
        let inner_trace = result
            .trace
            .iter()
            .find(|t| t.key == inner_key)
            .expect("expected a trace entry for the skipped inner operation");
        assert_eq!(inner_trace.status, OperationTraceStatus::Skipped);
        assert_eq!(
            inner_trace.message.as_deref(),
            Some("Requires mod \"Harmony\" to be active")
        );

        let diagnostic = result
            .diagnostics
            .iter()
            .find(|d| d.code == "patch_find_mod_dependency_not_active")
            .expect("expected a patch_find_mod_dependency_not_active diagnostic");
        assert_eq!(diagnostic.key, Some(inner_key));
        assert!(
            diagnostic.message.contains("\"Harmony\""),
            "{}",
            diagnostic.message
        );
    }

    #[test]
    fn find_mod_with_nomatch_branch_is_normal_branching_not_a_skip() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationFindMod".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::FindMod {
                mods: vec!["Harmony".to_string()],
                match_op: Some(Box::new(PatchOperationNode {
                    id: 1,
                    class_name: "PatchOperationAdd".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Add(PathedValueOrderOperation {
                        xpath: Some("Defs/ThingDef".to_string()),
                        value_xml: Some("<label>hi</label>".to_string()),
                        order: None,
                    }),
                    span: None,
                })),
                nomatch_op: Some(Box::new(PatchOperationNode {
                    id: 2,
                    class_name: "PatchOperationAdd".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Add(PathedValueOrderOperation {
                        xpath: Some("Defs/ThingDef".to_string()),
                        value_xml: Some("<label>fallback</label>".to_string()),
                        order: None,
                    }),
                    span: None,
                })),
            },
            span: None,
        };
        let options = PatchApplyOptions {
            active_mod_names: &[],
            custom_operations: &BTreeMap::new(),
            disabled: &HashSet::new(),
        };
        let ops = vec![TopLevelOperation {
            location_id: "loc".to_string(),
            relative_path: "Patches/Test.xml".to_string(),
            node: &node,
        }];
        let result = apply_patch_operations(doc, &ops, &options);

        // A `nomatch` branch is normal, expected behavior -- it ran, so no "why didn't this
        // apply" diagnostic should fire.
        assert!(!result
            .diagnostics
            .iter()
            .any(|d| d.code == "patch_find_mod_dependency_not_active"));
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert_eq!(
            first_child_element_named(thing_def, "label").map(element_text),
            Some(Some("fallback".to_string()))
        );
    }

    #[test]
    fn disabled_operation_is_skipped_and_treated_as_success() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = combined_doc(doc, "<ThingDef><a/></ThingDef>");
        let node = PatchOperationNode {
            id: 5,
            class_name: "PatchOperationRemove".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Remove(PathedOperation {
                xpath: Some("Defs/ThingDef/a".to_string()),
            }),
            span: None,
        };
        let mut disabled = HashSet::new();
        disabled.insert(key(5));
        let options = PatchApplyOptions {
            active_mod_names: &[],
            custom_operations: &BTreeMap::new(),
            disabled: &disabled,
        };
        let ops = vec![TopLevelOperation {
            location_id: "loc".to_string(),
            relative_path: "Patches/Test.xml".to_string(),
            node: &node,
        }];
        let result = apply_patch_operations(doc, &ops, &options);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Skipped);
        let thing_def = child_elements_named(defs, "ThingDef")[0];
        assert!(first_child_element_named(thing_def, "a").is_some());
    }

    #[test]
    fn unsupported_custom_operation_marks_partial() {
        let package = Package::new();
        let doc = package.as_document();
        combined_doc(doc, "<ThingDef></ThingDef>");
        let raw = "<Operation Class=\"MyMod.PatchOperationFoo\"></Operation>";
        let file = parse_patch_file("Patches/Custom.xml", &format!("<Patch>{}</Patch>", raw));
        let node = &file.operations[0];
        let result = apply_one(doc, node);
        assert!(result.is_partial);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Unsupported);
    }

    #[test]
    fn xpath_no_match_reports_diagnostic_distinct_from_xpath_error() {
        let package = Package::new();
        let doc = package.as_document();
        combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationAdd".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Add(PathedValueOrderOperation {
                xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
                value_xml: Some("<label>hi</label>".to_string()),
                order: None,
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Failed);
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "patch_apply_xpath_no_match"));
        assert!(!result
            .diagnostics
            .iter()
            .any(|d| d.code == "patch_apply_xpath_error"));
    }

    #[test]
    fn success_always_masking_a_real_failure_reports_diagnostic() {
        let package = Package::new();
        let doc = package.as_document();
        combined_doc(doc, "<ThingDef></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationRemove".to_string(),
            success: PatchSuccessMode::Always,
            attributes: vec![],
            kind: PatchOperationKind::Remove(PathedOperation {
                xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "patch_apply_success_always_masks_failure"));
    }

    #[test]
    fn success_always_does_not_report_masking_diagnostic_when_operation_really_succeeds() {
        let package = Package::new();
        let doc = package.as_document();
        combined_doc(doc, "<ThingDef><target/></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationRemove".to_string(),
            success: PatchSuccessMode::Always,
            attributes: vec![],
            kind: PatchOperationKind::Remove(PathedOperation {
                xpath: Some("Defs/ThingDef/target".to_string()),
            }),
            span: None,
        };
        let result = apply_one(doc, &node);
        assert_eq!(result.trace[0].status, OperationTraceStatus::Applied);
        assert!(!result
            .diagnostics
            .iter()
            .any(|d| d.code == "patch_apply_success_always_masks_failure"));
    }

    #[test]
    fn sequence_short_circuit_reports_diagnostic_with_skipped_count() {
        let package = Package::new();
        let doc = package.as_document();
        combined_doc(doc, "<ThingDef><a/><b/></ThingDef>");
        let node = PatchOperationNode {
            id: 0,
            class_name: "PatchOperationSequence".to_string(),
            success: PatchSuccessMode::Normal,
            attributes: vec![],
            kind: PatchOperationKind::Sequence(vec![
                PatchOperationNode {
                    id: 1,
                    class_name: "PatchOperationRemove".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Remove(PathedOperation {
                        xpath: Some("Defs/ThingDef/doesNotExist".to_string()),
                    }),
                    span: None,
                },
                PatchOperationNode {
                    id: 2,
                    class_name: "PatchOperationRemove".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Remove(PathedOperation {
                        xpath: Some("Defs/ThingDef/a".to_string()),
                    }),
                    span: None,
                },
                PatchOperationNode {
                    id: 3,
                    class_name: "PatchOperationRemove".to_string(),
                    success: PatchSuccessMode::Normal,
                    attributes: vec![],
                    kind: PatchOperationKind::Remove(PathedOperation {
                        xpath: Some("Defs/ThingDef/b".to_string()),
                    }),
                    span: None,
                },
            ]),
            span: None,
        };
        let result = apply_one(doc, &node);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|d| d.code == "patch_apply_sequence_short_circuited")
            .expect("expected a sequence short-circuit diagnostic");
        assert!(
            diagnostic
                .message
                .contains("2 subsequent operations skipped"),
            "{}",
            diagnostic.message
        );
        assert_eq!(diagnostic.key, Some(key(0)));
    }

    fn element_text(el: Element<'_>) -> Option<String> {
        super::super::dom::element_text(el)
    }
}
