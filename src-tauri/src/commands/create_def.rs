use crate::def_index::{apply_replacement_overlay, DefIndexReplacement};
use crate::project_model::{AppError, LocationKind, ProjectSettings};
use crate::schema_pack::{
    build_schema_catalog, lookup_object_field_inherited, DefTemplate, DefTypeSchema, FieldTypeKind,
    SchemaCatalog, XmlFieldShape,
};
use crate::services::{def_index_cache, validation, xml_editor as xml_editor_service};
use crate::settings_store::load_settings;
use crate::xml_document::{parse_to_document, XmlEditorDocumentLoadResult};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDefResult {
    pub editor_document: XmlEditorDocumentLoadResult,
    pub inserted_node_id: Option<usize>,
    pub inserted_def_type: String,
    pub inserted_def_name: Option<String>,
}

fn app_error(code: &str, message: impl Into<String>) -> AppError {
    AppError {
        code: code.to_string(),
        message: message.into(),
        details: None,
        args: crate::diagnostics::DiagnosticArgs::new(),
    }
}

fn app_error_with_args(
    code: &str,
    message: impl Into<String>,
    args: crate::diagnostics::DiagnosticArgs,
) -> AppError {
    app_error(code, message).with_args(args)
}

/// Verify `project_id` refers to a registered, writable project location. Mirrors
/// `commands::def_templates::require_writable_project` / `commands::form_views::require_writable_project`
/// exactly: every mutating create-def command is scoped to (and can mutate) a project's XML files,
/// so it must reject unknown/read-only/non-project ids rather than trusting whatever id the caller
/// passes through. Split into two distinct codes -- `create_def_invalid_target` (no such id) vs.
/// `create_def_target_not_editable` (a real id that is read-only or not a project) -- because the
/// two conditions have different causes and, unlike the "not found" case, the "not editable" case
/// has no `projectId` to report if a bare code with no args were reused for both (see the sibling
/// fix in `commands::def_templates`/`commands::form_views`, and Plan.md's "one code, one meaning"
/// diagnostic-code contract).
fn require_writable_project(settings: &ProjectSettings, project_id: &str) -> Result<(), AppError> {
    let location = settings
        .locations
        .iter()
        .find(|l| l.id == project_id)
        .ok_or_else(|| {
            app_error_with_args(
                "create_def_invalid_target",
                format!("No project with id '{}'.", project_id),
                crate::diagnostics::diagnostic_args([("projectId", project_id.into())]),
            )
        })?;
    if location.read_only || location.kind != LocationKind::Project {
        return Err(app_error_with_args(
            "create_def_target_not_editable",
            format!("The project '{}' is not editable.", project_id),
            crate::diagnostics::diagnostic_args([("projectId", project_id.into())]),
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn create_def_from_template(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
    def_type: String,
    template_id: Option<String>,
    field_values: BTreeMap<String, serde_json::Value>,
) -> Result<CreateDefResult, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    // Load schema catalog and look up the def type.
    let catalog_result = build_schema_catalog(&[], None);
    let catalog = &catalog_result.catalog;

    let def_type_schema = catalog.def_types.get(&def_type).ok_or_else(|| {
        app_error_with_args(
            "create_def_unknown_def_type",
            format!("Unknown def type '{}'.", def_type),
            crate::diagnostics::diagnostic_args([("defType", def_type.as_str().into())]),
        )
    })?;

    if def_type_schema.abstract_type {
        return Err(app_error_with_args(
            "create_def_unknown_def_type",
            format!(
                "Def type '{}' is abstract and cannot be created directly.",
                def_type
            ),
            crate::diagnostics::diagnostic_args([("defType", def_type.as_str().into())]),
        ));
    }

    // Resolve template (None = blank).
    let template: Option<&DefTemplate> = match &template_id {
        Some(id) => {
            let tpl = def_type_schema.templates.get(id).ok_or_else(|| {
                app_error_with_args(
                    "create_def_unknown_template",
                    format!("Template '{}' not found for '{}'.", id, def_type),
                    crate::diagnostics::diagnostic_args([
                        ("templateId", id.as_str().into()),
                        ("defType", def_type.as_str().into()),
                    ]),
                )
            })?;
            Some(tpl)
        }
        None => None,
    };

    // Merge field values: template base, then caller overrides.
    let mut merged: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    if let Some(tpl) = template {
        for (k, v) in &tpl.field_values {
            merged.insert(k.clone(), v.clone());
        }
    }
    for (k, v) in &field_values {
        merged.insert(k.clone(), v.clone());
    }

    // Extract defName: None if absent, non-string, or blank.
    let def_name: Option<String> = merged
        .get("defName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());

    // Normalize merged so build_field_lines sees the canonical state:
    // absent when defName is blank/invalid, or the trimmed value when present.
    // This prevents a caller supplying defName:"" or defName:"  " from emitting
    // a <defName> element while skipping pattern and duplicate checks.
    match &def_name {
        Some(dn) => {
            merged.insert("defName".to_string(), serde_json::Value::String(dn.clone()));
        }
        None => {
            merged.remove("defName");
        }
    }

    // Validate defName against schema pattern when present.
    if let Some(ref dn) = def_name {
        let all_fields_map = collect_all_fields(&def_type, catalog);
        if let Some(def_name_schema) = all_fields_map.get("defName") {
            if let Some(hints) = &def_name_schema.validation_hints {
                if let Some(pattern) = &hints.pattern {
                    if !matches_simple_pattern(dn, pattern) {
                        return Err(app_error_with_args(
                            "create_def_invalid_field_value",
                            format!(
                                "defName '{}' contains invalid characters. \
                                 Only letters, digits, underscores, and hyphens are allowed.",
                                dn
                            ),
                            crate::diagnostics::diagnostic_args([
                                ("fieldName", "defName".into()),
                                ("fieldValue", dn.as_str().into()),
                            ]),
                        ));
                    }
                }
            }
        }
    }

    // Reject a buffer that already has fatal parse errors - insertion would
    // produce nonsensical XML and the error would be misleading.
    let current_doc = parse_to_document(&relative_path, &raw_xml);
    if current_doc.had_fatal_parse_error {
        return Err(app_error(
            "create_def_xml_insert_failed",
            "The current XML buffer has parse errors. Fix them before inserting a new def.",
        ));
    }

    // Duplicate check via project index overlay (only when defName is present).
    let base_index = def_index_cache::load_for_project(&app, &settings, &project_id, false)?;
    let def_index = apply_replacement_overlay(
        (*base_index).clone(),
        &settings,
        DefIndexReplacement {
            location_id: &project_id,
            relative_path: &relative_path,
            source: &raw_xml,
        },
    );
    if let Some(ref dn) = def_name {
        if !def_index.find_project_duplicates(&def_type, dn).is_empty() {
            return Err(app_error_with_args(
                "create_def_duplicate_def_name",
                format!(
                    "A '{}' def named '{}' already exists in this project.",
                    def_type, dn
                ),
                crate::diagnostics::diagnostic_args([
                    ("defType", def_type.as_str().into()),
                    ("defName", dn.as_str().into()),
                ]),
            ));
        }
    }

    // Expand and insert the new def.
    let new_raw_xml = create_and_insert_def(
        &raw_xml,
        &def_type,
        &merged,
        def_type_schema,
        catalog,
        template,
    )?;

    // Re-parse and validate against the project.
    let mut fresh_doc = parse_to_document(&relative_path, &new_raw_xml);
    if fresh_doc.had_fatal_parse_error {
        return Err(app_error(
            "create_def_xml_insert_failed",
            "Insertion produced invalid XML. The def could not be inserted.",
        ));
    }
    validation::validate_doc_for_project(
        &app,
        &settings,
        &project_id,
        &relative_path,
        &mut fresh_doc,
    )?;

    // Find the inserted def's node ID (only possible when defName is known).
    let inserted_node_id = if let Some(ref dn) = def_name {
        fresh_doc
            .def_summaries
            .iter()
            .find(|s| s.def_type == def_type && s.def_name.as_deref() == Some(dn.as_str()))
            .map(|s| s.node_id)
    } else {
        None
    };

    let editor_document =
        xml_editor_service::build_editor_result(project_id, fresh_doc, new_raw_xml);

    Ok(CreateDefResult {
        editor_document,
        inserted_node_id,
        inserted_def_type: def_type,
        inserted_def_name: def_name,
    })
}

// ---------------------------------------------------------------------------
// Template expansion and XML insertion
// ---------------------------------------------------------------------------

/// Expand the template and splice the new def into `raw_xml`, returning the
/// updated document string.
fn create_and_insert_def(
    raw_xml: &str,
    def_type: &str,
    merged_values: &BTreeMap<String, serde_json::Value>,
    def_type_schema: &DefTypeSchema,
    catalog: &SchemaCatalog,
    template: Option<&DefTemplate>,
) -> Result<String, AppError> {
    // Expand self-closing <Defs /> so the scanner can find the closing tag.
    let expanded;
    let effective_xml: &str = if let Some(exp) = expand_self_closing_defs(raw_xml) {
        expanded = exp;
        &expanded
    } else {
        raw_xml
    };

    let child_indent = detect_child_indent(effective_xml);
    let inner_indent = format!("{}  ", child_indent);

    // Build the ordered field lines.
    let field_lines = build_field_lines(
        merged_values,
        def_type_schema,
        catalog,
        template,
        &inner_indent,
    )?;

    // Assemble the def block. The first line has no leading indent because
    // insert_def_block prepends `child_indent` when inserting.
    let mut def_block = format!("<{}>", def_type);
    for line in &field_lines {
        def_block.push('\n');
        def_block.push_str(line);
    }
    def_block.push('\n');
    def_block.push_str(&child_indent);
    def_block.push_str(&format!("</{}>", def_type));

    insert_def_block(effective_xml, &def_block, &child_indent)
}

/// Return the lines for fields inside the new def element, each prefixed with
/// `inner_indent`.
fn build_field_lines(
    merged_values: &BTreeMap<String, serde_json::Value>,
    def_type_schema: &DefTypeSchema,
    catalog: &SchemaCatalog,
    template: Option<&DefTemplate>,
    inner_indent: &str,
) -> Result<Vec<String>, AppError> {
    let include_required = template.is_none_or(|t| t.include_required_fields);

    // Determine canonical field order for the def type.
    let field_order = collect_effective_field_order(def_type_schema, catalog);

    // Collect all fields with their schemas for required-field checks.
    let all_fields = collect_all_fields_from_schema(def_type_schema, catalog);

    // Build the set of field names to include, in order:
    //  1. Fields from merged_values that exist in field_order (preserves canonical order)
    //  2. Remaining merged_values fields not in field_order
    //  3. Required fields not yet included (if include_required)
    let mut included: Vec<String> = Vec::new();
    let mut included_set: HashSet<String> = HashSet::new();

    // Step 1+2: merged_values fields in field_order, then remaining merged fields.
    for name in &field_order {
        if !included_set.contains(name) && merged_values.contains_key(name) {
            included.push(name.clone());
            included_set.insert(name.clone());
        }
    }
    for name in merged_values.keys() {
        if !included_set.contains(name) {
            included.push(name.clone());
            included_set.insert(name.clone());
        }
    }

    // Step 3: required fields not yet included.
    if include_required {
        for name in &field_order {
            if included_set.contains(name) {
                continue;
            }
            if let Some(field_schema) = all_fields.get(name) {
                if field_schema.required {
                    // Check for unsupported required fields with no value.
                    let xml_shape = &field_schema.xml;
                    if matches!(
                        xml_shape,
                        XmlFieldShape::Object | XmlFieldShape::NamedChildrenMap
                    ) {
                        return Err(app_error_with_args(
                            "create_def_unsupported_required_field",
                            format!(
                                "Required field '{}' has an unsupported XML shape and cannot be generated. \
                                 Provide a value for it in the template or supply it as a field value.",
                                name
                            ),
                            crate::diagnostics::diagnostic_args([(
                                "fieldName",
                                name.as_str().into(),
                            )]),
                        ));
                    }
                    included.push(name.clone());
                    included_set.insert(name.clone());
                }
            }
        }
    }

    // Render each included field to an XML line.
    let mut lines = Vec::new();
    for name in &included {
        let value = merged_values.get(name);
        let field_schema = all_fields.get(name);

        let xml_shape = field_schema.map(|f| &f.xml);
        let field_kind = field_schema.map(|f| &f.field_type.kind);

        // If the value is missing for a required field, use the schema defaultValue.
        let effective_value: Option<&serde_json::Value>;
        let default_storage;
        if let Some(v) = value {
            effective_value = Some(v);
        } else if let Some(schema) = field_schema {
            if schema.required {
                if let Some(dv) = &schema.default_value {
                    default_storage = dv.clone();
                    effective_value = Some(&default_storage);
                } else {
                    return Err(app_error_with_args(
                        "create_def_missing_required_field",
                        format!(
                            "Required field '{}' has no value and no schema default.",
                            name
                        ),
                        crate::diagnostics::diagnostic_args([("fieldName", name.as_str().into())]),
                    ));
                }
            } else {
                continue; // Optional field with no value - skip.
            }
        } else {
            continue; // Unknown field with no value - skip.
        };

        let val = effective_value.unwrap();

        // Render based on XML shape.
        let use_li_list = matches!(xml_shape, Some(XmlFieldShape::ListOfLi));

        if use_li_list {
            // Expect an array value.
            if let serde_json::Value::Array(items) = val {
                let mut inner = format!("{}<{}>", inner_indent, name);
                for item in items {
                    let s = json_value_to_string(item).map_err(|_| {
                        app_error_with_args(
                            "create_def_invalid_field_value",
                            format!("List items for '{}' must be strings or numbers.", name),
                            crate::diagnostics::diagnostic_args([(
                                "fieldName",
                                name.as_str().into(),
                            )]),
                        )
                    })?;
                    let s = apply_placeholders(&s, merged_values);
                    inner.push_str(&format!(
                        "\n{}  <li>{}</li>",
                        inner_indent,
                        escape_xml_text(&s)
                    ));
                }
                inner.push_str(&format!("\n{}</{}>", inner_indent, name));
                lines.push(inner);
            } else if let Some(s) = val.as_str() {
                let s = apply_placeholders(s, merged_values);
                lines.push(format!(
                    "{}<{}>\n{}  <li>{}</li>\n{}</{}>",
                    inner_indent,
                    name,
                    inner_indent,
                    escape_xml_text(&s),
                    inner_indent,
                    name
                ));
            }
        } else if matches!(
            xml_shape,
            Some(XmlFieldShape::Object) | Some(XmlFieldShape::NamedChildrenMap)
        ) {
            if let serde_json::Value::Object(obj) = val {
                // Look up the object schema to determine which keys are XML attributes.
                let schema_ref = field_schema.and_then(|f| f.field_type.schema_ref.as_deref());

                let mut attr_buf = String::new();
                let mut child_lines: Vec<String> = Vec::new();

                for (k, v) in obj {
                    let is_attr = schema_ref
                        .and_then(|r| lookup_object_field_inherited(catalog, r, k))
                        .map(|f| f.xml == XmlFieldShape::Attribute)
                        .unwrap_or(false);

                    let s = json_value_to_string(v).unwrap_or_default();
                    let s = apply_placeholders(&s, merged_values);
                    if is_attr {
                        attr_buf.push(' ');
                        attr_buf.push_str(k);
                        attr_buf.push_str("=\"");
                        attr_buf.push_str(&escape_xml_attr(&s));
                        attr_buf.push('"');
                    } else {
                        child_lines.push(format!(
                            "{}  <{}>{}</{}>",
                            inner_indent,
                            k,
                            escape_xml_text(&s),
                            k
                        ));
                    }
                }

                if child_lines.is_empty() {
                    lines.push(format!("{}<{}{}/>", inner_indent, name, attr_buf));
                } else {
                    let mut block = format!("{}<{}{}>", inner_indent, name, attr_buf);
                    for child_line in &child_lines {
                        block.push('\n');
                        block.push_str(child_line);
                    }
                    block.push('\n');
                    block.push_str(&format!("{}</{}>", inner_indent, name));
                    lines.push(block);
                }
            } else if let serde_json::Value::String(s) = val {
                let s = apply_placeholders(s, merged_values);
                lines.push(format!(
                    "{}<{}>{}</{}>",
                    inner_indent,
                    name,
                    escape_xml_text(&s),
                    name
                ));
            }
        } else if matches!(field_kind, Some(FieldTypeKind::Boolean)) {
            let b = match val {
                serde_json::Value::Bool(b) => *b,
                serde_json::Value::String(s) => s.eq_ignore_ascii_case("true"),
                serde_json::Value::Number(n) => n.as_i64().is_some_and(|i| i != 0),
                _ => {
                    return Err(app_error_with_args(
                        "create_def_invalid_field_value",
                        format!("Field '{}' requires a boolean value.", name),
                        crate::diagnostics::diagnostic_args([("fieldName", name.as_str().into())]),
                    ))
                }
            };
            lines.push(format!("{}<{}>{}</{}>", inner_indent, name, b, name));
        } else {
            // Default: render as simple element text.
            let text = json_value_to_string(val).map_err(|_| {
                app_error_with_args(
                    "create_def_invalid_field_value",
                    format!("Field '{}' has an unsupported JSON value type.", name),
                    crate::diagnostics::diagnostic_args([("fieldName", name.as_str().into())]),
                )
            })?;
            let text = apply_placeholders(&text, merged_values);
            lines.push(format!(
                "{}<{}>{}</{}>",
                inner_indent,
                name,
                escape_xml_text(&text),
                name
            ));
        }
    }

    Ok(lines)
}

/// Replace `{{fieldName}}` placeholders in `s` with the corresponding values
/// from `merged_values`.
fn apply_placeholders(s: &str, values: &BTreeMap<String, serde_json::Value>) -> String {
    if !s.contains("{{") {
        return s.to_string();
    }
    let mut result = s.to_string();
    for (key, val) in values {
        let placeholder = format!("{{{{{}}}}}", key);
        if result.contains(&placeholder) {
            if let Ok(replacement) = json_value_to_string(val) {
                result = result.replace(&placeholder, &replacement);
            }
        }
    }
    result
}

/// Convert a serde_json::Value to a string representation for XML text content.
fn json_value_to_string(val: &serde_json::Value) -> Result<String, ()> {
    match val {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        _ => Err(()),
    }
}

/// Escape XML text content (&, <, >).
fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape XML attribute value (&, <, ").
fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

/// Collect the effective field order for a def type: merge field_order from
/// the def type schema and all ancestors, with own fields taking precedence.
fn collect_effective_field_order(
    def_type_schema: &DefTypeSchema,
    catalog: &SchemaCatalog,
) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut order: Vec<String> = Vec::new();

    for name in &def_type_schema.field_order {
        if seen.insert(name.clone()) {
            order.push(name.clone());
        }
    }

    collect_inherited_field_order(def_type_schema, catalog, &mut seen, &mut order);

    order
}

fn collect_inherited_field_order(
    schema: &DefTypeSchema,
    catalog: &SchemaCatalog,
    seen: &mut HashSet<String>,
    order: &mut Vec<String>,
) {
    for parent_name in &schema.inherits {
        if let Some(parent) = catalog.def_types.get(parent_name) {
            for name in &parent.field_order {
                if seen.insert(name.clone()) {
                    order.push(name.clone());
                }
            }
            collect_inherited_field_order(parent, catalog, seen, order);
        }
    }
}

/// Collect all fields reachable from a def type name, walking the inheritance chain.
///
/// `pub(crate)` so `create_def_from_user_template` (in `commands::def_templates`)
/// can validate a user-supplied `defName` against the same schema pattern hints.
pub(crate) fn collect_all_fields<'a>(
    def_type: &str,
    catalog: &'a SchemaCatalog,
) -> BTreeMap<String, &'a crate::schema_pack::FieldSchema> {
    let mut result: BTreeMap<String, &crate::schema_pack::FieldSchema> = BTreeMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    collect_fields_recursive(def_type, catalog, &mut result, &mut visited);
    result
}

/// Collect all fields from a schema struct directly (without going through the name lookup).
fn collect_all_fields_from_schema<'a>(
    def_type_schema: &'a DefTypeSchema,
    catalog: &'a SchemaCatalog,
) -> BTreeMap<String, &'a crate::schema_pack::FieldSchema> {
    let mut result: BTreeMap<String, &crate::schema_pack::FieldSchema> = BTreeMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    // Process parents first so own fields override.
    for parent in &def_type_schema.inherits {
        collect_fields_recursive(parent, catalog, &mut result, &mut visited);
    }
    for (name, field) in &def_type_schema.fields {
        result.insert(name.clone(), field);
    }
    result
}

fn collect_fields_recursive<'a>(
    def_type: &str,
    catalog: &'a SchemaCatalog,
    result: &mut BTreeMap<String, &'a crate::schema_pack::FieldSchema>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(def_type.to_string()) {
        return;
    }
    if let Some(schema) = catalog.def_types.get(def_type) {
        for parent in &schema.inherits {
            collect_fields_recursive(parent, catalog, result, visited);
        }
        for (name, field) in &schema.fields {
            result.insert(name.clone(), field);
        }
    }
}

// ---------------------------------------------------------------------------
// XML string-level insertion
// ---------------------------------------------------------------------------

/// If `raw_xml` contains a self-closing `<...Defs .../>` element, expand it to
/// `<...Defs ...>\n</...Defs>` so `find_defs_close_pos` can locate the tag.
///
/// `pub(crate)` so `create_def_from_user_template` (in `commands::def_templates`)
/// can reuse the same insertion pipeline instead of duplicating it.
pub(crate) fn expand_self_closing_defs(raw_xml: &str) -> Option<String> {
    let bytes = raw_xml.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // Skip closing tags, comments, and processing instructions.
        if i + 1 < bytes.len()
            && (bytes[i + 1] == b'/' || bytes[i + 1] == b'!' || bytes[i + 1] == b'?')
        {
            i += 1;
            continue;
        }
        let name_start = i + 1;
        let mut j = name_start;
        while j < bytes.len()
            && bytes[j] != b'>'
            && bytes[j] != b'/'
            && bytes[j] != b' '
            && bytes[j] != b'\t'
            && bytes[j] != b'\n'
            && bytes[j] != b'\r'
        {
            j += 1;
        }
        let tag_name = &raw_xml[name_start..j];
        let base = tag_name.rfind(':').map_or(tag_name, |p| &tag_name[p + 1..]);
        if base == "Defs" {
            // Scan forward to the '>' looking for '/'.
            let mut k = j;
            while k < bytes.len() && bytes[k] != b'>' {
                k += 1;
            }
            if k > 0 && bytes[k - 1] == b'/' {
                // Self-closing. Build the expanded form.
                let attrs = raw_xml[j..k - 1].trim();
                let open = if attrs.is_empty() {
                    format!("<{}>", tag_name)
                } else {
                    format!("<{} {}>", tag_name, attrs)
                };
                let close = format!("</{}>", tag_name);
                return Some(format!(
                    "{}{}\n{}{}",
                    &raw_xml[..i],
                    open,
                    close,
                    &raw_xml[k + 1..]
                ));
            }
        }
        i += 1;
    }
    None
}

/// Find the byte position of the closing `</Defs>` (or `</...Defs>`) tag.
fn find_defs_close_pos(raw_xml: &str) -> Option<usize> {
    let bytes = raw_xml.as_bytes();
    let close_pattern = b"</";
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(close_pattern) {
            let tag_start = i;
            i += 2;
            while i < bytes.len() && bytes[i] != b'>' && bytes[i] != b' ' {
                i += 1;
            }
            let tag_end = i;
            let tag_name = &raw_xml[tag_start + 2..tag_end];
            let base = tag_name.rfind(':').map_or(tag_name, |p| &tag_name[p + 1..]);
            if base == "Defs" {
                return Some(tag_start);
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Detect the child element indentation from the raw XML by finding the first
/// line that is pure whitespace followed by `<`.
pub(crate) fn detect_child_indent(raw_xml: &str) -> String {
    for line_start_idx in raw_xml.match_indices('\n').map(|(i, _)| i + 1) {
        let rest = &raw_xml[line_start_idx..];
        let spaces_len = rest
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .map(char::len_utf8)
            .sum::<usize>();
        if spaces_len > 0 && rest[spaces_len..].starts_with('<') {
            return rest[..spaces_len].to_string();
        }
    }
    "  ".to_string()
}

/// Splice `def_block` into `raw_xml` immediately before the closing `</Defs>`
/// tag, using `child_indent` as the indentation prefix for the new def element.
pub(crate) fn insert_def_block(
    raw_xml: &str,
    def_block: &str,
    child_indent: &str,
) -> Result<String, AppError> {
    let close_pos = find_defs_close_pos(raw_xml).ok_or_else(|| {
        app_error(
            "create_def_xml_insert_failed",
            "Could not find a </Defs> closing tag in the XML buffer.",
        )
    })?;

    let before = &raw_xml[..close_pos];
    let after = &raw_xml[close_pos..];

    let leading_newline = if before.ends_with('\n') { "" } else { "\n" };

    let mut result = String::with_capacity(raw_xml.len() + def_block.len() + 64);
    result.push_str(before);
    result.push_str(leading_newline);
    result.push_str(child_indent);
    result.push_str(def_block);
    result.push('\n');
    result.push_str(after);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Pattern validation (no regex crate needed)
// ---------------------------------------------------------------------------

/// Validate `s` against a simple anchored character-class pattern of the form
/// `^[...]+$` or `^[...]*$`. Returns `true` for any pattern it cannot parse
/// (conservative: unknown patterns are not enforced).
///
/// `pub(crate)` so `create_def_from_user_template` (in `commands::def_templates`)
/// can reuse the same `defName` pattern validation.
pub(crate) fn matches_simple_pattern(s: &str, pattern: &str) -> bool {
    let p = pattern.trim();
    if !p.starts_with('^') || !p.ends_with('$') {
        return true;
    }
    let inner = &p[1..p.len() - 1];
    if !inner.starts_with('[') {
        return true;
    }
    let class_end = match inner.find(']') {
        Some(i) => i,
        None => return true,
    };
    let class_content = &inner[1..class_end];
    let quantifier = &inner[class_end + 1..];
    let min_count: usize = match quantifier {
        "+" => 1,
        "*" => 0,
        _ => return true,
    };

    if s.len() < min_count {
        return false;
    }
    s.chars().all(|c| char_in_class(c, class_content))
}

fn char_in_class(c: char, class: &str) -> bool {
    let chars: Vec<char> = class.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 2 < chars.len() && chars[i + 1] == '-' {
            if c >= chars[i] && c <= chars[i + 2] {
                return true;
            }
            i += 3;
        } else {
            if c == chars[i] {
                return true;
            }
            i += 1;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod project_validation_tests {
    use super::*;
    use crate::project_model::{RegisteredLocation, SourceType};
    use std::path::Path;
    use time::OffsetDateTime;

    fn make_location(id: &str, kind: LocationKind, read_only: bool) -> RegisteredLocation {
        RegisteredLocation {
            id: id.to_string(),
            display_name: id.to_string(),
            root_path: Path::new("/tmp").join(id).to_string_lossy().to_string(),
            kind,
            source_type: SourceType::Folder,
            read_only,
            mod_id: None,
            game_version: None,
            expansion_name: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    fn make_settings(locations: Vec<RegisteredLocation>) -> ProjectSettings {
        ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations,
            active_project_id: None,
        }
    }

    #[test]
    fn accepts_a_writable_project_location() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, false)]);
        assert!(require_writable_project(&settings, "proj1").is_ok());
    }

    #[test]
    fn rejects_unknown_project_id_with_projectid_arg() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, false)]);
        let err = require_writable_project(&settings, "does-not-exist").unwrap_err();
        assert_eq!(err.code, "create_def_invalid_target");
        assert_eq!(
            err.args.get("projectId"),
            Some(&crate::diagnostics::DiagnosticArgValue::Text(
                "does-not-exist".to_string()
            ))
        );
    }

    #[test]
    fn rejects_source_locations_as_not_editable_with_projectid_arg() {
        // Source locations are always read_only, but this asserts on `kind`
        // independently in case that invariant ever changes.
        let settings = make_settings(vec![make_location("src1", LocationKind::Source, true)]);
        let err = require_writable_project(&settings, "src1").unwrap_err();
        assert_eq!(err.code, "create_def_target_not_editable");
        assert_eq!(
            err.args.get("projectId"),
            Some(&crate::diagnostics::DiagnosticArgValue::Text(
                "src1".to_string()
            ))
        );
    }

    #[test]
    fn rejects_read_only_project_locations_as_not_editable() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, true)]);
        let err = require_writable_project(&settings, "proj1").unwrap_err();
        assert_eq!(err.code, "create_def_target_not_editable");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_child_indent_two_spaces() {
        let xml = "<Defs>\n  <ThingDef>\n    <defName>Foo</defName>\n  </ThingDef>\n</Defs>";
        assert_eq!(detect_child_indent(xml), "  ");
    }

    #[test]
    fn detect_child_indent_four_spaces() {
        let xml = "<Defs>\n    <ThingDef/>\n</Defs>";
        assert_eq!(detect_child_indent(xml), "    ");
    }

    #[test]
    fn detect_child_indent_default_when_no_children() {
        let xml = "<Defs>\n</Defs>";
        assert_eq!(detect_child_indent(xml), "  ");
    }

    #[test]
    fn find_defs_close_pos_basic() {
        let xml = "<Defs>\n  <ThingDef/>\n</Defs>";
        let pos = find_defs_close_pos(xml).unwrap();
        assert_eq!(&xml[pos..], "</Defs>");
    }

    #[test]
    fn find_defs_close_pos_namespaced() {
        let xml = "<ns:Defs>\n</ns:Defs>";
        let pos = find_defs_close_pos(xml).unwrap();
        assert_eq!(&xml[pos..], "</ns:Defs>");
    }

    #[test]
    fn insert_def_block_appends_before_close() {
        let xml = "<Defs>\n  <ThingDef>\n    <defName>A</defName>\n  </ThingDef>\n</Defs>";
        let block = "<ThingDef>\n    <defName>B</defName>\n  </ThingDef>";
        let result = insert_def_block(xml, block, "  ").unwrap();
        assert!(result.contains("<defName>A</defName>"));
        assert!(result.contains("<defName>B</defName>"));
        assert!(result.ends_with("</Defs>"));
    }

    #[test]
    fn escape_xml_text_handles_entities() {
        assert_eq!(escape_xml_text("a & b < c > d"), "a &amp; b &lt; c &gt; d");
    }

    #[test]
    fn expand_self_closing_defs_basic() {
        let xml = "<Defs />";
        let result = expand_self_closing_defs(xml).unwrap();
        assert_eq!(result, "<Defs>\n</Defs>");
    }

    #[test]
    fn expand_self_closing_defs_no_space() {
        let xml = "<Defs/>";
        let result = expand_self_closing_defs(xml).unwrap();
        assert_eq!(result, "<Defs>\n</Defs>");
    }

    #[test]
    fn expand_self_closing_defs_with_attrs() {
        let xml = "<Defs xmlns=\"foo\" />";
        let result = expand_self_closing_defs(xml).unwrap();
        assert!(result.starts_with("<Defs xmlns=\"foo\">"));
        assert!(result.contains("</Defs>"));
    }

    #[test]
    fn expand_self_closing_defs_normal_not_expanded() {
        let xml = "<Defs>\n</Defs>";
        assert!(expand_self_closing_defs(xml).is_none());
    }

    #[test]
    fn matches_simple_pattern_valid() {
        assert!(matches_simple_pattern("MyThing_123", "^[a-zA-Z0-9_-]+$"));
        assert!(matches_simple_pattern("foo-bar", "^[a-zA-Z0-9_-]+$"));
    }

    #[test]
    fn matches_simple_pattern_invalid() {
        assert!(!matches_simple_pattern("has space", "^[a-zA-Z0-9_-]+$"));
        assert!(!matches_simple_pattern("dot.name", "^[a-zA-Z0-9_-]+$"));
        assert!(!matches_simple_pattern("", "^[a-zA-Z0-9_-]+$"));
    }

    #[test]
    fn matches_simple_pattern_star_allows_empty() {
        assert!(matches_simple_pattern("", "^[a-zA-Z0-9_-]*$"));
    }

    #[test]
    fn apply_placeholders_substitutes_values() {
        let mut vals: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        vals.insert(
            "defName".to_string(),
            serde_json::Value::String("Foo".to_string()),
        );
        let result = apply_placeholders("Thing_{{defName}}", &vals);
        assert_eq!(result, "Thing_Foo");
    }

    #[test]
    fn apply_placeholders_noop_when_no_braces() {
        let vals: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        let result = apply_placeholders("plain text", &vals);
        assert_eq!(result, "plain text");
    }

    fn thing_def_schema_and_catalog() -> (crate::schema_pack::SchemaCatalog,) {
        let result = crate::schema_pack::build_schema_catalog(&[], None);
        (result.catalog,)
    }

    #[test]
    fn build_field_lines_no_def_name_emits_no_def_name_element() {
        let (catalog,) = thing_def_schema_and_catalog();
        let schema = catalog
            .def_types
            .get("ThingDef")
            .expect("ThingDef in built-in catalog");
        let merged: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        let lines = build_field_lines(&merged, schema, &catalog, None, "  ").unwrap();
        assert!(
            !lines.iter().any(|l| l.contains("<defName>")),
            "no defName in merged should produce no <defName> element: {:?}",
            lines
        );
    }

    #[test]
    fn build_field_lines_with_def_name_emits_element() {
        let (catalog,) = thing_def_schema_and_catalog();
        let schema = catalog
            .def_types
            .get("ThingDef")
            .expect("ThingDef in built-in catalog");
        let mut merged: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        merged.insert(
            "defName".to_string(),
            serde_json::Value::String("MyRock".to_string()),
        );
        let lines = build_field_lines(&merged, schema, &catalog, None, "  ").unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l.contains("<defName>MyRock</defName>")),
            "defName in merged should produce <defName>MyRock</defName>: {:?}",
            lines
        );
    }

    #[test]
    fn build_field_lines_blank_def_name_after_normalization_emits_no_element() {
        // Simulate what create_def_from_template does: blank defName is removed
        // from merged before build_field_lines is called.
        let (catalog,) = thing_def_schema_and_catalog();
        let schema = catalog
            .def_types
            .get("ThingDef")
            .expect("ThingDef in built-in catalog");
        let mut merged: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        // Start with a whitespace-only defName, then apply the normalization.
        merged.insert(
            "defName".to_string(),
            serde_json::Value::String("   ".to_string()),
        );
        let def_name: Option<String> = merged
            .get("defName")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());
        match &def_name {
            Some(dn) => {
                merged.insert("defName".to_string(), serde_json::Value::String(dn.clone()));
            }
            None => {
                merged.remove("defName");
            }
        }
        let lines = build_field_lines(&merged, schema, &catalog, None, "  ").unwrap();
        assert!(
            !lines.iter().any(|l| l.contains("<defName>")),
            "blank defName after normalization must not emit a <defName> element: {:?}",
            lines
        );
    }
}
