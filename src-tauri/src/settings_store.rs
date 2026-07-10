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
fn migrate_settings_json(raw: &str) -> Result<String, serde_json::Error> {
    let mut value: serde_json::Value = serde_json::from_str(raw)?;

    if let Some(obj) = value.as_object_mut() {
        let schema_version = obj
            .get("schemaVersion")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        if schema_version < 2 {
            obj.insert("schemaVersion".into(), serde_json::json!(2));
        }
        // Always insert a default gameVersion if it is absent, regardless of schemaVersion,
        // so partially-written or externally-edited files don't fail deserialization.
        if !obj.contains_key("gameVersion") {
            obj.insert("gameVersion".into(), serde_json::json!("1.6"));
        }

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
        assert_eq!(migrated["schemaVersion"], 2);
        assert_eq!(migrated["gameVersion"], "1.6");
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
        assert_eq!(migrated["schemaVersion"], 2);
    }

    #[test]
    fn migration_leaves_non_expansion_types_alone() {
        let v1 = r#"{"schemaVersion":1,"locations":[{"sourceType":"baseGame"}]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&migrate_settings_json(v1).unwrap()).unwrap();
        assert_eq!(migrated["locations"][0]["sourceType"], "baseGame");
    }
}
