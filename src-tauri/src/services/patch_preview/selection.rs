use std::collections::{HashMap, HashSet};

use sxd_document::dom::{ChildOfElement, Document, Element, ParentOfChild};
use sxd_xpath::nodeset::Node as XPathNode;

use crate::patches::dom::{element_text, first_child_element_named, select_nodes};

pub(super) fn def_name_of(el: Element<'_>) -> Option<String> {
    first_child_element_named(el, "defName").and_then(element_text)
}

/// True if `el` is the Def the caller identified via `def_type`/`def_name`: a `def_type` element
/// whose `<defName>` equals `def_name`, or -- falling back for `Abstract="True"` parent templates,
/// which are never deserialized as real Defs and so have no `defName` at all -- whose `Name`
/// attribute equals `def_name`. This is the "or node identity" half of issue 08's "Preview selected
/// Def by Def type and defName or node identity" requirement: the frontend has no way to identify
/// an abstract template except by its `Name` (there is no correlation between the editor's
/// `nodeId`s and this preview engine's independently-parsed combined document), so `Name` is used
/// as that alternate identity wherever the selected Def is looked up by (`def_type`, `def_name`).
pub(super) fn matches_selected_def(el: Element<'_>, def_type: &str, def_name: &str) -> bool {
    if el.name().local_part() != def_type {
        return false;
    }
    def_name_of(el).as_deref() == Some(def_name) || el.attribute_value("Name") == Some(def_name)
}

pub(super) fn top_level_def_elements<'d>(defs_root: Element<'d>) -> Vec<Element<'d>> {
    defs_root
        .children()
        .into_iter()
        .filter_map(|c| match c {
            ChildOfElement::Element(el) => Some(el),
            _ => None,
        })
        .collect()
}

/// The `Name` values of the selected Def's own registration (if any) and every named ancestor
/// reached by walking `ParentName`, evaluated against the *pre-patch* document. Used to
/// runtime-correlate patch operations whose XPath the impact graph could not statically resolve
/// to a Def (see `xpath_touches_target`) -- most commonly an operation targeting an abstract
/// parent by `[@Name="..."]`, which `patches::impact_graph::infer_xpath_target` classifies as
/// `XPathTarget::Unsupported` rather than a specific Def or DefType.
///
/// This is a pre-patch approximation: a patch that itself adds/changes/removes `ParentName`
/// partway through the stream can change the *real* ancestor chain after it runs (`docs/patches-editor/07-preview-engine.md`'s
/// required "patch changes or removes ParentName" fixture is exactly this), and such a
/// mid-stream change is not reflected here. Good enough to catch the common case (a stable
/// abstract parent modified before inheritance) without needing per-operation intermediate
/// document snapshots.
pub(super) fn pre_patch_ancestor_names(
    top_level_defs: &[Element<'_>],
    def_type: &str,
    def_name: &str,
) -> HashSet<String> {
    let by_name: HashMap<&str, Element<'_>> = top_level_defs
        .iter()
        .filter_map(|&el| el.attribute_value("Name").map(|name| (name, el)))
        .collect();

    let mut names = HashSet::new();
    let Some(&target) = top_level_defs
        .iter()
        .find(|&&el| matches_selected_def(el, def_type, def_name))
    else {
        return names;
    };

    let mut current = target;
    let mut visited: HashSet<String> = HashSet::new();
    loop {
        if let Some(name) = current.attribute_value("Name") {
            names.insert(name.to_string());
        }
        let Some(parent_name) = current.attribute_value("ParentName") else {
            break;
        };
        if !visited.insert(parent_name.to_string()) {
            break; // cycle guard
        }
        let Some(&parent_el) = by_name.get(parent_name) else {
            break;
        };
        current = parent_el;
    }
    names
}

/// Runtime correlation for statically-unresolvable XPaths (see `pre_patch_ancestor_names`):
/// evaluates `xpath` against the pre-patch document and reports whether any matched element is
/// the selected Def itself, or a named ancestor in its pre-patch `ParentName` chain.
pub(super) fn xpath_touches_target(
    document: Document<'_>,
    xpath: &str,
    def_type: &str,
    def_name: &str,
    ancestor_names: &HashSet<String>,
) -> bool {
    let Ok(nodes) = select_nodes(document, xpath) else {
        return false;
    };
    for node in nodes {
        let XPathNode::Element(mut el) = node else {
            continue;
        };
        // The xpath commonly targets a *field inside* the parent (e.g.
        // `Defs/ThingDef[@Name="Base"]/statBases/MoveSpeed`), not the parent element itself, so
        // walk up through ancestors -- not just the matched node -- looking for the selected Def
        // or a named ancestor in its pre-patch inheritance chain.
        loop {
            if matches_selected_def(el, def_type, def_name) {
                return true;
            }
            if let Some(name) = el.attribute_value("Name") {
                if ancestor_names.contains(name) {
                    return true;
                }
            }
            match el.parent() {
                Some(ParentOfChild::Element(parent)) => el = parent,
                _ => break,
            }
        }
    }
    false
}
