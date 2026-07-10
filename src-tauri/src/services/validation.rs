use crate::def_index::{apply_replacement_overlay, DefIndexReplacement};
use crate::project_model::{AppError, ProjectSettings};
use crate::rimworld_load_folders::read_load_folders_version_keys;
use crate::schema_pack::build_schema_catalog;
use crate::services::def_index_cache;
use crate::xml_document::{
    validate_about_metadata_document, validate_document, ValidationContext, XmlDocument,
    XmlDocumentProfile,
};
use std::path::PathBuf;
use tauri::AppHandle;

fn find_location_root(settings: &ProjectSettings, location_id: &str) -> Option<PathBuf> {
    settings
        .locations
        .iter()
        .find(|l| l.id == location_id)
        .map(|l| PathBuf::from(&l.root_path))
}

pub(crate) fn validate_doc_for_project(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    relative_path: &str,
    doc: &mut XmlDocument,
) -> Result<(), AppError> {
    let _span = crate::instrumentation::span_with_tags(
        app,
        "validation.validateDocForProject",
        [("relativePath".to_string(), relative_path.to_string())],
    );

    if doc.profile == XmlDocumentProfile::About {
        let load_folders_versions = find_location_root(settings, project_id)
            .and_then(|root| read_load_folders_version_keys(&root));
        doc.validation_diagnostics =
            validate_about_metadata_document(doc, load_folders_versions.as_deref());
        return Ok(());
    }

    let catalog_result = build_schema_catalog(&[], None);
    let base_index = def_index_cache::load_for_project(app, settings, project_id, false)?;
    let def_index = apply_replacement_overlay(
        (*base_index).clone(),
        settings,
        DefIndexReplacement {
            location_id: project_id,
            relative_path,
            source: doc.source.as_str(),
        },
    );
    let context = ValidationContext {
        catalog: &catalog_result.catalog,
        def_index: &def_index,
    };
    doc.validation_diagnostics = validate_document(doc, &context);
    Ok(())
}

/// Validates a source location document using the project's index without
/// overlaying the document as a project entry. This avoids false duplicate
/// or source diagnostics that would occur if the source file were treated as
/// a project-owned file.
pub(crate) fn validate_doc_for_source(
    app: &AppHandle,
    settings: &ProjectSettings,
    project_id: &str,
    location_id: &str,
    doc: &mut XmlDocument,
) -> Result<(), AppError> {
    if doc.profile == XmlDocumentProfile::About {
        let load_folders_versions = find_location_root(settings, location_id)
            .and_then(|root| read_load_folders_version_keys(&root));
        doc.validation_diagnostics =
            validate_about_metadata_document(doc, load_folders_versions.as_deref());
        return Ok(());
    }

    let catalog_result = build_schema_catalog(&[], None);
    let base_index = def_index_cache::load_for_project(app, settings, project_id, false)?;
    let context = ValidationContext {
        catalog: &catalog_result.catalog,
        def_index: &base_index,
    };
    doc.validation_diagnostics = validate_document(doc, &context);
    Ok(())
}
