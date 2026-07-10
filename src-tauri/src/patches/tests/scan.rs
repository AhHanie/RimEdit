use std::fs;

use crate::project_model::LocationKind;

use super::super::scan::scan_indexable_patch_xml_files;

use super::{location, settings_with_locations, temp_dir};

#[test]
fn scans_root_patches_folder() {
    let root = temp_dir();
    fs::create_dir(root.join("Patches")).unwrap();
    fs::write(root.join("Patches").join("foo.xml"), "<Patch></Patch>").unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc.clone()], "project");

    let scan = scan_indexable_patch_xml_files(&settings, &loc).unwrap();

    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].relative_path, "Patches/foo.xml");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scans_versioned_patches_folder() {
    let root = temp_dir();
    fs::create_dir_all(root.join("1.6").join("Patches")).unwrap();
    fs::write(
        root.join("1.6").join("Patches").join("foo.xml"),
        "<Patch></Patch>",
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc.clone()], "project");

    let scan = scan_indexable_patch_xml_files(&settings, &loc).unwrap();

    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].relative_path, "1.6/Patches/foo.xml");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scans_multiple_load_folders_xml_folders_in_precedence_order() {
    let root = temp_dir();
    fs::write(
        root.join("LoadFolders.xml"),
        r#"<loadFolders><v1.6><li>1.6</li><li>Common</li></v1.6></loadFolders>"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("1.6").join("Patches")).unwrap();
    fs::create_dir_all(root.join("Common").join("Patches")).unwrap();
    fs::write(
        root.join("1.6").join("Patches").join("a.xml"),
        "<Patch></Patch>",
    )
    .unwrap();
    fs::write(
        root.join("Common").join("Patches").join("b.xml"),
        "<Patch></Patch>",
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc.clone()], "project");

    let scan = scan_indexable_patch_xml_files(&settings, &loc).unwrap();

    // The last-listed <li> (Common) has the highest load precedence, matching
    // `resolve_load_folders`'s existing "reverse list order" behavior for Defs.
    let paths: Vec<&str> = scan
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    assert_eq!(paths, vec!["Common/Patches/b.xml", "1.6/Patches/a.xml"]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn load_folders_xml_ignores_conditional_attributes() {
    let root = temp_dir();
    fs::write(
        root.join("LoadFolders.xml"),
        r#"<loadFolders><v1.6>
            <li>1.6</li>
            <li IfModActive="some.mod.id">1.6/Compat</li>
        </v1.6></loadFolders>"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("1.6").join("Patches")).unwrap();
    fs::create_dir_all(root.join("1.6").join("Compat").join("Patches")).unwrap();
    fs::write(
        root.join("1.6").join("Patches").join("a.xml"),
        "<Patch></Patch>",
    )
    .unwrap();
    fs::write(
        root.join("1.6")
            .join("Compat")
            .join("Patches")
            .join("b.xml"),
        "<Patch></Patch>",
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc.clone()], "project");

    let scan = scan_indexable_patch_xml_files(&settings, &loc).unwrap();

    // The conditional folder is included unconditionally: IfModActive is ignored for indexing.
    assert_eq!(scan.files.len(), 2);
    assert!(scan
        .files
        .iter()
        .any(|f| f.relative_path == "1.6/Compat/Patches/b.xml"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn does_not_scan_defs_folder() {
    let root = temp_dir();
    fs::create_dir(root.join("Defs")).unwrap();
    fs::write(
        root.join("Defs").join("foo.xml"),
        "<Defs><ThingDef><defName>Wall</defName></ThingDef></Defs>",
    )
    .unwrap();
    let loc = location(&root, "project", LocationKind::Project);
    let settings = settings_with_locations(vec![loc.clone()], "project");

    let scan = scan_indexable_patch_xml_files(&settings, &loc).unwrap();

    assert!(scan.files.is_empty());
    fs::remove_dir_all(&root).ok();
}
