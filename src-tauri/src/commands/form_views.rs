use crate::form_views::{
    self, BaseSchemaViewReference, CustomFormView, CustomFormViewUpdate, FormViewOrigin,
    FormViewStoreWarning, FormViewTarget, LastSelectedFormView, NewCustomFormView,
    SelectedFormViewRef,
};
use crate::project_model::{AppError, LocationKind, ProjectSettings};
use crate::settings_store::load_settings;
use serde::Serialize;
use tauri::AppHandle;

fn app_error(code: &str, message: impl Into<String>) -> AppError {
    AppError {
        code: code.to_string(),
        message: message.into(),
        details: None,
    }
}

/// Verify `project_id` refers to a registered, writable project location. Mirrors
/// `commands::def_templates::require_writable_project` exactly: every mutating Form View
/// command is scoped to (and can mutate) a project's custom-view store, so each one must
/// reject unknown/read-only/non-project ids rather than trusting whatever id the caller
/// passes through to the store.
fn require_writable_project(settings: &ProjectSettings, project_id: &str) -> Result<(), AppError> {
    let location = settings
        .locations
        .iter()
        .find(|l| l.id == project_id)
        .ok_or_else(|| {
            app_error(
                "form_view_invalid_target",
                format!("No project with id '{}'.", project_id),
            )
        })?;
    if location.read_only || location.kind != LocationKind::Project {
        return Err(app_error(
            "form_view_invalid_target",
            "Target location is read-only or is not a project.",
        ));
    }
    Ok(())
}

/// Verify `project_id` refers to *some* registered location -- project or source, writable or
/// read-only. This is the minimal check for the read-only commands below (`list_custom_form_views`,
/// `get_last_selected_form_view`): they must still reject an arbitrary/nonexistent id rather than
/// silently reading (or, worse, creating an empty store file for) a project that was never
/// registered, but Plan.md section 6 explicitly does not require them to be *writable* --
/// "Read-only source tabs can select project custom views but cannot save one." Mirrors the same
/// "is this a real registered location" check already used by
/// `commands::def_index::read_indexed_def_xml` (there is no existing shared helper for it; both
/// call sites inline the same `locations.iter().any(...)` check).
fn require_registered_project(
    settings: &ProjectSettings,
    project_id: &str,
) -> Result<(), AppError> {
    if !settings.locations.iter().any(|l| l.id == project_id) {
        return Err(app_error(
            "form_view_invalid_target",
            format!("No registered project with id '{}'.", project_id),
        ));
    }
    Ok(())
}

fn parse_origin(origin: &str) -> Result<FormViewOrigin, AppError> {
    match origin {
        "default" => Ok(FormViewOrigin::Default),
        "schema" => Ok(FormViewOrigin::Schema),
        "custom" => Ok(FormViewOrigin::Custom),
        other => Err(app_error(
            "form_view_invalid_origin",
            format!("Unknown Form View origin '{}'.", other),
        )),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCustomFormViewsResult {
    pub views: Vec<CustomFormView>,
    pub warning: Option<FormViewStoreWarning>,
}

/// Read-only: does NOT require a *writable* project, but does still call
/// `require_registered_project` -- Plan.md section 6: "Read-only source tabs can select project
/// custom views but cannot save one" -- so listing (and, below, `get_last_selected_form_view`)
/// only needs `project_id` to resolve to *some* known registered location, not a writable one.
/// Every mutation command in this file calls `require_writable_project` instead.
#[tauri::command]
pub fn list_custom_form_views(
    app: AppHandle,
    project_id: String,
    game_version: Option<String>,
    def_type: Option<String>,
) -> Result<ListCustomFormViewsResult, AppError> {
    let settings = load_settings(&app)?;
    require_registered_project(&settings, &project_id)?;

    let (views, warning) = form_views::list_custom_views(
        &app,
        &project_id,
        game_version.as_deref(),
        def_type.as_deref(),
    )?;
    Ok(ListCustomFormViewsResult { views, warning })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn create_custom_form_view(
    app: AppHandle,
    project_id: String,
    game_version: String,
    def_type: String,
    name: String,
    hidden_field_ids: Vec<String>,
    description: Option<String>,
    base_schema_view: Option<BaseSchemaViewReference>,
) -> Result<CustomFormView, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    form_views::create_view(
        &app,
        &project_id,
        NewCustomFormView {
            target: FormViewTarget {
                game_version,
                def_type,
            },
            name,
            description,
            hidden_field_ids,
            base_schema_view,
        },
    )
}

/// `description`/`clear_description` together express the three states
/// `CustomFormViewUpdate.description: Option<Option<String>>` needs (leave unchanged / clear /
/// set) as two plain, unambiguous IPC parameters rather than relying on serde to deserialize a
/// doubly-nested `Option<Option<String>>` command argument directly: an ordinary JSON `null`
/// collapses that to a single `None` over Tauri's IPC boundary, which cannot distinguish "the
/// caller omitted this" from "the caller explicitly wants it cleared." `clear_description: true`
/// wins over any provided `description` value; the frontend API only ever sets one or the other.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_custom_form_view(
    app: AppHandle,
    project_id: String,
    view_id: String,
    name: Option<String>,
    hidden_field_ids: Option<Vec<String>>,
    description: Option<String>,
    clear_description: bool,
) -> Result<CustomFormView, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    let description = if clear_description {
        Some(None)
    } else {
        description.map(Some)
    };

    form_views::update_view(
        &app,
        &project_id,
        &view_id,
        CustomFormViewUpdate {
            name,
            hidden_field_ids,
            description,
        },
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCustomFormViewResult {
    pub deleted_id: String,
}

#[tauri::command]
pub fn delete_custom_form_view(
    app: AppHandle,
    project_id: String,
    view_id: String,
) -> Result<DeleteCustomFormViewResult, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    form_views::delete_view(&app, &project_id, &view_id)?;
    Ok(DeleteCustomFormViewResult {
        deleted_id: view_id,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCustomFormViewStoreResult {
    pub backup_path: Option<String>,
}

/// Corruption/incompatible-version recovery per Plan.md section 12 and issue 04 step 6: "no
/// overwrite; explicit backup-and-reset command/action." Any existing store file -- corrupt,
/// project-id-mismatched, or an unreadable newer version -- is renamed to a timestamped `.bak`
/// sibling (never deleted, so a user can still recover data by hand) and replaced with a fresh
/// empty v1 store. Requires a writable project: this is a destructive, mutating disk operation,
/// not a read.
#[tauri::command]
pub fn reset_custom_form_view_store(
    app: AppHandle,
    project_id: String,
) -> Result<ResetCustomFormViewStoreResult, AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    let backup_path = form_views::reset_store(&app, &project_id)?;
    Ok(ResetCustomFormViewStoreResult {
        backup_path: backup_path.map(|p| p.to_string_lossy().to_string()),
    })
}

/// Persists the last clean (non-overridden) view selection for a `{gameVersion, defType}`
/// scope. This *does* require a writable project even though merely *selecting* a view to
/// render doesn't otherwise need one: setting this preference is a real disk write, and
/// Plan.md section 6 says "Form View preferences must be saved after successful
/// selection/mutation" without carving out a read-only-project exception the way it does for
/// listing/selecting-in-memory. A read-only source tab can still select and view a project's
/// custom views; it just won't have anywhere durable to remember that choice. See
/// `get_last_selected_form_view` below for the read side, which -- like listing -- does not
/// require a writable project.
#[tauri::command]
pub fn set_last_selected_form_view(
    app: AppHandle,
    project_id: String,
    game_version: String,
    def_type: String,
    origin: String,
    id: String,
) -> Result<(), AppError> {
    let settings = load_settings(&app)?;
    require_writable_project(&settings, &project_id)?;

    let origin = parse_origin(&origin)?;
    form_views::set_last_selected(
        &app,
        &project_id,
        LastSelectedFormView {
            game_version,
            def_type,
            view: SelectedFormViewRef { origin, id },
        },
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLastSelectedFormViewResult {
    pub selected: Option<SelectedFormViewRef>,
    pub warning: Option<FormViewStoreWarning>,
}

/// Read-only, mirrors `list_custom_form_views`: requires only `require_registered_project`, not
/// a writable one.
#[tauri::command]
pub fn get_last_selected_form_view(
    app: AppHandle,
    project_id: String,
    game_version: String,
    def_type: String,
) -> Result<GetLastSelectedFormViewResult, AppError> {
    let settings = load_settings(&app)?;
    require_registered_project(&settings, &project_id)?;

    let (selected, warning) =
        form_views::get_last_selected(&app, &project_id, &game_version, &def_type)?;
    Ok(GetLastSelectedFormViewResult { selected, warning })
}

#[cfg(test)]
mod project_validation_tests {
    use super::*;
    use crate::project_model::{RegisteredLocation, SourceType};
    use std::path::Path;
    use time::OffsetDateTime;

    fn make_location(id: &str, kind: LocationKind, read_only: bool) -> RegisteredLocation {
        RegisteredLocation {
            id: id.to_string(),
            display_name: id.to_string(),
            root_path: Path::new("/tmp").join(id).to_string_lossy().to_string(),
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

    fn make_settings(locations: Vec<RegisteredLocation>) -> ProjectSettings {
        ProjectSettings {
            schema_version: 2,
            game_version: "1.6".to_string(),
            locations,
            active_project_id: None,
        }
    }

    #[test]
    fn accepts_a_writable_project_location() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, false)]);
        assert!(require_writable_project(&settings, "proj1").is_ok());
    }

    #[test]
    fn rejects_unknown_project_id() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, false)]);
        let err = require_writable_project(&settings, "does-not-exist").unwrap_err();
        assert_eq!(err.code, "form_view_invalid_target");
    }

    #[test]
    fn rejects_source_locations() {
        let settings = make_settings(vec![make_location("src1", LocationKind::Source, true)]);
        let err = require_writable_project(&settings, "src1").unwrap_err();
        assert_eq!(err.code, "form_view_invalid_target");
    }

    #[test]
    fn rejects_read_only_project_locations() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, true)]);
        let err = require_writable_project(&settings, "proj1").unwrap_err();
        assert_eq!(err.code, "form_view_invalid_target");
    }

    #[test]
    fn require_registered_project_accepts_a_writable_project_location() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, false)]);
        assert!(require_registered_project(&settings, "proj1").is_ok());
    }

    #[test]
    fn require_registered_project_accepts_a_read_only_source_location() {
        // The whole point of this check vs. `require_writable_project`: a read-only source
        // location must still pass, since read commands (listing, get-last-selected) don't
        // need write access -- only *some* registered location.
        let settings = make_settings(vec![make_location("src1", LocationKind::Source, true)]);
        assert!(require_registered_project(&settings, "src1").is_ok());
    }

    #[test]
    fn require_registered_project_accepts_a_read_only_project_location() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, true)]);
        assert!(require_registered_project(&settings, "proj1").is_ok());
    }

    #[test]
    fn require_registered_project_rejects_an_unregistered_id() {
        let settings = make_settings(vec![make_location("proj1", LocationKind::Project, false)]);
        let err = require_registered_project(&settings, "does-not-exist").unwrap_err();
        assert_eq!(err.code, "form_view_invalid_target");
    }

    #[test]
    fn parse_origin_accepts_known_values_and_rejects_others() {
        assert!(matches!(
            parse_origin("default"),
            Ok(FormViewOrigin::Default)
        ));
        assert!(matches!(parse_origin("schema"), Ok(FormViewOrigin::Schema)));
        assert!(matches!(parse_origin("custom"), Ok(FormViewOrigin::Custom)));
        let err = parse_origin("bogus").unwrap_err();
        assert_eq!(err.code, "form_view_invalid_origin");
    }
}
