use std::collections::HashSet;

use crate::xml_document::model::{XmlDocument, XmlNodeId, XmlNodeKind};
use crate::xml_document::parse_to_document;

use super::model::{
    AttributeOperation, AttributeValueOperation, PatchDiagnostic, PatchFile, PatchOperationId,
    PatchOperationKind, PatchOperationNode, PatchOrderMode, PatchSpan, PatchSuccessMode,
    PathedOperation, PathedValueOperation, PathedValueOrderOperation, SetNameOperation,
    UnknownPatchOperation, XmlAttributeModel, BUILT_IN_OPERATION_CLASSES,
};

pub fn parse_patch_file(relative_path: &str, source: &str) -> PatchFile {
    let doc = parse_to_document(relative_path, source);

    let xml_declaration = doc.top_level_nodes.iter().find_map(|&id| {
        if let XmlNodeKind::ProcessingInstruction(_) = doc.nodes[id].kind {
            Some(doc.source[doc.nodes[id].span.start..doc.nodes[id].span.end].to_string())
        } else {
            None
        }
    });

    if doc.had_fatal_parse_error {
        let diagnostics = doc
            .parse_diagnostics
            .iter()
            .map(|d| {
                PatchDiagnostic::new(d.line, d.column, d.message.clone())
                    .with_code(d.code.clone())
                    .with_args(d.args.clone())
            })
            .collect();
        return PatchFile {
            relative_path: relative_path.to_string(),
            xml_declaration,
            operations: Vec::new(),
            diagnostics,
            had_fatal_parse_error: true,
        };
    }

    let mut diagnostics: Vec<PatchDiagnostic> = doc
        .parse_diagnostics
        .iter()
        .map(|d| {
            PatchDiagnostic::new(d.line, d.column, d.message.clone())
                .with_code(d.code.clone())
                .with_args(d.args.clone())
        })
        .collect();

    let root_elements: Vec<XmlNodeId> = doc
        .top_level_nodes
        .iter()
        .copied()
        .filter(|&id| matches!(doc.nodes[id].kind, XmlNodeKind::Element(_)))
        .collect();

    if root_elements.len() > 1 {
        diagnostics.push(
            PatchDiagnostic::new(
                None,
                None,
                format!(
                    "patch file has {} root elements; exactly one is required",
                    root_elements.len()
                ),
            )
            .with_code("patch_invalid_root_element_count")
            .with_args(crate::diagnostics::diagnostic_args([(
                "elementCount",
                root_elements.len().into(),
            )])),
        );
        return PatchFile {
            relative_path: relative_path.to_string(),
            xml_declaration,
            operations: Vec::new(),
            diagnostics,
            had_fatal_parse_error: false,
        };
    }

    let Some(root_id) = root_elements.first().copied() else {
        diagnostics.push(
            PatchDiagnostic::new(None, None, "patch file has no root element")
                .with_code("patch_missing_root_element"),
        );
        return PatchFile {
            relative_path: relative_path.to_string(),
            xml_declaration,
            operations: Vec::new(),
            diagnostics,
            had_fatal_parse_error: false,
        };
    };

    let root_name = match &doc.nodes[root_id].kind {
        XmlNodeKind::Element(el) => el.name.clone(),
        _ => unreachable!("root_id was matched as an Element above"),
    };

    if root_name != "Patch" {
        diagnostics.push(
            PatchDiagnostic::new(
                Some(doc.nodes[root_id].span.line),
                Some(doc.nodes[root_id].span.column),
                format!("root element must be <Patch>, found <{}>", root_name),
            )
            .with_code("patch_invalid_root_element_name")
            .with_args(crate::diagnostics::diagnostic_args([(
                "rootName",
                root_name.as_str().into(),
            )])),
        );
        return PatchFile {
            relative_path: relative_path.to_string(),
            xml_declaration,
            operations: Vec::new(),
            diagnostics,
            had_fatal_parse_error: false,
        };
    }

    let mut next_id: PatchOperationId = 0;
    let mut operations = Vec::new();
    for &child_id in &doc.nodes[root_id].children {
        let el = match &doc.nodes[child_id].kind {
            XmlNodeKind::Element(el) => el,
            _ => continue,
        };
        if el.name != "Operation" {
            diagnostics.push(
                PatchDiagnostic::new(
                    Some(doc.nodes[child_id].span.line),
                    Some(doc.nodes[child_id].span.column),
                    format!(
                        "unexpected child <{}> of <Patch>; expected <Operation>",
                        el.name
                    ),
                )
                .with_code("patch_unexpected_child_element")
                .with_args(crate::diagnostics::diagnostic_args([
                    ("parentName", "Patch".into()),
                    ("childName", el.name.as_str().into()),
                ])),
            );
            continue;
        }
        operations.push(parse_operation_element(
            &doc,
            child_id,
            &mut next_id,
            &mut diagnostics,
        ));
    }

    PatchFile {
        relative_path: relative_path.to_string(),
        xml_declaration,
        operations,
        diagnostics,
        had_fatal_parse_error: false,
    }
}

fn parse_operation_element(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    next_id: &mut PatchOperationId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> PatchOperationNode {
    let id = *next_id;
    *next_id += 1;

    let node = &doc.nodes[elem_id];
    let span = Some(PatchSpan {
        start: node.span.start,
        end: node.span.end,
        line: node.span.line,
        column: node.span.column,
    });

    let el = match &node.kind {
        XmlNodeKind::Element(el) => el,
        _ => unreachable!("caller guarantees elem_id is an Element"),
    };

    let mut class_name = String::new();
    let mut attributes = Vec::new();
    for attr in &el.attributes {
        if attr.name.eq_ignore_ascii_case("class") {
            class_name = attr.value.clone();
        } else {
            attributes.push(XmlAttributeModel {
                name: attr.name.clone(),
                value: attr.value.clone(),
            });
        }
    }

    if class_name.is_empty() {
        diagnostics.push(
            PatchDiagnostic::new(
                Some(node.span.line),
                Some(node.span.column),
                format!("<{}> is missing a Class attribute", el.name),
            )
            .with_code("patch_missing_class_attribute")
            .with_args(crate::diagnostics::diagnostic_args([(
                "elementName",
                el.name.as_str().into(),
            )])),
        );
    }

    // Mirrors RimWorld's DirectXmlToObject, which logs "defines the same field twice" for
    // repeated child element names and then lets the last one win (each SetValue overwrites).
    let mut seen_field_names: HashSet<&str> = HashSet::new();
    for &child_id in &node.children {
        if let XmlNodeKind::Element(ref child_el) = doc.nodes[child_id].kind {
            if !seen_field_names.insert(child_el.name.as_str()) {
                diagnostics.push(
                    PatchDiagnostic::new(
                        Some(doc.nodes[child_id].span.line),
                        Some(doc.nodes[child_id].span.column),
                        format!(
                            "<{}> defines the field <{}> twice; the last one wins",
                            el.name, child_el.name
                        ),
                    )
                    .with_code("patch_duplicate_field")
                    .with_args(crate::diagnostics::diagnostic_args([(
                        "fieldName",
                        child_el.name.as_str().into(),
                    )])),
                );
            }
        }
    }

    let success = child_element_named(doc, elem_id, "success")
        .map(|cid| element_text(doc, cid))
        .and_then(|text| match PatchSuccessMode::from_xml_str(&text) {
            Some(mode) => Some(mode),
            None => {
                diagnostics.push(
                    PatchDiagnostic::new(
                        Some(node.span.line),
                        Some(node.span.column),
                        format!("unrecognized <success> value '{}'", text),
                    )
                    .with_code("patch_unrecognized_success_value")
                    .with_args(crate::diagnostics::diagnostic_args([(
                        "value",
                        text.as_str().into(),
                    )])),
                );
                None
            }
        })
        .unwrap_or_default();

    let kind = if BUILT_IN_OPERATION_CLASSES.contains(&class_name.as_str()) {
        match unrecognized_field_name(doc, elem_id, &class_name) {
            // A known class with a field RimEdit doesn't model (e.g. a mod-authored extra child,
            // or a field this parser hasn't caught up to) would otherwise be silently dropped by
            // `parse_known_kind`, which only reads the specific fields it knows about -- so the
            // whole operation falls back to `Unknown` (raw XML) instead, preserving it exactly.
            Some(unrecognized) => {
                diagnostics.push(
                    PatchDiagnostic::new(
                        Some(node.span.line),
                        Some(node.span.column),
                        format!(
                            "<{}> has a field <{}> not recognized for {}; editing this operation as raw XML to avoid losing it",
                            el.name, unrecognized, class_name
                        ),
                    )
                    .with_code("patch_unrecognized_field_for_class")
                    .with_args(crate::diagnostics::diagnostic_args([
                        ("fieldName", unrecognized.into()),
                        ("className", class_name.as_str().into()),
                    ])),
                );
                PatchOperationKind::Unknown(UnknownPatchOperation {
                    raw_xml: doc.source[node.span.start..node.span.end].to_string(),
                })
            }
            None => parse_known_kind(doc, elem_id, &class_name, next_id, diagnostics),
        }
    } else {
        PatchOperationKind::Unknown(UnknownPatchOperation {
            raw_xml: doc.source[node.span.start..node.span.end].to_string(),
        })
    };

    PatchOperationNode {
        id,
        class_name,
        success,
        attributes,
        kind,
        span,
    }
}

/// Direct child element names each built-in class's typed model actually reads. `success` is
/// always allowed (checked separately, above) and left out of every list.
fn recognized_field_names(class_name: &str) -> &'static [&'static str] {
    match class_name {
        "PatchOperationAdd" | "PatchOperationInsert" => &["xpath", "value", "order"],
        "PatchOperationRemove" | "PatchOperationTest" => &["xpath"],
        "PatchOperationReplace" | "PatchOperationAddModExtension" => &["xpath", "value"],
        "PatchOperationAttributeAdd" | "PatchOperationAttributeSet" => {
            &["xpath", "attribute", "value"]
        }
        "PatchOperationAttributeRemove" => &["xpath", "attribute"],
        "PatchOperationSetName" => &["xpath", "name"],
        "PatchOperationSequence" => &["operations"],
        "PatchOperationFindMod" => &["mods", "match", "nomatch"],
        "PatchOperationConditional" => &["xpath", "match", "nomatch"],
        _ => &[],
    }
}

/// The name of the first direct child element that isn't `success` or one of `class_name`'s
/// recognized fields, if any.
fn unrecognized_field_name(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    class_name: &str,
) -> Option<String> {
    let recognized = recognized_field_names(class_name);
    for &child_id in &doc.nodes[elem_id].children {
        if let XmlNodeKind::Element(ref child_el) = doc.nodes[child_id].kind {
            if child_el.name.eq_ignore_ascii_case("success") {
                continue;
            }
            if !recognized
                .iter()
                .any(|r| child_el.name.eq_ignore_ascii_case(r))
            {
                return Some(child_el.name.clone());
            }
        }
    }
    None
}

fn parse_known_kind(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    class_name: &str,
    next_id: &mut PatchOperationId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> PatchOperationKind {
    match class_name {
        "PatchOperationAdd" => {
            PatchOperationKind::Add(read_pathed_value_order(doc, elem_id, diagnostics))
        }
        "PatchOperationInsert" => {
            PatchOperationKind::Insert(read_pathed_value_order(doc, elem_id, diagnostics))
        }
        "PatchOperationRemove" => {
            PatchOperationKind::Remove(read_pathed(doc, elem_id, diagnostics))
        }
        "PatchOperationReplace" => {
            PatchOperationKind::Replace(read_pathed_value(doc, elem_id, diagnostics))
        }
        "PatchOperationAddModExtension" => {
            PatchOperationKind::AddModExtension(read_pathed_value(doc, elem_id, diagnostics))
        }
        "PatchOperationAttributeAdd" => {
            PatchOperationKind::AttributeAdd(read_attribute_value(doc, elem_id, diagnostics))
        }
        "PatchOperationAttributeSet" => {
            PatchOperationKind::AttributeSet(read_attribute_value(doc, elem_id, diagnostics))
        }
        "PatchOperationAttributeRemove" => {
            PatchOperationKind::AttributeRemove(read_attribute(doc, elem_id, diagnostics))
        }
        "PatchOperationSetName" => {
            PatchOperationKind::SetName(read_set_name(doc, elem_id, diagnostics))
        }
        "PatchOperationTest" => PatchOperationKind::Test(read_pathed(doc, elem_id, diagnostics)),
        "PatchOperationSequence" => {
            PatchOperationKind::Sequence(read_sequence(doc, elem_id, next_id, diagnostics))
        }
        "PatchOperationFindMod" => {
            let mods = read_mods(doc, elem_id, diagnostics);
            let (match_op, nomatch_op) = read_match_nomatch(doc, elem_id, next_id, diagnostics);
            PatchOperationKind::FindMod {
                mods,
                match_op,
                nomatch_op,
            }
        }
        "PatchOperationConditional" => {
            let xpath = read_xpath(doc, elem_id, diagnostics);
            let (match_op, nomatch_op) = read_match_nomatch(doc, elem_id, next_id, diagnostics);
            PatchOperationKind::Conditional {
                xpath,
                match_op,
                nomatch_op,
            }
        }
        _ => unreachable!("class_name checked against BUILT_IN_OPERATION_CLASSES by caller"),
    }
}

fn read_xpath(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> Option<String> {
    match child_element_named(doc, elem_id, "xpath") {
        Some(cid) => Some(element_text(doc, cid)),
        None => {
            diagnostics.push(missing_field_diagnostic(doc, elem_id, "xpath"));
            None
        }
    }
}

fn read_pathed(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> PathedOperation {
    PathedOperation {
        xpath: read_xpath(doc, elem_id, diagnostics),
    }
}

fn read_value_xml(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> Option<String> {
    match child_element_named(doc, elem_id, "value") {
        Some(cid) => Some(element_inner_xml(doc, cid)),
        None => {
            diagnostics.push(missing_field_diagnostic(doc, elem_id, "value"));
            None
        }
    }
}

fn read_pathed_value(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> PathedValueOperation {
    PathedValueOperation {
        xpath: read_xpath(doc, elem_id, diagnostics),
        value_xml: read_value_xml(doc, elem_id, diagnostics),
    }
}

fn read_pathed_value_order(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> PathedValueOrderOperation {
    let xpath = read_xpath(doc, elem_id, diagnostics);
    let value_xml = read_value_xml(doc, elem_id, diagnostics);
    let order = child_element_named(doc, elem_id, "order")
        .map(|cid| element_text(doc, cid))
        .and_then(|text| match PatchOrderMode::from_xml_str(&text) {
            Some(mode) => Some(mode),
            None => {
                diagnostics.push(
                    PatchDiagnostic::new(
                        Some(doc.nodes[elem_id].span.line),
                        Some(doc.nodes[elem_id].span.column),
                        format!("unrecognized <order> value '{}'", text),
                    )
                    .with_code("patch_unrecognized_order_value")
                    .with_args(crate::diagnostics::diagnostic_args([(
                        "value",
                        text.as_str().into(),
                    )])),
                );
                None
            }
        });
    PathedValueOrderOperation {
        xpath,
        value_xml,
        order,
    }
}

fn read_attribute_field(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> Option<String> {
    match child_element_named(doc, elem_id, "attribute") {
        Some(cid) => Some(element_text(doc, cid)),
        None => {
            diagnostics.push(missing_field_diagnostic(doc, elem_id, "attribute"));
            None
        }
    }
}

fn read_attribute_value(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> AttributeValueOperation {
    let xpath = read_xpath(doc, elem_id, diagnostics);
    let attribute = read_attribute_field(doc, elem_id, diagnostics);
    let value = match child_element_named(doc, elem_id, "value") {
        Some(cid) => Some(element_text(doc, cid)),
        None => {
            diagnostics.push(missing_field_diagnostic(doc, elem_id, "value"));
            None
        }
    };
    AttributeValueOperation {
        xpath,
        attribute,
        value,
    }
}

fn read_attribute(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> AttributeOperation {
    AttributeOperation {
        xpath: read_xpath(doc, elem_id, diagnostics),
        attribute: read_attribute_field(doc, elem_id, diagnostics),
    }
}

fn read_set_name(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> SetNameOperation {
    let xpath = read_xpath(doc, elem_id, diagnostics);
    let name = match child_element_named(doc, elem_id, "name") {
        Some(cid) => Some(element_text(doc, cid)),
        None => {
            diagnostics.push(missing_field_diagnostic(doc, elem_id, "name"));
            None
        }
    };
    SetNameOperation { xpath, name }
}

fn read_sequence(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    next_id: &mut PatchOperationId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> Vec<PatchOperationNode> {
    let Some(ops_id) = child_element_named(doc, elem_id, "operations") else {
        diagnostics.push(missing_field_diagnostic(doc, elem_id, "operations"));
        return Vec::new();
    };
    let mut result = Vec::new();
    for &child_id in &doc.nodes[ops_id].children {
        let el = match &doc.nodes[child_id].kind {
            XmlNodeKind::Element(el) => el,
            _ => continue,
        };
        if el.name != "li" {
            diagnostics.push(
                PatchDiagnostic::new(
                    Some(doc.nodes[child_id].span.line),
                    Some(doc.nodes[child_id].span.column),
                    format!(
                        "unexpected child <{}> of <operations>; expected <li>",
                        el.name
                    ),
                )
                .with_code("patch_unexpected_child_element")
                .with_args(crate::diagnostics::diagnostic_args([
                    ("parentName", "operations".into()),
                    ("childName", el.name.as_str().into()),
                ])),
            );
            continue;
        }
        result.push(parse_operation_element(doc, child_id, next_id, diagnostics));
    }
    result
}

fn read_mods(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> Vec<String> {
    match child_element_named(doc, elem_id, "mods") {
        Some(mods_id) => {
            let mut mods = Vec::new();
            for &cid in &doc.nodes[mods_id].children {
                let el = match &doc.nodes[cid].kind {
                    XmlNodeKind::Element(el) => el,
                    _ => continue,
                };
                if el.name != "li" {
                    diagnostics.push(
                        PatchDiagnostic::new(
                            Some(doc.nodes[cid].span.line),
                            Some(doc.nodes[cid].span.column),
                            format!("unexpected child <{}> of <mods>; expected <li>", el.name),
                        )
                        .with_code("patch_unexpected_child_element")
                        .with_args(crate::diagnostics::diagnostic_args([
                            ("parentName", "mods".into()),
                            ("childName", el.name.as_str().into()),
                        ])),
                    );
                    continue;
                }
                mods.push(element_text(doc, cid));
            }
            mods
        }
        None => {
            diagnostics.push(missing_field_diagnostic(doc, elem_id, "mods"));
            Vec::new()
        }
    }
}

type MatchNomatch = (
    Option<Box<PatchOperationNode>>,
    Option<Box<PatchOperationNode>>,
);

fn read_match_nomatch(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    next_id: &mut PatchOperationId,
    diagnostics: &mut Vec<PatchDiagnostic>,
) -> MatchNomatch {
    let match_op = child_element_named(doc, elem_id, "match")
        .map(|cid| Box::new(parse_operation_element(doc, cid, next_id, diagnostics)));
    let nomatch_op = child_element_named(doc, elem_id, "nomatch")
        .map(|cid| Box::new(parse_operation_element(doc, cid, next_id, diagnostics)));
    (match_op, nomatch_op)
}

/// The *last* matching child, mirroring RimWorld's DirectXmlToObject: repeated field elements
/// each overwrite the previous value via `FieldInfo.SetValue`, so the final one wins.
pub(crate) fn child_element_named(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    name: &str,
) -> Option<XmlNodeId> {
    doc.nodes[elem_id].children.iter().rev().copied().find(|&cid| {
        matches!(&doc.nodes[cid].kind, XmlNodeKind::Element(el) if el.name.eq_ignore_ascii_case(name))
    })
}

/// Concatenated, trimmed text content of an element. Returns `""` for an element that exists
/// but has no (or only whitespace) text -- callers must check for the element's presence
/// separately via [`child_element_named`] to distinguish "missing" from "present but empty".
pub(crate) fn element_text(doc: &XmlDocument, elem_id: XmlNodeId) -> String {
    let mut buf = String::new();
    for &child_id in &doc.nodes[elem_id].children {
        match &doc.nodes[child_id].kind {
            XmlNodeKind::Text(t) => buf.push_str(&t.value),
            XmlNodeKind::CData(t) => buf.push_str(&t.value),
            _ => {}
        }
    }
    buf.trim().to_string()
}

/// Exact source substring between an element's start and end tags, preserving original
/// formatting for round-tripping arbitrary XML payloads (`<value>` contents).
fn element_inner_xml(doc: &XmlDocument, elem_id: XmlNodeId) -> String {
    let el = match &doc.nodes[elem_id].kind {
        XmlNodeKind::Element(el) => el,
        _ => return String::new(),
    };
    if el.self_closing {
        return String::new();
    }
    let start = el.start_tag_span.end;
    let end = el.end_tag_span.as_ref().map(|s| s.start).unwrap_or(start);
    doc.source[start..end].to_string()
}

fn missing_field_diagnostic(doc: &XmlDocument, elem_id: XmlNodeId, field: &str) -> PatchDiagnostic {
    let node = &doc.nodes[elem_id];
    PatchDiagnostic::new(
        Some(node.span.line),
        Some(node.span.column),
        format!("missing required <{}> field", field),
    )
    .with_code("patch_missing_required_field")
    .with_args(crate::diagnostics::diagnostic_args([(
        "fieldName",
        field.into(),
    )]))
}
