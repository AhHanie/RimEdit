use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocationKind {
    Project,
    Source,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceType {
    BaseGame,
    LocalMod,
    SteamWorkshop,
    Folder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredLocation {
    pub id: String,
    pub display_name: String,
    pub root_path: String,
    pub kind: LocationKind,
    pub source_type: SourceType,
    pub read_only: bool,
    pub mod_id: Option<String>,
    pub game_version: Option<String>,
    /// Kept for backward-compatibility with existing settings files; not used by new code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expansion_name: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredLocationDraft {
    pub display_name: String,
    pub root_path: String,
    pub kind: LocationKind,
    pub source_type: SourceType,
    pub mod_id: Option<String>,
    pub game_version: Option<String>,
}

impl RegisteredLocationDraft {
    pub fn read_only_for_kind(kind: &LocationKind) -> bool {
        match kind {
            LocationKind::Project => false,
            LocationKind::Source => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredLocationUpdate {
    pub id: String,
    pub display_name: String,
    pub source_type: SourceType,
    pub mod_id: Option<String>,
    pub game_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    pub schema_version: u8,
    pub game_version: String,
    /// Global app UI locale (BCP-47, e.g. `"en"`). Validated against
    /// `crate::locale::SUPPORTED_LOCALES`. Despite `ProjectSettings`' name this
    /// field is app-wide, not per-project -- see `docs/i18n/issues/02-global-locale-settings.md`.
    pub locale: String,
    pub locations: Vec<RegisteredLocation>,
    pub active_project_id: Option<String>,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            schema_version: 3,
            game_version: "1.6".to_string(),
            locale: crate::locale::FALLBACK_LOCALE.to_string(),
            locations: Vec::new(),
            active_project_id: None,
        }
    }
}

impl ProjectSettings {
    pub fn has_duplicate_path(&self, canonical_path: &str) -> bool {
        self.locations.iter().any(|l| l.root_path == canonical_path)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingActiveProjectNotice {
    pub id: String,
    pub display_name: String,
    pub root_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettingsLoadResult {
    pub settings: ProjectSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_active_project: Option<MissingActiveProjectNotice>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Option<HashMap<String, String>>,
    /// Typed, literal interpolation arguments for `code` (see `crate::diagnostics` module docs).
    /// Additive alongside the still-English `message`; omitted from JSON when empty.
    #[serde(
        default,
        skip_serializing_if = "crate::diagnostics::DiagnosticArgs::is_empty"
    )]
    pub args: crate::diagnostics::DiagnosticArgs,
}

impl AppError {
    /// Builds an `AppError` from a [`crate::diagnostics::DiagnosticRef`] (code + args built
    /// together, see that type's docs) plus a compatibility `message`.
    pub fn from_ref(
        reference: crate::diagnostics::DiagnosticRef,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: reference.code,
            message: message.into(),
            details: None,
            args: reference.args,
        }
    }

    /// Attaches typed args to an already-built `AppError`, for producers that want to populate
    /// `args` without repeating the whole struct literal.
    pub fn with_args(mut self, args: crate::diagnostics::DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Failed to read settings: {0}")]
    ReadFailed(String),
    #[error("Failed to write settings: {0}")]
    WriteFailed(String),
    #[error("Path is invalid: {0}")]
    InvalidPath(String),
    #[error("Location not found: {0}")]
    NotFound(String),
    #[error("Invalid game version: {0}")]
    InvalidGameVersion(String),
    #[error("Game version not available in installed schema packs: {0}")]
    GameVersionSchemaUnavailable(String),
    #[error("Unsupported locale: {0}")]
    UnsupportedLocale(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<StoreError> for AppError {
    fn from(e: StoreError) -> Self {
        let code = match &e {
            StoreError::ReadFailed(_) => "settings_read_failed",
            StoreError::WriteFailed(_) => "settings_write_failed",
            StoreError::InvalidPath(_) => "invalid_location_path",
            StoreError::NotFound(_) => "location_not_found",
            StoreError::InvalidGameVersion(_) => "invalid_game_version",
            StoreError::GameVersionSchemaUnavailable(_) => "game_version_schema_unavailable",
            StoreError::UnsupportedLocale(_) => "unsupported_locale",
            StoreError::Io(_) => "io_error",
            StoreError::Json(_) => "json_error",
        };
        // Typed args mirror the single literal value each `StoreError` variant already carries in
        // its `thiserror` message, for producers that only ever have one value to attach.
        let args = match &e {
            StoreError::InvalidPath(path) | StoreError::NotFound(path) => {
                crate::diagnostics::diagnostic_args([("path", path.as_str().into())])
            }
            StoreError::InvalidGameVersion(v) | StoreError::GameVersionSchemaUnavailable(v) => {
                crate::diagnostics::diagnostic_args([("gameVersion", v.as_str().into())])
            }
            StoreError::UnsupportedLocale(locale) => {
                crate::diagnostics::diagnostic_args([("locale", locale.as_str().into())])
            }
            _ => crate::diagnostics::DiagnosticArgs::new(),
        };
        AppError {
            code: code.to_string(),
            message: e.to_string(),
            details: None,
            args,
        }
    }
}

/// Parse and validate a game version string as a major.minor pair.
///
/// Valid: `"1.6"`, `"1.5"`, `"2.0"`.
/// Invalid: blank, `"1"`, `"1.6.1234"`, `"v1.6"`.
pub fn parse_major_minor(s: &str) -> Option<(u16, u16)> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 2 {
        return None;
    }
    let major = parts[0].parse::<u16>().ok()?;
    let minor = parts[1].parse::<u16>().ok()?;
    Some((major, minor))
}

/// Validate a game version string for use as the project game version.
///
/// Must match `^\d+\.\d+$`.
pub fn validate_project_game_version(v: &str) -> Result<(), StoreError> {
    let trimmed = v.trim();
    if trimmed.is_empty() {
        return Err(StoreError::InvalidGameVersion(
            "Game version must not be blank.".to_string(),
        ));
    }
    if parse_major_minor(trimmed).is_none() {
        return Err(StoreError::InvalidGameVersion(format!(
            "Game version must be Major.Minor (e.g. \"1.6\"), got: {:?}",
            v
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_wire_shape_omits_empty_args() {
        let err: AppError = StoreError::NotFound("loc-1".to_string()).into();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "location_not_found");
        assert_eq!(json["args"]["path"], "loc-1");
    }

    #[test]
    fn app_error_from_ref_carries_code_and_args() {
        let err = AppError::from_ref(
            crate::diagnostics::DiagnosticRef::code("project_not_found")
                .with_arg("projectId", "abc"),
            "No project location found for id 'abc'.",
        );
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "project_not_found");
        assert_eq!(json["args"]["projectId"], "abc");
    }

    #[test]
    fn app_error_without_args_omits_the_field_entirely() {
        let err = AppError {
            code: "io_error".to_string(),
            message: "boom".to_string(),
            details: None,
            args: crate::diagnostics::DiagnosticArgs::new(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert!(json.get("args").is_none());
    }

    #[test]
    fn default_settings_have_no_locations() {
        let s = ProjectSettings::default();
        assert!(s.locations.is_empty());
        assert_eq!(s.schema_version, 3);
        assert!(s.active_project_id.is_none());
        assert_eq!(s.game_version, "1.6");
        assert_eq!(s.locale, "en");
    }

    #[test]
    fn project_kind_is_not_read_only() {
        assert!(!RegisteredLocationDraft::read_only_for_kind(
            &LocationKind::Project
        ));
    }

    #[test]
    fn source_kind_is_read_only() {
        assert!(RegisteredLocationDraft::read_only_for_kind(
            &LocationKind::Source
        ));
    }

    #[test]
    fn serialization_roundtrip() {
        let s = ProjectSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        let s2: ProjectSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s.schema_version, s2.schema_version);
        assert_eq!(s.game_version, s2.game_version);
        assert_eq!(s.locale, s2.locale);
        assert_eq!(s.locations.len(), s2.locations.len());
        assert!(s2.active_project_id.is_none());
    }

    #[test]
    fn duplicate_canonical_paths_are_rejected() {
        let mut settings = ProjectSettings::default();
        let loc = RegisteredLocation {
            id: "1".to_string(),
            display_name: "Test".to_string(),
            root_path: "C:\\some\\path".to_string(),
            kind: LocationKind::Project,
            source_type: SourceType::Folder,
            read_only: false,
            mod_id: None,
            game_version: None,
            expansion_name: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };
        settings.locations.push(loc);
        assert!(settings.has_duplicate_path("C:\\some\\path"));
        assert!(!settings.has_duplicate_path("C:\\other\\path"));
    }

    #[test]
    fn enum_serializes_to_camel_case() {
        assert_eq!(
            serde_json::to_string(&LocationKind::Project).unwrap(),
            "\"project\""
        );
        assert_eq!(
            serde_json::to_string(&LocationKind::Source).unwrap(),
            "\"source\""
        );
        assert_eq!(
            serde_json::to_string(&SourceType::BaseGame).unwrap(),
            "\"baseGame\""
        );
        assert_eq!(
            serde_json::to_string(&SourceType::SteamWorkshop).unwrap(),
            "\"steamWorkshop\""
        );
    }

    #[test]
    fn parse_major_minor_valid() {
        assert_eq!(parse_major_minor("1.6"), Some((1, 6)));
        assert_eq!(parse_major_minor("2.0"), Some((2, 0)));
        assert_eq!(parse_major_minor("1.5"), Some((1, 5)));
    }

    #[test]
    fn parse_major_minor_invalid() {
        assert!(parse_major_minor("").is_none());
        assert!(parse_major_minor("1").is_none());
        assert!(parse_major_minor("1.6.1234").is_none());
        assert!(parse_major_minor("v1.6").is_none());
        assert!(parse_major_minor("abc").is_none());
    }

    #[test]
    fn validate_project_game_version_rejects_invalid() {
        assert!(validate_project_game_version("").is_err());
        assert!(validate_project_game_version("1.6.1234").is_err());
        assert!(validate_project_game_version("v1.6").is_err());
        assert!(validate_project_game_version("1").is_err());
    }

    #[test]
    fn validate_project_game_version_accepts_valid() {
        assert!(validate_project_game_version("1.6").is_ok());
        assert!(validate_project_game_version("1.5").is_ok());
        assert!(validate_project_game_version("2.0").is_ok());
    }
}
