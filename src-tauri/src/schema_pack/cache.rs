use super::{build_schema_catalog_with_locale, resolve_catalog_locale, SchemaCatalog};
use std::sync::{Arc, Mutex};

/// Caches the built-in-only schema catalog (no extra schema roots) keyed by `(gameVersion,
/// locale)`, for callers that rebuild it on a hot path (e.g. per-keystroke XPath completion,
/// whose suggestion `detail` text surfaces catalog labels to the user -- see issue 06). Built-in
/// packs are embedded at compile time and never change during the process lifetime, so a plain
/// string-keyed cache is sufficient -- no file fingerprinting needed (contrast `DefIndexState`,
/// which caches against a filesystem-derived index).
///
/// `locale` is resolved through [`resolve_catalog_locale`] (the application locale registry)
/// before it becomes part of the cache key, so an unsupported/garbage locale string always
/// collapses onto the same fallback-locale entry rather than growing the cache unboundedly. This
/// never caches an *external* (extra-schema-root) catalog -- callers needing those always call
/// `build_schema_catalog`/`build_schema_catalog_with_locale` directly, uncached.
#[derive(Default)]
pub struct SchemaCatalogCacheState {
    inner: Mutex<Option<(String, String, Arc<SchemaCatalog>)>>,
}

impl SchemaCatalogCacheState {
    pub fn get_or_build(
        &self,
        game_version: Option<&str>,
        locale: Option<&str>,
    ) -> Arc<SchemaCatalog> {
        let game_version_key = game_version.unwrap_or("").to_string();
        let locale_key = resolve_catalog_locale(locale);
        if let Ok(guard) = self.inner.lock() {
            if let Some((cached_game_version, cached_locale, catalog)) = guard.as_ref() {
                if *cached_game_version == game_version_key && *cached_locale == locale_key {
                    return Arc::clone(catalog);
                }
            }
        }
        let catalog = Arc::new(
            build_schema_catalog_with_locale(&[], game_version, Some(&locale_key)).catalog,
        );
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some((game_version_key, locale_key, Arc::clone(&catalog)));
        }
        catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_resolved_locale_is_a_cache_hit() {
        let cache = SchemaCatalogCacheState::default();
        let first = cache.get_or_build(Some("1.6"), Some("en"));
        let second = cache.get_or_build(Some("1.6"), Some("en"));
        assert!(
            Arc::ptr_eq(&first, &second),
            "identical (gameVersion, locale) must reuse the cached catalog"
        );
    }

    #[test]
    fn unsupported_locale_collapses_onto_the_fallback_entry() {
        let cache = SchemaCatalogCacheState::default();
        let en = cache.get_or_build(Some("1.6"), Some("en"));
        // "xx" is not in `crate::locale::SUPPORTED_LOCALES`, so it resolves to the same fallback
        // locale as "en" and must hit the very same cache entry, not build (and retain) a second
        // one keyed on the raw unsupported string.
        let unsupported = cache.get_or_build(Some("1.6"), Some("xx"));
        let none = cache.get_or_build(Some("1.6"), None);
        assert!(Arc::ptr_eq(&en, &unsupported));
        assert!(Arc::ptr_eq(&en, &none));
    }

    #[test]
    fn different_game_version_is_a_cache_miss_even_with_the_same_locale() {
        let cache = SchemaCatalogCacheState::default();
        let v16 = cache.get_or_build(Some("1.6"), Some("en"));
        let v15 = cache.get_or_build(Some("1.5"), Some("en"));
        assert!(
            !Arc::ptr_eq(&v16, &v15),
            "distinct game versions must not share a cache entry"
        );
        // Switching back to the first game version rebuilds again (single-entry cache), not a
        // stale/contaminated hit from the other game version's slot.
        let v16_again = cache.get_or_build(Some("1.6"), Some("en"));
        assert!(!Arc::ptr_eq(&v15, &v16_again));
    }
}
