use super::*;
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};
use std::fs;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

fn make_location(
    id: &str,
    display_name: &str,
    root: &Path,
    kind: LocationKind,
    read_only: bool,
) -> RegisteredLocation {
    RegisteredLocation {
        id: id.to_string(),
        display_name: display_name.to_string(),
        root_path: root.to_string_lossy().to_string(),
        kind,
        source_type: SourceType::Folder,
        read_only,
        mod_id: None,
        game_version: None,
        expansion_name: None,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn make_settings(root: &Path) -> ProjectSettings {
    ProjectSettings {
        schema_version: 2,
        game_version: "1.6".to_string(),
        locations: vec![make_location(
            "proj1",
            "Test Project",
            root,
            LocationKind::Project,
            false,
        )],
        active_project_id: Some("proj1".to_string()),
    }
}

fn make_base_game_location(id: &str, display_name: &str, root: &Path) -> RegisteredLocation {
    let mut location = make_location(id, display_name, root, LocationKind::Source, true);
    location.source_type = SourceType::BaseGame;
    location.mod_id = None;
    location
}

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rimedit_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
fn create_symlink_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn create_symlink_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(src, dst)
}

#[test]
fn scan_includes_xml_and_uppercase_xml() {
    let dir = temp_dir();
    fs::write(dir.join("a.xml"), "<a/>").unwrap();
    fs::write(dir.join("b.XML"), "<b/>").unwrap();
    fs::write(dir.join("c.txt"), "text").unwrap();
    let settings = make_settings(&dir);
    let scan = scan_xml_files(&settings, "proj1").unwrap();
    let names: Vec<&str> = scan.files.iter().map(|f| f.file_name.as_str()).collect();
    assert!(names.contains(&"a.xml"), "expected a.xml");
    assert!(names.contains(&"b.XML"), "expected b.XML");
    assert!(!names.contains(&"c.txt"), "c.txt should be excluded");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn scan_excludes_unsupported_files() {
    let dir = temp_dir();
    fs::write(dir.join("image.png"), [0u8]).unwrap();
    fs::write(dir.join("noext"), "data").unwrap();
    fs::write(dir.join("good.xml"), "<x/>").unwrap();
    let settings = make_settings(&dir);
    let scan = scan_xml_files(&settings, "proj1").unwrap();
    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].file_name, "good.xml");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn scan_empty_directory_returns_empty_list() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let scan = scan_xml_files(&settings, "proj1").unwrap();
    assert!(scan.files.is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn relative_path_uses_forward_slashes() {
    let dir = temp_dir();
    let sub = dir.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("file.xml"), "<f/>").unwrap();
    let settings = make_settings(&dir);
    let scan = scan_xml_files(&settings, "proj1").unwrap();
    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].relative_path, "sub/file.xml");
    assert_eq!(scan.files[0].folder_path, "sub");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn root_files_have_empty_folder_path() {
    let dir = temp_dir();
    fs::write(dir.join("root.xml"), "<r/>").unwrap();
    let settings = make_settings(&dir);
    let scan = scan_xml_files(&settings, "proj1").unwrap();
    assert_eq!(scan.files[0].folder_path, "");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn absolute_path_is_rejected() {
    let dir = temp_dir();
    fs::write(dir.join("ok.xml"), "<x/>").unwrap();
    let settings = make_settings(&dir);
    let result = validate_and_resolve(&settings, "proj1", "C:\\Windows\\system32\\file.xml");
    assert!(matches!(result, Err(ProjectFileError::FileOutsideRoot)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn parent_traversal_is_rejected() {
    let dir = temp_dir();
    fs::write(dir.join("ok.xml"), "<x/>").unwrap();
    let settings = make_settings(&dir);
    let result = validate_and_resolve(&settings, "proj1", "../other.xml");
    assert!(matches!(result, Err(ProjectFileError::FileOutsideRoot)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn non_xml_file_is_rejected_at_open() {
    let dir = temp_dir();
    fs::write(dir.join("data.txt"), "text").unwrap();
    let settings = make_settings(&dir);
    let result = validate_and_resolve(&settings, "proj1", "data.txt");
    assert!(matches!(result, Err(ProjectFileError::UnsupportedFile)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_project_returns_project_not_found() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let result = scan_xml_files(&settings, "nonexistent");
    assert!(matches!(result, Err(ProjectFileError::ProjectNotFound(_))));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn read_only_project_returns_not_editable() {
    let dir = temp_dir();
    let mut settings = make_settings(&dir);
    settings.locations[0].read_only = true;
    let result = scan_xml_files(&settings, "proj1");
    assert!(matches!(
        result,
        Err(ProjectFileError::ProjectNotEditable(_))
    ));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn validate_and_resolve_location_reads_read_only_source() {
    let project_dir = temp_dir();
    let source_dir = temp_dir();
    fs::write(source_dir.join("ThingDefs.xml"), "<Defs/>").unwrap();
    let mut settings = make_settings(&project_dir);
    settings.locations.push(make_location(
        "core",
        "Core",
        &source_dir,
        LocationKind::Source,
        true,
    ));

    let result = validate_and_resolve_location(&settings, "core", "ThingDefs.xml");

    assert!(result.is_ok(), "read-only source XML should resolve");
    fs::remove_dir_all(&project_dir).ok();
    fs::remove_dir_all(&source_dir).ok();
}

#[test]
fn base_game_scan_keeps_duplicate_def_paths_from_peer_content_packs() {
    let data_dir = temp_dir();
    let core_items = data_dir.join("Core").join("Defs").join("ThingDefs_Items");
    let biotech_items = data_dir
        .join("Biotech")
        .join("Defs")
        .join("ThingDefs_Items");
    let core_buildings = data_dir
        .join("Core")
        .join("Defs")
        .join("ThingDefs_Buildings");
    let anomaly_buildings = data_dir
        .join("Anomaly")
        .join("Defs")
        .join("ThingDefs_Buildings");
    fs::create_dir_all(&core_items).unwrap();
    fs::create_dir_all(&biotech_items).unwrap();
    fs::create_dir_all(&core_buildings).unwrap();
    fs::create_dir_all(&anomaly_buildings).unwrap();
    fs::create_dir_all(data_dir.join("NotAContentPack")).unwrap();
    fs::write(core_items.join("Items_Unfinished.xml"), "<Defs/>").unwrap();
    fs::write(biotech_items.join("Items_Unfinished.xml"), "<Defs/>").unwrap();
    fs::write(core_buildings.join("Buildings_Production.xml"), "<Defs/>").unwrap();
    fs::write(
        anomaly_buildings.join("Buildings_Production.xml"),
        "<Defs/>",
    )
    .unwrap();

    let settings = ProjectSettings {
        schema_version: 2,
        game_version: "1.6".to_string(),
        locations: vec![make_base_game_location("base", "RimWorld Data", &data_dir)],
        active_project_id: None,
    };

    let scan = scan_indexable_def_xml_files(&settings, &settings.locations[0]).unwrap();
    let paths: Vec<&str> = scan
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect();

    assert!(paths.contains(&"Core/Defs/ThingDefs_Items/Items_Unfinished.xml"));
    assert!(paths.contains(&"Biotech/Defs/ThingDefs_Items/Items_Unfinished.xml"));
    assert!(paths.contains(&"Core/Defs/ThingDefs_Buildings/Buildings_Production.xml"));
    assert!(paths.contains(&"Anomaly/Defs/ThingDefs_Buildings/Buildings_Production.xml"));
    assert!(!paths
        .iter()
        .any(|path| path.starts_with("NotAContentPack/")));
    fs::remove_dir_all(&data_dir).ok();
}

#[test]
fn mod_load_folder_scan_still_shadows_duplicate_relative_paths() {
    let mod_dir = temp_dir();
    fs::create_dir_all(mod_dir.join("Low").join("Defs")).unwrap();
    fs::create_dir_all(mod_dir.join("High").join("Defs")).unwrap();
    fs::write(
        mod_dir.join("LoadFolders.xml"),
        r#"<loadFolders><v1.6><li>Low</li><li>High</li></v1.6></loadFolders>"#,
    )
    .unwrap();
    fs::write(mod_dir.join("Low").join("Defs").join("Same.xml"), "<Defs/>").unwrap();
    fs::write(
        mod_dir.join("High").join("Defs").join("Same.xml"),
        "<Defs/>",
    )
    .unwrap();

    let settings = ProjectSettings {
        schema_version: 2,
        game_version: "1.6".to_string(),
        locations: vec![make_location(
            "source",
            "Source Mod",
            &mod_dir,
            LocationKind::Source,
            true,
        )],
        active_project_id: None,
    };

    let scan = scan_indexable_def_xml_files(&settings, &settings.locations[0]).unwrap();
    let paths: Vec<&str> = scan
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect();

    assert_eq!(paths, vec!["High/Defs/Same.xml"]);
    fs::remove_dir_all(&mod_dir).ok();
}

#[test]
fn all_files_scan_includes_xml_txt_png_extensionless() {
    let dir = temp_dir();
    fs::write(dir.join("defs.xml"), "<x/>").unwrap();
    fs::write(dir.join("notes.txt"), "hi").unwrap();
    fs::write(dir.join("icon.png"), [0u8]).unwrap();
    fs::write(dir.join("noext"), "data").unwrap();
    let settings = make_settings(&dir);
    let scan = scan_all_project_files(&settings, "proj1").unwrap();
    let names: Vec<&str> = scan.files.iter().map(|f| f.file_name.as_str()).collect();
    assert!(names.contains(&"defs.xml"));
    assert!(names.contains(&"notes.txt"));
    assert!(names.contains(&"icon.png"));
    assert!(names.contains(&"noext"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn all_files_scan_includes_empty_folder() {
    let dir = temp_dir();
    fs::create_dir(dir.join("EmptyFolder")).unwrap();
    let settings = make_settings(&dir);
    let scan = scan_all_project_files(&settings, "proj1").unwrap();
    let folder_names: Vec<&str> = scan
        .folders
        .iter()
        .map(|f| f.folder_name.as_str())
        .collect();
    assert!(folder_names.contains(&"EmptyFolder"));
    assert!(scan.files.is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn all_files_scan_includes_folder_with_only_non_xml_files() {
    let dir = temp_dir();
    let sub = dir.join("Assets");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("icon.png"), [0u8]).unwrap();
    fs::write(sub.join("notes.txt"), "hi").unwrap();
    let settings = make_settings(&dir);
    let scan = scan_all_project_files(&settings, "proj1").unwrap();
    assert!(scan.folders.iter().any(|f| f.folder_name == "Assets"));
    assert_eq!(scan.files.len(), 2);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn all_files_scan_nested_files() {
    let dir = temp_dir();
    let deep = dir.join("Defs").join("ThingDefs");
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("Things.xml"), "<x/>").unwrap();
    let settings = make_settings(&dir);
    let scan = scan_all_project_files(&settings, "proj1").unwrap();
    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].relative_path, "Defs/ThingDefs/Things.xml");
    assert!(scan.folders.iter().any(|f| f.relative_path == "Defs"));
    assert!(scan
        .folders
        .iter()
        .any(|f| f.relative_path == "Defs/ThingDefs"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn all_files_scan_marks_versioned_patch_file_active() {
    let dir = temp_dir();
    let patches_dir = dir.join("1.6").join("Patches");
    fs::create_dir_all(&patches_dir).unwrap();
    fs::write(patches_dir.join("Foo.xml"), "<Patch/>").unwrap();
    let settings = make_settings(&dir);
    let scan = scan_all_project_files(&settings, "proj1").unwrap();
    let file = scan
        .files
        .iter()
        .find(|f| f.relative_path == "1.6/Patches/Foo.xml")
        .expect("expected 1.6/Patches/Foo.xml to be scanned");
    assert_eq!(file.active_for_game_version, Some(true));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn all_files_scan_marks_patch_file_inactive_for_other_version() {
    let dir = temp_dir();
    let patches_dir = dir.join("1.7").join("Patches");
    fs::create_dir_all(&patches_dir).unwrap();
    fs::write(patches_dir.join("Foo.xml"), "<Patch/>").unwrap();
    let settings = make_settings(&dir); // game_version is "1.6"
    let scan = scan_all_project_files(&settings, "proj1").unwrap();
    let file = scan
        .files
        .iter()
        .find(|f| f.relative_path == "1.7/Patches/Foo.xml")
        .expect("expected 1.7/Patches/Foo.xml to be scanned");
    assert_eq!(file.active_for_game_version, Some(false));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn all_files_scan_skips_symlinks() {
    let dir = temp_dir();
    let target = dir.join("target.xml");
    let link = dir.join("linked.xml");
    fs::write(&target, "<x/>").unwrap();
    if create_symlink_file(&target, &link).is_err() {
        fs::remove_dir_all(&dir).ok();
        return;
    }

    let settings = make_settings(&dir);
    let scan = scan_all_project_files(&settings, "proj1").unwrap();

    assert!(scan.files.iter().any(|f| f.relative_path == "target.xml"));
    assert!(!scan.files.iter().any(|f| f.relative_path == "linked.xml"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn xml_only_scan_remains_xml_only() {
    let dir = temp_dir();
    fs::write(dir.join("defs.xml"), "<x/>").unwrap();
    fs::write(dir.join("notes.txt"), "hi").unwrap();
    fs::write(dir.join("icon.png"), [0u8]).unwrap();
    let settings = make_settings(&dir);
    let scan = scan_xml_files(&settings, "proj1").unwrap();
    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].file_name, "defs.xml");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_xml_file_is_empty() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let entry = create_project_file(&settings, "proj1", "", "Things.xml", None).unwrap();
    assert_eq!(entry.file_name, "Things.xml");
    assert_eq!(entry.relative_path, "Things.xml");
    let contents = fs::read_to_string(dir.join("Things.xml")).unwrap();
    assert!(contents.is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_file_writes_provided_contents_verbatim() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let patch_template = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Patch>\n</Patch>\n";
    let entry = create_project_file(
        &settings,
        "proj1",
        "",
        "NewPatches.xml",
        Some(patch_template),
    )
    .unwrap();
    assert_eq!(entry.file_name, "NewPatches.xml");
    let contents = fs::read_to_string(dir.join("NewPatches.xml")).unwrap();
    assert_eq!(contents, patch_template);
    assert!(!contents.contains("<Defs>"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_txt_file_is_empty() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let entry = create_project_file(&settings, "proj1", "", "notes.txt", None).unwrap();
    assert_eq!(entry.file_name, "notes.txt");
    let contents = fs::read_to_string(dir.join("notes.txt")).unwrap();
    assert!(contents.is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_file_in_subfolder() {
    let dir = temp_dir();
    fs::create_dir(dir.join("Defs")).unwrap();
    let settings = make_settings(&dir);
    let entry = create_project_file(&settings, "proj1", "Defs", "ThingDefs.xml", None).unwrap();
    assert_eq!(entry.relative_path, "Defs/ThingDefs.xml");
    assert_eq!(entry.folder_path, "Defs");
    assert!(dir.join("Defs").join("ThingDefs.xml").exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_file_rejects_duplicate() {
    let dir = temp_dir();
    fs::write(dir.join("existing.xml"), "<x/>").unwrap();
    let settings = make_settings(&dir);
    let result = create_project_file(&settings, "proj1", "", "existing.xml", None);
    assert!(matches!(
        result,
        Err(ProjectFileError::PathAlreadyExists(_))
    ));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_file_rejects_path_traversal() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let result = create_project_file(&settings, "proj1", "..", "evil.xml", None);
    assert!(matches!(result, Err(ProjectFileError::FileOutsideRoot)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn mutation_parent_rejects_current_dir_component() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let result = create_project_file(&settings, "proj1", ".", "evil.xml", None);
    assert!(matches!(result, Err(ProjectFileError::FileOutsideRoot)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn mutation_existing_path_rejects_current_dir_component() {
    let dir = temp_dir();
    fs::write(dir.join("notes.txt"), "hi").unwrap();
    let settings = make_settings(&dir);
    let result = rename_project_path(&settings, "proj1", "./notes.txt", "renamed.txt", "file");
    assert!(matches!(result, Err(ProjectFileError::FileOutsideRoot)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_file_rejects_invalid_name() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let result = create_project_file(&settings, "proj1", "", "bad/name.xml", None);
    assert!(matches!(result, Err(ProjectFileError::InvalidFileName(_))));
    let result2 = create_project_file(&settings, "proj1", "", "", None);
    assert!(matches!(result2, Err(ProjectFileError::InvalidFileName(_))));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_folder_succeeds_and_appears_in_scan() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let entry = create_project_folder(&settings, "proj1", "", "NewFolder").unwrap();
    assert_eq!(entry.folder_name, "NewFolder");
    assert_eq!(entry.relative_path, "NewFolder");
    let scan = scan_all_project_files(&settings, "proj1").unwrap();
    assert!(scan.folders.iter().any(|f| f.folder_name == "NewFolder"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_file_allows_extension_change() {
    let dir = temp_dir();
    fs::write(dir.join("notes.txt"), "hi").unwrap();
    let settings = make_settings(&dir);
    let result = rename_project_path(&settings, "proj1", "notes.txt", "notes.md", "file").unwrap();
    assert_eq!(result.old_path, "notes.txt");
    assert_eq!(result.new_path, "notes.md");
    assert!(dir.join("notes.md").exists());
    assert!(!dir.join("notes.txt").exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_folder_succeeds() {
    let dir = temp_dir();
    fs::create_dir(dir.join("OldName")).unwrap();
    let settings = make_settings(&dir);
    let result = rename_project_path(&settings, "proj1", "OldName", "NewName", "folder").unwrap();
    assert_eq!(result.new_path, "NewName");
    assert!(dir.join("NewName").is_dir());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_case_only_on_windows() {
    let dir = temp_dir();
    fs::write(dir.join("notes.txt"), "hi").unwrap();
    let settings = make_settings(&dir);
    let result = rename_project_path(&settings, "proj1", "notes.txt", "Notes.txt", "file");
    assert!(result.is_ok(), "case-only rename failed: {:?}", result);
    let r = result.unwrap();
    assert_eq!(r.new_path, "Notes.txt");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_root_is_rejected() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let result = rename_project_path(&settings, "proj1", "", "NewName", "folder");
    assert!(matches!(result, Err(ProjectFileError::CannotModifyRoot)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_unknown_kind_returns_kind_mismatch() {
    let dir = temp_dir();
    fs::write(dir.join("notes.txt"), "hi").unwrap();
    let settings = make_settings(&dir);
    let result = rename_project_path(&settings, "proj1", "notes.txt", "renamed.txt", "asset");
    assert!(matches!(result, Err(ProjectFileError::KindMismatch(_))));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn delete_file_removes_it() {
    let dir = temp_dir();
    fs::write(dir.join("gone.txt"), "bye").unwrap();
    let settings = make_settings(&dir);
    let result = delete_project_path(&settings, "proj1", "gone.txt", "file").unwrap();
    assert_eq!(result.old_path, "gone.txt");
    assert!(!dir.join("gone.txt").exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn delete_folder_removes_recursively() {
    let dir = temp_dir();
    let sub = dir.join("Sub");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("file.xml"), "<x/>").unwrap();
    fs::write(sub.join("notes.txt"), "hi").unwrap();
    let settings = make_settings(&dir);
    delete_project_path(&settings, "proj1", "Sub", "folder").unwrap();
    assert!(!sub.exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn delete_root_is_rejected() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let result = delete_project_path(&settings, "proj1", "", "folder");
    assert!(matches!(result, Err(ProjectFileError::CannotModifyRoot)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn delete_path_traversal_rejected() {
    let dir = temp_dir();
    let settings = make_settings(&dir);
    let result = delete_project_path(&settings, "proj1", "../other", "folder");
    assert!(matches!(result, Err(ProjectFileError::FileOutsideRoot)));
    fs::remove_dir_all(&dir).ok();
}
