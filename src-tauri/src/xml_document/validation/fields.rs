use super::context::ValidationContext;
use super::diagnostics as diag;
use super::references::validate_field_references;
use super::scalar::{check_numeric_bounds, is_valid_scalar_value, valid_color, valid_vector};
use super::shapes;
use super::xml::{element_child_names, scalar_text};
use crate::diagnostics::diagnostic_args;
use crate::schema_pack::{
    collect_all_object_inherited_fields, lookup_object_field_inherited, FieldSchema, FieldType,
    FieldTypeKind, XmlFieldShape,
};
use crate::xml_document::diagnostics::ValidationDiagnostic;
use crate::xml_document::model::{DefSummary, XmlDocument, XmlNodeId, XmlNodeKind};

const MAX_OBJECT_VALIDATION_DEPTH: u8 = 8;

pub(super) fn validate_schema_field(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_path: &str,
    field_schema: &FieldSchema,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    validate_field_shape(
        doc,
        summary,
        field_node_id,
        field_path,
        field_schema,
        diagnostics,
    );
    validate_field_type(
        doc,
        summary,
        field_node_id,
        field_path,
        field_schema,
        context,
        diagnostics,
    );
    validate_field_references(
        doc,
        summary,
        field_node_id,
        field_path,
        field_schema,
        context,
        diagnostics,
    );
    validate_object_list_items(
        doc,
        summary,
        field_node_id,
        field_path,
        field_schema,
        context,
        diagnostics,
    );
    // Note: validate_object_children for top-level object fields is called from
    // document.rs::validate_def_fields, which delegates here for schema field checks.
}

fn validate_field_shape(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    field_schema: &FieldSchema,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let element_child_names = element_child_names(doc, field_node_id);
    let has_element_children = !element_child_names.is_empty();
    let has_scalar_text = scalar_text(doc, field_node_id)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    if let Some(message) = shapes::shape_mismatch_message(
        field_name,
        field_schema,
        has_element_children,
        has_scalar_text,
        &element_child_names,
    ) {
        diagnostics.push(
            diag::warning_at_node(
                doc,
                field_node_id,
                &summary.def_type,
                summary.def_name.as_deref(),
                "validation_field_shape_mismatch",
                message,
            )
            .with_field_path(field_name)
            .with_args(diagnostic_args([("fieldName", field_name.into())])),
        );
    }
}

fn validate_field_type(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    field_schema: &FieldSchema,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // TypedReferenceList is validated in validate_field_references; skip scalar type checks.
    if field_schema.xml == XmlFieldShape::TypedReferenceList {
        return;
    }

    if field_schema.xml == XmlFieldShape::KeyedValueList {
        validate_keyed_value_list_items(
            doc,
            summary,
            field_node_id,
            field_name,
            field_schema,
            context,
            diagnostics,
        );
        return;
    }

    if field_schema.xml == XmlFieldShape::KeyedObjectList {
        validate_keyed_object_list_items(
            doc,
            summary,
            field_node_id,
            field_name,
            field_schema,
            context,
            diagnostics,
        );
        return;
    }

    if field_schema.xml == XmlFieldShape::KeyedObjectMap {
        validate_keyed_object_map_items(
            doc,
            summary,
            field_node_id,
            field_name,
            field_schema,
            context,
            diagnostics,
        );
        return;
    }

    if field_schema.xml == XmlFieldShape::FlagsText {
        validate_flags_text_values(
            doc,
            summary,
            field_node_id,
            field_name,
            field_schema,
            diagnostics,
        );
        return;
    }

    let text = scalar_text(doc, field_node_id).unwrap_or_default();
    let trimmed = text.trim();
    let element_child_names = element_child_names(doc, field_node_id);

    let valid = match &field_schema.field_type.kind {
        FieldTypeKind::String
        | FieldTypeKind::LocalizedString
        | FieldTypeKind::TypeName
        | FieldTypeKind::DefReference
        | FieldTypeKind::Enum
        | FieldTypeKind::Unknown
        | FieldTypeKind::Unrecognized => true,
        FieldTypeKind::Integer => !trimmed.is_empty() && trimmed.parse::<i64>().is_ok(),
        FieldTypeKind::Float => !trimmed.is_empty() && trimmed.parse::<f64>().is_ok(),
        FieldTypeKind::Boolean => {
            matches!(trimmed, "true" | "false" | "True" | "False" | "1" | "0")
        }
        FieldTypeKind::Vector2 => valid_vector(trimmed, 2),
        FieldTypeKind::Vector3 => valid_vector(trimmed, 3),
        FieldTypeKind::Color => valid_color(trimmed),
        FieldTypeKind::List => {
            trimmed.is_empty()
                && (field_schema.xml == XmlFieldShape::KeyedValueList
                    || element_child_names.is_empty()
                    || element_child_names.iter().all(|n| n == "li"))
        }
        FieldTypeKind::Object | FieldTypeKind::StatMap => {
            !element_child_names.is_empty() || trimmed.is_empty()
        }
        FieldTypeKind::IntRange => {
            // "min~max" or a single integer (RimWorld treats it as min == max)
            if let Some((a, b)) = trimmed.split_once('~') {
                a.trim().parse::<i64>().is_ok() && b.trim().parse::<i64>().is_ok()
            } else {
                trimmed.parse::<i64>().is_ok()
            }
        }
        FieldTypeKind::FloatRange => {
            // "min~max" or a single float (RimWorld treats it as min == max)
            if let Some((a, b)) = trimmed.split_once('~') {
                a.trim().parse::<f64>().is_ok() && b.trim().parse::<f64>().is_ok()
            } else {
                trimmed.parse::<f64>().is_ok()
            }
        }
    };

    if !valid {
        diagnostics.push(
            diag::error_at_node(
                doc,
                field_node_id,
                &summary.def_type,
                summary.def_name.as_deref(),
                "validation_field_type_mismatch",
                format!(
                    "Field '{}' has a value that does not match the expected {:?} type.",
                    field_name, field_schema.field_type.kind
                ),
            )
            .with_field_path(field_name)
            .with_args(diagnostic_args([
                ("fieldName", field_name.into()),
                (
                    "expectedType",
                    format!("{:?}", field_schema.field_type.kind).into(),
                ),
            ])),
        );
        return;
    }

    if let Some(hints) = field_schema.validation_hints.as_ref() {
        if let Some(range_msg) =
            check_numeric_bounds(trimmed, &field_schema.field_type.kind, hints.min, hints.max)
        {
            diagnostics.push(
                diag::error_at_node(
                    doc,
                    field_node_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_field_out_of_range",
                    format!("Field '{field_name}': {range_msg}."),
                )
                .with_field_path(field_name)
                .with_args(diagnostic_args([
                    ("fieldName", field_name.into()),
                    ("detail", range_msg.into()),
                ])),
            );
        }
    }
}

pub(super) fn default_value_as_str(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn validate_keyed_value_list_items(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    field_schema: &FieldSchema,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let text = scalar_text(doc, field_node_id).unwrap_or_default();
    let element_child_names = element_child_names(doc, field_node_id);
    let valid_container = text.trim().is_empty()
        && (element_child_names.is_empty() || element_child_names.iter().all(|n| n != "li"));
    if !valid_container {
        diagnostics.push(
            diag::error_at_node(
                doc,
                field_node_id,
                &summary.def_type,
                summary.def_name.as_deref(),
                "validation_field_type_mismatch",
                format!(
                    "Field '{}' has a value that does not match the expected {:?} type.",
                    field_name, field_schema.field_type.kind
                ),
            )
            .with_field_path(field_name)
            .with_args(diagnostic_args([
                ("fieldName", field_name.into()),
                (
                    "expectedType",
                    format!("{:?}", field_schema.field_type.kind).into(),
                ),
            ])),
        );
        return;
    }

    let Some(value_field_name) = field_schema.value_field.as_deref() else {
        return;
    };
    let Some(items) = field_schema.items.as_ref() else {
        return;
    };
    if items.kind != FieldTypeKind::Object {
        return;
    }
    let Some(item_schema_ref) = items.schema_ref.as_deref() else {
        return;
    };
    let Some(value_field_schema) =
        lookup_object_field_inherited(context.catalog, item_schema_ref, value_field_name)
    else {
        return;
    };

    for &child_id in &doc.nodes[field_node_id].children {
        let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
            continue;
        };
        let value = scalar_text(doc, child_id).unwrap_or_default();
        let trimmed = value.trim();
        let is_valid = if trimmed.is_empty() {
            // Empty child value is valid only when the schema default both exists and is itself
            // a valid scalar for the field type (mirrors LoadDataFromXmlCustom behaviour).
            match &value_field_schema.default_value {
                None => false,
                Some(default) => default_value_as_str(default)
                    .map(|s| is_valid_scalar_value(&s, &value_field_schema.field_type.kind))
                    .unwrap_or(false),
            }
        } else {
            is_valid_scalar_value(trimmed, &value_field_schema.field_type.kind)
        };
        if !is_valid {
            diagnostics.push(
                diag::error_at_node(
                    doc,
                    child_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_field_type_mismatch",
                    format!(
                        "Field '{}.{}' has a value that does not match the expected {:?} type.",
                        field_name, child_el.name, value_field_schema.field_type.kind
                    ),
                )
                .with_field_path(format!("{field_name}.{}", child_el.name))
                .with_args(diagnostic_args([
                    (
                        "fieldName",
                        format!("{field_name}.{}", child_el.name).into(),
                    ),
                    (
                        "expectedType",
                        format!("{:?}", value_field_schema.field_type.kind).into(),
                    ),
                ])),
            );
        }
    }
}

fn validate_keyed_object_list_items(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    field_schema: &FieldSchema,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let text = scalar_text(doc, field_node_id).unwrap_or_default();
    if !text.trim().is_empty() {
        // Scalar text in a keyed object list container is invalid - shape validation already
        // reports this; skip item-level checks.
        return;
    }

    let Some(ref items) = field_schema.items else {
        return;
    };
    if items.kind != FieldTypeKind::Object {
        return;
    }
    let Some(ref base_schema_ref) = items.schema_ref else {
        return;
    };

    let key_field = field_schema.key_field.as_deref();
    let default_value_field = field_schema.default_value_field.as_deref();

    for &item_id in &doc.nodes[field_node_id].children {
        let XmlNodeKind::Element(item_el) = &doc.nodes[item_id].kind else {
            continue;
        };
        let key_name = item_el.name.clone();

        // Handle defaultValueField shorthand: item contains only scalar text (no element children).
        if let Some(dvf) = default_value_field {
            let item_text = scalar_text(doc, item_id).unwrap_or_default();
            let item_trimmed = item_text.trim();
            if !item_trimmed.is_empty() {
                if let Some(dvf_schema) =
                    lookup_object_field_inherited(context.catalog, base_schema_ref, dvf)
                {
                    if !is_valid_scalar_value(item_trimmed, &dvf_schema.field_type.kind) {
                        diagnostics.push(
                            diag::error_at_node(
                                doc,
                                item_id,
                                &summary.def_type,
                                summary.def_name.as_deref(),
                                "validation_field_type_mismatch",
                                format!(
                                    "Field '{field_name}[{key_name}]' shorthand value does not \
                                     match the expected {:?} type for '{dvf}'.",
                                    dvf_schema.field_type.kind
                                ),
                            )
                            .with_field_path(format!("{field_name}.{key_name}"))
                            .with_args(diagnostic_args([
                                ("fieldName", format!("{field_name}[{key_name}]").into()),
                                ("valueField", dvf.into()),
                                (
                                    "expectedType",
                                    format!("{:?}", dvf_schema.field_type.kind).into(),
                                ),
                            ])),
                        );
                    }
                }
                // Scalar shorthand items have no element children to validate.
                continue;
            }
        }

        let all_known_fields =
            collect_all_object_inherited_fields(context.catalog, base_schema_ref);

        for &child_id in &doc.nodes[item_id].children {
            let child_el = match &doc.nodes[child_id].kind {
                XmlNodeKind::Element(e) => e,
                _ => continue,
            };
            let child_name = child_el.name.as_str();

            // The key field lives in the element name, not as a child element.
            if key_field == Some(child_name) {
                continue;
            }

            if !all_known_fields.contains(child_name) {
                diagnostics.push(
                    diag::warning_at_node(
                        doc,
                        child_id,
                        &summary.def_type,
                        summary.def_name.as_deref(),
                        "validation_unknown_object_field",
                        format!("Unknown field '{child_name}' in {field_name}[{key_name}]."),
                    )
                    .with_field_path(format!("{field_name}.{key_name}.{child_name}"))
                    .with_args(diagnostic_args([
                        ("fieldName", child_name.into()),
                        ("containerPath", format!("{field_name}[{key_name}]").into()),
                    ])),
                );
                continue;
            }

            if let Some(child_field_schema) =
                lookup_object_field_inherited(context.catalog, base_schema_ref, child_name)
            {
                let child_field_path = format!("{field_name}.{key_name}.{child_name}");
                validate_schema_field(
                    doc,
                    summary,
                    child_id,
                    &child_field_path,
                    child_field_schema,
                    context,
                    diagnostics,
                );
                validate_object_children(
                    doc,
                    summary,
                    child_id,
                    child_name,
                    child_field_schema,
                    &child_field_path,
                    context,
                    diagnostics,
                    0,
                );
            }
        }
    }
}

fn validate_keyed_object_map_items(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    field_schema: &FieldSchema,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let Some(ref items) = field_schema.items else {
        return;
    };
    if items.kind != FieldTypeKind::Object {
        return;
    }
    let Some(ref base_schema_ref) = items.schema_ref else {
        return;
    };

    let mut seen_keys: Vec<String> = Vec::new();

    for &li_id in &doc.nodes[field_node_id].children {
        let XmlNodeKind::Element(li_el) = &doc.nodes[li_id].kind else {
            continue;
        };
        if li_el.name != "li" {
            continue;
        }

        let mut key_id: Option<XmlNodeId> = None;
        let mut value_id: Option<XmlNodeId> = None;
        for &child_id in &doc.nodes[li_id].children {
            let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
                continue;
            };
            match child_el.name.as_str() {
                "key" if key_id.is_none() => key_id = Some(child_id),
                "value" if value_id.is_none() => value_id = Some(child_id),
                _ => {}
            }
        }

        let Some(key_id) = key_id else {
            diagnostics.push(
                diag::warning_at_node(
                    doc,
                    li_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_keyed_object_map_missing_key",
                    format!("Field '{field_name}' map entry is missing a <key> child."),
                )
                .with_field_path(field_name)
                .with_args(diagnostic_args([("fieldName", field_name.into())])),
            );
            if value_id.is_none() {
                diagnostics.push(
                    diag::warning_at_node(
                        doc,
                        li_id,
                        &summary.def_type,
                        summary.def_name.as_deref(),
                        "validation_keyed_object_map_missing_value",
                        format!("Field '{field_name}' map entry is missing a <value> child."),
                    )
                    .with_field_path(field_name)
                    .with_args(diagnostic_args([("fieldName", field_name.into())])),
                );
            }
            continue;
        };

        if value_id.is_none() {
            diagnostics.push(
                diag::warning_at_node(
                    doc,
                    li_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_keyed_object_map_missing_value",
                    format!("Field '{field_name}' map entry is missing a <value> child."),
                )
                .with_field_path(field_name)
                .with_args(diagnostic_args([("fieldName", field_name.into())])),
            );
        }

        let key_text = scalar_text(doc, key_id).unwrap_or_default();
        let key_trimmed = key_text.trim().to_string();

        if !key_trimmed.is_empty() {
            if seen_keys.contains(&key_trimmed) {
                diagnostics.push(
                    diag::warning_at_node(
                        doc,
                        key_id,
                        &summary.def_type,
                        summary.def_name.as_deref(),
                        "validation_keyed_object_map_duplicate_key",
                        format!("Field '{field_name}' has duplicate key '{key_trimmed}'."),
                    )
                    .with_field_path(format!("{field_name}.{key_trimmed}"))
                    .with_args(diagnostic_args([
                        ("fieldName", field_name.into()),
                        ("key", key_trimmed.as_str().into()),
                    ])),
                );
            } else {
                seen_keys.push(key_trimmed.clone());
            }
        }

        let Some(value_id) = value_id else {
            continue;
        };
        let all_known_fields =
            collect_all_object_inherited_fields(context.catalog, base_schema_ref);

        for &child_id in &doc.nodes[value_id].children {
            let child_el = match &doc.nodes[child_id].kind {
                XmlNodeKind::Element(e) => e,
                _ => continue,
            };
            let child_name = child_el.name.as_str();
            let child_field_path = format!("{field_name}[{key_trimmed}].{child_name}");

            if !all_known_fields.contains(child_name) {
                diagnostics.push(
                    diag::warning_at_node(
                        doc,
                        child_id,
                        &summary.def_type,
                        summary.def_name.as_deref(),
                        "validation_unknown_object_field",
                        format!("Unknown field '{child_name}' in {field_name}[{key_trimmed}]."),
                    )
                    .with_field_path(&child_field_path)
                    .with_args(diagnostic_args([
                        ("fieldName", child_name.into()),
                        (
                            "containerPath",
                            format!("{field_name}[{key_trimmed}]").into(),
                        ),
                    ])),
                );
                continue;
            }

            if let Some(child_field_schema) =
                lookup_object_field_inherited(context.catalog, base_schema_ref, child_name)
            {
                validate_schema_field(
                    doc,
                    summary,
                    child_id,
                    &child_field_path,
                    child_field_schema,
                    context,
                    diagnostics,
                );
                validate_object_children(
                    doc,
                    summary,
                    child_id,
                    child_name,
                    child_field_schema,
                    &child_field_path,
                    context,
                    diagnostics,
                    0,
                );
            }
        }
    }
}

fn validate_flags_text_values(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    field_schema: &FieldSchema,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let Some(hints) = field_schema.validation_hints.as_ref() else {
        return;
    };
    let Some(allowed) = hints.allowed_values.as_ref() else {
        return;
    };
    if allowed.is_empty() {
        return;
    }

    let text = scalar_text(doc, field_node_id).unwrap_or_default();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }

    for token in trimmed.split(',') {
        let flag = token.trim();
        if flag.is_empty() {
            continue;
        }
        if !allowed.iter().any(|v| v == flag) {
            diagnostics.push(
                diag::warning_at_node(
                    doc,
                    field_node_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_flags_text_unknown_value",
                    format!(
                        "Field '{}' contains unknown flag value '{}'. Expected one of: {}.",
                        field_name,
                        flag,
                        allowed.join(", ")
                    ),
                )
                .with_field_path(field_name)
                .with_args(diagnostic_args([
                    ("fieldName", field_name.into()),
                    ("flagValue", flag.into()),
                    ("allowedValues", allowed.clone().into()),
                ])),
            );
        }
    }
}

/// Recursively validate element children of a schema-backed object node.
///
/// Handles discriminated single-element objects (e.g. `<inParam Class="...">`) by
/// reading the discriminator attribute from `container_node_id` when the base schema
/// has a discriminator defined.
///
/// `depth` caps recursion to prevent infinite loops from self-referential schemas.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_object_children(
    doc: &XmlDocument,
    summary: &DefSummary,
    container_node_id: XmlNodeId,
    _container_field_name: &str,
    container_field_schema: &FieldSchema,
    field_path_prefix: &str,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
    depth: u8,
) {
    if depth >= MAX_OBJECT_VALIDATION_DEPTH {
        return;
    }

    if container_field_schema.field_type.kind != FieldTypeKind::Object {
        return;
    }
    let Some(ref base_schema_ref) = container_field_schema.field_type.schema_ref else {
        return;
    };
    if !matches!(
        container_field_schema.xml,
        XmlFieldShape::Object | XmlFieldShape::Element
    ) {
        return;
    }

    // Resolve discriminator on the container element itself (e.g. <inParam Class="...">).
    // Also emit a diagnostic when the discriminator attribute is required but absent.
    let effective_schema_ref = resolve_single_object_discriminator(
        doc,
        container_node_id,
        base_schema_ref,
        field_path_prefix,
        summary,
        context,
        diagnostics,
    );
    let schema_ref = effective_schema_ref
        .as_deref()
        .unwrap_or(base_schema_ref.as_str());

    let children: Vec<XmlNodeId> = doc
        .nodes
        .get(container_node_id)
        .map(|n| n.children.clone())
        .unwrap_or_default();

    for child_id in children {
        let XmlNodeKind::Element(child_el) = &doc.nodes[child_id].kind else {
            continue;
        };
        let child_name = child_el.name.as_str();
        let child_field_path = format!("{field_path_prefix}.{child_name}");

        let Some(child_field_schema) =
            lookup_object_field_inherited(context.catalog, schema_ref, child_name)
        else {
            diagnostics.push(
                diag::warning_at_node(
                    doc,
                    child_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_unknown_object_field",
                    format!("Unknown field '{child_name}' in {field_path_prefix} ({schema_ref})."),
                )
                .with_field_path(&child_field_path)
                .with_args(diagnostic_args([
                    ("fieldName", child_name.into()),
                    ("containerPath", field_path_prefix.into()),
                    ("schemaRef", schema_ref.into()),
                ])),
            );
            continue;
        };

        validate_schema_field(
            doc,
            summary,
            child_id,
            &child_field_path,
            child_field_schema,
            context,
            diagnostics,
        );

        // Recurse into deeper nested objects.
        validate_object_children(
            doc,
            summary,
            child_id,
            child_name,
            child_field_schema,
            &child_field_path,
            context,
            diagnostics,
            depth + 1,
        );
    }
}

/// Resolve a discriminated schema ref for a single object element (not a list item).
///
/// When the object-type schema at `base_schema_ref` has a discriminator, this reads
/// the discriminator attribute (e.g. `Class`) directly from the element at `node_id`
/// and returns the matching variant schema ref. When the attribute is required but absent
/// (`allowMissing: false`), a `validation_missing_required_class` diagnostic is emitted.
fn resolve_single_object_discriminator(
    doc: &XmlDocument,
    node_id: XmlNodeId,
    base_schema_ref: &str,
    field_path: &str,
    summary: &DefSummary,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) -> Option<String> {
    let schema = context.catalog.object_types.get(base_schema_ref)?;
    let disc = schema.discriminator.as_ref()?;

    let element = match &doc.nodes.get(node_id)?.kind {
        XmlNodeKind::Element(e) => e,
        _ => return None,
    };

    let attr_value = element
        .attributes
        .iter()
        .find(|a| a.name == disc.attribute)
        .map(|a| a.value.as_str());

    match attr_value {
        Some(class_val) => {
            if let Some(target_ref) = disc.variants.get(class_val) {
                Some(target_ref.clone())
            } else {
                // Unknown variant - fall back to base.
                Some(base_schema_ref.to_string())
            }
        }
        None => {
            // No discriminator attribute - use fallback or base.
            if disc.allow_missing {
                Some(
                    disc.fallback_schema_ref
                        .clone()
                        .unwrap_or_else(|| base_schema_ref.to_string()),
                )
            } else {
                diagnostics.push(
                    diag::warning_at_node(
                        doc,
                        node_id,
                        &summary.def_type,
                        summary.def_name.as_deref(),
                        "validation_missing_required_class",
                        format!(
                            "{field_path} is missing the required '{}' attribute.",
                            disc.attribute
                        ),
                    )
                    .with_field_path(field_path)
                    .with_args(diagnostic_args([
                        ("fieldName", field_path.into()),
                        ("attribute", disc.attribute.as_str().into()),
                    ])),
                );
                Some(base_schema_ref.to_string())
            }
        }
    }
}

fn validate_scalar_list_items(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    items: &FieldType,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    use super::scalar::is_valid_scalar_value;
    let mut li_index: usize = 0;
    for &li_id in &doc.nodes[field_node_id].children {
        let li_el = match &doc.nodes[li_id].kind {
            XmlNodeKind::Element(e) => e,
            _ => continue,
        };
        if li_el.name != "li" {
            continue;
        }
        let index = li_index;
        li_index += 1;
        let text = scalar_text(doc, li_id).unwrap_or_default();
        let trimmed = text.trim();
        if !is_valid_scalar_value(trimmed, &items.kind) {
            diagnostics.push(
                diag::error_at_node(
                    doc,
                    li_id,
                    &summary.def_type,
                    summary.def_name.as_deref(),
                    "validation_field_type_mismatch",
                    format!(
                        "Field '{}[{}]' has a value that does not match the expected {:?} type.",
                        field_name, index, items.kind
                    ),
                )
                .with_field_path(format!("{field_name}[{index}]"))
                .with_args(diagnostic_args([
                    ("fieldName", format!("{field_name}[{index}]").into()),
                    ("expectedType", format!("{:?}", items.kind).into()),
                ])),
            );
        }
    }
}

fn validate_object_list_items(
    doc: &XmlDocument,
    summary: &DefSummary,
    field_node_id: XmlNodeId,
    field_name: &str,
    field_schema: &FieldSchema,
    context: &ValidationContext<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    if field_schema.xml != XmlFieldShape::ListOfLi {
        return;
    }
    let Some(ref items) = field_schema.items else {
        return;
    };
    if items.kind != FieldTypeKind::Object {
        validate_scalar_list_items(doc, summary, field_node_id, field_name, items, diagnostics);
        return;
    }
    let Some(ref base_schema_ref) = items.schema_ref else {
        return;
    };
    let Some(base_obj_schema) = context.catalog.object_types.get(base_schema_ref) else {
        return;
    };
    let discriminator = base_obj_schema.discriminator.as_ref();

    for &li_id in &doc.nodes[field_node_id].children {
        let li_el = match &doc.nodes[li_id].kind {
            XmlNodeKind::Element(e) => e,
            _ => continue,
        };
        if li_el.name != "li" {
            continue;
        }

        // Resolve discriminator attribute value.
        let class_value: Option<&str> = discriminator.and_then(|disc| {
            li_el
                .attributes
                .iter()
                .find(|a| a.name == disc.attribute)
                .map(|a| a.value.as_str())
        });

        // Select the effective schema ref for this item.
        let effective_schema_ref: Option<String> = if let Some(disc) = discriminator {
            if let Some(class_val) = class_value {
                if let Some(target_ref) = disc.variants.get(class_val) {
                    // Known variant - validate against the variant schema.
                    Some(target_ref.clone())
                } else if disc.allow_unknown {
                    // Unknown mod class - fall back to base fields only.
                    Some(base_schema_ref.clone())
                } else {
                    diagnostics.push(
                        diag::warning_at_node(
                            doc,
                            li_id,
                            &summary.def_type,
                            summary.def_name.as_deref(),
                            "validation_unknown_object_class",
                            format!(
                                "Unknown {field_name} Class '{class_val}'. \
                                 Add a schema pack that provides a schema for this class.",
                            ),
                        )
                        .with_field_path(field_name)
                        .with_args(diagnostic_args([
                            ("fieldName", field_name.into()),
                            ("classValue", class_val.into()),
                        ])),
                    );
                    // Still validate against base fields.
                    Some(base_schema_ref.clone())
                }
            } else if disc.allow_missing {
                disc.fallback_schema_ref
                    .as_deref()
                    .map(str::to_string)
                    .or_else(|| Some(base_schema_ref.clone()))
            } else {
                // Discriminator attribute required but absent - warn and still validate base fields.
                diagnostics.push(
                    diag::warning_at_node(
                        doc,
                        li_id,
                        &summary.def_type,
                        summary.def_name.as_deref(),
                        "validation_missing_required_class",
                        format!("{field_name} list item is missing the required Class attribute.",),
                    )
                    .with_field_path(field_name)
                    .with_args(diagnostic_args([("fieldName", field_name.into())])),
                );
                Some(base_schema_ref.clone())
            }
        } else {
            Some(base_schema_ref.clone())
        };

        let Some(ref schema_ref) = effective_schema_ref else {
            continue;
        };

        // Collect all known field names including inherited ones.
        let all_known_fields = collect_all_object_inherited_fields(context.catalog, schema_ref);

        for &child_id in &doc.nodes[li_id].children {
            let child_el = match &doc.nodes[child_id].kind {
                XmlNodeKind::Element(e) => e,
                _ => continue,
            };
            let child_name = child_el.name.as_str();

            if all_known_fields.contains(child_name) {
                // Field is known - validate fully (scalar type, references, nested lists/objects).
                if let Some(child_field_schema) =
                    lookup_object_field_inherited(context.catalog, schema_ref, child_name)
                {
                    let child_field_path = format!("{field_name}[li].{child_name}");
                    validate_field_type(
                        doc,
                        summary,
                        child_id,
                        &child_field_path,
                        child_field_schema,
                        context,
                        diagnostics,
                    );
                    // Recurse into nested object-list fields (e.g. paramMappings inside SubSoundDef).
                    validate_object_list_items(
                        doc,
                        summary,
                        child_id,
                        &child_field_path,
                        child_field_schema,
                        context,
                        diagnostics,
                    );
                    // Recurse into nested object fields (e.g. inParam, outParam inside SoundParameterMapping).
                    validate_object_children(
                        doc,
                        summary,
                        child_id,
                        child_name,
                        child_field_schema,
                        &child_field_path,
                        context,
                        diagnostics,
                        0,
                    );
                }
            } else {
                diagnostics.push(
                    diag::warning_at_node(
                        doc,
                        child_id,
                        &summary.def_type,
                        summary.def_name.as_deref(),
                        "validation_unknown_object_field",
                        format!("Unknown field '{child_name}' in {field_name} list item."),
                    )
                    .with_field_path(format!("{field_name}[li].{child_name}"))
                    .with_args(diagnostic_args([
                        ("fieldName", child_name.into()),
                        ("containerPath", field_name.into()),
                    ])),
                );
            }
        }
    }
}
