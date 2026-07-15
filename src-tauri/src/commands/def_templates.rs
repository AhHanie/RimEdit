use super::create_def::{
    collect_all_fields, detect_child_indent, expand_self_closing_defs, insert_def_block,
    matches_simple_pattern, CreateDefResult,
};
use crate::def_index::{apply_replacement_overlay, DefIndex, DefIndexReplacement};
use crate::def_templates::{self, NewUserDefTemplate, UserDefTemplate, UserDefTemplateSummary};
use crate::project_files::validate_and_resolve_location;
use crate::project_model::{AppError, LocationKind, ProjectSettings};
use crate::schema_pack::build_schema_catalog;
use crate::services::{def_index_cache, validation, xml_editor as xml_editor_service};
use crate::settings_store::load_settings;
use crate::xml_document::{
    apply_xml_edit, parse_to_document, serialize_xml_document, XmlEdit, XmlEditContext,
};
use serde::Serialize;
use tauri::AppHandle;

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

/// Verify `project_id` refers to a registered, writable project location.
/// Every def-template command is scoped to (and can mutate) a project's
/// template store, so each one must reject unknown/read-only/non-project ids
/// rather than trusting whatever id the caller passes through to the store.
/// Uses two distinct codes -- `def_template_invalid_target` (no such id) vs.
/// `def_template_target_not_editable` (a real id that is read-only or not a
/// project) -- since the two conditions have different causes and only the
/// former can be caused by an arbitrary caller-supplied id. Mirrors
/// `commands::create_def::require_writable_project` /
/// `commands::form_views::require_writable_project` exactly.
fn require_writable_project(settings: &ProjectSettings, project_id: &str) -> Result<(), AppError> {
    let location = settings
        .locations
        .iter()
        .find(|l| l.id == project_id)
        .ok_or_else(|| {
            app_error_with_args(
                "def_template_invalid_target",
                format!("No project with id '{}'.", project_id),
                crate::diagnostics::diagnostic_args([("projectId", project_id.into())]),
            )
        })?;
    if location.read_only || location.kind != LocationKind::Project {
        return Err(app_error_with_args(
            "def_template_target_not_editable",
            format!("The project '{}' is not editable.", project_id),
            crate::diagnostics::diagnostic_args([("projectId", project_id.into())]),
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn list_user_def_templates(
    app: AppHandle,
    project_id: String,
    def_type: String,
) -> Result<Vec<UserDefTemplateSummary>, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    let templates = def_templates::list_templates(&app, &project_id, &def_type)?;
    Ok(templates.iter().map(UserDefTemplateSummary::from).collect())
}

/// Result of extracting the selected Def's exact source XML from a raw buffer.
#[derive(Debug)]
struct ExtractedDef {
    def_type: String,
    xml: String,
    original_def_name: Option<String>,
    original_label: Option<String>,
}

/// Parse `raw_xml`, locate the `DefSummary` matching `node_id`, and slice out the
/// exact source span for that Def. Re-parses the extracted slice to confirm it
/// stands alone as exactly one Def of the expected type. Pure/no-`AppHandle` so
/// it can be unit tested directly.
fn extract_selected_def_xml(
    relative_path: &str,
    raw_xml: &str,
    node_id: usize,
) -> Result<ExtractedDef, AppError> {
    let doc = parse_to_document(relative_path, raw_xml);
    if doc.had_fatal_parse_error {
        return Err(app_error(
            "save_def_template_parse_error",
            "The current XML buffer has parse errors. Fix them before saving a template.",
        ));
    }

    let def_summary = doc
        .def_summaries
        .iter()
        .find(|d| d.node_id == node_id)
        .ok_or_else(|| {
            app_error(
                "save_def_template_def_not_found",
                "The selected Def could not be found in the current buffer.",
            )
        })?;

    let def_type = def_summary.def_type.clone();
    let original_def_name = def_summary.def_name.clone();
    let original_label = def_summary.label.clone();

    let node = &doc.nodes[node_id];
    let extracted_xml = doc.source[node.span.start..node.span.end].to_string();

    // Validate the extracted slice stands alone as exactly one Def of the expected type.
    let extracted_doc = parse_to_document(relative_path, &extracted_xml);
    if extracted_doc.had_fatal_parse_error || extracted_doc.def_summaries.len() != 1 {
        return Err(app_error(
            "save_def_template_extraction_failed",
            "Could not extract a single valid Def from the current selection.",
        ));
    }
    if extracted_doc.def_summaries[0].def_type != def_type {
        return Err(app_error(
            "save_def_template_extraction_failed",
            "Extracted Def type did not match the selected Def.",
        ));
    }

    Ok(ExtractedDef {
        def_type,
        xml: extracted_xml,
        original_def_name,
        original_label,
    })
}

#[tauri::command]
pub fn save_user_def_template(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
    node_id: usize,
    name: String,
) -> Result<UserDefTemplate, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err(app_error(
            "save_def_template_invalid_name",
            "Template name must not be blank.",
        ));
    }

    let extracted = extract_selected_def_xml(&relative_path, &raw_xml, node_id)?;

    def_templates::save_template(
        &app,
        &project_id,
        NewUserDefTemplate {
            def_type: extracted.def_type,
            name: trimmed_name.to_string(),
            description: None,
            xml: extracted.xml,
            original_def_name: extracted.original_def_name,
            original_label: extracted.original_label,
            source_relative_path: Some(relative_path),
            game_version: Some(settings.game_version.clone()),
        },
    )
}

/// Replace the def block's `<defName>` element with `new_def_name`, inserting one
/// if the block has none. Reuses `apply_xml_edit`'s `SetChildElementText` - the
/// same machinery the interactive form editor uses to edit a live document - so
/// indentation/whitespace conventions are preserved and only the `defName`
/// element's text changes; every other attribute, comment, and nested list in
/// the block survives untouched. Pure/no-`AppHandle` so it can be unit tested
/// directly (mirrors `extract_selected_def_xml` above). Shared by
/// `create_def_from_user_template` (over a saved template's XML) and
/// `create_def_from_indexed_def` (over an indexed def's extracted XML).
fn set_or_insert_def_name(
    def_type: &str,
    block_xml: &str,
    new_def_name: &str,
) -> Result<String, AppError> {
    let mut doc = parse_to_document("clone_source.xml", block_xml);
    if doc.had_fatal_parse_error || doc.def_summaries.len() != 1 {
        return Err(app_error(
            "create_def_clone_invalid_def_block",
            "The source XML could not be parsed as a single Def.",
        ));
    }
    if doc.def_summaries[0].def_type != def_type {
        return Err(app_error(
            "create_def_clone_invalid_def_block",
            "The source Def's type no longer matches its record.",
        ));
    }
    let node_id = doc.def_summaries[0].node_id;

    // A `<Defs>` wrapper with exactly one child also produces exactly one
    // DefSummary, but `node_id` would then point at the *inner* Def while
    // `serialize_xml_document` below serializes the whole document - wrapper
    // included. Splicing that into the target buffer would nest a `<Defs>`
    // inside the target's own `<Defs>`. Require the Def to be a genuine
    // top-level element (never true for XML `save_user_def_template` or
    // `extract_indexed_def_xml` produce, but defensive against a
    // hand-edited/corrupted store or an unusual source file).
    if !doc.top_level_nodes.contains(&node_id) {
        return Err(app_error(
            "create_def_clone_invalid_def_block",
            "The source XML must be a single Def element, not wrapped in another element.",
        ));
    }

    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: node_id,
            child_name: "defName".to_string(),
            value: new_def_name.to_string(),
        },
        &XmlEditContext::default(),
    )
    .map_err(|e| app_error("create_def_clone_invalid_def_block", e.to_string()))?;

    Ok(serialize_xml_document(&doc))
}

/// Reject a blank/whitespace-only `def_name`, returning the trimmed value.
/// Pure so it is unit testable directly. Shared by `create_def_from_user_template`
/// and `create_def_from_indexed_def`.
fn require_non_blank_def_name(def_name: &str) -> Result<&str, AppError> {
    let trimmed = def_name.trim();
    if trimmed.is_empty() {
        return Err(app_error(
            "create_def_clone_invalid_def_name",
            "Def name must not be blank.",
        ));
    }
    Ok(trimmed)
}

/// Validate `def_name` against the `defName` field's schema pattern hint for
/// `def_type`, when the catalog declares one. Pure/no-`AppHandle` (schema
/// catalog construction doesn't need one either) so it is unit testable
/// directly, matching `create_def_from_template`'s inline pattern check.
///
/// No built-in schema pack currently declares a `defName` pattern hint for
/// any def type, so this branch is presently dormant in production - it
/// exists so a future pack can add one without touching this command. That
/// also means there is no real-catalog fixture to exercise the rejection
/// branch in a test; `matches_simple_pattern` (the character-class matcher
/// this delegates to) has its own direct unit tests in `create_def.rs`.
/// Shared by `create_def_from_user_template` and `create_def_from_indexed_def`.
fn validate_def_name_pattern(def_type: &str, def_name: &str) -> Result<(), AppError> {
    let catalog_result = build_schema_catalog(&[], None);
    let all_fields = collect_all_fields(def_type, &catalog_result.catalog);
    if let Some(def_name_schema) = all_fields.get("defName") {
        if let Some(hints) = &def_name_schema.validation_hints {
            if let Some(pattern) = &hints.pattern {
                if !matches_simple_pattern(def_name, pattern) {
                    return Err(app_error_with_args(
                        "create_def_clone_invalid_def_name",
                        format!(
                            "defName '{}' contains invalid characters. \
                             Only letters, digits, underscores, and hyphens are allowed.",
                            def_name
                        ),
                        crate::diagnostics::diagnostic_args([("fieldValue", def_name.into())]),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Reject `def_name` when a writable project-owned `def_type` Def already uses
/// it. Pure - takes an already-loaded `DefIndex` rather than loading one via
/// `AppHandle`, so it is unit testable directly against a hand-built index.
/// Shared by `create_def_from_user_template` and `create_def_from_indexed_def`.
fn reject_duplicate_def_name(
    def_index: &DefIndex,
    def_type: &str,
    def_name: &str,
) -> Result<(), AppError> {
    if !def_index
        .find_project_duplicates(def_type, def_name)
        .is_empty()
    {
        return Err(app_error_with_args(
            "create_def_clone_duplicate_def_name",
            format!(
                "A '{}' def named '{}' already exists in this project.",
                def_type, def_name
            ),
            crate::diagnostics::diagnostic_args([
                ("defType", def_type.into()),
                ("defName", def_name.into()),
            ]),
        ));
    }
    Ok(())
}

/// Reject an indexed-def clone request when the requested source def can no
/// longer be found in `index` at the given identity. Pure - takes an
/// already-loaded `DefIndex` so it is unit testable directly against a
/// hand-built index. Callers must pass the *base* (non-overlaid) index: the
/// clone source is always read straight off disk, never through a
/// target-buffer overlay, so existence must be checked against the same
/// disk-reflecting view. `source_node_id`, when given, disambiguates between
/// multiple defs sharing the same type/name in one file - the same identity
/// `extract_indexed_def_xml` uses.
fn reject_missing_indexed_source(
    index: &DefIndex,
    source_location_id: &str,
    source_relative_path: &str,
    source_def_type: &str,
    source_def_name: &str,
    source_node_id: Option<usize>,
) -> Result<(), AppError> {
    let found = index.defs.iter().any(|d| {
        d.source.location_id == source_location_id
            && d.relative_path == source_relative_path
            && d.def_type == source_def_type
            && d.def_name == source_def_name
            && source_node_id.is_none_or(|nid| d.node_id == Some(nid))
    });
    if !found {
        return Err(app_error_with_args(
            "create_def_from_indexed_def_source_not_found",
            format!(
                "'{}' ({}) was not found in the index at '{}'.",
                source_def_name, source_def_type, source_relative_path
            ),
            crate::diagnostics::diagnostic_args([
                ("defName", source_def_name.into()),
                ("defType", source_def_type.into()),
                ("relativePath", source_relative_path.into()),
            ]),
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn create_def_from_user_template(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
    template_id: String,
    def_name: String,
) -> Result<CreateDefResult, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    let trimmed_def_name = require_non_blank_def_name(&def_name)?;

    let template = def_templates::get_template(&app, &project_id, &template_id)?;
    validate_def_name_pattern(&template.def_type, trimmed_def_name)?;

    // Reject a target buffer that already has fatal parse errors - insertion would
    // produce nonsensical XML and any resulting error would be misleading.
    let current_doc = parse_to_document(&relative_path, &raw_xml);
    if current_doc.had_fatal_parse_error {
        return Err(app_error(
            "create_def_from_user_template_target_parse_error",
            "The current XML buffer has parse errors. Fix them before inserting a new def.",
        ));
    }

    // Duplicate check via the project index overlay, same approach as create_def_from_template.
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
    reject_duplicate_def_name(&def_index, &template.def_type, trimmed_def_name)?;

    let new_def_block =
        set_or_insert_def_name(&template.def_type, &template.xml, trimmed_def_name)?;

    // Splice into the target buffer using the same insertion pipeline as
    // create_def_from_template (self-closing <Defs/> expansion + insert-before-close).
    let expanded;
    let effective_xml: &str = if let Some(exp) = expand_self_closing_defs(&raw_xml) {
        expanded = exp;
        &expanded
    } else {
        &raw_xml
    };
    let child_indent = detect_child_indent(effective_xml);
    let new_raw_xml = insert_def_block(effective_xml, &new_def_block, &child_indent)?;

    let mut fresh_doc = parse_to_document(&relative_path, &new_raw_xml);
    if fresh_doc.had_fatal_parse_error {
        return Err(app_error(
            "create_def_from_user_template_insert_failed",
            "Insertion produced invalid XML. The def could not be inserted.",
        ));
    }
    // Attaches validation_diagnostics to fresh_doc for the frontend to display;
    // deliberately does not reject on blocking diagnostics here, matching
    // create_def_from_template and every other editor-buffer-mutating command
    // (apply_xml_editor_edit, parse_xml_editor_buffer, ...). Hard-blocking on
    // diagnostics happens once, at save time, in preview_project_xml_save /
    // save_project_xml_file - not at every intermediate edit - so inserting a
    // template that still needs further editing before it fully validates
    // behaves the same as inserting a built-in template or hand-typing a Def.
    validation::validate_doc_for_project(
        &app,
        &settings,
        &project_id,
        &relative_path,
        &mut fresh_doc,
    )?;

    let inserted_node_id = fresh_doc
        .def_summaries
        .iter()
        .find(|s| {
            s.def_type == template.def_type && s.def_name.as_deref() == Some(trimmed_def_name)
        })
        .map(|s| s.node_id);

    let editor_document =
        xml_editor_service::build_editor_result(project_id, fresh_doc, new_raw_xml);

    Ok(CreateDefResult {
        editor_document,
        inserted_node_id,
        inserted_def_type: template.def_type,
        inserted_def_name: Some(trimmed_def_name.to_string()),
    })
}

/// Result of extracting a single Def's exact source XML out of an indexed
/// source file. Mirrors `ExtractedDef` above, but keyed by the caller-supplied
/// expected identity (`def_type`/`def_name`) rather than a `node_id` in the
/// currently-open buffer, since the source file is on disk, not open for editing.
#[derive(Debug)]
struct IndexedDefCloneSource {
    def_type: String,
    xml: String,
}

/// Parse `source_raw_xml`, locate the `DefSummary` matching `expected_def_type`/
/// `expected_def_name`, and slice out its exact source span. When more than one
/// Def in the source file shares that type/name, `source_node_id` (from the
/// index entry the frontend selected) disambiguates; without it, ambiguity is
/// rejected rather than guessing. Re-parses the extracted slice to confirm it
/// stands alone as exactly one Def of the expected type, exactly like
/// `extract_selected_def_xml` above. Pure/no-`AppHandle` so it can be unit
/// tested directly.
fn extract_indexed_def_xml(
    source_relative_path: &str,
    source_raw_xml: &str,
    expected_def_type: &str,
    expected_def_name: &str,
    source_node_id: Option<usize>,
) -> Result<IndexedDefCloneSource, AppError> {
    let doc = parse_to_document(source_relative_path, source_raw_xml);
    if doc.had_fatal_parse_error {
        return Err(app_error(
            "create_def_from_indexed_def_source_parse_error",
            "The source file has parse errors and cannot be cloned from.",
        ));
    }

    let matches: Vec<_> = doc
        .def_summaries
        .iter()
        .filter(|d| {
            d.def_type == expected_def_type && d.def_name.as_deref() == Some(expected_def_name)
        })
        .collect();

    let def_summary = if let Some(node_id) = source_node_id {
        matches
            .into_iter()
            .find(|d| d.node_id == node_id)
            .ok_or_else(|| {
                app_error(
                    "create_def_from_indexed_def_invalid_source",
                    "The selected Def could not be found at the given location in the source file.",
                )
            })?
    } else {
        match matches.len() {
            0 => {
                return Err(app_error(
                    "create_def_from_indexed_def_invalid_source",
                    "The selected Def could not be found in the source file.",
                ))
            }
            1 => matches[0],
            _ => {
                return Err(app_error(
                    "create_def_from_indexed_def_invalid_source",
                    "Multiple Defs with this name were found in the source file; the selection is ambiguous.",
                ))
            }
        }
    };

    let node = &doc.nodes[def_summary.node_id];
    let extracted_xml = doc.source[node.span.start..node.span.end].to_string();

    // Validate the extracted slice stands alone as exactly one Def of the expected type.
    let extracted_doc = parse_to_document(source_relative_path, &extracted_xml);
    if extracted_doc.had_fatal_parse_error || extracted_doc.def_summaries.len() != 1 {
        return Err(app_error(
            "create_def_from_indexed_def_invalid_source",
            "Could not extract a single valid Def from the source file.",
        ));
    }
    if extracted_doc.def_summaries[0].def_type != expected_def_type {
        return Err(app_error(
            "create_def_from_indexed_def_invalid_source",
            "Extracted Def type did not match the selected Def.",
        ));
    }

    Ok(IndexedDefCloneSource {
        def_type: expected_def_type.to_string(),
        xml: extracted_xml,
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn create_def_from_indexed_def(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
    source_location_id: String,
    source_relative_path: String,
    source_def_type: String,
    source_def_name: String,
    source_node_id: Option<usize>,
    def_name: String,
) -> Result<CreateDefResult, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    let trimmed_def_name = require_non_blank_def_name(&def_name)?;
    validate_def_name_pattern(&source_def_type, trimmed_def_name)?;

    // Reject a target buffer that already has fatal parse errors - insertion would
    // produce nonsensical XML and any resulting error would be misleading.
    let current_doc = parse_to_document(&relative_path, &raw_xml);
    if current_doc.had_fatal_parse_error {
        return Err(app_error(
            "create_def_from_indexed_def_target_parse_error",
            "The current XML buffer has parse errors. Fix them before inserting a new def.",
        ));
    }

    // Duplicate check via the project index overlay, same approach as create_def_from_user_template.
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
    reject_duplicate_def_name(&def_index, &source_def_type, trimmed_def_name)?;

    // Verify the requested source def still exists in the current def index -
    // the selection could have gone stale between search and create (file edited
    // or removed on disk since the index was last built). Checked against
    // `base_index`, not the overlaid `def_index` above: the overlay replaces
    // whatever the index knows about the *target* file with the unsaved buffer's
    // content, but the clone source is always read straight off disk below, so
    // checking existence against the overlay would (when source and target are
    // the same file) validate against the buffer while extracting from disk -
    // wrongly rejecting a clone of a still-on-disk def the buffer has since
    // edited away, or wrongly accepting one it hasn't.
    reject_missing_indexed_source(
        &base_index,
        &source_location_id,
        &source_relative_path,
        &source_def_type,
        &source_def_name,
        source_node_id,
    )?;

    // Read the source file directly off disk - never trust a raw_xml buffer from the
    // frontend for the clone source, only for the target (validate_and_resolve_location
    // rejects path traversal/absolute paths/non-XML extensions).
    let canonical =
        validate_and_resolve_location(&settings, &source_location_id, &source_relative_path)
            .map_err(AppError::from)?;
    let source_raw_xml = std::fs::read_to_string(&canonical).map_err(|e| {
        app_error_with_args(
            "create_def_from_indexed_def_source_read_failed",
            format!("Failed to read '{}': {}", canonical.display(), e),
            crate::diagnostics::diagnostic_args([(
                "path",
                canonical.to_string_lossy().into_owned().into(),
            )]),
        )
    })?;

    let extracted = extract_indexed_def_xml(
        &source_relative_path,
        &source_raw_xml,
        &source_def_type,
        &source_def_name,
        source_node_id,
    )?;

    let new_def_block =
        set_or_insert_def_name(&extracted.def_type, &extracted.xml, trimmed_def_name)?;

    // Splice into the target buffer using the same insertion pipeline as
    // create_def_from_user_template.
    let expanded;
    let effective_xml: &str = if let Some(exp) = expand_self_closing_defs(&raw_xml) {
        expanded = exp;
        &expanded
    } else {
        &raw_xml
    };
    let child_indent = detect_child_indent(effective_xml);
    let new_raw_xml = insert_def_block(effective_xml, &new_def_block, &child_indent)?;

    let mut fresh_doc = parse_to_document(&relative_path, &new_raw_xml);
    if fresh_doc.had_fatal_parse_error {
        return Err(app_error(
            "create_def_from_indexed_def_insert_failed",
            "Insertion produced invalid XML. The def could not be inserted.",
        ));
    }
    // See create_def_from_user_template above for why blocking diagnostics don't
    // reject here - the same intermediate-edit-vs-save-time distinction applies.
    validation::validate_doc_for_project(
        &app,
        &settings,
        &project_id,
        &relative_path,
        &mut fresh_doc,
    )?;

    let inserted_node_id = fresh_doc
        .def_summaries
        .iter()
        .find(|s| s.def_type == source_def_type && s.def_name.as_deref() == Some(trimmed_def_name))
        .map(|s| s.node_id);

    let editor_document =
        xml_editor_service::build_editor_result(project_id, fresh_doc, new_raw_xml);

    Ok(CreateDefResult {
        editor_document,
        inserted_node_id,
        inserted_def_type: source_def_type,
        inserted_def_name: Some(trimmed_def_name.to_string()),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUserDefTemplateResult {
    pub deleted_id: String,
}

#[tauri::command]
pub fn delete_user_def_template(
    app: AppHandle,
    project_id: String,
    template_id: String,
) -> Result<DeleteUserDefTemplateResult, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    def_templates::delete_template(&app, &project_id, &template_id)?;
    Ok(DeleteUserDefTemplateResult {
        deleted_id: template_id,
    })
}

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
    fn rejects_unknown_project_id() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, false)]);
        let err = require_writable_project(&settings, "does-not-exist").unwrap_err();
        assert_eq!(err.code, "def_template_invalid_target");
    }

    #[test]
    fn rejects_source_locations() {
        // Source locations are always read_only, but this asserts on `kind`
        // independently in case that invariant ever changes.
        let settings = make_settings(vec![make_location("src1", LocationKind::Source, true)]);
        let err = require_writable_project(&settings, "src1").unwrap_err();
        assert_eq!(err.code, "def_template_target_not_editable");
        assert_eq!(
            err.args.get("projectId"),
            Some(&crate::diagnostics::DiagnosticArgValue::Text(
                "src1".to_string()
            ))
        );
    }

    #[test]
    fn rejects_read_only_project_locations() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, true)]);
        let err = require_writable_project(&settings, "proj1").unwrap_err();
        assert_eq!(err.code, "def_template_target_not_editable");
    }
}

#[cfg(test)]
mod extraction_tests {
    use super::*;

    #[test]
    fn extracts_exact_xml_preserving_unknown_fields_attrs_lists_and_comments() {
        let raw = r#"<Defs>
  <ThingDef ParentName="BaseWeapon">
    <!-- a helpful comment -->
    <defName>Gun_Autopistol</defName>
    <label>autopistol</label>
    <someUnknownField>42</someUnknownField>
    <tradeTags>
      <li>Weapon</li>
      <li>RangedWeapon</li>
    </tradeTags>
  </ThingDef>
  <ThingDef>
    <defName>Other</defName>
  </ThingDef>
</Defs>"#;

        let doc = parse_to_document("Defs/Weapons.xml", raw);
        assert!(!doc.had_fatal_parse_error);
        let target = doc
            .def_summaries
            .iter()
            .find(|d| d.def_name.as_deref() == Some("Gun_Autopistol"))
            .unwrap();

        let extracted = extract_selected_def_xml("Defs/Weapons.xml", raw, target.node_id).unwrap();

        assert_eq!(extracted.def_type, "ThingDef");
        assert_eq!(
            extracted.original_def_name.as_deref(),
            Some("Gun_Autopistol")
        );
        assert_eq!(extracted.original_label.as_deref(), Some("autopistol"));
        assert!(extracted
            .xml
            .starts_with(r#"<ThingDef ParentName="BaseWeapon">"#));
        assert!(extracted.xml.contains("<!-- a helpful comment -->"));
        assert!(extracted
            .xml
            .contains("<someUnknownField>42</someUnknownField>"));
        assert!(extracted.xml.contains("<li>Weapon</li>"));
        assert!(extracted.xml.contains("<li>RangedWeapon</li>"));
        assert!(!extracted.xml.contains("Other"));
    }

    #[test]
    fn rejects_fatal_parse_errors() {
        let raw = "<Defs><ThingDef><defName>Broken</ThingDef></Defs>";
        let err = extract_selected_def_xml("Defs/Broken.xml", raw, 0).unwrap_err();
        assert_eq!(err.code, "save_def_template_parse_error");
    }

    #[test]
    fn rejects_unknown_node_id() {
        let raw = "<Defs><ThingDef><defName>Foo</defName></ThingDef></Defs>";
        let doc = parse_to_document("Defs/Foo.xml", raw);
        let missing_id = doc.nodes.len() + 100;
        let err = extract_selected_def_xml("Defs/Foo.xml", raw, missing_id).unwrap_err();
        assert_eq!(err.code, "save_def_template_def_not_found");
    }

    #[test]
    fn extracts_single_root_def_without_defs_wrapper() {
        let raw = "<ThingDef>\n  <defName>Solo</defName>\n</ThingDef>";
        let doc = parse_to_document("Defs/Solo.xml", raw);
        let target = doc.def_summaries.first().unwrap();
        let extracted = extract_selected_def_xml("Defs/Solo.xml", raw, target.node_id).unwrap();
        assert_eq!(extracted.def_type, "ThingDef");
        assert_eq!(extracted.xml, raw);
    }
}

#[cfg(test)]
mod extract_indexed_def_xml_tests {
    use super::*;

    #[test]
    fn extracts_exact_xml_preserving_unknown_fields_attrs_lists_and_comments() {
        let raw = r#"<Defs>
  <ThingDef ParentName="BaseWeapon">
    <!-- a helpful comment -->
    <defName>Gun_Autopistol</defName>
    <label>autopistol</label>
    <someUnknownField>42</someUnknownField>
    <tradeTags>
      <li>Weapon</li>
      <li>RangedWeapon</li>
    </tradeTags>
  </ThingDef>
  <ThingDef>
    <defName>Other</defName>
  </ThingDef>
</Defs>"#;

        let extracted =
            extract_indexed_def_xml("Defs/Weapons.xml", raw, "ThingDef", "Gun_Autopistol", None)
                .unwrap();

        assert_eq!(extracted.def_type, "ThingDef");
        assert!(extracted
            .xml
            .starts_with(r#"<ThingDef ParentName="BaseWeapon">"#));
        assert!(extracted.xml.contains("<!-- a helpful comment -->"));
        assert!(extracted
            .xml
            .contains("<someUnknownField>42</someUnknownField>"));
        assert!(extracted.xml.contains("<li>Weapon</li>"));
        assert!(extracted.xml.contains("<li>RangedWeapon</li>"));
        assert!(!extracted.xml.contains("Other"));
    }

    #[test]
    fn rejects_fatal_parse_errors() {
        let raw = "<Defs><ThingDef><defName>Broken</ThingDef></Defs>";
        let err = extract_indexed_def_xml("Defs/Broken.xml", raw, "ThingDef", "Broken", None)
            .unwrap_err();
        assert_eq!(err.code, "create_def_from_indexed_def_source_parse_error");
    }

    #[test]
    fn rejects_source_with_no_matching_def() {
        let raw = "<Defs><ThingDef><defName>Foo</defName></ThingDef></Defs>";
        let err = extract_indexed_def_xml("Defs/Foo.xml", raw, "ThingDef", "DoesNotExist", None)
            .unwrap_err();
        assert_eq!(err.code, "create_def_from_indexed_def_invalid_source");
    }

    #[test]
    fn rejects_ambiguous_duplicate_defs_without_node_id() {
        let raw = "<Defs><ThingDef><defName>Dup</defName><label>one</label></ThingDef><ThingDef><defName>Dup</defName><label>two</label></ThingDef></Defs>";
        let err =
            extract_indexed_def_xml("Defs/Dup.xml", raw, "ThingDef", "Dup", None).unwrap_err();
        assert_eq!(err.code, "create_def_from_indexed_def_invalid_source");
    }

    #[test]
    fn uses_node_id_to_disambiguate_duplicate_defs() {
        let raw = "<Defs><ThingDef><defName>Dup</defName><label>one</label></ThingDef><ThingDef><defName>Dup</defName><label>two</label></ThingDef></Defs>";
        let doc = parse_to_document("Defs/Dup.xml", raw);
        let second = doc
            .def_summaries
            .iter()
            .find(|d| d.label.as_deref() == Some("two"))
            .unwrap();

        let extracted =
            extract_indexed_def_xml("Defs/Dup.xml", raw, "ThingDef", "Dup", Some(second.node_id))
                .unwrap();

        assert!(extracted.xml.contains("<label>two</label>"));
        assert!(!extracted.xml.contains("<label>one</label>"));
    }

    #[test]
    fn extracts_single_root_def_without_defs_wrapper() {
        let raw = "<ThingDef>\n  <defName>Solo</defName>\n</ThingDef>";
        let extracted =
            extract_indexed_def_xml("Defs/Solo.xml", raw, "ThingDef", "Solo", None).unwrap();
        assert_eq!(extracted.def_type, "ThingDef");
        assert_eq!(extracted.xml, raw);
    }

    /// Reproduces the wizard's real-world failure cloning vanilla `Gun_AssaultRifle`
    /// from `Core/Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml`. The real file
    /// has a leading UTF-8 BOM and CRLF line endings, a `ParentName` + `Name`
    /// attribute pair, nested object/list fields, and a following def sharing the
    /// same `ThingDef` element name - reproduced here byte-for-byte (BOM and CRLF
    /// included) since the actual bug was quick-xml silently excluding the BOM's
    /// bytes from every reported position, desyncing every node span from
    /// `doc.source` by the BOM's byte length. See `parser::bom_tests` for a direct
    /// test of that span-offset fix.
    #[test]
    fn extracts_a_realistic_vanilla_def_with_bom_and_crlf_line_endings() {
        let raw = "\u{feff}<Defs>\r\n  <ThingDef ParentName=\"BaseBullet\">\r\n    <defName>Bullet_LMG</defName>\r\n    <label>LMG bullet</label>\r\n  </ThingDef>\r\n  <ThingDef ParentName=\"BaseHumanMakeableGun\" Name=\"Gun_AssaultRifle\">\r\n    <defName>Gun_AssaultRifle</defName>\r\n    <label>assault rifle</label>\r\n    <statBases>\r\n      <WorkToMake>40000</WorkToMake>\r\n    </statBases>\r\n    <verbs>\r\n      <li>\r\n        <defaultProjectile>Bullet_AssaultRifle</defaultProjectile>\r\n      </li>\r\n    </verbs>\r\n    <weaponTags>\r\n      <li>IndustrialGunAdvanced</li>\r\n      <li>AssaultRifle</li>\r\n    </weaponTags>\r\n  </ThingDef>\r\n  <ThingDef ParentName=\"BaseBullet\">\r\n    <defName>Bullet_AssaultRifle</defName>\r\n    <label>assault rifle bullet</label>\r\n  </ThingDef>\r\n</Defs>";

        let doc = parse_to_document("Core/Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml", raw);
        let target = doc
            .def_summaries
            .iter()
            .find(|d| d.def_name.as_deref() == Some("Gun_AssaultRifle"))
            .unwrap();

        let extracted = extract_indexed_def_xml(
            "Core/Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml",
            raw,
            "ThingDef",
            "Gun_AssaultRifle",
            Some(target.node_id),
        )
        .unwrap();

        assert_eq!(extracted.def_type, "ThingDef");
        assert!(extracted.xml.starts_with(
            "<ThingDef ParentName=\"BaseHumanMakeableGun\" Name=\"Gun_AssaultRifle\">"
        ));
        assert!(extracted.xml.trim_end().ends_with("</ThingDef>"));
        assert!(extracted
            .xml
            .contains("<defName>Gun_AssaultRifle</defName>"));
        assert!(extracted.xml.contains("<label>assault rifle</label>"));
        assert!(!extracted.xml.contains("Bullet_LMG"));
        assert!(!extracted
            .xml
            .contains("<defName>Bullet_AssaultRifle</defName>"));
    }
}

#[cfg(test)]
mod set_or_insert_def_name_tests {
    use super::*;

    #[test]
    fn replaces_an_existing_def_name_preserving_everything_else() {
        let template = "<ThingDef ParentName=\"BaseWeapon\">\n  <!-- a comment -->\n  <defName>Gun_Autopistol</defName>\n  <label>autopistol</label>\n  <tradeTags>\n    <li>Weapon</li>\n  </tradeTags>\n</ThingDef>";

        let result = set_or_insert_def_name("ThingDef", template, "Gun_MyPistol").unwrap();

        assert!(result.contains("<defName>Gun_MyPistol</defName>"));
        assert!(!result.contains("Gun_Autopistol"));
        assert!(result.contains("ParentName=\"BaseWeapon\""));
        assert!(result.contains("<!-- a comment -->"));
        assert!(result.contains("<label>autopistol</label>"));
        assert!(result.contains("<li>Weapon</li>"));
    }

    #[test]
    fn inserts_a_def_name_when_the_template_has_none() {
        // Templates are saved from arbitrary selected Defs, so a template
        // extracted from an abstract/template-only Def may have no <defName>.
        let template = "<ThingDef Name=\"BaseWeaponAbstract\" Abstract=\"True\">\n  <label>weapon</label>\n</ThingDef>";

        let result = set_or_insert_def_name("ThingDef", template, "Gun_NewWeapon").unwrap();

        assert!(result.contains("<defName>Gun_NewWeapon</defName>"));
        assert!(result.contains("Name=\"BaseWeaponAbstract\""));
        assert!(result.contains("<label>weapon</label>"));
    }

    #[test]
    fn rejects_a_def_type_mismatch() {
        let template = "<PawnKindDef>\n  <defName>Colonist</defName>\n</PawnKindDef>";
        let err = set_or_insert_def_name("ThingDef", template, "NewName").unwrap_err();
        assert_eq!(err.code, "create_def_clone_invalid_def_block");
    }

    #[test]
    fn rejects_template_xml_that_is_not_a_single_def() {
        // A corrupted/legacy stored template could contain more than one Def
        // under a `<Defs>` wrapper; extract_def_summaries expands that into
        // multiple candidates instead of treating it as a single root element.
        let template =
            "<Defs><ThingDef><defName>A</defName></ThingDef><ThingDef><defName>B</defName></ThingDef></Defs>";
        let err = set_or_insert_def_name("ThingDef", template, "NewName").unwrap_err();
        assert_eq!(err.code, "create_def_clone_invalid_def_block");
    }

    #[test]
    fn rejects_fatally_broken_template_xml() {
        let template = "<ThingDef><defName>Broken</ThingDef>";
        let err = set_or_insert_def_name("ThingDef", template, "NewName").unwrap_err();
        assert_eq!(err.code, "create_def_clone_invalid_def_block");
    }

    #[test]
    fn rejects_a_defs_wrapped_single_child_template() {
        // A `<Defs>` wrapper around exactly one Def also produces exactly one
        // DefSummary (unlike the two-child case above), but the matched node_id
        // then points at the *inner* Def while serialization would still emit
        // the wrapper - splicing that in would nest a `<Defs>` inside the
        // target's own `<Defs>`. This must be rejected, not silently accepted.
        let template = "<Defs><ThingDef><defName>A</defName></ThingDef></Defs>";
        let err = set_or_insert_def_name("ThingDef", template, "NewName").unwrap_err();
        assert_eq!(err.code, "create_def_clone_invalid_def_block");
    }
}

#[cfg(test)]
mod full_pipeline_tests {
    use super::*;

    /// Composes the same pure steps `create_def_from_user_template` chains together
    /// (`set_or_insert_def_name` -> `expand_self_closing_defs` -> `detect_child_indent` ->
    /// `insert_def_block`), without an `AppHandle`, to prove the whole splice - not just
    /// each piece in isolation - preserves unknown fields, attributes, comments, and a
    /// nested object-list (`comps`-style, with a `Class`-discriminated `<li>`) once the
    /// template's Def block lands in a real target document alongside an existing Def.
    #[test]
    fn splices_a_user_template_into_a_target_doc_preserving_nested_object_list_xml() {
        let target =
            "<Defs>\n  <ThingDef>\n    <defName>Existing</defName>\n  </ThingDef>\n</Defs>";
        let template_xml = r#"<ThingDef ParentName="BaseWeapon">
  <!-- a helpful comment -->
  <defName>Gun_Autopistol</defName>
  <label>autopistol</label>
  <someUnknownField>42</someUnknownField>
  <comps>
    <li Class="CompProperties_Explosive">
      <explosiveRadius>3.9</explosiveRadius>
    </li>
  </comps>
</ThingDef>"#;

        let new_def_block =
            set_or_insert_def_name("ThingDef", template_xml, "Gun_MyPistol").unwrap();

        let expanded;
        let effective_xml: &str = if let Some(exp) = expand_self_closing_defs(target) {
            expanded = exp;
            &expanded
        } else {
            target
        };
        let child_indent = detect_child_indent(effective_xml);
        let new_raw_xml = insert_def_block(effective_xml, &new_def_block, &child_indent).unwrap();

        let doc = parse_to_document("Defs/Weapons.xml", &new_raw_xml);
        assert!(!doc.had_fatal_parse_error);
        assert_eq!(doc.def_summaries.len(), 2);
        assert!(doc
            .def_summaries
            .iter()
            .any(|d| d.def_name.as_deref() == Some("Gun_MyPistol")));
        assert!(doc
            .def_summaries
            .iter()
            .any(|d| d.def_name.as_deref() == Some("Existing")));

        assert!(new_raw_xml.contains("<defName>Gun_MyPistol</defName>"));
        assert!(!new_raw_xml.contains("Gun_Autopistol"));
        assert!(new_raw_xml.contains(r#"ParentName="BaseWeapon""#));
        assert!(new_raw_xml.contains("<!-- a helpful comment -->"));
        assert!(new_raw_xml.contains("<someUnknownField>42</someUnknownField>"));
        assert!(new_raw_xml.contains(r#"<li Class="CompProperties_Explosive">"#));
        assert!(new_raw_xml.contains("<explosiveRadius>3.9</explosiveRadius>"));
    }

    /// Same shape as `splices_a_user_template_into_a_target_doc_preserving_nested_object_list_xml`
    /// above, but starting from `extract_indexed_def_xml` over a source file's raw XML instead of
    /// a saved template's XML - proving the indexed-def clone path preserves the same detail
    /// (attrs, comments, unknown fields, nested object-list) end to end.
    #[test]
    fn clones_an_indexed_def_into_a_target_doc_preserving_nested_object_list_xml() {
        let source_raw = r#"<Defs>
  <ThingDef ParentName="BaseWeapon">
    <!-- a helpful comment -->
    <defName>Gun_Autopistol</defName>
    <label>autopistol</label>
    <someUnknownField>42</someUnknownField>
    <comps>
      <li Class="CompProperties_Explosive">
        <explosiveRadius>3.9</explosiveRadius>
      </li>
    </comps>
  </ThingDef>
</Defs>"#;
        let target =
            "<Defs>\n  <ThingDef>\n    <defName>Existing</defName>\n  </ThingDef>\n</Defs>";

        let extracted = extract_indexed_def_xml(
            "Defs/Weapons.xml",
            source_raw,
            "ThingDef",
            "Gun_Autopistol",
            None,
        )
        .unwrap();
        let new_def_block =
            set_or_insert_def_name(&extracted.def_type, &extracted.xml, "Gun_MyPistol").unwrap();

        let expanded;
        let effective_xml: &str = if let Some(exp) = expand_self_closing_defs(target) {
            expanded = exp;
            &expanded
        } else {
            target
        };
        let child_indent = detect_child_indent(effective_xml);
        let new_raw_xml = insert_def_block(effective_xml, &new_def_block, &child_indent).unwrap();

        let doc = parse_to_document("Defs/Weapons.xml", &new_raw_xml);
        assert!(!doc.had_fatal_parse_error);
        assert_eq!(doc.def_summaries.len(), 2);
        assert!(doc
            .def_summaries
            .iter()
            .any(|d| d.def_name.as_deref() == Some("Gun_MyPistol")));
        assert!(doc
            .def_summaries
            .iter()
            .any(|d| d.def_name.as_deref() == Some("Existing")));

        assert!(new_raw_xml.contains("<defName>Gun_MyPistol</defName>"));
        assert!(!new_raw_xml.contains("Gun_Autopistol"));
        assert!(new_raw_xml.contains(r#"ParentName="BaseWeapon""#));
        assert!(new_raw_xml.contains("<!-- a helpful comment -->"));
        assert!(new_raw_xml.contains("<someUnknownField>42</someUnknownField>"));
        assert!(new_raw_xml.contains(r#"<li Class="CompProperties_Explosive">"#));
        assert!(new_raw_xml.contains("<explosiveRadius>3.9</explosiveRadius>"));
    }
}

#[cfg(test)]
mod duplicate_check_pipeline_tests {
    use super::*;
    use crate::project_model::{RegisteredLocation, SourceType};
    use std::path::Path;
    use time::OffsetDateTime;

    fn make_location(id: &str) -> RegisteredLocation {
        RegisteredLocation {
            id: id.to_string(),
            display_name: id.to_string(),
            root_path: Path::new("/tmp").join(id).to_string_lossy().to_string(),
            kind: LocationKind::Project,
            source_type: SourceType::Folder,
            read_only: false,
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

    /// Exercises `apply_replacement_overlay` followed by `reject_duplicate_def_name`
    /// together, in the same order `create_def_from_user_template` calls them (see the
    /// command body above). `reject_duplicate_def_name_tests` only covers the reject
    /// helper against a hand-built `DefIndex`, which would still pass if the command
    /// stopped applying the current-buffer overlay first - so this proves the overlay
    /// step itself contributes the candidate to the index the reject check runs
    /// against, not just the base (on-disk) index.
    #[test]
    fn rejects_a_def_name_that_only_exists_in_the_current_unsaved_buffer() {
        let settings = make_settings(vec![make_location("proj1")]);
        let base_index = DefIndex::default();
        let raw_xml = "<Defs><ThingDef><defName>Gun_Autopistol</defName></ThingDef></Defs>";

        let overlaid = apply_replacement_overlay(
            base_index,
            &settings,
            DefIndexReplacement {
                location_id: "proj1",
                relative_path: "Defs/Weapons.xml",
                source: raw_xml,
            },
        );

        let err = reject_duplicate_def_name(&overlaid, "ThingDef", "Gun_Autopistol").unwrap_err();
        assert_eq!(err.code, "create_def_clone_duplicate_def_name");
    }

    #[test]
    fn allows_a_def_name_absent_from_the_overlaid_buffer() {
        let settings = make_settings(vec![make_location("proj1")]);
        let base_index = DefIndex::default();
        let raw_xml = "<Defs><ThingDef><defName>Gun_Other</defName></ThingDef></Defs>";

        let overlaid = apply_replacement_overlay(
            base_index,
            &settings,
            DefIndexReplacement {
                location_id: "proj1",
                relative_path: "Defs/Weapons.xml",
                source: raw_xml,
            },
        );

        assert!(reject_duplicate_def_name(&overlaid, "ThingDef", "Gun_Autopistol").is_ok());
    }
}

#[cfg(test)]
mod require_non_blank_def_name_tests {
    use super::*;

    #[test]
    fn accepts_and_trims_a_valid_name() {
        assert_eq!(
            require_non_blank_def_name("  Gun_Autopistol  ").unwrap(),
            "Gun_Autopistol"
        );
    }

    #[test]
    fn rejects_an_empty_name() {
        let err = require_non_blank_def_name("").unwrap_err();
        assert_eq!(err.code, "create_def_clone_invalid_def_name");
    }

    #[test]
    fn rejects_a_whitespace_only_name() {
        let err = require_non_blank_def_name("   ").unwrap_err();
        assert_eq!(err.code, "create_def_clone_invalid_def_name");
    }
}

#[cfg(test)]
mod reject_duplicate_def_name_tests {
    use super::*;
    use crate::def_index::{DefIdentityKey, IndexedDef, IndexedDefSource, IndexedSourceKind};
    use crate::project_model::SourceType;

    fn project_def(def_type: &str, def_name: &str, read_only: bool) -> IndexedDef {
        IndexedDef {
            key: DefIdentityKey {
                def_type: def_type.to_string(),
                def_name: def_name.to_string(),
            },
            def_type: def_type.to_string(),
            def_name: def_name.to_string(),
            label: None,
            parent_name: None,
            relative_path: "Defs/Sample.xml".to_string(),
            node_id: None,
            line: None,
            column: None,
            source: IndexedDefSource {
                location_id: "proj1".to_string(),
                location_name: "proj1".to_string(),
                source_kind: IndexedSourceKind::Project,
                source_type: SourceType::Folder,
                read_only,
                mod_id: None,
                game_version: None,
                expansion_name: None,
            },
            fields: Vec::new(),
            def_name_lower: def_name.to_lowercase(),
            label_lower: String::new(),
        }
    }

    fn source_def(def_type: &str, def_name: &str) -> IndexedDef {
        let mut d = project_def(def_type, def_name, true);
        d.source.source_kind = IndexedSourceKind::Source;
        d
    }

    #[test]
    fn allows_a_name_with_no_existing_defs() {
        let index = DefIndex::default();
        assert!(reject_duplicate_def_name(&index, "ThingDef", "Gun_New").is_ok());
    }

    #[test]
    fn rejects_a_writable_project_duplicate() {
        let index = DefIndex {
            defs: vec![project_def("ThingDef", "Gun_Autopistol", false)],
            ..Default::default()
        };
        let err = reject_duplicate_def_name(&index, "ThingDef", "Gun_Autopistol").unwrap_err();
        assert_eq!(err.code, "create_def_clone_duplicate_def_name");
    }

    #[test]
    fn ignores_a_different_def_type_with_the_same_name() {
        let index = DefIndex {
            defs: vec![project_def("PawnKindDef", "Gun_Autopistol", false)],
            ..Default::default()
        };
        assert!(reject_duplicate_def_name(&index, "ThingDef", "Gun_Autopistol").is_ok());
    }

    #[test]
    fn ignores_a_read_only_project_def() {
        // Mirrors find_project_duplicates: a read-only project entry (e.g. a
        // base-game def surfaced under a project overlay) does not block reuse.
        let index = DefIndex {
            defs: vec![project_def("ThingDef", "Gun_Autopistol", true)],
            ..Default::default()
        };
        assert!(reject_duplicate_def_name(&index, "ThingDef", "Gun_Autopistol").is_ok());
    }

    #[test]
    fn ignores_a_source_only_def() {
        let index = DefIndex {
            defs: vec![source_def("ThingDef", "Gun_Autopistol")],
            ..Default::default()
        };
        assert!(reject_duplicate_def_name(&index, "ThingDef", "Gun_Autopistol").is_ok());
    }
}

#[cfg(test)]
mod reject_missing_indexed_source_tests {
    use super::*;
    use crate::def_index::{DefIdentityKey, IndexedDef, IndexedDefSource, IndexedSourceKind};
    use crate::project_model::SourceType;

    fn indexed_def(def_type: &str, def_name: &str, node_id: Option<usize>) -> IndexedDef {
        IndexedDef {
            key: DefIdentityKey {
                def_type: def_type.to_string(),
                def_name: def_name.to_string(),
            },
            def_type: def_type.to_string(),
            def_name: def_name.to_string(),
            label: None,
            parent_name: None,
            relative_path: "Defs/Weapons.xml".to_string(),
            node_id,
            line: None,
            column: None,
            source: IndexedDefSource {
                location_id: "core1".to_string(),
                location_name: "Core".to_string(),
                source_kind: IndexedSourceKind::Source,
                source_type: SourceType::BaseGame,
                read_only: true,
                mod_id: None,
                game_version: None,
                expansion_name: None,
            },
            fields: Vec::new(),
            def_name_lower: def_name.to_lowercase(),
            label_lower: String::new(),
        }
    }

    #[test]
    fn allows_a_def_that_is_still_indexed() {
        let index = DefIndex {
            defs: vec![indexed_def("ThingDef", "Gun_Autopistol", Some(3))],
            ..Default::default()
        };
        assert!(reject_missing_indexed_source(
            &index,
            "core1",
            "Defs/Weapons.xml",
            "ThingDef",
            "Gun_Autopistol",
            None,
        )
        .is_ok());
    }

    #[test]
    fn rejects_a_def_absent_from_the_index() {
        let index = DefIndex::default();
        let err = reject_missing_indexed_source(
            &index,
            "core1",
            "Defs/Weapons.xml",
            "ThingDef",
            "Gun_Autopistol",
            None,
        )
        .unwrap_err();
        assert_eq!(err.code, "create_def_from_indexed_def_source_not_found");
    }

    #[test]
    fn rejects_a_location_or_path_mismatch() {
        let index = DefIndex {
            defs: vec![indexed_def("ThingDef", "Gun_Autopistol", None)],
            ..Default::default()
        };
        assert!(reject_missing_indexed_source(
            &index,
            "core1",
            "Defs/OtherFile.xml",
            "ThingDef",
            "Gun_Autopistol",
            None,
        )
        .is_err());
    }

    #[test]
    fn uses_source_node_id_to_disambiguate_same_named_duplicates() {
        // Two defs share (type, name) in the same file, at different node ids -
        // the ambiguous case extract_indexed_def_xml itself must disambiguate.
        let index = DefIndex {
            defs: vec![
                indexed_def("ThingDef", "Dup", Some(1)),
                indexed_def("ThingDef", "Dup", Some(2)),
            ],
            ..Default::default()
        };

        assert!(reject_missing_indexed_source(
            &index,
            "core1",
            "Defs/Weapons.xml",
            "ThingDef",
            "Dup",
            Some(2),
        )
        .is_ok());

        let err = reject_missing_indexed_source(
            &index,
            "core1",
            "Defs/Weapons.xml",
            "ThingDef",
            "Dup",
            Some(99),
        )
        .unwrap_err();
        assert_eq!(err.code, "create_def_from_indexed_def_source_not_found");
    }

    #[test]
    fn without_a_node_id_any_matching_duplicate_satisfies_the_check() {
        let index = DefIndex {
            defs: vec![indexed_def("ThingDef", "Dup", Some(1))],
            ..Default::default()
        };
        assert!(reject_missing_indexed_source(
            &index,
            "core1",
            "Defs/Weapons.xml",
            "ThingDef",
            "Dup",
            None,
        )
        .is_ok());
    }
}

#[cfg(test)]
mod source_existence_pipeline_tests {
    use super::*;
    use crate::def_index::{DefIdentityKey, IndexedDef, IndexedDefSource, IndexedSourceKind};
    use crate::project_model::{RegisteredLocation, SourceType};
    use std::path::Path;
    use time::OffsetDateTime;

    fn make_location(id: &str) -> RegisteredLocation {
        RegisteredLocation {
            id: id.to_string(),
            display_name: id.to_string(),
            root_path: Path::new("/tmp").join(id).to_string_lossy().to_string(),
            kind: LocationKind::Project,
            source_type: SourceType::Folder,
            read_only: false,
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

    /// Reproduces a bug where, when the clone source lives in the
    /// same file as the target buffer being edited, checking existence against the
    /// *overlaid* index (which reflects the unsaved buffer) rather than the *base*
    /// (on-disk-reflecting) index disagrees with where `create_def_from_indexed_def`
    /// actually reads the clone source from (always disk, never the buffer). Here the
    /// buffer has since renamed the def away from the name the user searched for and
    /// selected, but the on-disk file (and thus the actual clone source) still has the
    /// original name - so the check must pass against `base_index`, matching what
    /// extraction will find, even though the overlay no longer has that name at all.
    #[test]
    fn a_same_file_clone_source_stays_valid_against_the_base_index_after_an_unsaved_rename() {
        let settings = make_settings(vec![make_location("proj1")]);
        let base_index = DefIndex {
            defs: vec![{
                let mut d = source_indexed_def("ThingDef", "Gun_Autopistol");
                d.source.location_id = "proj1".to_string();
                d.source.source_kind = IndexedSourceKind::Project;
                d.source.read_only = false;
                d
            }],
            ..Default::default()
        };
        // The buffer has renamed the def away from "Gun_Autopistol" since the index
        // was built, but the file has not been saved - disk (and thus the real clone
        // source) is unchanged.
        let unsaved_buffer =
            "<Defs><ThingDef><defName>Gun_Autopistol_Renamed</defName></ThingDef></Defs>";

        let overlaid = apply_replacement_overlay(
            base_index.clone(),
            &settings,
            DefIndexReplacement {
                location_id: "proj1",
                relative_path: "Defs/Weapons.xml",
                source: unsaved_buffer,
            },
        );

        // The bug: checking against the overlay would reject a still-valid clone.
        assert!(reject_missing_indexed_source(
            &overlaid,
            "proj1",
            "Defs/Weapons.xml",
            "ThingDef",
            "Gun_Autopistol",
            None,
        )
        .is_err());

        // The fix: checking against the base index accepts it, matching what
        // extraction will actually find when it reads the source file off disk.
        assert!(reject_missing_indexed_source(
            &base_index,
            "proj1",
            "Defs/Weapons.xml",
            "ThingDef",
            "Gun_Autopistol",
            None,
        )
        .is_ok());
    }

    fn source_indexed_def(def_type: &str, def_name: &str) -> IndexedDef {
        IndexedDef {
            key: DefIdentityKey {
                def_type: def_type.to_string(),
                def_name: def_name.to_string(),
            },
            def_type: def_type.to_string(),
            def_name: def_name.to_string(),
            label: None,
            parent_name: None,
            relative_path: "Defs/Weapons.xml".to_string(),
            node_id: Some(1),
            line: None,
            column: None,
            source: IndexedDefSource {
                location_id: "proj1".to_string(),
                location_name: "proj1".to_string(),
                source_kind: IndexedSourceKind::Source,
                source_type: SourceType::Folder,
                read_only: true,
                mod_id: None,
                game_version: None,
                expansion_name: None,
            },
            fields: Vec::new(),
            def_name_lower: def_name.to_lowercase(),
            label_lower: String::new(),
        }
    }
}
