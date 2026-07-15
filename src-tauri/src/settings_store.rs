use crate::locale::resolve_locale;
use crate::project_model::{AppError, ProjectSettings, StoreError};
use crate::services::app_paths;
use std::path::PathBuf;
use tauri::AppHandle;

fn settings_path(
    app: &AppHandle,
    err: impl Fn(String) -> StoreError,
) -> Result<PathBuf, StoreError> {
    app_paths::app_storage_dir(app, "settings_path_failed")
        .map(|d| d.join("settings.json"))
        .map_err(|e| err(e.message))
}

/// Migrate a raw settings JSON string from any schema version to the current format.
///
/// - v1 → v2: adds `gameVersion: "1.6"` if absent; converts `expansion` source
///   types to `folder` and removes `expansionName`.
/// - v2 → v3: adds `locale: "en"` if absent.
fn migrate_settings_json(raw: &str) -> Result<String, serde_json::Error> {
    let mut value: serde_json::Value = serde_json::from_str(raw)?;

    if let Some(obj) = value.as_object_mut() {
        let schema_version = obj
            .get("schemaVersion")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        if schema_version < 3 {
            obj.insert("schemaVersion".into(), serde_json::json!(3));
        }
        // Always insert a default gameVersion if it is absent, regardless of schemaVersion,
        // so partially-written or externally-edited files don't fail deserialization.
        if !obj.contains_key("gameVersion") {
            obj.insert("gameVersion".into(), serde_json::json!("1.6"));
        }
        // Always normalize locale, regardless of schemaVersion: absent (legacy/partial
        // files) becomes the fallback locale, and any unsupported/unknown persisted
        // value (e.g. a downgrade from a future build, or hand-edited settings.json)
        // is normalized rather than rejected.
        let locale = obj.get("locale").and_then(|v| v.as_str()).unwrap_or("");
        obj.insert("locale".into(), serde_json::json!(resolve_locale(locale)));

        // Migrate expansion → folder source type records.
        if let Some(locations) = obj.get_mut("locations").and_then(|v| v.as_array_mut()) {
            for loc in locations.iter_mut() {
                if let Some(loc_obj) = loc.as_object_mut() {
                    if loc_obj.get("sourceType").and_then(|v| v.as_str()) == Some("expansion") {
                        loc_obj.insert("sourceType".into(), serde_json::json!("folder"));
                        loc_obj.remove("expansionName");
                    }
                }
            }
        }
    }

    serde_json::to_string(&value)
}

pub fn load_settings(app: &AppHandle) -> Result<ProjectSettings, AppError> {
    let path = settings_path(app, StoreError::ReadFailed).map_err(AppError::from)?;
    if !path.exists() {
        return Ok(ProjectSettings::default());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| StoreError::ReadFailed(format!("{}: {}", path.display(), e)))
        .map_err(AppError::from)?;

    let migrated = migrate_settings_json(&raw)
        .map_err(|e| {
            StoreError::ReadFailed(format!(
                "Settings migration error in {}: {}",
                path.display(),
                e
            ))
        })
        .map_err(AppError::from)?;

    serde_json::from_str::<ProjectSettings>(&migrated)
        .map_err(|e| {
            StoreError::ReadFailed(format!("JSON parse error in {}: {}", path.display(), e))
        })
        .map_err(AppError::from)
}

pub fn save_settings(app: &AppHandle, settings: &ProjectSettings) -> Result<(), AppError> {
    let path = settings_path(app, StoreError::WriteFailed).map_err(AppError::from)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| StoreError::WriteFailed(e.to_string()))
            .map_err(AppError::from)?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| StoreError::WriteFailed(e.to_string()))
        .map_err(AppError::from)?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)
        .map_err(|e| StoreError::WriteFailed(format!("{}: {}", tmp_path.display(), e)))
        .map_err(AppError::from)?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|e| StoreError::WriteFailed(format!("rename failed: {}", e)))
        .map_err(AppError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::migrate_settings_json;

    #[test]
    fn migration_adds_game_version_to_v1_settings() {
        let v1 = r#"{"schemaVersion":1,"locations":[],"activeProjectId":null}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v1).unwrap()).unwrap();
        assert_eq!(migrated["schemaVersion"], 3);
        assert_eq!(migrated["gameVersion"], "1.6");
        assert_eq!(migrated["locale"], "en");
    }

    #[test]
    fn migration_preserves_existing_game_version() {
        let v1 = r#"{"schemaVersion":2,"gameVersion":"1.5","locations":[]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v1).unwrap()).unwrap();
        assert_eq!(migrated["gameVersion"], "1.5");
    }

    #[test]
    fn migration_converts_expansion_to_folder() {
        let v1 = r#"{"schemaVersion":1,"locations":[{"sourceType":"expansion","expansionName":"Royalty"}]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v1).unwrap()).unwrap();
        let loc = &migrated["locations"][0];
        assert_eq!(loc["sourceType"], "folder");
        assert!(loc.get("expansionName").is_none() || loc["expansionName"].is_null());
    }

    #[test]
    fn migration_inserts_game_version_even_at_schema_v2() {
        let v2_no_version = r#"{"schemaVersion":2,"locations":[]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v2_no_version).unwrap()).unwrap();
        assert_eq!(migrated["gameVersion"], "1.6");
        assert_eq!(migrated["schemaVersion"], 3);
    }

    #[test]
    fn migration_adds_locale_to_v2_settings() {
        let v2 = r#"{"schemaVersion":2,"gameVersion":"1.6","locations":[]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v2).unwrap()).unwrap();
        assert_eq!(migrated["schemaVersion"], 3);
        assert_eq!(migrated["locale"], "en");
    }

    #[test]
    fn migration_preserves_supported_locale() {
        let v3 = r#"{"schemaVersion":3,"gameVersion":"1.6","locale":"en","locations":[]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v3).unwrap()).unwrap();
        assert_eq!(migrated["locale"], "en");
    }

    #[test]
    fn migration_normalizes_unsupported_persisted_locale_to_fallback() {
        let v3 = r#"{"schemaVersion":3,"gameVersion":"1.6","locale":"fr","locations":[]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v3).unwrap()).unwrap();
        assert_eq!(migrated["locale"], "en");
    }

    #[test]
    fn migration_leaves_non_expansion_types_alone() {
        let v1 = r#"{"schemaVersion":1,"locations":[{"sourceType":"baseGame"}]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v1).unwrap()).unwrap();
        assert_eq!(migrated["locations"][0]["sourceType"], "baseGame");
    }
}
