use std::collections::HashMap;

use sxd_document::dom::{ChildOfElement, Document};

use crate::patches::dom::parse_fragment;
use crate::patches::{
    OperationTraceEntry, OperationTraceStatus, PatchFile, PatchIndex, PatchOperationKey,
    PatchOperationKind, PatchPreviewSupport,
};

use super::model::{PatchPreviewConflictDiagnostic, PatchPreviewOperationSummary};
use super::operation_lookup::find_operation_node;

/// Every top-level element tag name in a `PatchOperationAdd`'s `<value>` payload -- a payload can
/// add more than one sibling field at once (e.g. `<value><description/><label/></value>`), and
/// every one of them is a candidate for `detect_visible_conflicts`'s `patch_conflict_duplicate_add_child`,
/// not just the first. Excludes `<li>`: that's RimWorld's list-item convention, never a scalar
/// field name, so two operations each adding an `<li>` to the same list container at the same
/// xpath is completely normal (that's how a list field accumulates entries across multiple
/// patches) rather than a duplicate-scalar-field risk. Parses `value_xml` into `document` purely
/// to read tag names; the parsed nodes are never attached to any tree and are dropped (garbage in
/// the arena) once this returns.
fn add_value_root_tags(document: Document<'_>, value_xml: &str) -> Vec<String> {
    parse_fragment(document, value_xml)
        .nodes
        .into_iter()
        .filter_map(|n| match n {
            ChildOfElement::Element(el) => Some(el.name().local_part().to_string()),
            _ => None,
        })
        .filter(|name| name != "li")
        .collect()
}

/// Detects specific, explainable conflicts among the operations already known to affect the
/// selected Def (`summaries`, already scoped and sorted in default preview order by the caller).
/// Each detected conflict gets its own diagnostic code so the UI/tests can distinguish *why* two
/// operations conflict, complementing `PatchImpactGraph::conflicts_involving_def`'s generic
/// "more than one operation touches this Def" signal (kept separately, above):
///
/// - `patch_conflict_duplicate_replace_or_remove`: two or more `Replace`/`Remove` operations
///   target the exact same xpath -- at most one can meaningfully "win".
/// - `patch_conflict_duplicate_add_child`: two or more `Add` operations at the same xpath add a
///   child element with the same tag name -- a likely duplicate if that field expects one value.
/// - `patch_conflict_targets_removed_node`: a later operation (by actual apply order) targets a
///   node an earlier `Remove` operation already removed (same xpath, or a path nested under it).
/// - `patch_conflict_custom_operation_unpreviewable`: a `Custom`/`Unknown`-classified operation
///   affects this Def but has no preview support, so the preview's final XML is only an
///   approximation for it.
///
/// Only operations that actually took effect in *this* preview request can conflict with each
/// other: a disabled (`Skipped`) or `Failed` (e.g. no xpath match) operation changed nothing, so
/// it must not still be reported as conflicting once the user disables one side of a duplicate
/// pair, nor should its removed/added effect be assumed by an ordering check. `Unsupported`
/// (custom/unknown, assumed successful per the apply engine's own model -- see `apply_node`)
/// still counts, since its real effect on the document is unknown. `trace` (the actual
/// visit-order record from applying `final_order`, the caller's already reorder-adjusted
/// operation stream) is used instead of static `file_order`/`operation_id` to decide "later" for
/// `patch_conflict_targets_removed_node`, so reordering the preview changes which operations are
/// flagged, not just which operations run.
pub(super) fn detect_visible_conflicts(
    summaries: &[PatchPreviewOperationSummary],
    trace: &[OperationTraceEntry],
    patch_index: &PatchIndex,
    patch_files: &[PatchFile],
    document: Document<'_>,
    def_type: &str,
    def_name: &str,
) -> Vec<PatchPreviewConflictDiagnostic> {
    let mut diagnostics = Vec::new();

    let order_index: HashMap<&PatchOperationKey, usize> = trace
        .iter()
        .enumerate()
        .map(|(i, entry)| (&entry.key, i))
        .collect();

    let effective: Vec<&PatchPreviewOperationSummary> = summaries
        .iter()
        .filter(|s| {
            matches!(
                s.status,
                Some(OperationTraceStatus::Applied) | Some(OperationTraceStatus::Unsupported)
            )
        })
        .collect();

    // (a) two or more Replace/Remove operations target the exact same xpath.
    let mut by_xpath: HashMap<&str, Vec<&PatchPreviewOperationSummary>> = HashMap::new();
    for &s in &effective {
        if matches!(
            s.class_name.as_str(),
            "PatchOperationReplace" | "PatchOperationRemove"
        ) {
            if let Some(xpath) = s.xpath.as_deref() {
                by_xpath.entry(xpath).or_default().push(s);
            }
        }
    }
    for (xpath, group) in &by_xpath {
        if group.len() > 1 {
            for s in group {
                diagnostics.push(
                    PatchPreviewConflictDiagnostic::new(
                        "patch_conflict_duplicate_replace_or_remove",
                        s.key.clone(),
                        format!(
                            "{} operations target \"{}\" with Replace/Remove -- verify only one should apply",
                            group.len(),
                            xpath
                        ),
                    )
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("count", group.len().into()),
                        ("xpath", (*xpath).into()),
                    ])),
                );
            }
        }
    }

    // (b) two or more Add operations at the same xpath add a child with the same tag name.
    let mut by_xpath_and_tag: HashMap<(&str, String), Vec<&PatchPreviewOperationSummary>> =
        HashMap::new();
    for &s in &effective {
        if s.class_name != "PatchOperationAdd" {
            continue;
        }
        let Some(xpath) = s.xpath.as_deref() else {
            continue;
        };
        let Some(node) = find_operation_node(patch_index, patch_files, &s.key) else {
            continue;
        };
        let PatchOperationKind::Add(op) = &node.kind else {
            continue;
        };
        let Some(value_xml) = op.value_xml.as_deref() else {
            continue;
        };
        for tag in add_value_root_tags(document, value_xml) {
            by_xpath_and_tag.entry((xpath, tag)).or_default().push(s);
        }
    }
    for ((xpath, tag), group) in &by_xpath_and_tag {
        if group.len() > 1 {
            for s in group {
                diagnostics.push(
                    PatchPreviewConflictDiagnostic::new(
                        "patch_conflict_duplicate_add_child",
                        s.key.clone(),
                        format!(
                            "{} Add operations add a <{}> child at \"{}\" -- if this field expects a single value, only the last one applied will take effect",
                            group.len(),
                            tag,
                            xpath
                        ),
                    )
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("count", group.len().into()),
                        ("childName", tag.as_str().into()),
                        ("xpath", (*xpath).into()),
                    ])),
                );
            }
        }
    }

    // (c) a later operation (by actual apply order) targets a node an earlier Remove already
    // removed -- either the exact same xpath, or a path nested under it. Unlike (a)/(b)/(f), the
    // *later* side (`s`) deliberately is not restricted to `effective`: a "no match" `Failed`
    // result on `s` is the direct, expected symptom of this exact conflict (its target no longer
    // exists), not a reason to hide the diagnostic -- only a `Skipped` (disabled) `s` is excluded,
    // since a disabled operation isn't really trying to act on anything. The earlier `remove` side
    // still requires `Applied` specifically: only then did it actually remove the node in this
    // preview request (a disabled or failed Remove removed nothing for `s` to conflict with).
    let removes: Vec<&PatchPreviewOperationSummary> = summaries
        .iter()
        .filter(|s| {
            s.class_name == "PatchOperationRemove"
                && s.status == Some(OperationTraceStatus::Applied)
        })
        .collect();
    for remove in &removes {
        let Some(remove_xpath) = remove.xpath.as_deref() else {
            continue;
        };
        let Some(&remove_pos) = order_index.get(&remove.key) else {
            continue;
        };
        for s in summaries {
            if matches!(s.status, Some(OperationTraceStatus::Skipped)) {
                continue;
            }
            if remove.key == s.key {
                continue;
            }
            let Some(&s_pos) = order_index.get(&s.key) else {
                continue;
            };
            if s_pos <= remove_pos {
                continue;
            }
            let Some(xpath) = s.xpath.as_deref() else {
                continue;
            };
            let targets_removed =
                xpath == remove_xpath || xpath.starts_with(&format!("{}/", remove_xpath));
            if targets_removed {
                diagnostics.push(
                    PatchPreviewConflictDiagnostic::new(
                        "patch_conflict_targets_removed_node",
                        s.key.clone(),
                        format!(
                            "Targets \"{}\", which an earlier operation removes via \"{}\" -- this operation may have nothing to act on",
                            xpath, remove_xpath
                        ),
                    )
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("xpath", xpath.into()),
                        ("removeXpath", remove_xpath.into()),
                    ])),
                );
            }
        }
    }

    // (f) a custom/unknown operation affects this Def but cannot be previewed.
    for &s in &effective {
        if let PatchPreviewSupport::Unsupported { reason } = &s.preview_support {
            diagnostics.push(
                PatchPreviewConflictDiagnostic::new(
                    "patch_conflict_custom_operation_unpreviewable",
                    s.key.clone(),
                    format!(
                        "'{}' may affect {} {}, but cannot be previewed: {}",
                        s.class_name, def_type, def_name, reason
                    ),
                )
                .with_args(crate::diagnostics::diagnostic_args([
                    ("className", s.class_name.as_str().into()),
                    ("defType", def_type.into()),
                    ("defName", def_name.into()),
                    ("reason", reason.as_str().into()),
                ])),
            );
        }
    }

    diagnostics
}
