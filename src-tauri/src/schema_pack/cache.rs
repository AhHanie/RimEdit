use super::{build_schema_catalog, SchemaCatalog};
use std::sync::{Arc, Mutex};

/// Caches the built-in-only schema catalog (no extra schema roots) keyed by game version, for
/// callers that rebuild it on a hot path (e.g. per-keystroke XPath completion). Built-in packs
/// are embedded at compile time and never change during the process lifetime, so a plain
/// string-keyed cache is sufficient -- no file fingerprinting needed (contrast `DefIndexState`,
/// which caches against a filesystem-derived index).
#[derive(Default)]
pub struct SchemaCatalogCacheState {
    inner: Mutex<Option<(String, Arc<SchemaCatalog>)>>,
}

impl SchemaCatalogCacheState {
    pub fn get_or_build(&self, game_version: Option<&str>) -> Arc<SchemaCatalog> {
        let key = game_version.unwrap_or("").to_string();
        if let Ok(guard) = self.inner.lock() {
            if let Some((cached_key, catalog)) = guard.as_ref() {
                if *cached_key == key {
                    return Arc::clone(catalog);
                }
            }
        }
        let catalog = Arc::new(build_schema_catalog(&[], game_version).catalog);
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some((key, Arc::clone(&catalog)));
        }
        catalog
    }
}
