// Unit-test coverage for `build_support/path_safety.rs`, the path-escape guard `build.rs` uses
// (via `include!`) to validate every manifest-declared directory (defTypeDirectories/
// objectTypeDirectories/patchOperationDirectories/localesDirectory) before joining it onto a
// built-in pack's root and walking it for embedding. `cargo test` never executes `build.rs`
// itself, so the only way to unit-test the exact logic it runs at compile time is to pull the
// same source file into an ordinary test module via `include!` -- this file exists solely for
// that purpose and defines no other test infrastructure.
//
// `build.rs`'s built-in sidecar discovery (including
// `localesDirectory`) joined a manifest-declared directory directly onto the pack root with no
// escape validation, unlike the runtime external-pack loader's `resolve_manifest_relative_dir` in
// `schema_pack/loader.rs`, which rejects absolute entries and `..` escapes. These tests prove the
// shared helper now used by both `build.rs` and the runtime loader's sibling logic behaves
// identically to that established, already-tested runtime pattern.

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/build_support/path_safety.rs"
));

use std::path::Path;

#[test]
fn ordinary_relative_entry_resolves_within_root() {
    let root = Path::new("/pack/root");
    let resolved = resolve_relative_dir_within_root(root, "locales");
    assert_eq!(resolved, Some(root.join("locales")));
}

#[test]
fn nested_relative_entry_resolves_within_root() {
    let root = Path::new("/pack/root");
    let resolved = resolve_relative_dir_within_root(root, "def-types/Sub");
    assert_eq!(resolved, Some(root.join("def-types/Sub")));
}

#[test]
fn parent_dir_escape_is_rejected() {
    let root = Path::new("/pack/root");
    assert_eq!(
        resolve_relative_dir_within_root(root, "../evil"),
        None,
        "a leading '..' component must be rejected"
    );
}

#[test]
fn parent_dir_escape_after_a_valid_prefix_is_rejected() {
    let root = Path::new("/pack/root");
    assert_eq!(
        resolve_relative_dir_within_root(root, "locales/../../evil"),
        None,
        "a '..' anywhere in the joined path must be rejected, not just a leading one"
    );
}

#[test]
fn absolute_entry_is_rejected() {
    let root = Path::new("/pack/root");
    #[cfg(windows)]
    let absolute = "C:\\Windows\\System32";
    #[cfg(not(windows))]
    let absolute = "/etc";
    assert_eq!(
        resolve_relative_dir_within_root(root, absolute),
        None,
        "an absolute entry must be rejected outright, since joining it would replace the root"
    );
}

#[test]
fn dot_current_dir_entry_resolves_to_root_itself() {
    let root = Path::new("/pack/root");
    let resolved = resolve_relative_dir_within_root(root, ".");
    assert_eq!(
        resolved,
        Some(root.join(".")),
        "a bare '.' is a legitimate (if unusual) same-directory reference, not an escape"
    );
}

// An intermediate symlink component that points outside `root`
// escapes both lexical checks above (no '..' segment, not absolute) yet still resolves outside
// the pack root once the OS follows it. Mirrors `schema_pack::tests::locale`'s equivalent runtime
// test; see that file's `create_dir_symlink` doc comment for why symlink creation is tried and
// gracefully skipped rather than failed when unavailable in the current environment.

#[cfg(unix)]
fn create_dir_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn create_dir_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(src, dst)
}

#[test]
fn intermediate_symlink_escape_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("outside temp dir");

    let root = tmp.path().join("pack-root");
    std::fs::create_dir(&root).unwrap();

    let outside_locales = outside.path().join("locales");
    std::fs::create_dir(&outside_locales).unwrap();

    let link = root.join("link");
    if create_dir_symlink(outside.path(), &link).is_err() {
        // No symlink privilege in this environment -- nothing meaningful to assert.
        return;
    }

    assert_eq!(
        resolve_relative_dir_within_root(&root, "link/locales"),
        None,
        "a 'locales' subdirectory reached only via a symlink pointing outside root must be \
         rejected, even though 'locales' itself is not a symlink"
    );
}
