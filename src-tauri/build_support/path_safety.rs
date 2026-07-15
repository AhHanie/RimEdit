// Shared by `build.rs` (via `include!`) and `schema_pack::tests::build_path_safety` (also via
// `include!`, since `cargo test` never executes `build.rs` itself, so this is the only way to get
// unit-test coverage over the exact logic `build.rs` runs at compile time).
//
// Mirrors the runtime external-pack loader's `resolve_manifest_relative_dir` in
// `src/schema_pack/loader.rs` as closely as practical for a build-time context: an absolute
// `entry`, or an `entry` whose path (once joined onto `root`) contains a `..` (`ParentDir`)
// component, is rejected as a pack-root escape attempt. Returns `None` on rejection, `Some` with
// the joined path otherwise. Unlike the runtime loader (a Tauri command that can report a
// recoverable `SchemaLoadDiagnostic` and simply skip the offending directory), `build.rs` has no
// diagnostics channel and no ability to "skip and keep going" -- a built-in pack manifest that
// declares an escaping directory is a build-time authoring bug in this repository's own shipped
// content, not third-party/external input, so the caller is expected to fail the build outright
// (via `panic!`) rather than silently drop the directory.
// This lexical-only check (a literal '..' component, or an absolute entry) has the identical
// gap the runtime loader's sibling function had before being fixed -- a path component that is
// itself a symlink pointing outside `root` (e.g. a
// manifest declaring `"localesDirectory": "link/locales"` where `link` is a symlink to somewhere
// outside the pack and `locales` is a real, non-symlink subdirectory reached through it) passes
// both lexical checks yet still resolves outside the pack root once the OS follows it. Fixed here
// too, mirroring `schema_pack/loader.rs`'s `resolve_manifest_relative_dir`: canonicalize both
// `root` and the joined `candidate` (which follows every path component, including intermediate
// symlinks) and require containment. Canonicalization failure -- most commonly `root` or
// `candidate` not existing, which is routine in this module's own unit tests
// (`schema_pack::tests::build_path_safety`, which exercise fabricated non-existent paths like
// `/pack/root`) as well as for a legitimately-missing optional `localesDirectory` at real build
// time -- is not itself treated as an escape; `collect_json_files_for_build`/
// `collect_flat_json_files_for_build` already no-op on a directory that doesn't exist.
#[allow(dead_code)]
fn resolve_relative_dir_within_root(
    root: &std::path::Path,
    entry: &str,
) -> Option<std::path::PathBuf> {
    if std::path::Path::new(entry).is_absolute() {
        return None;
    }
    let candidate = root.join(entry);
    for component in candidate.components() {
        if component == std::path::Component::ParentDir {
            return None;
        }
    }
    if let (Ok(canonical_root), Ok(canonical_candidate)) =
        (root.canonicalize(), candidate.canonicalize())
    {
        if !canonical_candidate.starts_with(&canonical_root) {
            return None;
        }
    }
    Some(candidate)
}
