use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const CURRENT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDefTemplateStore {
    pub schema_version: u8,
    pub project_id: String,
    pub templates: Vec<UserDefTemplate>,
}

impl UserDefTemplateStore {
    pub fn empty(project_id: impl Into<String>) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project_id.into(),
            templates: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDefTemplate {
    pub id: String,
    pub def_type: String,
    pub name: String,
    pub description: Option<String>,
    pub xml: String,
    pub original_def_name: Option<String>,
    pub original_label: Option<String>,
    pub source_relative_path: Option<String>,
    pub game_version: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// Fields required to persist a new template. The store assigns `id` and timestamps.
#[derive(Debug, Clone)]
pub struct NewUserDefTemplate {
    pub def_type: String,
    pub name: String,
    pub description: Option<String>,
    pub xml: String,
    pub original_def_name: Option<String>,
    pub original_label: Option<String>,
    pub source_relative_path: Option<String>,
    pub game_version: Option<String>,
}

/// Lightweight listing projection that omits the full template `xml`, used by the
/// creation wizard so it does not need to fetch every template body up front.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDefTemplateSummary {
    pub id: String,
    pub def_type: String,
    pub name: String,
    pub description: Option<String>,
    pub original_def_name: Option<String>,
    pub original_label: Option<String>,
    pub source_relative_path: Option<String>,
    pub game_version: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<&UserDefTemplate> for UserDefTemplateSummary {
    fn from(t: &UserDefTemplate) -> Self {
        Self {
            id: t.id.clone(),
            def_type: t.def_type.clone(),
            name: t.name.clone(),
            description: t.description.clone(),
            original_def_name: t.original_def_name.clone(),
            original_label: t.original_label.clone(),
            source_relative_path: t.source_relative_path.clone(),
            game_version: t.game_version.clone(),
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}
