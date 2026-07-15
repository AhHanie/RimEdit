use std::{env, fs, path::Path};

// Shared path-escape guard, also `include!`d by a Rust unit test (see
// `src/schema_pack/tests/build_path_safety.rs`) since `cargo test` never runs `build.rs` itself.
include!("build_support/path_safety.rs");
// Shared symlink-rejecting file-collection walk, also `include!`d by a Rust unit test (see
// `src/schema_pack/tests/build_file_collection.rs`) for the same reason.
include!("build_support/file_collection.rs");

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
    println!(
        "cargo:rerun-if-changed={}",
        Path::new(&manifest_dir)
            .join("build_support")
            .join("path_safety.rs")
            .display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        Path::new(&manifest_dir)
            .join("build_support")
            .join("file_collection.rs")
            .display()
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("built_in_schema_packs.rs");

    // Generate BUILT_IN_SCHEMA_PACKS constant.
    // Each element: (manifest_label, manifest_content, def_files, obj_files, patch_op_files)
    // where def_files, obj_files, and patch_op_files are slices of (label, content) pairs.
    let mut out = String::new();
    out.push_str(
        "#[allow(clippy::type_complexity)]\npub const BUILT_IN_SCHEMA_PACKS: &[(&str, &str, &[(&str, &str)], &[(&str, &str)], &[(&str, &str)], &[(&str, &str)])] = &[\n",
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

        // locale sidecar files
        out.push_str("        &[\n");
        for (label, rel) in &pack.locale_files {
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
    /// Locale sidecar JSON files (issue 05) discovered under the manifest's declared
    /// `localesDirectory`, if any -- one flat file per BCP-47 locale tag (e.g. `locales/en.json`).
    locale_files: Vec<(String, String)>,
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

    let locales_dir: Option<String> = manifest_json
        .get("localesDirectory")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let manifest_rel = manifest_path
        .strip_prefix(manifest_dir)
        .unwrap_or(manifest_path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();

    let label = format!("built-in:{}/schema-pack.json", label_name);

    // Every directory declared by the manifest (defTypeDirectories/objectTypeDirectories/
    // patchOperationDirectories/localesDirectory) is validated to stay within `pack_root` before
    // it is ever joined and walked -- mirrors the runtime external-pack loader's
    // `resolve_manifest_relative_dir` safety check (see `schema_pack/loader.rs`). Built-in packs
    // are this repository's own shipped content, not third-party input, and `build.rs` has no
    // diagnostics channel to recover through, so an escaping entry here is treated as an
    // authoring bug and fails the build outright rather than silently embedding files from
    // outside the pack directory.
    let mut def_files = Vec::new();
    for dir in &def_dirs {
        let resolved = resolve_relative_dir_within_root(pack_root, dir).unwrap_or_else(|| {
            panic!(
                "schema pack '{label_name}': defTypeDirectories entry '{dir}' escapes the pack root via '..' or is absolute"
            )
        });
        collect_json_files_for_build(&resolved, manifest_dir, pack_root, &mut def_files);
    }
    def_files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut obj_files = Vec::new();
    for dir in &obj_dirs {
        let resolved = resolve_relative_dir_within_root(pack_root, dir).unwrap_or_else(|| {
            panic!(
                "schema pack '{label_name}': objectTypeDirectories entry '{dir}' escapes the pack root via '..' or is absolute"
            )
        });
        collect_json_files_for_build(&resolved, manifest_dir, pack_root, &mut obj_files);
    }
    obj_files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut patch_op_files = Vec::new();
    for dir in &patch_op_dirs {
        let resolved = resolve_relative_dir_within_root(pack_root, dir).unwrap_or_else(|| {
            panic!(
                "schema pack '{label_name}': patchOperationDirectories entry '{dir}' escapes the pack root via '..' or is absolute"
            )
        });
        collect_json_files_for_build(&resolved, manifest_dir, pack_root, &mut patch_op_files);
    }
    patch_op_files.sort_by(|a, b| a.0.cmp(&b.0));

    // Locale sidecars are a flat `locales/<tag>.json` layout (not recursive), unlike
    // def/object/patch-operation directories -- mirrors the runtime loader's
    // `read_locale_directory_files` in `schema_pack/loader.rs`.
    let mut locale_files = Vec::new();
    if let Some(dir) = &locales_dir {
        let resolved = resolve_relative_dir_within_root(pack_root, dir).unwrap_or_else(|| {
            panic!(
                "schema pack '{label_name}': localesDirectory entry '{dir}' escapes the pack root via '..' or is absolute"
            )
        });
        collect_flat_json_files_for_build(&resolved, manifest_dir, pack_root, &mut locale_files);
    }
    locale_files.sort_by(|a, b| a.0.cmp(&b.0));

    Some(PackInfo {
        label,
        manifest_rel,
        def_files,
        obj_files,
        patch_op_files,
        locale_files,
    })
}

// `collect_flat_json_files_for_build`, `collect_json_files_for_build`, and the `is_symlink` guard
// they both use now live in `build_support/file_collection.rs` (see the `include!` near the top of
// this file) so the exact symlink-rejecting logic build.rs runs at compile time can also be
// exercised by a real `#[test]` (see `src/schema_pack/tests/build_file_collection.rs`), mirroring
// how `build_support/path_safety.rs` is already shared the same way.
