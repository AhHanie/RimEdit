use crate::locale::validate_locale;
use crate::project_model::{
    validate_project_game_version, AppError, LocationKind, MissingActiveProjectNotice,
    ProjectSettings, RegisteredLocation, RegisteredLocationDraft, RegisteredLocationUpdate,
    StoreError,
};
use std::path::Path;
use time::OffsetDateTime;
use uuid::Uuid;

/// Check the persisted active project on startup, deactivating it if its
/// registered location is missing or its folder no longer exists on disk.
///
/// Returns `Some(notice)` only when a project location was found but its
/// `rootPath` is gone, so the frontend can surface a "folder not found"
/// message. An active id that doesn't resolve to a registered project
/// location at all is cleared defensively without producing a notice.
pub(crate) fn deactivate_missing_active_project(
    settings: &mut ProjectSettings,
) -> Option<MissingActiveProjectNotice> {
    let active_id = settings.active_project_id.clone()?;

    let location = settings
        .locations
        .iter()
        .find(|l| l.id == active_id && l.kind == LocationKind::Project);

    let Some(location) = location else {
        settings.active_project_id = None;
        return None;
    };

    let path = Path::new(&location.root_path);
    if path.exists() && path.is_dir() {
        return None;
    }

    let notice = MissingActiveProjectNotice {
        id: location.id.clone(),
        display_name: location.display_name.clone(),
        root_path: location.root_path.clone(),
    };
    settings.active_project_id = None;
    Some(notice)
}

pub(crate) fn upsert_location(
    settings: &mut ProjectSettings,
    draft: RegisteredLocationDraft,
) -> Result<bool, AppError> {
    let raw_path = Path::new(&draft.root_path);
    if !raw_path.exists() {
        return Err(
            StoreError::InvalidPath(format!("Path does not exist: {}", draft.root_path)).into(),
        );
    }
    if !raw_path.is_dir() {
        return Err(StoreError::InvalidPath(format!(
            "Path is not a directory: {}",
            draft.root_path
        ))
        .into());
    }
    let canonical = raw_path.canonicalize().map_err(|e| {
        StoreError::InvalidPath(format!("Cannot canonicalize {}: {}", draft.root_path, e))
    })?;
    let canonical_str = canonical.to_string_lossy().to_string();

    if settings.has_duplicate_path(&canonical_str) {
        return Ok(false);
    }

    let read_only = RegisteredLocationDraft::read_only_for_kind(&draft.kind);
    let now = OffsetDateTime::now_utc();
    let entry = RegisteredLocation {
        id: Uuid::new_v4().to_string(),
        display_name: draft.display_name,
        root_path: canonical_str,
        kind: draft.kind,
        source_type: draft.source_type,
        read_only,
        mod_id: draft.mod_id,
        game_version: draft.game_version,
        expansion_name: None,
        created_at: now,
        updated_at: now,
    };

    settings.locations.push(entry);
    Ok(true)
}

pub(crate) fn remove_location(settings: &mut ProjectSettings, id: &str) -> Result<(), AppError> {
    let before = settings.locations.len();
    settings.locations.retain(|l| l.id != id);
    if settings.locations.len() == before {
        return Err(StoreError::NotFound(id.to_string()).into());
    }
    if settings.active_project_id.as_deref() == Some(id) {
        settings.active_project_id = None;
    }
    Ok(())
}

pub(crate) fn set_active_project(
    settings: &mut ProjectSettings,
    id: Option<String>,
) -> Result<(), AppError> {
    if let Some(ref target_id) = id {
        let found = settings
            .locations
            .iter()
            .any(|l| &l.id == target_id && l.kind == LocationKind::Project);
        if !found {
            return Err(StoreError::NotFound(target_id.clone()).into());
        }
    }
    settings.active_project_id = id;
    Ok(())
}

pub(crate) fn update_location(
    settings: &mut ProjectSettings,
    update: RegisteredLocationUpdate,
) -> Result<(), AppError> {
    let location = settings
        .locations
        .iter_mut()
        .find(|l| l.id == update.id)
        .ok_or_else(|| StoreError::NotFound(update.id.clone()))?;
    let trimmed = update.display_name.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError {
            code: "invalid_display_name".to_string(),
            message: "Display name must not be blank.".to_string(),
            details: None,
            args: crate::diagnostics::DiagnosticArgs::new(),
        });
    }
    location.display_name = trimmed;
    location.source_type = update.source_type;
    location.mod_id = update.mod_id;
    location.game_version = update.game_version;
    location.updated_at = OffsetDateTime::now_utc();
    Ok(())
}

pub(crate) fn update_project_game_version(
    settings: &mut ProjectSettings,
    game_version: String,
    installed_versions: &[String],
) -> Result<(), AppError> {
    validate_project_game_version(&game_version)?;
    if !installed_versions.is_empty() && !installed_versions.iter().any(|v| v == &game_version) {
        return Err(StoreError::GameVersionSchemaUnavailable(game_version).into());
    }
    settings.game_version = game_version;
    Ok(())
}

pub(crate) fn update_app_locale(
    settings: &mut ProjectSettings,
    locale: String,
) -> Result<(), AppError> {
    validate_locale(&locale)?;
    settings.locale = locale;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_model::SourceType;

    fn make_location(kind: LocationKind, id: &str) -> RegisteredLocation {
        let read_only = RegisteredLocationDraft::read_only_for_kind(&kind);
        RegisteredLocation {
            id: id.to_string(),
            display_name: "Test".to_string(),
            root_path: "/some/path".to_string(),
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

    fn make_settings_with_version(game_version: &str) -> ProjectSettings {
        ProjectSettings {
            schema_version: 3,
            game_version: game_version.to_string(),
            locale: "en".to_string(),
            locations: vec![],
            active_project_id: None,
        }
    }

    #[test]
    fn removing_active_project_clears_active_project_id() {
        let id = "test-id".to_string();
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![make_location(LocationKind::Project, &id)],
            active_project_id: Some(id.clone()),
        };
        remove_location(&mut settings, &id).unwrap();
        assert!(settings.active_project_id.is_none());
        assert!(settings.locations.is_empty());
    }

    #[test]
    fn update_location_changes_display_name_and_updated_at() {
        let id = "loc1".to_string();
        let created = OffsetDateTime::now_utc();
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![RegisteredLocation {
                id: id.clone(),
                display_name: "Old Name".to_string(),
                root_path: "/some/path".to_string(),
                kind: LocationKind::Source,
                source_type: SourceType::Folder,
                read_only: true,
                mod_id: None,
                game_version: None,
                expansion_name: None,
                created_at: created,
                updated_at: created,
            }],
            active_project_id: None,
        };
        std::thread::sleep(std::time::Duration::from_millis(1));
        update_location(
            &mut settings,
            RegisteredLocationUpdate {
                id: id.clone(),
                display_name: "New Name".to_string(),
                source_type: SourceType::LocalMod,
                mod_id: None,
                game_version: None,
            },
        )
        .unwrap();
        let location = settings.locations.iter().find(|l| l.id == id).unwrap();
        assert_eq!(location.display_name, "New Name");
        assert_eq!(location.source_type, SourceType::LocalMod);
        assert!(location.updated_at > created);
        assert_eq!(location.created_at, created);
        assert_eq!(location.root_path, "/some/path");
    }

    #[test]
    fn update_location_rejects_blank_display_name() {
        let id = "loc1".to_string();
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![make_location(LocationKind::Source, &id)],
            active_project_id: None,
        };
        let result = update_location(
            &mut settings,
            RegisteredLocationUpdate {
                id,
                display_name: "   ".to_string(),
                source_type: SourceType::Folder,
                mod_id: None,
                game_version: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn update_location_missing_id_returns_not_found() {
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![make_location(LocationKind::Project, "existing")],
            active_project_id: None,
        };
        let result = update_location(
            &mut settings,
            RegisteredLocationUpdate {
                id: "nonexistent".to_string(),
                display_name: "Name".to_string(),
                source_type: SourceType::Folder,
                mod_id: None,
                game_version: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn update_location_preserves_immutable_fields() {
        let id = "loc1".to_string();
        let created = OffsetDateTime::now_utc();
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![RegisteredLocation {
                id: id.clone(),
                display_name: "Old".to_string(),
                root_path: "/original/path".to_string(),
                kind: LocationKind::Project,
                source_type: SourceType::Folder,
                read_only: false,
                mod_id: None,
                game_version: None,
                expansion_name: None,
                created_at: created,
                updated_at: created,
            }],
            active_project_id: None,
        };
        update_location(
            &mut settings,
            RegisteredLocationUpdate {
                id: id.clone(),
                display_name: "New".to_string(),
                source_type: SourceType::BaseGame,
                mod_id: None,
                game_version: None,
            },
        )
        .unwrap();
        let loc = settings.locations.iter().find(|l| l.id == id).unwrap();
        assert_eq!(loc.root_path, "/original/path");
        assert_eq!(loc.kind, LocationKind::Project);
        assert!(!loc.read_only);
        assert_eq!(loc.created_at, created);
    }

    #[test]
    fn update_project_game_version_accepts_valid() {
        let mut settings = make_settings_with_version("1.6");
        super::update_project_game_version(
            &mut settings,
            "1.5".to_string(),
            &["1.5".to_string(), "1.6".to_string()],
        )
        .unwrap();
        assert_eq!(settings.game_version, "1.5");
    }

    #[test]
    fn update_project_game_version_rejects_build_number() {
        let mut settings = make_settings_with_version("1.6");
        let result = super::update_project_game_version(&mut settings, "1.6.1234".to_string(), &[]);
        assert!(result.is_err());
    }

    #[test]
    fn update_project_game_version_rejects_unavailable_schema_version() {
        let mut settings = make_settings_with_version("1.6");
        let result = super::update_project_game_version(
            &mut settings,
            "2.0".to_string(),
            &["1.5".to_string(), "1.6".to_string()],
        );
        assert!(result.is_err());
    }

    #[test]
    fn update_project_game_version_allows_any_version_when_no_schema_versions() {
        let mut settings = make_settings_with_version("1.6");
        super::update_project_game_version(&mut settings, "2.0".to_string(), &[]).unwrap();
        assert_eq!(settings.game_version, "2.0");
    }

    #[test]
    fn update_app_locale_accepts_supported_locale() {
        let mut settings = make_settings_with_version("1.6");
        super::update_app_locale(&mut settings, "en".to_string()).unwrap();
        assert_eq!(settings.locale, "en");
    }

    #[test]
    fn update_app_locale_rejects_unsupported_locale() {
        let mut settings = make_settings_with_version("1.6");
        let before = settings.locale.clone();
        let result = super::update_app_locale(&mut settings, "fr".to_string());
        assert!(result.is_err());
        assert_eq!(settings.locale, before);
    }

    #[test]
    fn deactivate_missing_active_project_keeps_existing_directory_active() {
        let dir = tempfile::tempdir().unwrap();
        let id = "proj1".to_string();
        let mut location = make_location(LocationKind::Project, &id);
        location.root_path = dir.path().to_string_lossy().to_string();
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![location],
            active_project_id: Some(id.clone()),
        };
        let notice = super::deactivate_missing_active_project(&mut settings);
        assert!(notice.is_none());
        assert_eq!(settings.active_project_id, Some(id));
    }

    #[test]
    fn deactivate_missing_active_project_clears_missing_directory_and_returns_notice() {
        let dir = tempfile::tempdir().unwrap();
        let missing_path = dir.path().join("does-not-exist");
        let id = "proj1".to_string();
        let mut location = make_location(LocationKind::Project, &id);
        location.display_name = "My Mod".to_string();
        location.root_path = missing_path.to_string_lossy().to_string();
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![location],
            active_project_id: Some(id.clone()),
        };
        let notice = super::deactivate_missing_active_project(&mut settings).unwrap();
        assert!(settings.active_project_id.is_none());
        assert_eq!(notice.id, id);
        assert_eq!(notice.display_name, "My Mod");
        assert_eq!(notice.root_path, missing_path.to_string_lossy().to_string());
    }

    #[test]
    fn deactivate_missing_active_project_clears_unknown_id_without_notice() {
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![],
            active_project_id: Some("stale-id".to_string()),
        };
        let notice = super::deactivate_missing_active_project(&mut settings);
        assert!(notice.is_none());
        assert!(settings.active_project_id.is_none());
    }

    #[test]
    fn deactivate_missing_active_project_clears_id_pointing_at_source_location() {
        let id = "source1".to_string();
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![make_location(LocationKind::Source, &id)],
            active_project_id: Some(id),
        };
        let notice = super::deactivate_missing_active_project(&mut settings);
        assert!(notice.is_none());
        assert!(settings.active_project_id.is_none());
    }

    #[test]
    fn deactivate_missing_active_project_does_nothing_when_no_active_id() {
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![make_location(LocationKind::Project, "proj1")],
            active_project_id: None,
        };
        let notice = super::deactivate_missing_active_project(&mut settings);
        assert!(notice.is_none());
        assert!(settings.active_project_id.is_none());
    }

    #[test]
    fn deactivate_missing_active_project_ignores_missing_inactive_project() {
        let active_id = "active".to_string();
        let dir = tempfile::tempdir().unwrap();
        let mut active_location = make_location(LocationKind::Project, &active_id);
        active_location.root_path = dir.path().to_string_lossy().to_string();
        let mut inactive_location = make_location(LocationKind::Project, "inactive");
        inactive_location.root_path = "/never/created/path".to_string();
        let mut settings = ProjectSettings {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: "en".to_string(),
            locations: vec![active_location, inactive_location],
            active_project_id: Some(active_id.clone()),
        };
        let notice = super::deactivate_missing_active_project(&mut settings);
        assert!(notice.is_none());
        assert_eq!(settings.active_project_id, Some(active_id));
    }
}
