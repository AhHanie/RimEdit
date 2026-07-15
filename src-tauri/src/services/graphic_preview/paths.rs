use std::path::{Path, PathBuf};

use crate::project_model::AppError;

pub(super) const TEXTURE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "dds"];

/// Returns the canonical path of the usable `Textures` directory for a location, or `None`.
///
/// Checks `<location_root>/Textures` first. If the root looks like a RimWorld
/// version folder (e.g. `1.6`), also tries `<location_root>/../Textures` as a
/// fallback, ensuring the path stays within the parent directory boundary.
pub(super) fn verified_textures_root(location_root: &Path) -> Option<PathBuf> {
    let canonical_location = location_root.canonicalize().ok()?;

    let textures_path = canonical_location.join("Textures");
    if textures_path.exists() {
        let canonical_textures = textures_path.canonicalize().ok()?;
        if canonical_textures.strip_prefix(&canonical_location).is_ok() {
            return Some(canonical_textures);
        }
    }

    // Fallback for version-folder registrations like "1.6", "1.5", etc.
    let folder_name = canonical_location.file_name()?.to_str()?;
    if is_rimworld_version_folder(folder_name) {
        let canonical_parent = canonical_location.parent()?.canonicalize().ok()?;
        let fallback_textures = canonical_parent.join("Textures");
        if fallback_textures.exists() {
            let canonical_fallback = fallback_textures.canonicalize().ok()?;
            if canonical_fallback.strip_prefix(&canonical_parent).is_ok() {
                return Some(canonical_fallback);
            }
        }
    }

    None
}

pub(super) fn is_rimworld_version_folder(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() == 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Returns the canonical path of a texture file if it exists and resolves
/// within `canonical_textures_root`. Rejects individual symlink escapes.
pub(super) fn resolve_existing_texture_file(
    canonical_textures_root: &Path,
    relative_file: &str,
) -> Option<PathBuf> {
    let candidate =
        canonical_textures_root.join(relative_file.replace('/', std::path::MAIN_SEPARATOR_STR));
    if !candidate.exists() {
        return None;
    }
    let canonical_candidate = candidate.canonicalize().ok()?;
    if canonical_candidate
        .strip_prefix(canonical_textures_root)
        .is_ok()
    {
        Some(canonical_candidate)
    } else {
        None
    }
}

pub(super) fn normalize_texture_path(input: &str) -> Result<String, AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(invalid_path_error(
            "texture_path_empty",
            "Texture path must not be empty.",
        ));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(invalid_path_error(
            "texture_path_control_characters",
            "Texture path contains control characters.",
        ));
    }
    let normalized = trimmed.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err(invalid_path_error(
            "texture_path_not_relative",
            "Texture path must be relative to the Textures folder.",
        ));
    }
    // Reject Windows-style drive roots (e.g. C:/ or C:\)
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        return Err(invalid_path_error(
            "texture_path_not_relative",
            "Texture path must be relative to the Textures folder.",
        ));
    }
    // Strip leading "Textures/" prefix so callers can pass either form
    let without_prefix = normalized.strip_prefix("Textures/").unwrap_or(&normalized);
    for component in without_prefix.split('/') {
        match component {
            ".." => {
                return Err(invalid_path_error(
                    "texture_path_parent_dir_component",
                    "Texture path must not contain '..'.",
                ))
            }
            "." => {
                return Err(invalid_path_error(
                    "texture_path_current_dir_component",
                    "Texture path must not contain '.' components.",
                ))
            }
            "" => {
                return Err(invalid_path_error(
                    "texture_path_empty_segment",
                    "Texture path must not contain empty path segments.",
                ))
            }
            _ => {}
        }
    }
    Ok(strip_texture_extension(without_prefix))
}

pub(super) fn invalid_path_error(code: &str, msg: &str) -> AppError {
    AppError {
        code: code.to_string(),
        message: msg.to_string(),
        details: None,
        args: crate::diagnostics::DiagnosticArgs::new(),
    }
}

pub(super) fn strip_texture_extension(path: &str) -> String {
    let lower = path.to_lowercase();
    for ext in TEXTURE_EXTENSIONS {
        let suffix = format!(".{}", ext);
        if lower.ends_with(&suffix) {
            return path[..path.len() - suffix.len()].to_string();
        }
    }
    path.to_string()
}

pub(super) fn texture_relative_file_candidates(normalized_tex_path: &str) -> Vec<String> {
    TEXTURE_EXTENSIONS
        .iter()
        .map(|ext| format!("{}.{}", normalized_tex_path, ext))
        .collect()
}
