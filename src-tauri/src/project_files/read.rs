use super::{
    paths::{
        canonicalize_location_root_path, resolve_existing_xml_file_within_root,
        resolve_project_root,
    },
    ProjectFileContent, ProjectFileError,
};
use crate::project_model::ProjectSettings;
use std::path::PathBuf;

pub fn validate_and_resolve(
    settings: &ProjectSettings,
    project_id: &str,
    relative_path: &str,
) -> Result<PathBuf, ProjectFileError> {
    let root = resolve_project_root(settings, project_id)?;
    resolve_existing_xml_file_within_root(&root, relative_path)
}

pub fn validate_and_resolve_location(
    settings: &ProjectSettings,
    location_id: &str,
    relative_path: &str,
) -> Result<PathBuf, ProjectFileError> {
    let location = settings
        .locations
        .iter()
        .find(|l| l.id == location_id)
        .ok_or_else(|| ProjectFileError::ProjectNotFound(location_id.to_string()))?;

    let root = canonicalize_location_root_path(&location.root_path)?;
    resolve_existing_xml_file_within_root(&root, relative_path)
}

pub fn read_xml_file(
    settings: &ProjectSettings,
    project_id: &str,
    relative_path: &str,
) -> Result<ProjectFileContent, ProjectFileError> {
    let canonical = validate_and_resolve(settings, project_id, relative_path)?;
    let contents = std::fs::read_to_string(&canonical)
        .map_err(|e| ProjectFileError::FileNotFound(format!("{}: {}", relative_path, e)))?;
    Ok(ProjectFileContent {
        project_id: project_id.to_string(),
        relative_path: relative_path.to_string(),
        contents,
    })
}
