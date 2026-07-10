use crate::project_model::{LocationKind, SourceType};
use crate::xml_document::model::XmlNodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefIdentityKey {
    pub def_type: String,
    pub def_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexedSourceKind {
    Project,
    Source,
}

impl From<&LocationKind> for IndexedSourceKind {
    fn from(value: &LocationKind) -> Self {
        match value {
            LocationKind::Project => Self::Project,
            LocationKind::Source => Self::Source,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedDefSource {
    pub location_id: String,
    pub location_name: String,
    pub source_kind: IndexedSourceKind,
    pub source_type: SourceType,
    pub read_only: bool,
    pub mod_id: Option<String>,
    pub game_version: Option<String>,
    pub expansion_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedDef {
    pub key: DefIdentityKey,
    pub def_type: String,
    pub def_name: String,
    pub label: Option<String>,
    pub parent_name: Option<String>,
    pub relative_path: String,
    pub node_id: Option<XmlNodeId>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub source: IndexedDefSource,
    pub fields: Vec<IndexedDefField>,
    #[serde(skip, default)]
    pub def_name_lower: String,
    #[serde(skip, default)]
    pub label_lower: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedDefField {
    pub name: String,
    pub text_value: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefIndex {
    pub defs: Vec<IndexedDef>,
    pub errors: Vec<DefIndexError>,
    pub built_at_unix_ms: i64,
    #[serde(skip, default)]
    pub by_type: HashMap<String, Vec<usize>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefIndexError {
    pub location_id: String,
    pub location_name: String,
    pub source_kind: IndexedSourceKind,
    pub relative_path: Option<String>,
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct DefIndexSearchQuery {
    pub query: String,
    pub def_type: Option<String>,
    pub include_sources: bool,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefIndexSummary {
    pub indexed_defs: usize,
    pub project_defs: usize,
    pub source_defs: usize,
    pub errors: usize,
    pub built_at_unix_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefDuplicateQueryResult {
    pub project_occurrences: Vec<IndexedDef>,
    pub source_occurrences: Vec<IndexedDef>,
    pub blocking_project_duplicate: bool,
    pub source_duplicate_warning: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedDefSearchResult {
    pub def: IndexedDef,
    pub rank: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefTypeFacet {
    pub def_type: String,
    pub project_count: usize,
    pub source_count: usize,
    pub total_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefIndexFacetSummary {
    pub def_types: Vec<DefTypeFacet>,
    pub project_defs: usize,
    pub source_defs: usize,
    pub errors: usize,
}

pub struct DefIndexReplacement<'a> {
    pub location_id: &'a str,
    pub relative_path: &'a str,
    pub source: &'a str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefReferenceSuggestion {
    pub def_name: String,
    pub def_type: String,
    pub label: Option<String>,
    pub relative_path: String,
    pub node_id: Option<XmlNodeId>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub location_id: String,
    pub location_name: String,
    pub read_only: bool,
    pub rank: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DefReferenceResolution {
    #[serde(rename = "editableProjectDef")]
    EditableProjectDef {
        relative_path: String,
        node_id: Option<XmlNodeId>,
    },
    #[serde(rename = "readOnlySourceDef")]
    ReadOnlySourceDef {
        location_id: String,
        relative_path: String,
        node_id: Option<XmlNodeId>,
    },
    #[serde(rename = "missing")]
    Missing,
    #[serde(rename = "ambiguous")]
    Ambiguous,
}

impl DefIndex {
    pub fn rebuild_computed_fields(&mut self) {
        for def in &mut self.defs {
            def.def_name_lower = def.def_name.to_lowercase();
            def.label_lower = def.label.as_deref().unwrap_or("").to_lowercase();
        }
        self.by_type.clear();
        for (i, def) in self.defs.iter().enumerate() {
            self.by_type
                .entry(def.def_type.clone())
                .or_default()
                .push(i);
        }
    }

    pub fn remove_file(&mut self, location_id: &str, relative_path: &str) {
        let norm = relative_path.replace('\\', "/");
        self.defs
            .retain(|d| d.source.location_id != location_id || d.relative_path != norm);
        self.errors.retain(|e| {
            e.location_id != location_id || e.relative_path.as_deref() != Some(norm.as_str())
        });
    }

    pub fn remove_folder_prefix(&mut self, location_id: &str, folder_prefix: &str) {
        let norm = folder_prefix.replace('\\', "/");
        let prefix_slash = format!("{}/", norm);
        self.defs.retain(|d| {
            d.source.location_id != location_id
                || (!d.relative_path.starts_with(&prefix_slash) && d.relative_path != norm)
        });
        self.errors.retain(|e| {
            e.location_id != location_id
                || e.relative_path
                    .as_deref()
                    .map(|p| !p.starts_with(&prefix_slash) && p != norm)
                    .unwrap_or(true)
        });
    }

    pub fn mark_rebuilt_now(&mut self) {
        self.built_at_unix_ms =
            time::OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000;
    }

    pub fn find_by_key(&self, def_type: &str, def_name: &str) -> Vec<&IndexedDef> {
        self.defs
            .iter()
            .filter(|d| d.def_type == def_type && d.def_name == def_name)
            .collect()
    }

    pub fn find_project_duplicates(&self, def_type: &str, def_name: &str) -> Vec<&IndexedDef> {
        self.find_by_key(def_type, def_name)
            .into_iter()
            .filter(|d| d.source.source_kind == IndexedSourceKind::Project && !d.source.read_only)
            .collect()
    }

    pub fn find_source_duplicates(&self, def_type: &str, def_name: &str) -> Vec<&IndexedDef> {
        self.find_by_key(def_type, def_name)
            .into_iter()
            .filter(|d| d.source.source_kind == IndexedSourceKind::Source)
            .collect()
    }

    pub fn find_all_duplicates(&self, def_type: &str, def_name: &str) -> Vec<&IndexedDef> {
        self.find_by_key(def_type, def_name)
    }
}
