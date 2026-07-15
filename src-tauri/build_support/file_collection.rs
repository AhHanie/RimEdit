// Shared by `build.rs` (via `include!`) and `schema_pack::tests::build_file_collection` (also via
// `include!`, since `cargo test` never executes `build.rs` itself, so this is the only way to get
// unit-test coverage over the exact logic `build.rs` runs at compile time to decide which files on
// disk get embedded into the binary).
//
// Containment-checking is already fixed for the *declared directory path itself*
// (`resolve_relative_dir_within_root` in `path_safety.rs`, used both here
// at build time and by the runtime loader's `resolve_manifest_relative_dir`), including
// intermediate-symlink path components. This file closes a narrower, different gap: once build.rs
// is already inside a directory it has validated as safe, the walk that decides which entries
// inside it count as a file to embed or a subdirectory to recurse into previously used
// `Path::is_file`/`Path::is_dir`, which -- like `fs::metadata` generally -- transparently follow
// symlinks. A symlinked file (e.g. `locales/en.json` symlinked to somewhere outside the pack root)
// or a symlinked nested subdirectory would silently be embedded/recursed from wherever it actually
// points, even though the containing directory itself was validated. Mirrors the runtime loader's
// established pattern in `schema_pack/loader.rs` (`collect_json_files_recursive` /
// `read_locale_directory_files`), which rejects every entry via `symlink_metadata`-backed
// `is_symlink` (which does NOT follow the final path component) before treating it as real.
#[allow(dead_code)]
fn is_symlink(path: &std::path::Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Non-recursive: collects only direct `.json` children of `dir`, since locale sidecars are
/// always flat (`locales/<tag>.json`), never nested. Each entry is rejected via `is_symlink`
/// before being treated as a real file to embed.
#[allow(dead_code)]
fn collect_flat_json_files_for_build(
    dir: &std::path::Path,
    manifest_dir: &str,
    pack_root: &std::path::Path,
    out: &mut Vec<(String, String)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_symlink(&path) {
            continue;
        }
        if path.is_file() && path.extension().and_then(|x| x.to_str()) == Some("json") {
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
    }
}

/// Walk `dir` recursively collecting `(label_within_pack, relative_from_manifest_dir)` pairs for
/// JSON files. Intentionally mirrors the runtime recursive traversal in
/// `loader::collect_json_files_recursive`. Both a symlinked file and a symlinked nested
/// subdirectory are rejected via `is_symlink` before being treated as real.
#[allow(dead_code)]
fn collect_json_files_for_build(
    dir: &std::path::Path,
    manifest_dir: &str,
    pack_root: &std::path::Path,
    out: &mut Vec<(String, String)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_symlink(&path) {
            continue;
        }
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
