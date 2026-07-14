//! Built-in XPath-backed DOM mutation handlers: `Add`, `Insert`, `Remove`, `Replace`, the three
//! attribute operations, `AddModExtension`, `SetName`, and `Test`. Each handler only sees the
//! operation data, the current `Document`, and a diagnostic sink -- they have no knowledge of
//! patch-file ownership, active mods, disabled-operation policy, or `success`-attribute
//! adjustment (that lives in `super::engine`/`super::control_flow`).

use sxd_document::dom::{ChildOfElement, Document, Element, ParentOfChild};

use super::diagnostics::{
    missing_field_diagnostic, xpath_error_diagnostic, xpath_no_match_diagnostic,
};
use super::{ApplyDiagnostic, PatchOperationKey};
use crate::patches::dom::{
    clone_child_of_element, first_child_element_named, parse_fragment, select_elements,
    select_nodes,
};
use crate::patches::model::{
    AttributeOperation, AttributeValueOperation, PatchOrderMode, PathedOperation,
    PathedValueOperation, PathedValueOrderOperation, SetNameOperation,
};

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

pub(super) fn add_op<'d>(
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
pub(super) fn insert_op<'d>(
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

pub(super) fn remove_op<'d>(
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

pub(super) fn replace_op<'d>(
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

pub(super) fn attribute_add_op<'d>(
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

pub(super) fn attribute_set_op<'d>(
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

pub(super) fn attribute_remove_op<'d>(
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

pub(super) fn add_mod_extension_op<'d>(
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

pub(super) fn set_name_op<'d>(
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

pub(super) fn test_op<'d>(
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
