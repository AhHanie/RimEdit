use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::project_model::AppError;

/// In-memory map from opaque token to validated canonical file path.
/// Registered as Tauri state so the command layer can resolve tokens without
/// ever exposing absolute paths to the frontend. Arc-wrapped so the builder
/// can clone it into the custom protocol handler while also managing it as state.
#[derive(Clone, Default)]
pub(crate) struct AssetTokenCache {
    entries: Arc<Mutex<HashMap<String, PathBuf>>>,
}

impl AssetTokenCache {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(in crate::services::graphic_preview) fn register(&self, path: PathBuf) -> String {
        let token = Uuid::new_v4().to_string();
        self.entries
            .lock()
            .expect("AssetTokenCache mutex poisoned")
            .insert(token.clone(), path);
        token
    }

    pub(crate) fn resolve_asset_token(&self, token: &str) -> Option<PathBuf> {
        self.entries
            .lock()
            .expect("AssetTokenCache mutex poisoned")
            .get(token)
            .cloned()
    }
}

/// Returns the canonical `rimedit-asset://localhost/{token}` URL for a given token.
pub(crate) fn preview_asset_url(token: &str) -> String {
    format!("rimedit-asset://localhost/{}", token)
}

/// Extracts the opaque asset token from a custom-protocol URI.
///
/// New-style URLs use `rimedit-asset://localhost/{token}` so token is in the path.
/// Legacy URLs used `rimedit-asset://{token}` so token was in the host.
pub(crate) fn extract_asset_token(host: &str, path: &str) -> Option<String> {
    if host == "localhost" || host.ends_with(".localhost") {
        let t = path.trim_start_matches('/');
        return if t.is_empty() {
            None
        } else {
            Some(t.to_owned())
        };
    }
    if !host.is_empty() {
        return Some(host.to_owned());
    }
    let t = path.trim_start_matches('/');
    if !t.is_empty() {
        Some(t.to_owned())
    } else {
        None
    }
}

pub(crate) fn content_type_for_texture(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

pub(crate) fn is_browser_preview_supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("png") | Some("jpg") | Some("jpeg")
    )
}

/// Resolve a token to its bytes and MIME type, returning structured errors for
/// unknown tokens (404), unsupported formats like `.dds` (415), and read
/// failures (500).
pub(crate) fn read_preview_asset(
    cache: &AssetTokenCache,
    token: &str,
) -> Result<(Vec<u8>, &'static str), AppError> {
    let path = cache.resolve_asset_token(token).ok_or_else(|| AppError {
        code: "TOKEN_NOT_FOUND".into(),
        message: format!("Asset token not found: {token}"),
        details: None,
        args: crate::diagnostics::diagnostic_args([("token", token.into())]),
    })?;

    if !is_browser_preview_supported(&path) {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_string();
        return Err(AppError {
            code: "UNSUPPORTED_FORMAT".into(),
            message: format!("Unsupported texture format: {}", extension),
            details: None,
            args: crate::diagnostics::diagnostic_args([("extension", extension.into())]),
        });
    }

    let bytes = fs::read(&path).map_err(|e| AppError {
        code: "READ_FAILED".into(),
        message: format!("Failed to read asset: {e}"),
        details: None,
        args: crate::diagnostics::DiagnosticArgs::new(),
    })?;

    Ok((bytes, content_type_for_texture(&path)))
}
