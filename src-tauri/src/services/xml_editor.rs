use crate::project_files::{read_xml_file, validate_and_resolve_location};
use crate::project_model::AppError;
use crate::services::validation;
use crate::settings_store::load_settings;
use crate::xml_document::{
    apply_xml_edit, build_editor_view, parse_to_document, serialize_xml_document, XmlDocument,
    XmlEdit, XmlEditContext, XmlEditorDocumentLoadResult,
};
use tauri::AppHandle;

pub(crate) fn read_editor_document(
    app: &AppHandle,
    project_id: String,
    relative_path: String,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let settings = load_settings(app)?;
    let content = read_xml_file(&settings, &project_id, &relative_path).map_err(AppError::from)?;
    let mut doc = parse_to_document(&relative_path, &content.contents);
    if !doc.had_fatal_parse_error {
        validation::validate_doc_for_project(
            app,
            &settings,
            &project_id,
            &relative_path,
            &mut doc,
        )?;
    }
    Ok(build_editor_result(project_id, doc, content.contents))
}

pub(crate) fn read_location_editor_document(
    app: &AppHandle,
    project_id: String,
    location_id: String,
    relative_path: String,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let settings = load_settings(app)?;
    // Require project_id to be a known registered location so callers cannot
    // probe arbitrary paths or load with no validation context.
    if !settings.locations.iter().any(|l| l.id == project_id) {
        return Err(AppError {
            code: "project_not_found".to_string(),
            message: format!("No registered project with id '{}'.", project_id),
            details: None,
            args: crate::diagnostics::diagnostic_args([("projectId", project_id.into())]),
        });
    }
    let canonical = validate_and_resolve_location(&settings, &location_id, &relative_path)
        .map_err(AppError::from)?;
    let contents = std::fs::read_to_string(&canonical).map_err(|e| AppError {
        code: "file_read_failed".to_string(),
        message: e.to_string(),
        details: None,
        args: crate::diagnostics::diagnostic_args([(
            "path",
            canonical.to_string_lossy().into_owned().into(),
        )]),
    })?;
    let mut doc = parse_to_document(&relative_path, &contents);
    if !doc.had_fatal_parse_error {
        // Use source validation (no replacement overlay) so the source file is
        // not inserted as a project entry, avoiding false duplicate diagnostics.
        validation::validate_doc_for_source(app, &settings, &project_id, &location_id, &mut doc)?;
    }
    Ok(build_editor_result(project_id, doc, contents))
}

pub(crate) fn parse_editor_buffer(
    app: &AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let settings = load_settings(app)?;
    let mut doc = parse_to_document(&relative_path, &raw_xml);
    if !doc.had_fatal_parse_error {
        validation::validate_doc_for_project(
            app,
            &settings,
            &project_id,
            &relative_path,
            &mut doc,
        )?;
    }
    Ok(build_editor_result(project_id, doc, raw_xml))
}

pub(crate) fn apply_editor_edits(
    app: &AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
    edits: Vec<XmlEdit>,
    edit_context: Option<XmlEditContext>,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let settings = load_settings(app)?;
    let mut doc = parse_to_document(&relative_path, &raw_xml);
    if !doc.had_fatal_parse_error {
        validation::validate_doc_for_project(
            app,
            &settings,
            &project_id,
            &relative_path,
            &mut doc,
        )?;
    }
    if doc.had_fatal_parse_error {
        return Ok(build_editor_result(project_id, doc, raw_xml));
    }
    let context = edit_context.unwrap_or_default();
    for edit in edits {
        apply_xml_edit(&mut doc, edit, &context).map_err(|e| AppError {
            code: "xml_edit_failed".to_string(),
            message: e.to_string(),
            details: None,
            args: crate::diagnostics::DiagnosticArgs::new(),
        })?;
    }
    let new_xml = serialize_xml_document(&doc);
    // Re-parse so returned node IDs are aligned with the new raw XML.
    let mut fresh_doc = parse_to_document(&relative_path, &new_xml);
    if !fresh_doc.had_fatal_parse_error {
        validation::validate_doc_for_project(
            app,
            &settings,
            &project_id,
            &relative_path,
            &mut fresh_doc,
        )?;
    }
    Ok(build_editor_result(project_id, fresh_doc, new_xml))
}

pub(crate) fn build_editor_result(
    project_id: String,
    doc: XmlDocument,
    raw_xml: String,
) -> XmlEditorDocumentLoadResult {
    let document_view = if doc.had_fatal_parse_error {
        None
    } else {
        Some(build_editor_view(&doc))
    };
    XmlEditorDocumentLoadResult {
        project_id,
        relative_path: doc.relative_path,
        raw_xml,
        document: document_view,
        parse_diagnostics: doc.parse_diagnostics,
        validation_diagnostics: doc.validation_diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_doc_local(doc: &mut XmlDocument) {
        use crate::xml_document::{validate_document, ValidationContext};
        let catalog_result = crate::schema_pack::build_schema_catalog(&[], None);
        let def_index = crate::def_index::DefIndex::default();
        let context = ValidationContext {
            catalog: &catalog_result.catalog,
            def_index: &def_index,
        };
        doc.validation_diagnostics = validate_document(doc, &context);
    }

    fn apply_editor_edits_without_project_validation(
        project_id: String,
        relative_path: String,
        raw_xml: String,
        edits: Vec<XmlEdit>,
        edit_context: Option<XmlEditContext>,
    ) -> Result<XmlEditorDocumentLoadResult, AppError> {
        let mut doc = parse_to_document(&relative_path, &raw_xml);
        if !doc.had_fatal_parse_error {
            validate_doc_local(&mut doc);
        }
        if doc.had_fatal_parse_error {
            return Ok(build_editor_result(project_id, doc, raw_xml));
        }
        let context = edit_context.unwrap_or_default();
        for edit in edits {
            apply_xml_edit(&mut doc, edit, &context).map_err(|e| AppError {
                code: "xml_edit_failed".to_string(),
                message: e.to_string(),
                details: None,
                args: crate::diagnostics::DiagnosticArgs::new(),
            })?;
        }
        let new_xml = serialize_xml_document(&doc);
        let mut fresh_doc = parse_to_document(&relative_path, &new_xml);
        if !fresh_doc.had_fatal_parse_error {
            validate_doc_local(&mut fresh_doc);
        }
        Ok(build_editor_result(project_id, fresh_doc, new_xml))
    }

    #[test]
    fn batch_xml_editor_edits_apply_in_one_parse_cycle_result() {
        let src = r#"<Defs>
  <ThingDef>
    <defName>Steel</defName>
    <label>steel</label>
  </ThingDef>
</Defs>"#;
        let doc = parse_to_document("test.xml", src);
        let def_id = doc.def_summaries[0].node_id;

        let result = apply_editor_edits_without_project_validation(
            "project".to_string(),
            "test.xml".to_string(),
            src.to_string(),
            vec![
                XmlEdit::SetChildElementText {
                    parent_node_id: def_id,
                    child_name: "label".to_string(),
                    value: "iron".to_string(),
                },
                XmlEdit::SetChildElementText {
                    parent_node_id: def_id,
                    child_name: "description".to_string(),
                    value: "A useful metal.".to_string(),
                },
            ],
            None,
        )
        .unwrap();

        assert!(result.raw_xml.contains("<label>iron</label>"));
        assert!(result
            .raw_xml
            .contains("<description>A useful metal.</description>"));
        assert!(result.document.is_some());
    }

    #[test]
    fn batch_xml_editor_edits_roll_back_on_invalid_edit() {
        let src = r#"<Defs>
  <ThingDef>
    <defName>Steel</defName>
  </ThingDef>
</Defs>"#;

        let result = apply_editor_edits_without_project_validation(
            "project".to_string(),
            "test.xml".to_string(),
            src.to_string(),
            vec![XmlEdit::SetChildElementText {
                parent_node_id: 9999,
                child_name: "label".to_string(),
                value: "iron".to_string(),
            }],
            None,
        );

        assert!(result.is_err());
    }
}
