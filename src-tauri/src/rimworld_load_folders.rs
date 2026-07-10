use crate::project_model::{parse_major_minor, ProjectSettings, RegisteredLocation, SourceType};

/// Parse a `LoadFolders.xml` version key to `(major, minor)`, tolerating:
/// - a leading `v` prefix (`v1.6`, `v1.6.1234`)
/// - build/revision components (`1.6.1234`, `1.6.0.55`)
///
/// Only the first two numeric components are used for comparison. Returns `None`
/// if the key cannot produce a valid major/minor pair (e.g., `"default"`, `"abc"`).
fn parse_key_to_major_minor(key: &str) -> Option<(u16, u16)> {
    let k = key.trim_start_matches('v');
    let mut parts = k.split('.');
    let major = parts.next()?.parse::<u16>().ok()?;
    let minor = parts.next()?.parse::<u16>().ok()?;
    Some((major, minor))
}
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResolvedLoadFolder {
    pub absolute_path: PathBuf,
    /// Relative folder path from the location root (e.g. `"1.6"`, `"Common"`, `""`).
    /// Reserved for project-explorer active/inactive file display.
    #[allow(dead_code)]
    pub relative_folder: String,
    /// Reserved for project-explorer active/inactive file display.
    #[allow(dead_code)]
    pub source: LoadFolderSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadFolderSource {
    BaseGame,
    LoadFoldersXml,
    VersionFolderExact,
    VersionFolderFallback,
    CommonFolder,
    Root,
}

#[derive(Debug, Clone)]
pub struct LoadFolderDiagnostic {
    #[allow(dead_code)]
    pub code: String,
    #[allow(dead_code)]
    pub message: String,
}

#[derive(Debug)]
pub struct LoadFolderResolution {
    pub root_path: PathBuf,
    pub selected_folders: Vec<ResolvedLoadFolder>,
    /// Reserved for future surfacing in project explorer / index error reporting.
    #[allow(dead_code)]
    pub diagnostics: Vec<LoadFolderDiagnostic>,
    pub shadow_by_relative_path: bool,
}

// ---------------------------------------------------------------------------
// Main resolver
// ---------------------------------------------------------------------------

/// Resolve which folders to scan for Def XML files for a given location.
///
/// Rules:
/// 1. `BaseGame` sources return peer content-pack folders under the Data root.
/// 2. If `LoadFolders.xml` exists at the location root, use it.
/// 3. Otherwise apply RimWorld's conventional version-folder fallback.
pub fn resolve_load_folders(
    location: &RegisteredLocation,
    settings: &ProjectSettings,
) -> LoadFolderResolution {
    let root = PathBuf::from(&location.root_path);
    let mut diagnostics = Vec::new();

    if location.source_type == SourceType::BaseGame {
        // The BaseGame root is the RimWorld `Data` folder, which contains one
        // subdirectory per content pack (Core, Royalty, Ideology, …). Each pack
        // has its own `Defs/` folder, so return every direct subdirectory as a
        // separate load folder rather than the root itself.
        let mut pack_folders: Vec<ResolvedLoadFolder> = std::fs::read_dir(&root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_dir() && path.join("Defs").is_dir() {
                    let rel = entry.file_name().to_string_lossy().to_string();
                    Some(ResolvedLoadFolder {
                        absolute_path: path,
                        relative_folder: rel,
                        source: LoadFolderSource::BaseGame,
                    })
                } else {
                    None
                }
            })
            .collect();
        pack_folders.sort_by(|a, b| a.relative_folder.cmp(&b.relative_folder));

        // Fall back to the root itself if there are no subdirectories (e.g. a
        // non-standard install or a test fixture pointing at a single pack).
        if pack_folders.is_empty() {
            pack_folders.push(ResolvedLoadFolder {
                absolute_path: root.clone(),
                relative_folder: String::new(),
                source: LoadFolderSource::BaseGame,
            });
        }

        return LoadFolderResolution {
            selected_folders: pack_folders,
            root_path: root,
            diagnostics,
            shadow_by_relative_path: false,
        };
    }

    let selected_version = &settings.game_version;
    let selected_ver = parse_major_minor(selected_version);

    let load_folders_xml = root.join("LoadFolders.xml");
    if load_folders_xml.exists() {
        let folders = resolve_from_load_folders_xml(
            &root,
            &load_folders_xml,
            selected_version,
            selected_ver,
            &mut diagnostics,
        );
        return LoadFolderResolution {
            root_path: root,
            selected_folders: folders,
            diagnostics,
            shadow_by_relative_path: true,
        };
    }

    // Conventional fallback: version folder, Common, root.
    let folders =
        resolve_conventional_fallback(&root, selected_version, selected_ver, &mut diagnostics);
    LoadFolderResolution {
        root_path: root,
        selected_folders: folders,
        diagnostics,
        shadow_by_relative_path: true,
    }
}

/// Reads the version block keys declared in `LoadFolders.xml` at a location root, for
/// the About.xml validator's non-blocking cross-check against `supportedVersions`.
/// Keys are already stripped of a leading `v` by `parse_load_folders_xml`. Non-version
/// keys (e.g. `default`) are filtered out. Returns `None` if the file doesn't exist or
/// can't be read/parsed -- the caller should simply skip the cross-check in that case.
pub fn read_load_folders_version_keys(root: &Path) -> Option<Vec<String>> {
    let xml_path = root.join("LoadFolders.xml");
    let raw = std::fs::read_to_string(xml_path).ok()?;
    let blocks = parse_load_folders_xml(&raw).ok()?;
    Some(
        blocks
            .into_iter()
            .filter(|(key, _)| parse_key_to_major_minor(key).is_some())
            .map(|(key, _)| key)
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// LoadFolders.xml resolution
// ---------------------------------------------------------------------------

/// Parse `LoadFolders.xml` and return all folders listed under the best-matching
/// version block.
///
/// Matching priority (mirrors RimWorld's `InitLoadFolders`):
/// 1. Exact `selected_version` key (e.g. `v1.6` or `1.6`).
/// 2. Highest defined version ≤ `selected_ver` (major.minor only).
/// 3. `default` key if present.
///
/// Conditional attributes (`IfModActive` etc.) are intentionally ignored.
fn resolve_from_load_folders_xml(
    root: &Path,
    xml_path: &Path,
    selected_version: &str,
    selected_ver: Option<(u16, u16)>,
    diagnostics: &mut Vec<LoadFolderDiagnostic>,
) -> Vec<ResolvedLoadFolder> {
    let raw = match std::fs::read_to_string(xml_path) {
        Ok(s) => s,
        Err(e) => {
            diagnostics.push(LoadFolderDiagnostic {
                code: "load_folders_read_failed".to_string(),
                message: format!("Cannot read LoadFolders.xml: {}", e),
            });
            return vec![];
        }
    };

    let blocks = match parse_load_folders_xml(&raw) {
        Ok(b) => b,
        Err(e) => {
            diagnostics.push(LoadFolderDiagnostic {
                code: "load_folders_parse_failed".to_string(),
                message: format!("Cannot parse LoadFolders.xml: {}", e),
            });
            return vec![];
        }
    };

    // Find the best-matching block key.
    let chosen_entries = choose_load_folder_block(&blocks, selected_version, selected_ver);

    if chosen_entries.is_none() {
        diagnostics.push(LoadFolderDiagnostic {
            code: "load_folders_no_matching_block".to_string(),
            message: format!(
                "LoadFolders.xml has no block matching version {}",
                selected_version
            ),
        });
        return vec![];
    }

    let entries = chosen_entries.unwrap();
    let mut result = Vec::new();
    // RimWorld adds folders in reverse list order (foldersToLoadDescendingOrder), so the
    // last-listed entry has the highest precedence. Iterate in reverse here so that the
    // first element of `result` is the highest-priority folder (first-wins scanning).
    for entry in entries.iter().rev() {
        let rel = normalize_folder_path(entry);
        let abs = if rel.is_empty() {
            root.to_path_buf()
        } else {
            root.join(&rel)
        };
        if !abs.exists() {
            diagnostics.push(LoadFolderDiagnostic {
                code: "load_folder_missing".to_string(),
                message: format!("LoadFolders.xml references missing folder: {}", rel),
            });
            continue;
        }
        result.push(ResolvedLoadFolder {
            absolute_path: abs,
            relative_folder: rel,
            source: LoadFolderSource::LoadFoldersXml,
        });
    }
    result
}

/// Choose the best block from parsed `LoadFolders.xml` blocks.
///
/// Keys are matched using `parse_key_to_major_minor`, so `v1.6`, `1.6`, and
/// `1.6.1234` all match selected version `1.6`.
///
/// Priority (mirrors RimWorld's `InitLoadFolders`):
/// 1. Exact major/minor match.
/// 2. Highest defined major/minor ≤ selected (ignoring `default` and unparseable keys).
/// 3. `default` block.
fn choose_load_folder_block<'a>(
    blocks: &'a [(String, Vec<String>)],
    _selected_version: &str,
    selected_ver: Option<(u16, u16)>,
) -> Option<&'a Vec<String>> {
    let sel = selected_ver?; // if the project version doesn't parse, no match is possible

    // 1. Exact match: key normalizes to the same major/minor as selected.
    let exact = blocks
        .iter()
        .find(|(key, _)| parse_key_to_major_minor(key) == Some(sel));
    if let Some((_, entries)) = exact {
        return Some(entries);
    }

    // 2. Highest version ≤ selected (ignoring non-versioned keys).
    let mut best: Option<((u16, u16), &Vec<String>)> = None;
    for (key, entries) in blocks {
        if key == "default" || key.is_empty() {
            continue;
        }
        if let Some(ver) = parse_key_to_major_minor(key) {
            if ver <= sel && (best.is_none() || ver > best.as_ref().unwrap().0) {
                best = Some((ver, entries));
            }
        }
    }
    if let Some((_, entries)) = best {
        return Some(entries);
    }

    // 3. `default` block.
    blocks
        .iter()
        .find(|(key, _)| key == "default")
        .map(|(_, entries)| entries)
}

// ---------------------------------------------------------------------------
// Conventional fallback (no LoadFolders.xml)
// ---------------------------------------------------------------------------

fn resolve_conventional_fallback(
    root: &Path,
    selected_version: &str,
    selected_ver: Option<(u16, u16)>,
    diagnostics: &mut Vec<LoadFolderDiagnostic>,
) -> Vec<ResolvedLoadFolder> {
    let mut result = Vec::new();

    // 1. Exact version folder.
    let exact_dir = root.join(selected_version);
    if exact_dir.exists() && exact_dir.is_dir() {
        result.push(ResolvedLoadFolder {
            absolute_path: exact_dir,
            relative_folder: selected_version.to_string(),
            source: LoadFolderSource::VersionFolderExact,
        });
    } else if let Some(sel) = selected_ver {
        // 2. Highest parseable version folder ≤ selected.
        let best = find_best_version_folder(root, sel);
        if let Some((rel, abs)) = best {
            result.push(ResolvedLoadFolder {
                absolute_path: abs,
                relative_folder: rel,
                source: LoadFolderSource::VersionFolderFallback,
            });
        }
    }

    // 3. `Common` folder if present.
    let common = root.join("Common");
    if common.exists() && common.is_dir() {
        result.push(ResolvedLoadFolder {
            absolute_path: common,
            relative_folder: "Common".to_string(),
            source: LoadFolderSource::CommonFolder,
        });
    }

    // 4. Root folder.
    //
    // Include the root unless an exact version folder was found, or unless there
    // are no version-style folders at all (i.e., root-only mod structure).
    // For simplicity, always include root - if there are version sub-folders the
    // root typically has nothing in Defs/, but this matches RimWorld's behavior.
    result.push(ResolvedLoadFolder {
        absolute_path: root.to_path_buf(),
        relative_folder: String::new(),
        source: LoadFolderSource::Root,
    });

    if result.is_empty() {
        diagnostics.push(LoadFolderDiagnostic {
            code: "load_folder_no_folders_resolved".to_string(),
            message: format!("No load folders resolved for version {}", selected_version),
        });
    }

    result
}

/// Find the highest directory under `root` whose name parses as a version ≤ `selected`.
fn find_best_version_folder(root: &Path, selected: (u16, u16)) -> Option<(String, PathBuf)> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut best: Option<((u16, u16), String, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(ver) = parse_major_minor(&name) {
            if ver <= selected && (best.is_none() || ver > best.as_ref().unwrap().0) {
                best = Some((ver, name, path));
            }
        }
    }
    best.map(|(_, name, path)| (name, path))
}

// ---------------------------------------------------------------------------
// XML parsing for LoadFolders.xml
// ---------------------------------------------------------------------------

/// Parse `LoadFolders.xml` into a list of `(version_key, [folder_path])` pairs.
///
/// Example:
/// ```xml
/// <loadFolders>
///   <v1.6>
///     <li>1.6</li>
///     <li IfModActive="...">1.6/Compat</li>
///   </v1.6>
///   <default>
///     <li>1.5</li>
///   </default>
/// </loadFolders>
/// ```
fn parse_load_folders_xml(raw: &str) -> Result<Vec<(String, Vec<String>)>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(raw);
    reader.config_mut().trim_text(true);

    let mut result: Vec<(String, Vec<String>)> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_entries: Vec<String> = Vec::new();
    let mut in_li = false;
    let mut li_text = String::new();
    let mut depth = 0usize;

    loop {
        match reader.read_event() {
            Err(e) => return Err(e.to_string()),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                depth += 1;
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if depth == 2 {
                    // Direct child of <loadFolders> - a version block key.
                    // Strip leading `v` for normalisation but keep original for block key.
                    current_key = Some(name.trim_start_matches('v').to_string());
                    current_entries = Vec::new();
                } else if depth == 3 && name == "li" {
                    in_li = true;
                    li_text.clear();
                }
            }
            Ok(Event::End(_)) => {
                if depth == 3 && in_li {
                    current_entries.push(li_text.trim().to_string());
                    in_li = false;
                    li_text.clear();
                } else if depth == 2 {
                    if let Some(key) = current_key.take() {
                        result.push((key, std::mem::take(&mut current_entries)));
                    }
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Text(e)) if in_li => {
                li_text.push_str(e.unescape().unwrap_or_default().as_ref());
            }
            _ => {}
        }
    }

    Ok(result)
}

/// Normalize a folder path from `LoadFolders.xml` entry text.
///
/// - Replaces `\` with `/`.
/// - Strips leading `/` or `\`.
/// - Treats `/` and `\` (root path entries) as empty string (location root).
fn normalize_folder_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed == "/" || trimmed == "\\" {
        return String::new();
    }
    trimmed
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_blocks(pairs: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    #[test]
    fn choose_block_exact_match() {
        let blocks = make_blocks(&[("1.6", &["1.6/Defs"]), ("1.5", &["1.5/Defs"])]);
        let chosen = choose_load_folder_block(&blocks, "1.6", Some((1, 6)));
        assert_eq!(chosen.unwrap(), &vec!["1.6/Defs".to_string()]);
    }

    #[test]
    fn choose_block_strips_v_prefix() {
        // parse_load_folders_xml already strips 'v'; this ensures parse_key_to_major_minor
        // handles any residual 'v' if called with the raw element name.
        let blocks = make_blocks(&[("1.6", &["1.6"]), ("1.5", &["1.5"])]);
        let chosen = choose_load_folder_block(&blocks, "1.6", Some((1, 6)));
        assert_eq!(chosen.unwrap()[0], "1.6");
    }

    #[test]
    fn choose_block_build_number_key_matches() {
        // A key like v1.6.1234 (stored after v-strip as "1.6.1234") must match project "1.6".
        let blocks = make_blocks(&[("1.6.1234", &["1.6"]), ("1.5", &["1.5"])]);
        let chosen = choose_load_folder_block(&blocks, "1.6", Some((1, 6)));
        assert_eq!(chosen.unwrap()[0], "1.6");
    }

    #[test]
    fn choose_block_fallback_highest_le() {
        let blocks = make_blocks(&[("1.5", &["1.5"]), ("1.4", &["1.4"])]);
        let chosen = choose_load_folder_block(&blocks, "1.6", Some((1, 6)));
        assert_eq!(chosen.unwrap()[0], "1.5");
    }

    #[test]
    fn choose_block_fallback_uses_build_number_key() {
        let blocks = make_blocks(&[("1.5.3456", &["1.5"]), ("1.4", &["1.4"])]);
        let chosen = choose_load_folder_block(&blocks, "1.6", Some((1, 6)));
        assert_eq!(chosen.unwrap()[0], "1.5");
    }

    #[test]
    fn choose_block_default_fallback() {
        let blocks = make_blocks(&[("default", &["DefaultDefs"])]);
        let chosen = choose_load_folder_block(&blocks, "1.6", Some((1, 6)));
        assert_eq!(chosen.unwrap()[0], "DefaultDefs");
    }

    #[test]
    fn choose_block_no_match_returns_none() {
        let blocks = make_blocks(&[("1.7", &["1.7"])]);
        let chosen = choose_load_folder_block(&blocks, "1.6", Some((1, 6)));
        assert!(chosen.is_none());
    }

    #[test]
    fn parse_key_to_major_minor_basic() {
        assert_eq!(parse_key_to_major_minor("1.6"), Some((1, 6)));
        assert_eq!(parse_key_to_major_minor("v1.6"), Some((1, 6)));
        assert_eq!(parse_key_to_major_minor("1.6.1234"), Some((1, 6)));
        assert_eq!(parse_key_to_major_minor("v1.6.1234"), Some((1, 6)));
        assert_eq!(parse_key_to_major_minor("default"), None);
        assert_eq!(parse_key_to_major_minor("1"), None);
        assert_eq!(parse_key_to_major_minor(""), None);
    }

    #[test]
    fn parse_load_folders_xml_basic() {
        let xml = r#"<loadFolders><v1.6><li>1.6</li><li IfModActive="a">1.6/Biotech</li></v1.6><default><li>Defs</li></default></loadFolders>"#;
        let blocks = parse_load_folders_xml(xml).unwrap();
        assert_eq!(blocks.len(), 2);
        let (key, entries) = &blocks[0];
        assert_eq!(key, "1.6");
        assert_eq!(entries, &["1.6", "1.6/Biotech"]);
        let (key2, entries2) = &blocks[1];
        assert_eq!(key2, "default");
        assert_eq!(entries2, &["Defs"]);
    }

    #[test]
    fn parse_load_folders_xml_ignores_mod_conditions() {
        let xml = r#"<loadFolders><v1.6>
            <li>1.6</li>
            <li IfModActive="ludeon.rimworld.biotech">1.6/Biotech</li>
            <li IfModNotActive="some.mod">1.6/NoCompat</li>
        </v1.6></loadFolders>"#;
        let blocks = parse_load_folders_xml(xml).unwrap();
        assert_eq!(blocks[0].1.len(), 3);
    }

    #[test]
    fn normalize_folder_path_root_slash() {
        assert_eq!(normalize_folder_path("/"), "");
        assert_eq!(normalize_folder_path("\\"), "");
    }

    #[test]
    fn normalize_folder_path_backslash() {
        assert_eq!(normalize_folder_path("1.6\\Biotech"), "1.6/Biotech");
    }
}
