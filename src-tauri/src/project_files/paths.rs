use super::{ProjectFileError, ProjectFileKind};
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation};
use std::path::{Component, Path, PathBuf};

pub(crate) fn resolve_project_root(
    settings: &ProjectSettings,
    project_id: &str,
) -> Result<PathBuf, ProjectFileError> {
    let location = settings
        .locations
        .iter()
        .find(|l| l.id == project_id)
        .ok_or_else(|| ProjectFileError::ProjectNotFound(project_id.to_string()))?;

    if location.kind != LocationKind::Project || location.read_only {
        return Err(ProjectFileError::ProjectNotEditable(project_id.to_string()));
    }

    Path::new(&location.root_path).canonicalize().map_err(|e| {
        ProjectFileError::ScanFailed(format!("Cannot canonicalize project root: {}", e))
    })
}

pub(super) fn canonicalize_location_root(
    location: &RegisteredLocation,
) -> Result<PathBuf, ProjectFileError> {
    Path::new(&location.root_path).canonicalize().map_err(|e| {
        ProjectFileError::ScanFailed(format!(
            "Cannot canonicalize location root {}: {}",
            location.root_path, e
        ))
    })
}

pub(super) fn canonicalize_location_root_path(
    root_path: &str,
) -> Result<PathBuf, ProjectFileError> {
    Path::new(root_path).canonicalize().map_err(|e| {
        ProjectFileError::ScanFailed(format!("Cannot canonicalize location root: {}", e))
    })
}

pub(super) fn relative_path_to_forward_slash(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str().map(|s| s.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn folder_path_of(normalized: &str) -> String {
    match normalized.rfind('/') {
        Some(i) => normalized[..i].to_string(),
        None => String::new(),
    }
}

pub(super) fn join_paths(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", parent, name)
    }
}

pub(super) fn is_xml_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("xml"))
        .unwrap_or(false)
}

pub(super) fn infer_file_kind(ext: &str) -> ProjectFileKind {
    match ext.to_ascii_lowercase().as_str() {
        "xml" => ProjectFileKind::Xml,
        "txt" | "md" | "markdown" | "json" | "yaml" | "yml" | "csv" | "log" | "ini" | "cfg"
        | "patch" | "cs" | "toml" | "bat" | "sh" | "py" | "js" | "ts" | "lua" | "cpp" | "c"
        | "h" | "rs" | "html" | "css" => ProjectFileKind::Text,
        "png" | "jpg" | "jpeg" | "dds" | "webp" | "gif" | "bmp" | "tga" | "zip" | "rar" | "7z"
        | "dll" | "exe" | "wav" | "ogg" | "mp3" | "psd" | "ico" | "cur" => ProjectFileKind::Binary,
        _ => ProjectFileKind::Unknown,
    }
}

pub(super) fn validate_name_segment(name: &str) -> Result<(), ProjectFileError> {
    if name.is_empty() {
        return Err(ProjectFileError::InvalidFileName(
            "Name cannot be empty".to_string(),
        ));
    }
    if name == "." || name == ".." {
        return Err(ProjectFileError::InvalidFileName(format!(
            "'{}' is not a valid name",
            name
        )));
    }
    for c in name.chars() {
        if c == '/' || c == '\\' {
            return Err(ProjectFileError::InvalidFileName(
                "Name cannot contain path separators".to_string(),
            ));
        }
        if matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
            return Err(ProjectFileError::InvalidFileName(format!(
                "Name contains invalid character '{}'",
                c
            )));
        }
        if (c as u32) < 32 || c as u32 == 127 {
            return Err(ProjectFileError::InvalidFileName(
                "Name contains control characters".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_mutation_path_components(relative_path: &str) -> Result<(), ProjectFileError> {
    if relative_path.is_empty() {
        return Err(ProjectFileError::FileOutsideRoot);
    }
    let input = Path::new(relative_path);
    if input.is_absolute() {
        return Err(ProjectFileError::FileOutsideRoot);
    }
    for component in input.components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(ProjectFileError::FileOutsideRoot),
        }
    }
    Ok(())
}

pub(super) fn resolve_existing_path_within_root(
    root: &Path,
    relative_path: &str,
) -> Result<PathBuf, ProjectFileError> {
    validate_mutation_path_components(relative_path)?;
    let joined = root.join(Path::new(relative_path));
    let canonical = joined
        .canonicalize()
        .map_err(|_| ProjectFileError::FileNotFound(relative_path.to_string()))?;
    canonical
        .strip_prefix(root)
        .map_err(|_| ProjectFileError::FileOutsideRoot)?;
    Ok(canonical)
}

pub(super) fn resolve_parent_dir_for_create(
    root: &Path,
    parent_path: &str,
) -> Result<PathBuf, ProjectFileError> {
    if parent_path.is_empty() {
        return Ok(root.to_path_buf());
    }
    let canonical = resolve_existing_path_within_root(root, parent_path)?;
    if !canonical.is_dir() {
        return Err(ProjectFileError::FileNotFound(format!(
            "'{}' is not a directory",
            parent_path
        )));
    }
    Ok(canonical)
}

pub(super) fn resolve_existing_xml_file_within_root(
    root: &Path,
    relative_path: &str,
) -> Result<PathBuf, ProjectFileError> {
    if relative_path.is_empty() {
        return Err(ProjectFileError::FileOutsideRoot);
    }

    let input = Path::new(relative_path);

    if input.is_absolute() {
        return Err(ProjectFileError::FileOutsideRoot);
    }

    for component in input.components() {
        match component {
            Component::Normal(_) => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ProjectFileError::FileOutsideRoot);
            }
            Component::CurDir => {}
        }
    }

    let joined = root.join(input);
    let canonical = joined
        .canonicalize()
        .map_err(|_| ProjectFileError::FileNotFound(relative_path.to_string()))?;

    canonical
        .strip_prefix(root)
        .map_err(|_| ProjectFileError::FileOutsideRoot)?;

    if !canonical.is_file() {
        return Err(ProjectFileError::FileNotFound(relative_path.to_string()));
    }

    if !is_xml_extension(&canonical) {
        return Err(ProjectFileError::UnsupportedFile);
    }

    Ok(canonical)
}
