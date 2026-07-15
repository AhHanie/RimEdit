use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Scope key for a custom Form View: project (implicit -- the store file lives under the
/// project's own directory), selected game version, and concrete Def type. Matches Plan.md
/// section 3: "Scope custom views by project ID + selected game version + concrete Def type."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormViewTarget {
    pub game_version: String,
    pub def_type: String,
}

/// Optional provenance recorded when a custom view was copied/saved from a schema-defined
/// view (Plan.md section 6). Purely informational: never used to re-derive `hiddenFieldIds`,
/// so a missing/renamed base becomes a nonblocking "derived from unavailable view" notice at
/// resolution time (issue 05+), not a broken or deleted custom view. Field names mirror
/// `schema_pack::model::SchemaFormView`/`FormViewSource` (`view_id` <-> `SchemaFormView.id`,
/// `pack_id`/`pack_version` <-> `FormViewSource`, `declared_on_def_type` <->
/// `SchemaFormView.declared_on_def_type`) so a caller can construct one directly from a
/// resolved catalog view without renaming fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseSchemaViewReference {
    pub view_id: String,
    pub pack_id: String,
    pub pack_version: String,
    pub declared_on_def_type: String,
}

/// A project-owned, user-editable Form View. `hidden_field_ids` is always a materialized
/// snapshot -- never a diff against `base_schema_view` -- so later schema-view changes can
/// never silently rewrite a user's custom selection, and fields introduced by a later schema
/// release remain visible because they are simply absent from this set (Plan.md section 6).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFormView {
    pub id: String,
    pub target: FormViewTarget,
    pub name: String,
    pub description: Option<String>,
    /// Canonical top-level Def schema field keys hidden by this view. Entries that no longer
    /// exist in the live schema are retained verbatim on read/write -- compatibility filtering
    /// against the current catalog is a resolution-time (issue 05+) concern, not a storage one.
    pub hidden_field_ids: Vec<String>,
    pub base_schema_view: Option<BaseSchemaViewReference>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// Fields required to persist a new custom view. The store assigns `id` and timestamps.
#[derive(Debug, Clone)]
pub struct NewCustomFormView {
    pub target: FormViewTarget,
    pub name: String,
    pub description: Option<String>,
    pub hidden_field_ids: Vec<String>,
    pub base_schema_view: Option<BaseSchemaViewReference>,
}

/// Fields accepted when updating an existing custom view. Deliberately excludes `target` (a
/// custom view's scope is immutable after creation) and `base_schema_view` (provenance is set
/// only at creation; not needed by any issue-04 caller).
///
/// `name`/`hidden_field_ids` use plain `Option<T>`: `None` leaves the field unchanged, `Some`
/// replaces it -- there is no meaningful "explicitly clear to blank" state for either (a blank
/// name is rejected, and an empty hidden set is just "show everything", itself a valid `Some`).
///
/// `description` is `Option<Option<String>>` because it has three distinct states a caller can
/// mean: leave unchanged (`None`), explicitly clear to no description (`Some(None)`), or set new
/// text (`Some(Some(text))`). The Tauri command layer builds this from two plain, IPC-safe
/// parameters (`description: Option<String>` + `clear_description: bool`) rather than relying on
/// serde to deserialize a doubly-nested `Option` directly over IPC -- ordinary JSON `null`
/// collapses `Option<Option<T>>` to a single `None` there, losing exactly the distinction this
/// type exists to preserve. See `commands::form_views::update_custom_form_view`.
#[derive(Debug, Clone, Default)]
pub struct CustomFormViewUpdate {
    pub name: Option<String>,
    pub hidden_field_ids: Option<Vec<String>>,
    pub description: Option<Option<String>>,
}

/// Which kind of Form View a persisted selection reference points at. Mirrors
/// `ResolvedFormView.origin` in Plan.md section 3 (`"default" | "schema" | "custom"`), though
/// that TypeScript type belongs to issue 05's runtime resolver, not this store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormViewOrigin {
    Default,
    Schema,
    Custom,
}

/// A selected view reference as persisted in `preferences.lastSelected`. `id` is `"default"`
/// for the Default View, a schema `SchemaFormView.id` for a schema-defined view, or a
/// `CustomFormView.id` UUID for a custom view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedFormViewRef {
    pub origin: FormViewOrigin,
    pub id: String,
}

/// One entry in `preferences.lastSelected`: the last clean (non-overridden) view selection for
/// a given `{gameVersion, defType}` scope. At most one entry per scope; a new selection in the
/// same scope replaces the previous entry (Plan.md section 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastSelectedFormView {
    pub game_version: String,
    pub def_type: String,
    pub view: SelectedFormViewRef,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormViewPreferences {
    pub last_selected: Vec<LastSelectedFormView>,
}

/// The on-disk store shape at `{app config}/RimEdit/form-views/projects/<projectId>/form-views.json`.
/// Mirrors `def_templates::model::UserDefTemplateStore` (schema-versioned, project-scoped, atomic
/// write) but for custom Form Views + the last-selected-view preference. See Plan.md section 6
/// for the canonical JSON shape this type serializes to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFormViewStore {
    pub schema_version: u32,
    pub project_id: String,
    pub custom_views: Vec<CustomFormView>,
    pub preferences: FormViewPreferences,
}

impl UserFormViewStore {
    pub fn empty(project_id: impl Into<String>) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project_id.into(),
            custom_views: Vec::new(),
            preferences: FormViewPreferences::default(),
        }
    }
}

/// A nonfatal, user-facing warning surfaced alongside an otherwise-successful read (e.g. the
/// on-disk store's `schemaVersion` is newer than this build supports). Distinct from
/// `FormViewStoreError`: a warning still returns usable (if reduced) data, while an error means
/// the caller could not read the store's custom-view data at all.
///
/// `code` intentionally matches `FormViewStoreError`'s `form_view_unsupported_version` (see
/// `form_views::error`) -- both describe the exact same condition, just on the warning vs. hard
/// -error path, and the frontend's diagnostic catalog (`src/i18n/resources/en/diagnostics.json`)
/// keys off this one shared code.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormViewStoreWarning {
    pub code: String,
    /// Compatibility English text mirroring `code`/`args` (see `AppError`'s doc comment for the
    /// same pattern). Prefer rendering `code`/`args` through the frontend's shared diagnostic
    /// renderer; this remains only as a fallback for the migration window.
    pub message: String,
    #[serde(
        default,
        skip_serializing_if = "crate::diagnostics::DiagnosticArgs::is_empty"
    )]
    pub args: crate::diagnostics::DiagnosticArgs,
}

impl FormViewStoreWarning {
    pub fn unsupported_newer_version(schema_version: u32) -> Self {
        Self {
            code: "form_view_unsupported_version".to_string(),
            message: format!(
                "The custom Form View store was saved by a newer version of RimEdit \
                 (schema version {}). Custom views are unavailable in this session until \
                 RimEdit is upgraded.",
                schema_version
            ),
            args: crate::diagnostics::diagnostic_args([(
                "schemaVersion",
                (schema_version as i64).into(),
            )]),
        }
    }
}
