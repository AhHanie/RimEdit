use crate::project_model::{LocationKind, SourceType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectFileKind {
    Xml,
    Text,
    Binary,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileEntry {
    pub relative_path: String,
    pub folder_path: String,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub file_kind: ProjectFileKind,
    /// `Some(true)` if the XML file is in the active game-version load set,
    /// `Some(false)` if it exists but is not loaded for the current version,
    /// `None` for non-XML files or when the status has not been computed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_for_game_version: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFolderEntry {
    pub relative_path: String,
    pub folder_name: String,
    pub parent_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileScan {
    pub project_id: String,
    pub project_root: String,
    pub folders: Vec<ProjectFolderEntry>,
    pub files: Vec<ProjectFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationXmlFileScan {
    pub location_id: String,
    pub location_name: String,
    pub root_path: String,
    pub source_kind: LocationKind,
    pub source_type: SourceType,
    pub read_only: bool,
    pub mod_id: Option<String>,
    pub files: Vec<ProjectFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileContent {
    pub project_id: String,
    pub relative_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathMutationResult {
    pub old_path: String,
    pub new_path: String,
}
