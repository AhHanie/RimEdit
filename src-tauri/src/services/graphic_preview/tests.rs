use super::asset_protocol::{
    content_type_for_texture, extract_asset_token, is_browser_preview_supported, preview_asset_url,
    read_preview_asset, AssetTokenCache,
};
use super::paths::normalize_texture_path;
use super::resolve_graphic_preview_assets;
use crate::project_model::{LocationKind, ProjectSettings, RegisteredLocation, SourceType};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, Builder as TempBuilder};
use time::OffsetDateTime;

// -- asset serving helpers --

#[test]
fn content_type_png() {
    assert_eq!(content_type_for_texture(Path::new("tex.png")), "image/png");
}

#[test]
fn content_type_jpg_and_jpeg() {
    assert_eq!(content_type_for_texture(Path::new("tex.jpg")), "image/jpeg");
    assert_eq!(
        content_type_for_texture(Path::new("tex.jpeg")),
        "image/jpeg"
    );
}

#[test]
fn content_type_unknown_falls_back() {
    assert_eq!(
        content_type_for_texture(Path::new("tex.dds")),
        "application/octet-stream"
    );
    assert_eq!(
        content_type_for_texture(Path::new("tex.bmp")),
        "application/octet-stream"
    );
}

#[test]
fn browser_supported_extensions() {
    assert!(is_browser_preview_supported(Path::new("tex.png")));
    assert!(is_browser_preview_supported(Path::new("tex.jpg")));
    assert!(is_browser_preview_supported(Path::new("tex.jpeg")));
}

#[test]
fn browser_unsupported_extensions() {
    assert!(!is_browser_preview_supported(Path::new("tex.dds")));
    assert!(!is_browser_preview_supported(Path::new("tex.bmp")));
    assert!(!is_browser_preview_supported(Path::new("tex")));
}

#[test]
fn uppercase_extensions_are_supported() {
    assert!(is_browser_preview_supported(Path::new("tex.PNG")));
    assert!(is_browser_preview_supported(Path::new("tex.JPG")));
    assert!(is_browser_preview_supported(Path::new("tex.JPEG")));
}

#[test]
fn uppercase_extensions_have_correct_mime() {
    assert_eq!(content_type_for_texture(Path::new("tex.PNG")), "image/png");
    assert_eq!(content_type_for_texture(Path::new("tex.JPG")), "image/jpeg");
    assert_eq!(
        content_type_for_texture(Path::new("tex.JPEG")),
        "image/jpeg"
    );
}

#[test]
fn unknown_token_returns_token_not_found() {
    let cache = AssetTokenCache::new();
    let err = read_preview_asset(&cache, "no-such-token").unwrap_err();
    assert_eq!(err.code, "TOKEN_NOT_FOUND");
}

#[test]
fn dds_token_returns_unsupported_format() {
    let cache = AssetTokenCache::new();
    let tmp = TempBuilder::new().suffix(".dds").tempfile().unwrap();
    let token = cache.register(tmp.path().to_path_buf());
    let err = read_preview_asset(&cache, &token).unwrap_err();
    assert_eq!(err.code, "UNSUPPORTED_FORMAT");
}

#[test]
fn png_token_returns_bytes_and_mime() {
    let cache = AssetTokenCache::new();
    let mut tmp = TempBuilder::new().suffix(".png").tempfile().unwrap();
    use std::io::Write;
    tmp.write_all(b"fake-png").unwrap();
    let token = cache.register(tmp.path().to_path_buf());
    let (bytes, ct) = read_preview_asset(&cache, &token).unwrap();
    assert_eq!(ct, "image/png");
    assert_eq!(bytes, b"fake-png");
}

#[test]
fn clone_shares_registered_tokens() {
    let cache = AssetTokenCache::new();
    let tmp = TempBuilder::new().suffix(".png").tempfile().unwrap();
    let token = cache.register(tmp.path().to_path_buf());
    let cloned = cache.clone();
    assert!(cloned.resolve_asset_token(&token).is_some());
}

fn make_location(id: &str, name: &str, root_path: &str, kind: LocationKind) -> RegisteredLocation {
    RegisteredLocation {
        id: id.to_string(),
        display_name: name.to_string(),
        root_path: root_path.to_string(),
        kind,
        source_type: SourceType::Folder,
        read_only: false,
        mod_id: None,
        game_version: None,
        expansion_name: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn make_settings(
    locations: Vec<RegisteredLocation>,
    active_project_id: Option<&str>,
) -> ProjectSettings {
    ProjectSettings {
        schema_version: 2,
        game_version: "1.6".to_string(),
        locations,
        active_project_id: active_project_id.map(str::to_owned),
    }
}

fn cache() -> AssetTokenCache {
    AssetTokenCache::new()
}

// -- strategy coverage --

#[test]
fn all_schema_classes_have_explicit_strategy() {
    use super::strategy::{strategy_for_graphic_class, GraphicPreviewStrategy};
    let json_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema-packs/rimworld-core/1.6/object-types/Shared/GraphicData.json");
    let json_str = fs::read_to_string(&json_path).expect("GraphicData.json must be readable");
    let schema: serde_json::Value =
        serde_json::from_str(&json_str).expect("GraphicData.json must be valid JSON");
    let allowed_values = schema["fields"]["graphicClass"]["validationHints"]["allowedValues"]
        .as_array()
        .expect("graphicClass allowedValues must be an array");
    for entry in allowed_values {
        let class = entry
            .as_str()
            .expect("allowedValues entry must be a string");
        assert!(
            !matches!(
                strategy_for_graphic_class(class),
                GraphicPreviewStrategy::Unknown
            ),
            "Class '{}' in GraphicData.json maps to Unknown strategy",
            class
        );
    }
}

// -- normalize_texture_path --

#[test]
fn normalize_rejects_absolute_unix_path() {
    assert!(normalize_texture_path("/absolute/path").is_err());
}

#[test]
fn normalize_rejects_windows_drive_root() {
    assert!(normalize_texture_path("C:\\Things\\Foo").is_err());
    assert!(normalize_texture_path("C:/Things/Foo").is_err());
}

#[test]
fn normalize_rejects_dotdot() {
    assert!(normalize_texture_path("Things/../Evil").is_err());
    assert!(normalize_texture_path("../../etc/passwd").is_err());
}

#[test]
fn normalize_converts_backslashes_to_forward_slashes() {
    assert_eq!(
        normalize_texture_path("Things\\Item\\Foo").unwrap(),
        "Things/Item/Foo"
    );
}

#[test]
fn normalize_strips_textures_prefix() {
    assert_eq!(
        normalize_texture_path("Textures/Things/Foo").unwrap(),
        "Things/Foo"
    );
}

#[test]
fn normalize_strips_known_extensions() {
    assert_eq!(
        normalize_texture_path("Things/Foo.png").unwrap(),
        "Things/Foo"
    );
    assert_eq!(
        normalize_texture_path("Things/Foo.jpg").unwrap(),
        "Things/Foo"
    );
    assert_eq!(
        normalize_texture_path("Things/Foo.dds").unwrap(),
        "Things/Foo"
    );
}

#[test]
fn normalize_rejects_empty_path() {
    assert!(normalize_texture_path("").is_err());
    assert!(normalize_texture_path("   ").is_err());
}

// -- project_id validation --

#[test]
fn invalid_project_id_returns_project_not_found() {
    let settings = make_settings(vec![], None);
    let err = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "nonexistent",
        "Things/Foo",
        "Graphic_Single",
        None,
    )
    .unwrap_err();
    assert_eq!(err.code, "project_not_found");
}

#[test]
fn source_id_used_as_project_id_returns_project_not_found() {
    let dir = tempdir().unwrap();
    let src_loc = make_location(
        "src-1",
        "Source",
        dir.path().to_str().unwrap(),
        LocationKind::Source,
    );
    let settings = make_settings(vec![src_loc], None);
    let err = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "src-1",
        "Things/Foo",
        "Graphic_Single",
        None,
    )
    .unwrap_err();
    assert_eq!(err.code, "project_not_found");
}

// -- exact file resolution --

#[test]
fn exact_lookup_finds_png_in_project_location() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things").join("Item");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Foo.png"), b"fake png").unwrap();

    let loc = make_location(
        "proj-1",
        "My Project",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));
    let c = cache();

    let result = resolve_graphic_preview_assets(
        &settings,
        &c,
        "proj-1",
        "Things/Item/Foo",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, None);
    assert!(result.variants[0]
        .relative_texture_path
        .ends_with("Foo.png"));
    assert_eq!(result.variants[0].source_location_id, "proj-1");
    assert!(result.variants[0].asset_url.starts_with("rimedit-asset://"));
    assert!(result.variants[0].asset_token.is_some());
}

#[test]
fn missing_texture_returns_placeholder_and_warning() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("Textures")).unwrap();

    let loc = make_location(
        "proj-1",
        "My Project",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Item/Missing",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, Some(true));
    assert!(result.variants[0].asset_url.is_empty());
    assert!(!result.warnings.is_empty());
}

#[test]
fn project_location_overrides_source_location() {
    let proj_dir = tempdir().unwrap();
    let src_dir = tempdir().unwrap();

    let proj_textures = proj_dir.path().join("Textures").join("Things");
    let src_textures = src_dir.path().join("Textures").join("Things");
    fs::create_dir_all(&proj_textures).unwrap();
    fs::create_dir_all(&src_textures).unwrap();
    fs::write(proj_textures.join("Foo.png"), b"project").unwrap();
    fs::write(src_textures.join("Foo.png"), b"source").unwrap();

    let proj_loc = make_location(
        "proj-1",
        "My Project",
        proj_dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let src_loc = make_location(
        "src-1",
        "Source",
        src_dir.path().to_str().unwrap(),
        LocationKind::Source,
    );
    let settings = make_settings(vec![proj_loc, src_loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(result.variants[0].source_location_id, "proj-1");
}

#[test]
fn source_location_used_as_fallback_when_project_lacks_texture() {
    let proj_dir = tempdir().unwrap();
    let src_dir = tempdir().unwrap();

    fs::create_dir_all(proj_dir.path().join("Textures")).unwrap();
    let src_textures = src_dir.path().join("Textures").join("Things");
    fs::create_dir_all(&src_textures).unwrap();
    fs::write(src_textures.join("Foo.png"), b"source").unwrap();

    let proj_loc = make_location(
        "proj-1",
        "My Project",
        proj_dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let src_loc = make_location(
        "src-1",
        "Source",
        src_dir.path().to_str().unwrap(),
        LocationKind::Source,
    );
    let settings = make_settings(vec![proj_loc, src_loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(result.variants[0].source_location_id, "src-1");
    assert_eq!(result.variants[0].missing, None);
    assert!(result.variants[0].asset_url.starts_with("rimedit-asset://"));
}

// -- Single strategy classes --

#[test]
fn single_strategy_classes_resolve_to_one_variant() {
    for class in [
        "Graphic_Single_AgeSecs",
        "Graphic_Fleck",
        "Graphic_Terrain",
        "Graphic_Gas",
        "Graphic_Tiling",
        "Graphic_ActivityMask",
    ] {
        let dir = tempdir().unwrap();
        let textures = dir.path().join("Textures").join("Things");
        fs::create_dir_all(&textures).unwrap();
        fs::write(textures.join("Foo.png"), b"data").unwrap();

        let loc = make_location(
            "proj-1",
            "P",
            dir.path().to_str().unwrap(),
            LocationKind::Project,
        );
        let settings = make_settings(vec![loc], Some("proj-1"));

        let result = resolve_graphic_preview_assets(
            &settings,
            &cache(),
            "proj-1",
            "Things/Foo",
            class,
            None,
        )
        .unwrap();

        assert_eq!(
            result.variants.len(),
            1,
            "class '{}' should return one variant",
            class
        );
        assert_eq!(result.variants[0].role, "single", "class '{}' role", class);
        assert_eq!(
            result.variants[0].missing, None,
            "class '{}' should not be missing",
            class
        );
    }
}

// -- Graphic_Multi and directional multi --

#[test]
fn graphic_multi_resolves_all_four_directions() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    for suffix in ["_north", "_east", "_south", "_west"] {
        fs::write(textures.join(format!("Foo{}.png", suffix)), b"data").unwrap();
    }

    let loc = make_location(
        "proj-1",
        "My Project",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Multi",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 4);
    assert!(result.variants.iter().all(|v| v.missing.is_none()));
    let roles: Vec<&str> = result.variants.iter().map(|v| v.role.as_str()).collect();
    assert!(roles.contains(&"north"));
    assert!(roles.contains(&"east"));
    assert!(roles.contains(&"south"));
    assert!(roles.contains(&"west"));
}

#[test]
fn graphic_multi_partial_directions_returns_missing_placeholders() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Foo_south.png"), b"s").unwrap();

    let loc = make_location(
        "proj-1",
        "My Project",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Multi",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 4);
    assert_eq!(
        result
            .variants
            .iter()
            .find(|v| v.role == "south")
            .unwrap()
            .missing,
        None
    );
    assert_eq!(
        result
            .variants
            .iter()
            .find(|v| v.role == "north")
            .unwrap()
            .missing,
        Some(true)
    );
}

#[test]
fn graphic_multi_age_secs_resolves_four_directional_variants() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    for suffix in ["_north", "_east", "_south", "_west"] {
        fs::write(textures.join(format!("Foo{}.png", suffix)), b"data").unwrap();
    }

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Multi_AgeSecs",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 4);
    assert!(result.variants.iter().all(|v| v.missing.is_none()));
}

#[test]
fn graphic_multi_building_working_with_missing_direction() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Foo_south.png"), b"s").unwrap();
    fs::write(textures.join("Foo_east.png"), b"e").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Multi_BuildingWorking",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 4);
    assert_eq!(
        result
            .variants
            .iter()
            .find(|v| v.role == "north")
            .unwrap()
            .missing,
        Some(true)
    );
    assert_eq!(
        result
            .variants
            .iter()
            .find(|v| v.role == "west")
            .unwrap()
            .missing,
        Some(true)
    );
    assert_eq!(
        result
            .variants
            .iter()
            .find(|v| v.role == "south")
            .unwrap()
            .missing,
        None
    );
    assert_eq!(
        result
            .variants
            .iter()
            .find(|v| v.role == "east")
            .unwrap()
            .missing,
        None
    );
}

#[test]
fn graphic_multi_falls_back_to_base_when_all_directions_missing() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Foo.png"), b"base").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Multi",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].role, "single");
    assert_eq!(result.variants[0].missing, None);
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("fell back to single texture")));
}

// -- folder collection --

#[test]
fn graphic_random_scans_folder_and_excludes_mask_textures() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things").join("Foo");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("A.png"), b"a").unwrap();
    fs::write(textures.join("B.png"), b"b").unwrap();
    fs::write(textures.join("B_m.png"), b"mask").unwrap();

    let loc = make_location(
        "proj-1",
        "My Project",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Random",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 2);
    assert!(result.variants.iter().all(|v| v.missing.is_none()));
    assert!(!result
        .variants
        .iter()
        .any(|v| v.relative_texture_path.contains("B_m")));
    assert!(result
        .variants
        .iter()
        .all(|v| v.asset_url.starts_with("rimedit-asset://")));
}

#[test]
fn folder_collection_excludes_mask_suffix() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things").join("Foo");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("A.png"), b"a").unwrap();
    fs::write(textures.join("A_mask.png"), b"mask").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Random",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert!(!result
        .variants
        .iter()
        .any(|v| v.relative_texture_path.contains("_mask")));
}

#[test]
fn folder_collection_empty_folder_returns_missing_placeholder() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("Textures").join("Things").join("Empty")).unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Empty",
        "Graphic_Indexed",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, Some(true));
    assert_eq!(result.variants[0].role, "variant");
    assert!(!result.warnings.is_empty());
}

#[test]
fn folder_collection_project_overrides_source_for_same_file() {
    let proj_dir = tempdir().unwrap();
    let src_dir = tempdir().unwrap();

    let proj_textures = proj_dir.path().join("Textures").join("Things").join("Foo");
    let src_textures = src_dir.path().join("Textures").join("Things").join("Foo");
    fs::create_dir_all(&proj_textures).unwrap();
    fs::create_dir_all(&src_textures).unwrap();
    fs::write(proj_textures.join("A.png"), b"project").unwrap();
    fs::write(src_textures.join("A.png"), b"source").unwrap();

    let proj_loc = make_location(
        "proj-1",
        "My Project",
        proj_dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let src_loc = make_location(
        "src-1",
        "Source",
        src_dir.path().to_str().unwrap(),
        LocationKind::Source,
    );
    let settings = make_settings(vec![proj_loc, src_loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Random",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].source_location_id, "proj-1");
}

#[test]
fn folder_collection_source_fills_files_absent_in_project() {
    let proj_dir = tempdir().unwrap();
    let src_dir = tempdir().unwrap();

    fs::create_dir_all(proj_dir.path().join("Textures").join("Things").join("Foo")).unwrap();
    let src_textures = src_dir.path().join("Textures").join("Things").join("Foo");
    fs::create_dir_all(&src_textures).unwrap();
    fs::write(src_textures.join("B.png"), b"source").unwrap();

    let proj_loc = make_location(
        "proj-1",
        "My Project",
        proj_dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let src_loc = make_location(
        "src-1",
        "Source",
        src_dir.path().to_str().unwrap(),
        LocationKind::Source,
    );
    let settings = make_settings(vec![proj_loc, src_loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Random",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].source_location_id, "src-1");
    assert_eq!(result.variants[0].missing, None);
}

#[test]
fn folder_collection_directional_files_are_grouped_with_direction_labels() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things").join("Foo");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("A_north.png"), b"an").unwrap();
    fs::write(textures.join("A_south.png"), b"as").unwrap();
    fs::write(textures.join("B.png"), b"b").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Random",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 3);
    let a_north = result
        .variants
        .iter()
        .find(|v| v.relative_texture_path.contains("A_north"))
        .expect("A_north variant");
    let a_south = result
        .variants
        .iter()
        .find(|v| v.relative_texture_path.contains("A_south"))
        .expect("A_south variant");
    let b = result
        .variants
        .iter()
        .find(|v| v.relative_texture_path.contains("B.png"))
        .expect("B variant");
    assert_eq!(a_north.label, "Variant 1 North");
    assert_eq!(a_south.label, "Variant 1 South");
    assert_eq!(a_north.role, "north");
    assert_eq!(a_south.role, "south");
    assert_eq!(b.label, "Variant 2");
    assert_eq!(b.role, "variant");
}

#[test]
fn folder_collection_excludes_directional_mask_stems() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things").join("Foo");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Foo_north.png"), b"n").unwrap();
    fs::write(textures.join("Foo_northm.png"), b"mask").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Random",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert!(!result
        .variants
        .iter()
        .any(|v| v.relative_texture_path.contains("_northm")));
    assert_eq!(result.variants[0].role, "north");
    assert_eq!(result.variants[0].label, "Variant 1 North");
}

#[test]
fn stack_count_uses_semantic_labels() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things").join("Stack");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Stack1.png"), b"one").unwrap();
    fs::write(textures.join("Stack2.png"), b"two").unwrap();
    fs::write(textures.join("Stack3.png"), b"three").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Stack",
        "Graphic_StackCount",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 3);
    let labels: Vec<&str> = result.variants.iter().map(|v| v.label.as_str()).collect();
    assert!(
        labels.contains(&"Stack 1"),
        "expected 'Stack 1', got {:?}",
        labels
    );
    assert!(
        labels.contains(&"Stack partial"),
        "expected 'Stack partial', got {:?}",
        labels
    );
    assert!(
        labels.contains(&"Stack full"),
        "expected 'Stack full', got {:?}",
        labels
    );
}

// -- Appearances --

#[test]
fn appearances_returns_candidates_with_suffix_labels() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things").join("Stuff");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Blocks_Wood.png"), b"wood").unwrap();
    fs::write(textures.join("Blocks_Metal.png"), b"metal").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Stuff/Blocks",
        "Graphic_Appearances",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 2);
    assert!(result.variants.iter().all(|v| v.role == "appearance"));
    let labels: Vec<&str> = result.variants.iter().map(|v| v.label.as_str()).collect();
    assert!(
        labels.contains(&"Wood"),
        "expected 'Wood', got {:?}",
        labels
    );
    assert!(
        labels.contains(&"Metal"),
        "expected 'Metal', got {:?}",
        labels
    );
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("StuffAppearanceDef")));
}

#[test]
fn appearances_empty_folder_returns_missing_placeholder() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("Textures").join("Things")).unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Appearances",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, Some(true));
    assert_eq!(result.variants[0].role, "appearance");
    assert_eq!(result.variants[0].label, "Appearance 1");
}

// -- Special wrapper --

#[test]
fn graphic_linked_returns_base_single_with_warning() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Wall.png"), b"data").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Wall",
        "Graphic_Linked",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, None);
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("runtime context")));
}

#[test]
fn graphic_linked_corner_overlay_returns_base_single_and_warning() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Wall.png"), b"data").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Wall",
        "Graphic_LinkedCornerOverlay",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, None);
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("runtime context")));
}

#[test]
fn graphic_random_rotated_returns_base_single_with_warning() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Foo.png"), b"data").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_RandomRotated",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, None);
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("runtime context")));
}

#[test]
fn special_wrapper_missing_texture_returns_placeholder_and_warnings() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("Textures")).unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Missing",
        "Graphic_Shadow",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, Some(true));
    assert!(!result.warnings.is_empty());
}

// -- Unknown class fallback --

#[test]
fn unknown_class_warns_and_falls_back_to_single() {
    let dir = tempdir().unwrap();
    let textures = dir.path().join("Textures").join("Things");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Foo.png"), b"data").unwrap();

    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "MyMod_CustomGraphicClass",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].role, "single");
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("Unknown graphic class")));
}

// -- path traversal rejected --

#[test]
fn invalid_traversal_path_returns_error() {
    let dir = tempdir().unwrap();
    let loc = make_location(
        "proj-1",
        "P",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let err = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/../../etc/passwd",
        "Graphic_Single",
        None,
    )
    .unwrap_err();
    assert_eq!(err.code, "invalid_texture_path");
}

// -- preview_asset_url --

#[test]
fn preview_asset_url_produces_localhost_path_url() {
    assert_eq!(preview_asset_url("abc"), "rimedit-asset://localhost/abc");
}

// -- extract_asset_token --

#[test]
fn extract_asset_token_localhost_path() {
    assert_eq!(
        extract_asset_token("localhost", "/abc"),
        Some("abc".to_owned())
    );
}

#[test]
fn extract_asset_token_scheme_localhost_path() {
    assert_eq!(
        extract_asset_token("rimedit-asset.localhost", "/abc"),
        Some("abc".to_owned())
    );
}

#[test]
fn extract_asset_token_legacy_host_token() {
    assert_eq!(
        extract_asset_token("some-uuid-token", ""),
        Some("some-uuid-token".to_owned())
    );
}

#[test]
fn extract_asset_token_empty_inputs_return_none() {
    assert_eq!(extract_asset_token("", ""), None);
    assert_eq!(extract_asset_token("localhost", "/"), None);
    assert_eq!(extract_asset_token("localhost", ""), None);
}

// -- Matter Network style exact lookup --

#[test]
fn exact_lookup_finds_matter_network_style_texture() {
    let dir = tempdir().unwrap();
    let textures = dir
        .path()
        .join("Textures")
        .join("MatterNetwork")
        .join("Things")
        .join("Building");
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Controller.png"), b"controller").unwrap();

    let loc = make_location(
        "proj-mn",
        "Matter Network",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-mn"));
    let c = cache();

    let result = resolve_graphic_preview_assets(
        &settings,
        &c,
        "proj-mn",
        "MatterNetwork/Things/Building/Controller",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, None);
    assert!(result.variants[0]
        .relative_texture_path
        .ends_with("Controller.png"));
    assert!(result.variants[0]
        .asset_url
        .starts_with("rimedit-asset://localhost/"));
    assert!(result.variants[0].asset_token.is_some());
}

// -- diagnostics: no Textures directory --

#[test]
fn no_textures_dir_emits_warning() {
    let dir = tempdir().unwrap();
    let loc = make_location(
        "proj-1",
        "My Project",
        dir.path().to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("no Textures directory")));
}

// -- version-folder root fallback --

#[test]
fn version_folder_root_finds_parent_textures() {
    let mod_root = tempdir().unwrap();
    let version_root = mod_root.path().join("1.6");
    let textures = mod_root.path().join("Textures").join("Things");
    fs::create_dir_all(&version_root).unwrap();
    fs::create_dir_all(&textures).unwrap();
    fs::write(textures.join("Foo.png"), b"data").unwrap();

    let loc = make_location(
        "proj-1",
        "My Mod 1.6",
        version_root.to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, None);
    assert!(result.variants[0]
        .relative_texture_path
        .ends_with("Foo.png"));
}

#[test]
fn non_version_folder_root_does_not_fall_back_to_parent() {
    let mod_root = tempdir().unwrap();
    let named_root = mod_root.path().join("Core");
    let parent_textures = mod_root.path().join("Textures").join("Things");
    fs::create_dir_all(&named_root).unwrap();
    fs::create_dir_all(&parent_textures).unwrap();
    fs::write(parent_textures.join("Foo.png"), b"data").unwrap();

    let loc = make_location(
        "proj-1",
        "Core",
        named_root.to_str().unwrap(),
        LocationKind::Project,
    );
    let settings = make_settings(vec![loc], Some("proj-1"));

    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "proj-1",
        "Things/Foo",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].missing, Some(true));
}

// --- Fixture-backed preview resolver tests ---

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/graphic_preview")
        .join(name)
}

fn graphic_preview_project_location() -> RegisteredLocation {
    let root = fixture_root("project_mod");
    make_location(
        "fixture-proj",
        "Project Mod",
        root.to_str().unwrap(),
        LocationKind::Project,
    )
}

fn graphic_preview_source_location() -> RegisteredLocation {
    let root = fixture_root("source_mod");
    make_location(
        "fixture-src",
        "Source Mod",
        root.to_str().unwrap(),
        LocationKind::Source,
    )
}

fn graphic_preview_fixture_settings() -> ProjectSettings {
    make_settings(
        vec![
            graphic_preview_project_location(),
            graphic_preview_source_location(),
        ],
        Some("fixture-proj"),
    )
}

#[test]
fn fixture_single_resolves_project_texture() {
    let settings = graphic_preview_fixture_settings();
    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "fixture-proj",
        "Things/Fixture/Single/FixtureSingle",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(
        result.variants.len(),
        1,
        "single strategy must return exactly one variant"
    );
    assert_eq!(result.variants[0].role, "single");
    assert_eq!(
        result.variants[0].missing, None,
        "project texture must not be missing"
    );
    assert_eq!(
        result.variants[0].source_location_id, "fixture-proj",
        "project texture must come from project location"
    );
}

#[test]
fn fixture_multi_resolves_four_directional_textures() {
    let settings = graphic_preview_fixture_settings();
    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "fixture-proj",
        "Things/Fixture/Multi/FixtureMulti",
        "Graphic_Multi",
        None,
    )
    .unwrap();

    assert_eq!(
        result.variants.len(),
        4,
        "multi strategy must return four directional variants"
    );
    let roles: Vec<&str> = result.variants.iter().map(|v| v.role.as_str()).collect();
    for dir in &["north", "east", "south", "west"] {
        assert!(roles.contains(dir), "expected role '{dir}' in {:?}", roles);
    }
    for v in &result.variants {
        assert_eq!(v.missing, None, "all directional textures must be present");
    }
}

#[test]
fn fixture_random_resolves_folder_variants_and_excludes_masks() {
    let settings = graphic_preview_fixture_settings();
    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "fixture-proj",
        "Things/Fixture/Random",
        "Graphic_Random",
        None,
    )
    .unwrap();

    let paths: Vec<&str> = result
        .variants
        .iter()
        .map(|v| v.relative_texture_path.as_str())
        .collect();
    assert!(
        paths.iter().any(|p| p.contains("VariantA.png")),
        "VariantA must be present: {:?}",
        paths
    );
    assert!(
        paths.iter().any(|p| p.contains("VariantB.png")),
        "VariantB must be present: {:?}",
        paths
    );
    assert!(
        paths.iter().any(|p| p.contains("SourceOnly.png")),
        "SourceOnly must be included from source: {:?}",
        paths
    );
    assert!(
        !paths.iter().any(|p| p.contains("VariantA_m")),
        "mask texture VariantA_m must be excluded: {:?}",
        paths
    );
    for v in &result.variants {
        assert_eq!(v.missing, None, "all fixture variants must be present");
    }
}

#[test]
fn fixture_stack_count_uses_semantic_labels() {
    let settings = graphic_preview_fixture_settings();
    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "fixture-proj",
        "Things/Fixture/Stack",
        "Graphic_StackCount",
        None,
    )
    .unwrap();

    assert_eq!(
        result.variants.len(),
        3,
        "stack strategy must return three variants"
    );
    let labels: Vec<&str> = result.variants.iter().map(|v| v.label.as_str()).collect();
    assert!(
        labels.contains(&"Stack 1"),
        "expected 'Stack 1' in {:?}",
        labels
    );
    assert!(
        labels.contains(&"Stack partial"),
        "expected 'Stack partial' in {:?}",
        labels
    );
    assert!(
        labels.contains(&"Stack full"),
        "expected 'Stack full' in {:?}",
        labels
    );
}

#[test]
fn fixture_missing_texture_returns_missing_placeholder() {
    let settings = graphic_preview_fixture_settings();
    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "fixture-proj",
        "Things/Fixture/Missing/MissingThing",
        "Graphic_Single",
        None,
    )
    .unwrap();

    assert_eq!(
        result.variants.len(),
        1,
        "missing texture must still return one placeholder"
    );
    assert_eq!(
        result.variants[0].missing,
        Some(true),
        "placeholder must be marked missing"
    );
    assert!(
        !result.warnings.is_empty(),
        "missing texture must produce warnings"
    );
}

#[test]
fn fixture_unknown_class_warns_and_falls_back_to_single() {
    let settings = graphic_preview_fixture_settings();
    let result = resolve_graphic_preview_assets(
        &settings,
        &cache(),
        "fixture-proj",
        "Things/Fixture/Single/FixtureSingle",
        "MyMod_CustomGraphic",
        None,
    )
    .unwrap();

    assert_eq!(result.variants.len(), 1);
    assert_eq!(result.variants[0].role, "single");
    assert_eq!(
        result.variants[0].missing, None,
        "FixtureSingle.png must resolve via fallback"
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Unknown graphic class")),
        "unknown class must produce a warning: {:?}",
        result.warnings
    );
}
