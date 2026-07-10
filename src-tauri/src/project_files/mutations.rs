use super::{
    paths::{
        infer_file_kind, join_paths, relative_path_to_forward_slash,
        resolve_existing_path_within_root, resolve_parent_dir_for_create, resolve_project_root,
        validate_name_segment,
    },
    ProjectFileEntry, ProjectFileError, ProjectFolderEntry, ProjectPathMutationResult,
};
use crate::project_model::ProjectSettings;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathKind {
    File,
    Folder,
}

impl PathKind {
    fn parse(kind: &str) -> Result<Self, ProjectFileError> {
        match kind {
            "file" => Ok(Self::File),
            "folder" => Ok(Self::Folder),
            _ => Err(ProjectFileError::KindMismatch(format!(
                "unknown kind: {}",
                kind
            ))),
        }
    }

    fn validate(self, path: &Path, relative_path: &str) -> Result<(), ProjectFileError> {
        match self {
            Self::File if !path.is_file() => {
                Err(ProjectFileError::KindMismatch(relative_path.to_string()))
            }
            Self::Folder if !path.is_dir() => {
                Err(ProjectFileError::KindMismatch(relative_path.to_string()))
            }
            _ => Ok(()),
        }
    }
}

pub fn create_project_file(
    settings: &ProjectSettings,
    project_id: &str,
    parent_path: &str,
    file_name: &str,
    contents: Option<&str>,
) -> Result<ProjectFileEntry, ProjectFileError> {
    let root = resolve_project_root(settings, project_id)?;
    let parent_dir = resolve_parent_dir_for_create(&root, parent_path)?;
    validate_name_segment(file_name)?;

    let target = parent_dir.join(file_name);
    if target.exists() {
        let rel = join_paths(parent_path, file_name);
        return Err(ProjectFileError::PathAlreadyExists(rel));
    }

    let extension = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();

    let write_contents = contents.unwrap_or("");

    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot create file: {}", e)))?;

    if !write_contents.is_empty() {
        f.write_all(write_contents.as_bytes())
            .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot write file: {}", e)))?;
    }
    drop(f);

    let size_bytes = target.metadata().map(|m| m.len()).unwrap_or(0);
    let file_kind = infer_file_kind(&extension);
    let relative_path = join_paths(parent_path, file_name);
    let folder_path = parent_path.to_string();

    Ok(ProjectFileEntry {
        relative_path,
        folder_path,
        file_name: file_name.to_string(),
        extension,
        size_bytes,
        file_kind,
        active_for_game_version: None,
    })
}

pub fn create_project_folder(
    settings: &ProjectSettings,
    project_id: &str,
    parent_path: &str,
    folder_name: &str,
) -> Result<ProjectFolderEntry, ProjectFileError> {
    let root = resolve_project_root(settings, project_id)?;
    let parent_dir = resolve_parent_dir_for_create(&root, parent_path)?;
    validate_name_segment(folder_name)?;

    let target = parent_dir.join(folder_name);
    if target.exists() {
        let rel = join_paths(parent_path, folder_name);
        return Err(ProjectFileError::PathAlreadyExists(rel));
    }

    std::fs::create_dir(&target)
        .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot create directory: {}", e)))?;

    let relative_path = join_paths(parent_path, folder_name);

    Ok(ProjectFolderEntry {
        relative_path,
        folder_name: folder_name.to_string(),
        parent_path: parent_path.to_string(),
    })
}

pub fn rename_project_path(
    settings: &ProjectSettings,
    project_id: &str,
    relative_path: &str,
    new_name: &str,
    kind: &str,
) -> Result<ProjectPathMutationResult, ProjectFileError> {
    if relative_path.is_empty() {
        return Err(ProjectFileError::CannotModifyRoot);
    }
    let root = resolve_project_root(settings, project_id)?;
    let canonical_src = resolve_existing_path_within_root(&root, relative_path)?;
    let path_kind = PathKind::parse(kind)?;
    path_kind.validate(&canonical_src, relative_path)?;

    validate_name_segment(new_name)?;

    let parent = canonical_src
        .parent()
        .ok_or(ProjectFileError::FileOutsideRoot)?;
    let target = parent.join(new_name);

    let is_case_only = target.exists()
        && target
            .canonicalize()
            .map(|c| c == canonical_src)
            .unwrap_or(false);

    if is_case_only {
        rename_case_only(&canonical_src, &target, parent)?;
    } else if target.exists() {
        let parent_rel = parent
            .strip_prefix(&root)
            .map(relative_path_to_forward_slash)
            .unwrap_or_default();
        return Err(ProjectFileError::PathAlreadyExists(join_paths(
            &parent_rel,
            new_name,
        )));
    } else {
        std::fs::rename(&canonical_src, &target)
            .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot rename: {}", e)))?;
    }

    let parent_rel = parent
        .strip_prefix(&root)
        .map(relative_path_to_forward_slash)
        .unwrap_or_default();
    let new_path = join_paths(&parent_rel, new_name);

    Ok(ProjectPathMutationResult {
        old_path: relative_path.to_string(),
        new_path,
    })
}

fn rename_case_only(
    canonical_src: &Path,
    target: &Path,
    parent: &Path,
) -> Result<(), ProjectFileError> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let temp = parent.join(format!(".rimedit_tmp_{}", nanos));
    std::fs::rename(canonical_src, &temp)
        .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot rename (step 1): {}", e)))?;
    if let Err(e) = std::fs::rename(&temp, target) {
        let _ = std::fs::rename(&temp, canonical_src);
        return Err(ProjectFileError::ScanFailed(format!(
            "Cannot rename (step 2): {}",
            e
        )));
    }
    Ok(())
}

pub fn delete_project_path(
    settings: &ProjectSettings,
    project_id: &str,
    relative_path: &str,
    kind: &str,
) -> Result<ProjectPathMutationResult, ProjectFileError> {
    if relative_path.is_empty() {
        return Err(ProjectFileError::CannotModifyRoot);
    }
    let root = resolve_project_root(settings, project_id)?;
    let canonical_src = resolve_existing_path_within_root(&root, relative_path)?;
    let path_kind = PathKind::parse(kind)?;
    path_kind.validate(&canonical_src, relative_path)?;

    match path_kind {
        PathKind::File => std::fs::remove_file(&canonical_src)
            .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot delete file: {}", e)))?,
        PathKind::Folder => std::fs::remove_dir_all(&canonical_src)
            .map_err(|e| ProjectFileError::ScanFailed(format!("Cannot delete folder: {}", e)))?,
    }

    Ok(ProjectPathMutationResult {
        old_path: relative_path.to_string(),
        new_path: String::new(),
    })
}
