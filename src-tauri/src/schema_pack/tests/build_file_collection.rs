// Unit-test coverage for `build_support/file_collection.rs`, the symlink-rejecting file-collection
// walk `build.rs` uses (via `include!`) to decide which files under a manifest-validated directory
// actually get embedded into the binary. `cargo test` never executes `build.rs` itself, so the
// only way to unit-test the exact logic it runs at compile time is to pull the same source file
// into an ordinary test module via `include!` -- this file exists solely for that purpose and
// defines no other test infrastructure.
//
// Containment-checking is already fixed for the *declared directory
// path itself* (see `build_path_safety.rs`'s tests), including intermediate-symlink path
// components. This is a narrower, different gap: once build.rs is already inside a directory it
// has validated as safe, the walk that collects files to embed previously used `Path::is_file`/
// `Path::is_dir`, which transparently follow symlinks -- so an individual symlinked file, or a
// symlinked nested subdirectory, sitting directly inside an otherwise-safe directory would be
// silently embedded/recursed from wherever it actually points. These tests prove the shared walk
// now used by `build.rs` rejects both cases the same way the runtime loader's
// `collect_json_files_recursive`/`read_locale_directory_files` already do.

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/build_support/file_collection.rs"
));

use std::path::Path;

// Mirrors `build_path_safety.rs`'s established cross-platform pattern: a symlink helper, tried and
// gracefully skipped (not failed) by callers when creation errors, since Windows can require
// elevated privileges or Developer Mode to create symlinks and CI/dev environments vary.

#[cfg(unix)]
fn create_dir_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn create_dir_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(src, dst)
}

#[cfg(unix)]
fn create_file_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn create_file_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(src, dst)
}

#[test]
fn ordinary_json_files_are_collected_recursively() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path().join("pack-root");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(root.join("a.json"), "{}").unwrap();
    std::fs::write(nested.join("b.json"), "{}").unwrap();
    std::fs::write(root.join("ignored.txt"), "not json").unwrap();

    let mut out = Vec::new();
    let manifest_dir = tmp.path().to_string_lossy().to_string();
    collect_json_files_for_build(&root, &manifest_dir, &root, &mut out);

    let labels: Vec<&str> = out.iter().map(|(label, _)| label.as_str()).collect();
    assert_eq!(
        labels.len(),
        2,
        "expected exactly two json files, got: {labels:?}"
    );
    assert!(labels.contains(&"a.json"));
    assert!(labels.contains(&"nested/b.json"));
}

#[test]
fn ordinary_flat_json_files_are_collected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path().join("pack-root");
    let locales = root.join("locales");
    std::fs::create_dir_all(&locales).unwrap();
    std::fs::write(locales.join("en.json"), "{}").unwrap();

    let mut out = Vec::new();
    let manifest_dir = tmp.path().to_string_lossy().to_string();
    collect_flat_json_files_for_build(&locales, &manifest_dir, &root, &mut out);

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, "locales/en.json");
}

#[test]
fn symlinked_file_inside_a_validated_directory_is_skipped_recursive() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("outside temp dir");

    let root = tmp.path().join("pack-root");
    std::fs::create_dir(&root).unwrap();

    let outside_file = outside.path().join("secret.json");
    std::fs::write(&outside_file, r#"{"leak": true}"#).unwrap();

    let link = root.join("linked.json");
    if create_file_symlink(&outside_file, &link).is_err() {
        // No symlink privilege in this environment -- nothing meaningful to assert.
        return;
    }

    let mut out = Vec::new();
    let manifest_dir = tmp.path().to_string_lossy().to_string();
    collect_json_files_for_build(&root, &manifest_dir, &root, &mut out);

    assert!(
        out.is_empty(),
        "a symlinked json file must never be embedded, even though the containing directory was \
         validated as safe, got: {out:?}"
    );
}

#[test]
fn symlinked_file_inside_a_validated_directory_is_skipped_flat() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("outside temp dir");

    let root = tmp.path().join("pack-root");
    let locales = root.join("locales");
    std::fs::create_dir_all(&locales).unwrap();

    let outside_file = outside.path().join("en.json");
    std::fs::write(&outside_file, r#"{"leak": true}"#).unwrap();

    let link = locales.join("en.json");
    if create_file_symlink(&outside_file, &link).is_err() {
        // No symlink privilege in this environment -- nothing meaningful to assert.
        return;
    }

    let mut out = Vec::new();
    let manifest_dir = tmp.path().to_string_lossy().to_string();
    collect_flat_json_files_for_build(&locales, &manifest_dir, &root, &mut out);

    assert!(
        out.is_empty(),
        "a symlinked locale file must never be embedded, even though the containing directory was \
         validated as safe, got: {out:?}"
    );
}

#[test]
fn symlinked_nested_subdirectory_is_not_recursed_into() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("outside temp dir");

    let root = tmp.path().join("pack-root");
    std::fs::create_dir(&root).unwrap();

    let outside_dir = outside.path().join("sub");
    std::fs::create_dir(&outside_dir).unwrap();
    std::fs::write(outside_dir.join("secret.json"), r#"{"leak": true}"#).unwrap();

    let link = root.join("linked-dir");
    if create_dir_symlink(&outside_dir, &link).is_err() {
        // No symlink privilege in this environment -- nothing meaningful to assert.
        return;
    }

    let mut out = Vec::new();
    let manifest_dir = tmp.path().to_string_lossy().to_string();
    collect_json_files_for_build(&root, &manifest_dir, &root, &mut out);

    assert!(
        out.is_empty(),
        "a symlinked nested subdirectory must never be recursed into and embedded from, got: {out:?}"
    );
}
