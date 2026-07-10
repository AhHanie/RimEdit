use std::{env, fs, path::Path};

fn main() {
    tauri_build::build();

    // tauri-plugin-dialog imports TaskDialogIndirect from comctl32.dll, which only exists
    // in Common Controls v6. The Tauri app binary gets an embedded manifest that activates
    // comctl32 v6 at process startup, but the test binary does not have this manifest. Without
    // it, Windows loads comctl32 v5, which lacks TaskDialogIndirect, and the test binary
    // crashes at load time with STATUS_ENTRYPOINT_NOT_FOUND before any test runs.
    //
    // Fix: delay-load comctl32 so the import is resolved on first call rather than at startup.
    // Unit tests never call dialog functions, so the missing v6 entrypoint is never reached.
    // The same flag on the app binary is harmless: the activation context from the manifest
    // ensures comctl32 v6 is selected when the delay-load triggers.
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-arg=/DELAYLOAD:comctl32.dll");
        println!("cargo:rustc-link-arg=Delayimp.lib");
        println!("cargo:rustc-link-arg-tests=/DELAYLOAD:comctl32.dll");
        println!("cargo:rustc-link-arg-tests=Delayimp.lib");
    }

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let schema_packs_root = Path::new(&manifest_dir).join("schema-packs");

    // Discover all schema-pack.json manifests under schema-packs/.
    // Supports both flat layout (schema-packs/<name>/schema-pack.json)
    // and versioned layout (schema-packs/<name>/<version>/schema-pack.json).
    let manifests = discover_manifests(&schema_packs_root);

    // Emit rerun-if-changed for the entire schema-packs tree so cargo knows to
    // rebuild when any file changes.
    println!("cargo:rerun-if-changed={}", schema_packs_root.display());

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("built_in_schema_packs.rs");

    // Generate BUILT_IN_SCHEMA_PACKS constant.
    // Each element: (manifest_label, manifest_content, def_files, obj_files, patch_op_files)
    // where def_files, obj_files, and patch_op_files are slices of (label, content) pairs.
    let mut out = String::new();
    out.push_str(
        "#[allow(clippy::type_complexity)]\npub const BUILT_IN_SCHEMA_PACKS: &[(&str, &str, &[(&str, &str)], &[(&str, &str)], &[(&str, &str)])] = &[\n",
    );

    for pack in &manifests {
        // manifest
        out.push_str("    (\n");
        out.push_str(&format!("        \"{}\",\n", pack.label.replace('\\', "/")));
        out.push_str(&format!(
            "        include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{rel}\")),\n",
            rel = pack.manifest_rel.replace('\\', "/")
        ));

        // def files
        out.push_str("        &[\n");
        for (label, rel) in &pack.def_files {
            out.push_str(&format!(
                "            (\"{label}\", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{rel}\"))),\n",
                label = label.replace('\\', "/"),
                rel = rel.replace('\\', "/")
            ));
        }
        out.push_str("        ],\n");

        // object files
        out.push_str("        &[\n");
        for (label, rel) in &pack.obj_files {
            out.push_str(&format!(
                "            (\"{label}\", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{rel}\"))),\n",
                label = label.replace('\\', "/"),
                rel = rel.replace('\\', "/")
            ));
        }
        out.push_str("        ],\n");

        // patch operation metadata files
        out.push_str("        &[\n");
        for (label, rel) in &pack.patch_op_files {
            out.push_str(&format!(
                "            (\"{label}\", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{rel}\"))),\n",
                label = label.replace('\\', "/"),
                rel = rel.replace('\\', "/")
            ));
        }
        out.push_str("        ],\n");

        out.push_str("    ),\n");
    }
    out.push_str("];\n");

    fs::write(&out_path, out).expect("failed to write built_in_schema_packs.rs");
}

struct PackInfo {
    /// Human-readable label for the pack (e.g., `"built-in:rimworld-core/schema-pack.json"`).
    label: String,
    /// Relative path from CARGO_MANIFEST_DIR to the schema-pack.json file.
    manifest_rel: String,
    def_files: Vec<(String, String)>,
    obj_files: Vec<(String, String)>,
    patch_op_files: Vec<(String, String)>,
}

/// Walk `schema-packs/` and find all schema-pack.json manifests.
///
/// Supported layouts:
///   - `schema-packs/<name>/schema-pack.json`             (flat)
///   - `schema-packs/<name>/<version>/schema-pack.json`   (versioned)
fn discover_manifests(schema_packs_root: &Path) -> Vec<PackInfo> {
    let manifest_dir = schema_packs_root
        .parent()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut result = Vec::new();

    let pack_dirs = match fs::read_dir(schema_packs_root) {
        Ok(d) => d,
        Err(_) => return result,
    };

    for pack_entry in pack_dirs.flatten() {
        let pack_path = pack_entry.path();
        if !pack_path.is_dir() {
            continue;
        }
        let pack_name = pack_entry.file_name().to_string_lossy().to_string();

        // Flat layout: schema-packs/<name>/schema-pack.json
        let flat_manifest = pack_path.join("schema-pack.json");
        if flat_manifest.is_file() {
            if let Some(info) =
                load_pack_info(&manifest_dir, &pack_name, &pack_path, &flat_manifest)
            {
                result.push(info);
            }
        }

        // Versioned layout: schema-packs/<name>/<version>/schema-pack.json
        let version_dirs = match fs::read_dir(&pack_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for ver_entry in version_dirs.flatten() {
            let ver_path = ver_entry.path();
            if !ver_path.is_dir() {
                continue;
            }
            let versioned_manifest = ver_path.join("schema-pack.json");
            if versioned_manifest.is_file() {
                let ver_name = ver_entry.file_name().to_string_lossy().to_string();
                let label_name = format!("{}/{}", pack_name, ver_name);
                if let Some(info) =
                    load_pack_info(&manifest_dir, &label_name, &ver_path, &versioned_manifest)
                {
                    result.push(info);
                }
            }
        }
    }

    // Sort for deterministic output.
    result.sort_by(|a, b| a.label.cmp(&b.label));
    result
}

fn load_pack_info(
    manifest_dir: &str,
    label_name: &str,
    pack_root: &Path,
    manifest_path: &Path,
) -> Option<PackInfo> {
    // Read the manifest JSON to discover defTypeDirectories / objectTypeDirectories.
    let manifest_text = fs::read_to_string(manifest_path).ok()?;
    let manifest_json: serde_json::Value = serde_json::from_str(&manifest_text).ok()?;

    let def_dirs: Vec<String> = manifest_json
        .get("defTypeDirectories")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_else(|| vec!["def-types".to_string()]);

    let obj_dirs: Vec<String> = manifest_json
        .get("objectTypeDirectories")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let patch_op_dirs: Vec<String> = manifest_json
        .get("patchOperationDirectories")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let manifest_rel = manifest_path
        .strip_prefix(manifest_dir)
        .unwrap_or(manifest_path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();

    let label = format!("built-in:{}/schema-pack.json", label_name);

    let mut def_files = Vec::new();
    for dir in &def_dirs {
        collect_json_files_for_build(
            &pack_root.join(dir),
            manifest_dir,
            pack_root,
            &mut def_files,
        );
    }
    def_files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut obj_files = Vec::new();
    for dir in &obj_dirs {
        collect_json_files_for_build(
            &pack_root.join(dir),
            manifest_dir,
            pack_root,
            &mut obj_files,
        );
    }
    obj_files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut patch_op_files = Vec::new();
    for dir in &patch_op_dirs {
        collect_json_files_for_build(
            &pack_root.join(dir),
            manifest_dir,
            pack_root,
            &mut patch_op_files,
        );
    }
    patch_op_files.sort_by(|a, b| a.0.cmp(&b.0));

    Some(PackInfo {
        label,
        manifest_rel,
        def_files,
        obj_files,
        patch_op_files,
    })
}

/// Walk `dir` recursively collecting (label_within_pack, relative_from_manifest_dir) pairs for JSON files.
/// Intentionally mirrors the runtime recursive traversal in `loader::collect_json_files`.
fn collect_json_files_for_build(
    dir: &Path,
    manifest_dir: &str,
    pack_root: &Path,
    out: &mut Vec<(String, String)>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if path.extension().and_then(|x| x.to_str()) == Some("json") {
                let label = path
                    .strip_prefix(pack_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let rel = path
                    .strip_prefix(manifest_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/")
                    .trim_start_matches('/')
                    .to_string();
                out.push((label, rel));
            }
        } else if path.is_dir() {
            collect_json_files_for_build(&path, manifest_dir, pack_root, out);
        }
    }
}
