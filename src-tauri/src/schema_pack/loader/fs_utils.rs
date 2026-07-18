use std::path::{Component, Path, PathBuf};

pub(crate) fn discover_manifest_paths_in_root(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // <root>/schema-pack.json
    let direct = root.join("schema-pack.json");
    if direct.is_file() && !is_symlink(&direct) {
        paths.push(direct);
    }

    // <root>/About/schema-pack.json
    let about = root.join("About").join("schema-pack.json");
    if about.is_file() && !is_symlink(&about) {
        paths.push(about);
    }

    // <root>/SchemaPacks/<name>/schema-pack.json
    let schema_packs_dir = root.join("SchemaPacks");
    if schema_packs_dir.is_dir() && !is_symlink(&schema_packs_dir) {
        if let Ok(entries) = std::fs::read_dir(&schema_packs_dir) {
            for entry in entries.flatten() {
                let sub = entry.path();
                if sub.is_dir() && !is_symlink(&sub) {
                    let candidate = sub.join("schema-pack.json");
                    if candidate.is_file() && !is_symlink(&candidate) {
                        paths.push(candidate);
                    }
                }
            }
        }
    }

    paths
}

pub(super) fn resolve_manifest_relative_dir(manifest_dir: &Path, entry: &str) -> Option<PathBuf> {
    // Reject absolute entries - they would replace manifest_dir entirely when joined.
    if Path::new(entry).is_absolute() {
        return None;
    }
    let candidate = manifest_dir.join(entry);
    // Reject paths that contain '..' after joining - they might escape the pack root.
    for component in candidate.components() {
        if component == Component::ParentDir {
            return None;
        }
    }
    // The lexical checks above only catch a literal '..' segment or an absolute entry. A path
    // component that is itself a symlink pointing outside `manifest_dir` (e.g. a manifest
    // declaring `"localesDirectory": "link/locales"` where `link` is a symlink to somewhere
    // outside the pack and `locales` is a real, non-symlink subdirectory reached through it)
    // passes both of those checks yet still reaches outside the pack root once the OS resolves
    // it -- and the caller's separate `is_symlink(&resolved)` check only inspects the *final*
    // path component, not this intermediate one. `std::fs::canonicalize` resolves every
    // component, including intermediate symlinks, so canonicalize both sides here and require
    // containment. Canonicalization failure (most commonly: the directory doesn't exist yet, the
    // common case for the optional `localesDirectory`) is not itself treated as an escape --
    // every caller already reports a dedicated "directory missing" diagnostic (or, for locales,
    // silently yields no files) once it finds `!resolved.is_dir()`, and no file is ever read from
    // a path that fails to canonicalize in the first place.
    if let (Ok(canonical_root), Ok(canonical_candidate)) =
        (manifest_dir.canonicalize(), candidate.canonicalize())
    {
        if !canonical_candidate.starts_with(&canonical_root) {
            return None;
        }
    }
    Some(candidate)
}

/// Walk `dir` recursively, collecting all `.json` files at any depth.
/// Symlinks are skipped at every level. Callers sort the result for determinism.
pub(crate) fn collect_json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_json_files_recursive(dir, &mut files);
    files
}

fn collect_json_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_symlink(&path) {
                continue;
            }
            if path.is_file() {
                if path.extension().and_then(|x| x.to_str()) == Some("json") {
                    files.push(path);
                }
            } else if path.is_dir() {
                collect_json_files_recursive(&path, files);
            }
        }
    }
}

pub(super) fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}
