use super::fs_utils::{is_symlink, resolve_manifest_relative_dir};
use crate::schema_pack::locale::{
    is_plausible_locale_tag, parse_schema_pack_locale_file, SchemaLocaleOverlay,
};
use crate::schema_pack::model::SchemaLoadDiagnostic;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Parse an already-embedded (built-in) or already-read (external) set of `(label, raw_json)`
/// locale sidecar files into a `{localeTag -> overlay}` map. `label` for a built-in file is its
/// embedded relative path (e.g. `"locales/en.json"`); for an external file it is the full
/// filesystem path. Either way, the locale tag is derived from the file stem (name without the
/// `.json` extension), lower-cased for map-key consistency.
pub(crate) fn parse_locale_bundle(
    pack_id: &str,
    files: &[(&str, &str)],
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> BTreeMap<String, SchemaLocaleOverlay> {
    let mut bundle: BTreeMap<String, SchemaLocaleOverlay> = BTreeMap::new();
    for (label, raw) in files {
        let tag = Path::new(label)
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();

        if !is_plausible_locale_tag(&tag) {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_locale_invalid_tag",
                    format!("Locale file name '{}' is not a valid locale tag.", label),
                )
                .with_pack_id(pack_id)
                .with_path(*label)
                .with_args(crate::diagnostics::diagnostic_args([(
                    "fileName",
                    (*label).into(),
                )])),
            );
            continue;
        }

        if bundle.contains_key(&tag) {
            diags.push(
                SchemaLoadDiagnostic::warning(
                    "schema_pack_locale_duplicate_tag",
                    format!(
                        "Locale tag '{}' is declared more than once in pack '{}'; the later file is ignored.",
                        tag, pack_id
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(*label)
                .with_args(crate::diagnostics::diagnostic_args([
                    ("locale", tag.as_str().into()),
                    ("packId", pack_id.into()),
                ])),
            );
            continue;
        }

        let (overlay_opt, file_diags) = parse_schema_pack_locale_file(label, pack_id, &tag, raw);
        diags.extend(file_diags);
        if let Some(overlay) = overlay_opt {
            bundle.insert(tag, overlay);
        }
    }
    bundle
}

/// Read every `<tag>.json` file directly inside a pack's declared `localesDirectory` (not
/// recursive -- locale sidecars are a flat `locales/<bcp47>.json` layout per `Plan.md`) into
/// `(path_label, raw_content)` pairs, applying the same path-escape/symlink/size safety limits
/// used for def/object/patch-operation directories. A declared-but-missing/non-directory
/// `localesDirectory`, or an absent one entirely, silently yields no files and no diagnostic --
/// unlike `defTypeDirectories`/`objectTypeDirectories`, this directory is optional and
/// forward-looking: a pack may legitimately ship zero translations today.
pub(super) fn read_locale_directory_files(
    manifest_dir: &Path,
    pack_id: &str,
    path_label: &str,
    locales_directory: Option<&str>,
    diags: &mut Vec<SchemaLoadDiagnostic>,
) -> Vec<(String, String)> {
    let mut files = Vec::new();

    let Some(dir_entry) = locales_directory else {
        return files;
    };

    let resolved = match resolve_manifest_relative_dir(manifest_dir, dir_entry) {
        Some(p) => p,
        None => {
            diags.push(
                SchemaLoadDiagnostic::error(
                    "schema_pack_locale_directory_escape",
                    format!(
                        "localesDirectory entry '{}' escapes the pack root - skipping.",
                        dir_entry
                    ),
                )
                .with_pack_id(pack_id)
                .with_path(path_label)
                .with_args(crate::diagnostics::diagnostic_args([(
                    "directoryEntry",
                    dir_entry.into(),
                )])),
            );
            return files;
        }
    };

    if is_symlink(&resolved) {
        diags.push(
            SchemaLoadDiagnostic::warning(
                "schema_pack_locale_directory_symlink_rejected",
                format!(
                    "localesDirectory '{}' is a symlink and was skipped.",
                    resolved.display()
                ),
            )
            .with_pack_id(pack_id)
            .with_path(resolved.to_string_lossy())
            .with_args(crate::diagnostics::diagnostic_args([(
                "directory",
                resolved.to_string_lossy().into_owned().into(),
            )])),
        );
        return files;
    }

    if !resolved.is_dir() {
        return files;
    }

    let mut json_paths: Vec<PathBuf> = match std::fs::read_dir(&resolved) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && !is_symlink(p)
                    && p.extension().and_then(|x| x.to_str()) == Some("json")
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    json_paths.sort();

    for json_path in json_paths {
        let file_label = json_path.to_string_lossy().to_string();

        if let Ok(meta) = std::fs::metadata(&json_path) {
            if meta.len() > super::MAX_LOCALE_FILE_BYTES {
                diags.push(
                    SchemaLoadDiagnostic::warning(
                        "schema_pack_locale_file_too_large",
                        format!(
                            "Locale file exceeds 256 KiB limit, skipping: {}",
                            file_label
                        ),
                    )
                    .with_pack_id(pack_id)
                    .with_path(&file_label),
                );
                continue;
            }
        }

        match std::fs::read_to_string(&json_path) {
            Ok(raw) => files.push((file_label, raw)),
            Err(e) => {
                diags.push(
                    SchemaLoadDiagnostic::error(
                        "schema_pack_locale_file_read_failed",
                        format!("Cannot read locale file: {}", e),
                    )
                    .with_pack_id(pack_id)
                    .with_path(&file_label),
                );
            }
        }
    }

    files
}
