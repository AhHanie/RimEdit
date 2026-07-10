use super::context::ValidationContext;
use super::diagnostics as diag;
use super::scalar::is_valid_scalar_value;
use super::xml::scalar_text;
use crate::schema_pack::{collect_def_subtypes, FieldSchema, ReferenceMetadata, XmlFieldShape};
use crate::xml_document::diagnostics::ValidationDiagnostic;
use crate::xml_document::model::{DefSummary, XmlDocument, XmlNodeId, XmlNodeKind};

/// Return true when `def_name` exists in the index under `target_type` or any of its known
/// schema subtypes.  This lets a reference typed as "LayoutDef" resolve against a
/// "StructureLayoutDef" entry without requiring exact-type matches.
fn is_reference_resolved(
    context: &ValidationContext<'_>,
    target_type: &str,
    def_name: &str,
) -> bool {
    if !context
        .def_index
        .find_by_key(target_type, def_name)
        .is_empty()
    {
        return true;
    }
    for subtype in collect_def_subtypes(context.catalog, target_type) {
        if subtype != target_type && !context.def_index.find_by_key(&subtype, def_name).is_empty() {
            return true;
        }
    }
    false
}

/// Return the effective search targets for a reference, deduplicated and order-preserving.
/// When `acceptedDefTypes` is present and non-empty, those types are used exclusively.
/// Otherwise the nominal `defType` is used.
fn effective_targets(reference: &ReferenceMetadata) -> Vec<&str> {
    match &reference.accepted_def_types {
        Some(types) if !types.is_empty() => {
            let mut seen = std::collections::HashSet::new();
            types
                .iter()
                .map(|s| s.as_str())
                .filter(|s| seen.insert(*s))
                .collect()
        }
        _ => vec![reference.def_type.as_str()],
    }
}

/// Return true when `def_name` resolves under any of the supplied effective targets.
fn is_reference_resolved_multi(
    context: &ValidationContext<'_>,
    targets: &[&str],
    def_name: &str,
) -> bool {
    targets
        .iter()
        .any(|t| is_reference_resolved(context, t, def_name))
}

pub(super) fn validate_field_references(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    field_schema: &FieldSchema,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // Typed reference lists: each child element name is the def type, text is the def name.
    if field_schema.xml == XmlFieldShape::TypedReferenceList {
        if field_schema.typed_reference.is_some() {
            let field_node = doc.nodes.get(field_node_id);
            let children: Vec<XmlNodeId> =
                field_node.map(|n| n.children.clone()).unwrap_or_default();
            let mut seen: std::collections::HashSet<(String, String)> =
                std::collections::HashSet::new();
            for child_id in children {
                let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
                    continue;
                };
                let def_type = child_el.name.as_str();
                let def_name_raw = scalar_text(doc, child_id).unwrap_or_default();
                let def_name = def_name_raw.trim();
                if def_name.is_empty() {
                    diagnostics.push(
                        diag::warning_at_node(
                            doc,
                            child_id,
                            &summary.def_type,
                            summary.def_name.as_deref(),
                            "validation_invalid_typed_reference",
                            format!(
                                "Field '{}' has a '{}' hyperlink with no def name.",
                                field_name, def_type
                            ),
                        )
                        .with_field_path(format!("{field_name}.{def_type}")),
                    );
                    continue;
                }
                let pair = (def_type.to_string(), def_name.to_string());
                if seen.contains(&pair) {
                    diagnostics.push(
                        diag::warning_at_node(
                            doc,
                            child_id,
                            &summary.def_type,
                            summary.def_name.as_deref(),
                            "validation_duplicate_typed_reference",
                            format!(
                                "Field '{}' has duplicate hyperlink '{}.{}'.",
                                field_name, def_type, def_name
                            ),
                        )
                        .with_field_path(format!("{field_name}.{def_type}.{def_name}")),
                    );
                } else {
                    seen.insert(pair);
                }
                if context.def_index.find_by_key(def_type, def_name).is_empty() {
                    diagnostics.push(
                        diag::warning_at_node(
                            doc,
                            child_id,
                            &summary.def_type,
                            summary.def_name.as_deref(),
                            "validation_unresolved_typed_reference",
                            format!(
                                "Field '{}' hyperlink '{}.{}' was not found in the Def index.",
                                field_name, def_type, def_name
                            ),
                        )
                        .with_field_path(format!("{field_name}.{def_type}.{def_name}")),
                    );
                }
            }
        }
        return;
    }

    // Keyed object map: each <li> has a <key> child whose text is the def reference.
    if field_schema.xml == XmlFieldShape::KeyedObjectMap {
        if let Some(ref key_ref) = field_schema.key_reference {
            let targets = effective_targets(key_ref);
            let target_label = &key_ref.def_type;
            let field_node = doc.nodes.get(field_node_id);
            let children: Vec<XmlNodeId> =
                field_node.map(|n| n.children.clone()).unwrap_or_default();
            for li_id in children {
                let XmlNodeKind::Element(li_el) = &doc.nodes[li_id].kind else {
                    continue;
                };
                if li_el.name != "li" {
                    continue;
                }
                for &child_id in &doc.nodes[li_id].children {
                    let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
                        continue;
                    };
                    if child_el.name != "key" {
                        continue;
                    }
                    let text = scalar_text(doc, child_id).unwrap_or_default();
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        break;
                    }
                    if !is_reference_resolved_multi(context, &targets, trimmed) {
                        diagnostics.push(
                            diag::warning_at_node(
                                doc,
                                child_id,
                                &summary.def_type,
                                summary.def_name.as_deref(),
                                "validation_unresolved_map_key",
                                format!(
                                    "Field '{field_name}' map key '{trimmed}' ({target_label}) \
                                     was not found in the Def index.",
                                ),
                            )
                            .with_field_path(format!("{field_name}.{trimmed}")),
                        );
                    }
                    break;
                }
            }
        }
        return;
    }

    // Named-map, keyed-list, and keyed-object-list key references: each child element name is a def key.
    if matches!(
        field_schema.xml,
        XmlFieldShape::NamedChildrenMap
            | XmlFieldShape::KeyedValueList
            | XmlFieldShape::KeyedObjectList
    ) {
        if let Some(ref key_ref) = field_schema.key_reference {
            let targets = effective_targets(key_ref);
            let target_label = &key_ref.def_type;
            let field_node = doc.nodes.get(field_node_id);
            let children: Vec<XmlNodeId> =
                field_node.map(|n| n.children.clone()).unwrap_or_default();
            for child_id in children {
                let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
                    continue;
                };
                let key_name = child_el.name.as_str();
                if !is_reference_resolved_multi(context, &targets, key_name) {
                    diagnostics.push(
                        diag::warning_at_node(
                            doc,
                            child_id,
                            &summary.def_type,
                            summary.def_name.as_deref(),
                            "validation_unresolved_map_key",
                            format!(
                                "Field '{}' map key '{}' ({}) was not found in the Def index.",
                                field_name, key_name, target_label
                            ),
                        )
                        .with_field_path(format!("{field_name}.{key_name}")),
                    );
                }
            }
        }
        // Named-map value validation: if valueType is declared, validate each child's text.
        if field_schema.xml == XmlFieldShape::NamedChildrenMap {
            if let Some(ref vt) = field_schema.value_type {
                let field_node = doc.nodes.get(field_node_id);
                let children: Vec<XmlNodeId> =
                    field_node.map(|n| n.children.clone()).unwrap_or_default();
                for child_id in children {
                    let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
                        continue;
                    };
                    let text = scalar_text(doc, child_id).unwrap_or_default();
                    let trimmed = text.trim();
                    if !trimmed.is_empty() && !is_valid_scalar_value(trimmed, &vt.kind) {
                        diagnostics.push(
                            diag::error_at_node(
                                doc,
                                child_id,
                                &summary.def_type,
                                summary.def_name.as_deref(),
                                "validation_map_value_type_mismatch",
                                format!(
                                    "Field '{}' map entry '{}' value '{}' is not a valid {:?}.",
                                    field_name, child_el.name, trimmed, vt.kind
                                ),
                            )
                            .with_field_path(format!("{field_name}.{}", child_el.name)),
                        );
                    }
                }
            }
        }
        return;
    }

    let Some(ref reference) = field_schema.reference else {
        return;
    };
    let targets = effective_targets(reference);
    let target_label = &reference.def_type;

    match &field_schema.xml {
        XmlFieldShape::Element | XmlFieldShape::Text => {
            let text = scalar_text(doc, field_node_id).unwrap_or_default();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }
            if !is_reference_resolved_multi(context, &targets, trimmed) {
                let diag = if field_schema.required {
                    ValidationDiagnostic::error(
                        doc.relative_path.clone(),
                        Some(field_node_id),
                        Some(doc.nodes[field_node_id].span.line),
                        Some(doc.nodes[field_node_id].span.column),
                        "validation_unresolved_reference",
                        format!(
                            "Field '{}' references '{}' ({}) which was not found in the Def index.",
                            field_name, trimmed, target_label
                        ),
                    )
                } else {
                    ValidationDiagnostic::warning(
                        doc.relative_path.clone(),
                        Some(field_node_id),
                        Some(doc.nodes[field_node_id].span.line),
                        Some(doc.nodes[field_node_id].span.column),
                        "validation_unresolved_reference",
                        format!(
                            "Field '{}' references '{}' ({}) which was not found in the Def index.",
                            field_name, trimmed, target_label
                        ),
                    )
                };
                diagnostics.push(
                    diag.with_def(&summary.def_type, summary.def_name.as_deref())
                        .with_field_path(field_name),
                );
            }
        }
        XmlFieldShape::ListOfLi => {
            let field_node = doc.nodes.get(field_node_id);
            let children: Vec<XmlNodeId> =
                field_node.map(|n| n.children.clone()).unwrap_or_default();
            for child_id in children {
                let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
                    continue;
                };
                if child_el.name != "li" {
                    continue;
                }
                let text = scalar_text(doc, child_id).unwrap_or_default();
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if !is_reference_resolved_multi(context, &targets, trimmed) {
                    let diag = if field_schema.required {
                        ValidationDiagnostic::error(
                            doc.relative_path.clone(),
                            Some(child_id),
                            Some(doc.nodes[child_id].span.line),
                            Some(doc.nodes[child_id].span.column),
                            "validation_unresolved_reference",
                            format!(
                                "Field '{}' list item '{}' ({}) was not found in the Def index.",
                                field_name, trimmed, target_label
                            ),
                        )
                    } else {
                        ValidationDiagnostic::warning(
                            doc.relative_path.clone(),
                            Some(child_id),
                            Some(doc.nodes[child_id].span.line),
                            Some(doc.nodes[child_id].span.column),
                            "validation_unresolved_reference",
                            format!(
                                "Field '{}' list item '{}' ({}) was not found in the Def index.",
                                field_name, trimmed, target_label
                            ),
                        )
                    };
                    diagnostics.push(
                        diag.with_def(&summary.def_type, summary.def_name.as_deref())
                            .with_field_path(field_name),
                    );
                }
            }
        }
        _ => {}
    }
}
