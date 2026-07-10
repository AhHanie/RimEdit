use super::context::ValidationContext;
use super::diagnostics as diag;
use super::fields::{default_value_as_str, validate_object_children, validate_schema_field};
use super::xml::scalar_text;
use crate::schema_pack::{
    lookup_def_type, lookup_field, ValidationRule, ValidationRuleCondition, ValidationRuleOperator,
    XmlFieldShape,
};
use crate::xml_document::diagnostics::ValidationDiagnostic;
use crate::xml_document::model::{DefSummary, XmlDocument, XmlNodeKind};
use std::collections::{BTreeMap, HashSet};

pub(super) fn validate_document(
    doc: &XmlDocument,
    context: &ValidationContext<'_>,
) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();

    for summary in &doc.def_summaries {
        validate_def_identity(doc, summary, context, &mut diagnostics);

        if lookup_def_type(context.catalog, &summary.def_type).is_none() {
            diagnostics.push(
                ValidationDiagnostic::warning(
                    doc.relative_path.clone(),
                    Some(summary.node_id),
                    summary.line,
                    summary.column,
                    "validation_unknown_def_type",
                    format!(
                        "Unknown Def type '{}'; schema field checks were skipped.",
                        summary.def_type
                    ),
                )
                .with_def(&summary.def_type, summary.def_name.as_deref()),
            );
            continue;
        }

        validate_def_fields(doc, summary, context, &mut diagnostics);
    }

    diagnostics
}

fn validate_def_identity(
    doc: &XmlDocument,
    summary: &DefSummary,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let def_name = summary.def_name.as_deref().map(str::trim);
    if def_name.is_none() || def_name == Some("") {
        return;
    }

    let def_name = def_name.unwrap();
    let occurrences = context
        .def_index
        .find_project_duplicates(&summary.def_type, def_name);
    if occurrences.len() > 1 {
        let locations = diag::format_index_occurrences(&occurrences);
        diagnostics.push(
            ValidationDiagnostic::error(
                doc.relative_path.clone(),
                Some(summary.node_id),
                summary.line,
                summary.column,
                "validation_duplicate_def_name",
                format!(
                    "Duplicate {} defName '{}'. Occurrences: {}.",
                    summary.def_type, def_name, locations
                ),
            )
            .with_def(&summary.def_type, Some(def_name))
            .with_field_path("defName"),
        );
    }

    let source_occurrences = context
        .def_index
        .find_source_duplicates(&summary.def_type, def_name);
    if !source_occurrences.is_empty() {
        let locations = diag::format_index_occurrences(&source_occurrences);
        diagnostics.push(
            ValidationDiagnostic::warning(
                doc.relative_path.clone(),
                Some(summary.node_id),
                summary.line,
                summary.column,
                "validation_duplicate_source_def_name",
                format!(
                    "{} defName '{}' also exists in read-only sources: {}.",
                    summary.def_type, def_name, locations
                ),
            )
            .with_def(&summary.def_type, Some(def_name))
            .with_field_path("defName"),
        );
    }
}

fn validate_def_fields(
    doc: &XmlDocument,
    summary: &DefSummary,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let Some(def_node) = doc.nodes.get(summary.node_id) else {
        return;
    };

    for &child_id in &def_node.children {
        let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
            continue;
        };
        let field_name = child_el.name.as_str();
        let Some(field_schema) = lookup_field(context.catalog, &summary.def_type, field_name)
        else {
            diagnostics.push(
                diag::warning_at_node(
                    doc,
                    child_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_unknown_field",
                    format!("Unknown field '{}' on {}.", field_name, summary.def_type),
                )
                .with_field_path(field_name),
            );
            continue;
        };

        validate_schema_field(
            doc,
            summary,
            child_id,
            field_name,
            field_schema,
            context,
            diagnostics,
        );

        // Recurse into schema-backed object-type blocks (building, race, apparel, etc.).
        validate_object_children(
            doc,
            summary,
            child_id,
            field_name,
            field_schema,
            field_name,
            context,
            diagnostics,
            0,
        );
    }

    // Check that all required child-element fields are present.
    validate_required_fields_present(doc, summary, context, diagnostics);

    // Evaluate declarative validation rules (requiredWhen, etc.).
    evaluate_validation_rules(doc, summary, context, diagnostics);
}

fn validate_required_fields_present(
    doc: &XmlDocument,
    summary: &DefSummary,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let Some(def_node) = doc.nodes.get(summary.node_id) else {
        return;
    };

    // Collect names of existing child elements.
    let present: HashSet<&str> = def_node
        .children
        .iter()
        .filter_map(|&id| {
            if let XmlNodeKind::Element(e) = &doc.nodes[id].kind {
                Some(e.name.as_str())
            } else {
                None
            }
        })
        .collect();

    let mut visited: HashSet<String> = HashSet::new();
    check_required_fields_in_type(
        &summary.def_type,
        &present,
        doc,
        summary,
        context,
        diagnostics,
        &mut visited,
    );
}

fn check_required_fields_in_type(
    def_type: &str,
    present: &HashSet<&str>,
    doc: &XmlDocument,
    summary: &DefSummary,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(def_type.to_string()) {
        return;
    }
    let Some(schema) = context.catalog.def_types.get(def_type) else {
        return;
    };

    // Walk parents first so own fields take precedence when reporting.
    for parent in schema.inherits.clone() {
        check_required_fields_in_type(
            &parent,
            present,
            doc,
            summary,
            context,
            diagnostics,
            visited,
        );
    }

    for (field_name, field_schema) in &schema.fields {
        if !field_schema.required {
            continue;
        }
        // Only check shapes that correspond to child elements.
        if !matches!(
            field_schema.xml,
            XmlFieldShape::Element
                | XmlFieldShape::Object
                | XmlFieldShape::ListOfLi
                | XmlFieldShape::NamedChildrenMap
                | XmlFieldShape::KeyedValueList
                | XmlFieldShape::KeyedObjectList
                | XmlFieldShape::KeyedObjectMap
        ) {
            continue;
        }
        let is_present = present.contains(field_name.as_str())
            || field_schema
                .xml_aliases
                .iter()
                .any(|a| present.contains(a.as_str()));
        if !is_present {
            diagnostics.push(
                diag::warning_at_node(
                    doc,
                    summary.node_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_missing_required_field",
                    format!(
                        "Required field '{}' is missing from {}.",
                        field_name, summary.def_type
                    ),
                )
                .with_field_path(field_name),
            );
        }
    }
}

fn evaluate_validation_rules(
    doc: &XmlDocument,
    summary: &DefSummary,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let effective_rules = collect_effective_rules(&summary.def_type, context.catalog);
    for rule in effective_rules.values() {
        evaluate_rule(doc, summary, context, rule, diagnostics);
    }
}

fn collect_effective_rules(
    def_type: &str,
    catalog: &crate::schema_pack::SchemaCatalog,
) -> BTreeMap<String, ValidationRule> {
    let mut rules = BTreeMap::new();
    let mut visited = HashSet::new();
    collect_rules_recursive(def_type, catalog, &mut rules, &mut visited);
    rules
}

fn collect_rules_recursive(
    def_type: &str,
    catalog: &crate::schema_pack::SchemaCatalog,
    rules: &mut BTreeMap<String, ValidationRule>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(def_type.to_string()) {
        return;
    }
    let Some(schema) = catalog.def_types.get(def_type) else {
        return;
    };
    let inherits: Vec<String> = schema.inherits.clone();
    for parent in inherits {
        collect_rules_recursive(&parent, catalog, rules, visited);
    }
    for (id, rule) in &schema.validation_rules {
        rules.insert(id.clone(), rule.clone());
    }
}

fn evaluate_rule(
    doc: &XmlDocument,
    summary: &DefSummary,
    context: &ValidationContext<'_>,
    rule: &ValidationRule,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    match rule {
        ValidationRule::RequiredWhen {
            field,
            when,
            message,
        } => {
            if evaluate_condition(doc, summary, context, when)
                && !is_def_field_present(doc, summary, field, context)
            {
                diagnostics.push(
                    diag::warning_at_node(
                        doc,
                        summary.node_id,
                        &summary.def_type,
                        summary.def_name.as_deref(),
                        "validation_missing_required_field",
                        message.clone(),
                    )
                    .with_field_path(field),
                );
            }
        }
    }
}

fn is_def_field_present(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_name: &str,
    context: &ValidationContext<'_>,
) -> bool {
    let Some(def_node) = doc.nodes.get(summary.node_id) else {
        return false;
    };
    let aliases: Vec<String> = lookup_field(context.catalog, &summary.def_type, field_name)
        .map(|s| s.xml_aliases.clone())
        .unwrap_or_default();
    def_node.children.iter().any(|&child_id| {
        if let XmlNodeKind::Element(e) = &doc.nodes[child_id].kind {
            e.name == field_name || aliases.contains(&e.name)
        } else {
            false
        }
    })
}

fn evaluate_condition(
    doc: &XmlDocument,
    summary: &DefSummary,
    context: &ValidationContext<'_>,
    condition: &ValidationRuleCondition,
) -> bool {
    match &condition.operator {
        // present/absent check element existence, not scalar text content,
        // so that list and object condition fields work correctly.
        ValidationRuleOperator::Present => {
            is_def_field_present(doc, summary, &condition.field, context)
        }
        ValidationRuleOperator::Absent => {
            !is_def_field_present(doc, summary, &condition.field, context)
        }
        op => {
            let actual = get_def_field_text(doc, summary, &condition.field, context);
            let Some(ref expected) = condition.value else {
                return false;
            };
            let Some(ref actual_str) = actual else {
                return false;
            };
            let actual_trimmed = actual_str.trim();
            let ordering = compare_condition_values(actual_trimmed, expected);
            match op {
                ValidationRuleOperator::Equals => ordering == Some(std::cmp::Ordering::Equal),
                ValidationRuleOperator::NotEquals => ordering
                    .map(|o| o != std::cmp::Ordering::Equal)
                    .unwrap_or(false),
                ValidationRuleOperator::GreaterThan => {
                    ordering == Some(std::cmp::Ordering::Greater)
                }
                ValidationRuleOperator::GreaterThanOrEqual => {
                    matches!(
                        ordering,
                        Some(std::cmp::Ordering::Greater) | Some(std::cmp::Ordering::Equal)
                    )
                }
                ValidationRuleOperator::LessThan => ordering == Some(std::cmp::Ordering::Less),
                ValidationRuleOperator::LessThanOrEqual => {
                    matches!(
                        ordering,
                        Some(std::cmp::Ordering::Less) | Some(std::cmp::Ordering::Equal)
                    )
                }
                ValidationRuleOperator::Present | ValidationRuleOperator::Absent => unreachable!(),
            }
        }
    }
}

fn get_def_field_text(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_name: &str,
    context: &ValidationContext<'_>,
) -> Option<String> {
    let def_node = doc.nodes.get(summary.node_id)?;
    let field_schema = lookup_field(context.catalog, &summary.def_type, field_name);
    let aliases: Vec<String> = field_schema
        .map(|s| s.xml_aliases.clone())
        .unwrap_or_default();
    for &child_id in &def_node.children {
        if let XmlNodeKind::Element(e) = &doc.nodes[child_id].kind {
            if e.name == field_name || aliases.contains(&e.name) {
                return scalar_text(doc, child_id).map(|s| s.to_string());
            }
        }
    }
    // Fall back to schema default value.
    let field_schema = lookup_field(context.catalog, &summary.def_type, field_name)?;
    default_value_as_str(field_schema.default_value.as_ref()?)
}

fn compare_condition_values(
    actual: &str,
    expected: &serde_json::Value,
) -> Option<std::cmp::Ordering> {
    // Try numeric comparison first.
    if let Some(expected_f64) = expected.as_f64() {
        if let Ok(actual_f64) = actual.parse::<f64>() {
            return actual_f64.partial_cmp(&expected_f64);
        }
    }
    // Boolean comparison.
    if let Some(expected_bool) = expected.as_bool() {
        let actual_bool = matches!(actual, "true" | "True" | "1");
        return Some(actual_bool.cmp(&expected_bool));
    }
    // String comparison.
    if let Some(expected_str) = expected.as_str() {
        return Some(actual.cmp(expected_str));
    }
    None
}
