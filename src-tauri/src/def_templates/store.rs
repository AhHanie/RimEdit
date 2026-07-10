use super::error::DefTemplateError;
use super::model::{
    NewUserDefTemplate, UserDefTemplate, UserDefTemplateStore, CURRENT_SCHEMA_VERSION,
};
use crate::project_model::AppError;
use crate::services::app_paths;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use time::OffsetDateTime;

/// Root directory for all project-scoped template stores, i.e. `{app storage}/templates`.
fn templates_root(app: &AppHandle) -> Result<PathBuf, AppError> {
    app_paths::app_storage_dir(app, "def_template_path_failed").map(|d| d.join("templates"))
}

fn store_file_path(templates_root: &Path, project_id: &str) -> PathBuf {
    templates_root
        .join("projects")
        .join(project_id)
        .join("templates.json")
}

fn read_store(path: &Path, project_id: &str) -> Result<UserDefTemplateStore, DefTemplateError> {
    if !path.exists() {
        return Ok(UserDefTemplateStore::empty(project_id));
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| DefTemplateError::ReadFailed(format!("{}: {}", path.display(), e)))?;
    serde_json::from_str::<UserDefTemplateStore>(&raw).map_err(|e| {
        DefTemplateError::ReadFailed(format!("JSON parse error in {}: {}", path.display(), e))
    })
}

fn write_store(path: &Path, store: &UserDefTemplateStore) -> Result<(), DefTemplateError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DefTemplateError::WriteFailed(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| DefTemplateError::WriteFailed(e.to_string()))?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)
        .map_err(|e| DefTemplateError::WriteFailed(format!("{}: {}", tmp_path.display(), e)))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|e| DefTemplateError::WriteFailed(format!("rename failed: {}", e)))?;
    Ok(())
}

fn list_templates_in(
    templates_root: &Path,
    project_id: &str,
    def_type: &str,
) -> Result<Vec<UserDefTemplate>, DefTemplateError> {
    let path = store_file_path(templates_root, project_id);
    let store = read_store(&path, project_id)?;
    Ok(store
        .templates
        .into_iter()
        .filter(|t| t.def_type == def_type)
        .collect())
}

fn get_template_in(
    templates_root: &Path,
    project_id: &str,
    template_id: &str,
) -> Result<UserDefTemplate, DefTemplateError> {
    let path = store_file_path(templates_root, project_id);
    let store = read_store(&path, project_id)?;
    store
        .templates
        .into_iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| DefTemplateError::TemplateNotFound(template_id.to_string()))
}

fn save_template_in(
    templates_root: &Path,
    project_id: &str,
    new_template: NewUserDefTemplate,
) -> Result<UserDefTemplate, DefTemplateError> {
    let path = store_file_path(templates_root, project_id);
    let mut store = read_store(&path, project_id)?;

    let now = OffsetDateTime::now_utc();
    let template = UserDefTemplate {
        id: uuid::Uuid::new_v4().to_string(),
        def_type: new_template.def_type,
        name: new_template.name,
        description: new_template.description,
        xml: new_template.xml,
        original_def_name: new_template.original_def_name,
        original_label: new_template.original_label,
        source_relative_path: new_template.source_relative_path,
        game_version: new_template.game_version,
        created_at: now,
        updated_at: now,
    };

    store.schema_version = CURRENT_SCHEMA_VERSION;
    store.project_id = project_id.to_string();
    store.templates.push(template.clone());
    write_store(&path, &store)?;
    Ok(template)
}

fn delete_template_in(
    templates_root: &Path,
    project_id: &str,
    template_id: &str,
) -> Result<(), DefTemplateError> {
    let path = store_file_path(templates_root, project_id);
    let mut store = read_store(&path, project_id)?;

    let original_len = store.templates.len();
    store.templates.retain(|t| t.id != template_id);
    if store.templates.len() == original_len {
        return Err(DefTemplateError::TemplateNotFound(template_id.to_string()));
    }

    write_store(&path, &store)?;
    Ok(())
}

pub fn list_templates(
    app: &AppHandle,
    project_id: &str,
    def_type: &str,
) -> Result<Vec<UserDefTemplate>, AppError> {
    let root = templates_root(app)?;
    list_templates_in(&root, project_id, def_type).map_err(Into::into)
}

pub fn get_template(
    app: &AppHandle,
    project_id: &str,
    template_id: &str,
) -> Result<UserDefTemplate, AppError> {
    let root = templates_root(app)?;
    get_template_in(&root, project_id, template_id).map_err(Into::into)
}

pub fn save_template(
    app: &AppHandle,
    project_id: &str,
    new_template: NewUserDefTemplate,
) -> Result<UserDefTemplate, AppError> {
    let root = templates_root(app)?;
    save_template_in(&root, project_id, new_template).map_err(Into::into)
}

pub fn delete_template(
    app: &AppHandle,
    project_id: &str,
    template_id: &str,
) -> Result<(), AppError> {
    let root = templates_root(app)?;
    delete_template_in(&root, project_id, template_id).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rimedit_def_templates_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_template(def_type: &str, name: &str) -> NewUserDefTemplate {
        NewUserDefTemplate {
            def_type: def_type.to_string(),
            name: name.to_string(),
            description: None,
            xml: format!("<{0}><defName>Sample</defName></{0}>", def_type),
            original_def_name: Some("Sample".to_string()),
            original_label: Some("sample".to_string()),
            source_relative_path: Some("Defs/Sample.xml".to_string()),
            game_version: Some("1.6".to_string()),
        }
    }

    #[test]
    fn listing_for_a_missing_store_returns_empty() {
        let root = temp_dir();
        let result = list_templates_in(&root, "proj1", "ThingDef").unwrap();
        assert!(result.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn save_then_list_round_trips() {
        let root = temp_dir();
        let saved =
            save_template_in(&root, "proj1", sample_template("ThingDef", "Weapon base")).unwrap();

        let listed = list_templates_in(&root, "proj1", "ThingDef").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, saved.id);
        assert_eq!(listed[0].name, "Weapon base");
        assert_eq!(listed[0].xml, saved.xml);
        assert_eq!(listed[0].original_def_name.as_deref(), Some("Sample"));

        // The store file itself is pretty-printed JSON under the expected path.
        let path = store_file_path(&root, "proj1");
        assert!(path.exists());
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\n"), "expected pretty-printed JSON");
        assert!(raw.contains("\"schemaVersion\""));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn listing_filters_by_exact_def_type() {
        let root = temp_dir();
        save_template_in(&root, "proj1", sample_template("ThingDef", "Weapon base")).unwrap();
        save_template_in(&root, "proj1", sample_template("PawnKindDef", "Pawn base")).unwrap();

        let thing_defs = list_templates_in(&root, "proj1", "ThingDef").unwrap();
        assert_eq!(thing_defs.len(), 1);
        assert_eq!(thing_defs[0].def_type, "ThingDef");

        let none = list_templates_in(&root, "proj1", "RecipeDef").unwrap();
        assert!(none.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn templates_are_scoped_by_project() {
        let root = temp_dir();
        save_template_in(
            &root,
            "proj1",
            sample_template("ThingDef", "Proj1 template"),
        )
        .unwrap();
        save_template_in(
            &root,
            "proj2",
            sample_template("ThingDef", "Proj2 template"),
        )
        .unwrap();

        let proj1 = list_templates_in(&root, "proj1", "ThingDef").unwrap();
        let proj2 = list_templates_in(&root, "proj2", "ThingDef").unwrap();
        assert_eq!(proj1.len(), 1);
        assert_eq!(proj2.len(), 1);
        assert_eq!(proj1[0].name, "Proj1 template");
        assert_eq!(proj2[0].name, "Proj2 template");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_removes_only_the_matching_template_in_the_matching_project() {
        let root = temp_dir();
        let keep =
            save_template_in(&root, "proj1", sample_template("ThingDef", "Keep me")).unwrap();
        let doomed =
            save_template_in(&root, "proj1", sample_template("ThingDef", "Delete me")).unwrap();
        let other_project =
            save_template_in(&root, "proj2", sample_template("ThingDef", "Untouched")).unwrap();

        delete_template_in(&root, "proj1", &doomed.id).unwrap();

        let remaining = list_templates_in(&root, "proj1", "ThingDef").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, keep.id);

        let other = list_templates_in(&root, "proj2", "ThingDef").unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].id, other_project.id);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_unknown_template_id_errors() {
        let root = temp_dir();
        save_template_in(&root, "proj1", sample_template("ThingDef", "Keep me")).unwrap();

        let err = delete_template_in(&root, "proj1", "does-not-exist").unwrap_err();
        assert!(matches!(err, DefTemplateError::TemplateNotFound(_)));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn get_template_returns_the_matching_record() {
        let root = temp_dir();
        let saved =
            save_template_in(&root, "proj1", sample_template("ThingDef", "Weapon base")).unwrap();

        let fetched = get_template_in(&root, "proj1", &saved.id).unwrap();
        assert_eq!(fetched.id, saved.id);
        assert_eq!(fetched.xml, saved.xml);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn malformed_json_returns_a_read_error() {
        let root = temp_dir();
        let path = store_file_path(&root, "proj1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not valid json").unwrap();

        let err = list_templates_in(&root, "proj1", "ThingDef").unwrap_err();
        assert!(matches!(err, DefTemplateError::ReadFailed(_)));

        std::fs::remove_dir_all(&root).ok();
    }
}
