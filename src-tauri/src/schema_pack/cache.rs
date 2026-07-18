use super::loader::{collect_json_files, discover_manifest_paths_in_root};
use super::{build_schema_catalog_with_locale, resolve_catalog_locale, SchemaCatalog};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Caches built schema catalogs -- both the built-in-only catalog and, per Plan.md's "cache
/// localized catalogs for projects with external schema roots", catalogs built from a project's
/// registered-location external schema roots -- for callers that rebuild on a hot path (e.g.
/// per-keystroke XPath completion, whose suggestion `detail` text surfaces catalog labels to the
/// user -- see issue 06).
///
/// Keyed by `(gameVersion, locale, externalRootsSignature)`:
/// - `locale` is resolved through [`resolve_catalog_locale`] before it becomes part of the key, so
///   an unsupported/garbage locale string always collapses onto the same fallback-locale entry.
/// - `externalRootsSignature` ([`roots_signature`]) normalizes the root path list (sorted,
///   deduplicated) plus a cheap per-root discovery fingerprint: the modification time of every
///   `.json` file discovered under each pack's directory (manifest, def-types, object-types,
///   patch-operations, locales -- wherever they live) -- enough to invalidate on adding, removing,
///   or editing any schema-pack JSON file without re-*parsing* any of them on every keystroke just
///   to check whether a cache entry is still usable (stat calls only, never a content read). Built-
///   in packs are embedded at compile time and never change during the process lifetime, so the
///   empty-roots entry needs no such fingerprint.
///
/// This is a small bounded cache (see [`MAX_ENTRIES`]), not a single slot: a project with
/// registered locations still needs its built-in-only entry (e.g. for `load_schema_catalog`-style
/// callers) to coexist with its external-root entry, and a session may switch between a couple of
/// game versions/locales while editing.
///
/// [`SchemaCatalogCacheState::invalidate_all`] is called whenever project settings, registered
/// locations, or the selected game version change (see
/// `commands::project_settings::trigger_settings_reindex`), so a stale catalog for a project that
/// no longer has the same roots/game version is never served just because its signature happened
/// to still match.
pub struct SchemaCatalogCacheState {
    entries: Mutex<Vec<CacheEntry>>,
}

impl Default for SchemaCatalogCacheState {
    fn default() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }
}

/// Bounds the cache: a handful of (game version, locale, root-set) combinations can realistically
/// be live in one session (e.g. switching locale or game version while editing), but this must
/// stay small so a pathological sequence of distinct root sets can't grow the cache unboundedly.
const MAX_ENTRIES: usize = 8;

struct CacheEntry {
    game_version: String,
    locale: String,
    roots_signature: String,
    catalog: Arc<SchemaCatalog>,
}

impl SchemaCatalogCacheState {
    /// Get (or build) a catalog for `roots` as extra external schema-pack search roots (see
    /// `schema_pack::schema_pack_roots`; pass `&[]` for the built-in-only catalog), cached by
    /// `(gameVersion, locale, externalRootsSignature)`. Returns whether the call was served from
    /// the cache, for low-cardinality instrumentation tagging by callers.
    pub fn get_or_build_with_roots(
        &self,
        roots: &[PathBuf],
        game_version: Option<&str>,
        locale: Option<&str>,
    ) -> (Arc<SchemaCatalog>, bool) {
        let game_version_key = game_version.unwrap_or("").to_string();
        let locale_key = resolve_catalog_locale(locale);
        let roots_key = roots_signature(roots);

        if let Ok(mut guard) = self.entries.lock() {
            if let Some(pos) = guard.iter().position(|e| {
                e.game_version == game_version_key
                    && e.locale == locale_key
                    && e.roots_signature == roots_key
            }) {
                let entry = guard.remove(pos);
                let catalog = Arc::clone(&entry.catalog);
                guard.insert(0, entry);
                return (catalog, true);
            }
        }

        let catalog = Arc::new(
            build_schema_catalog_with_locale(roots, game_version, Some(&locale_key)).catalog,
        );
        if let Ok(mut guard) = self.entries.lock() {
            guard.insert(
                0,
                CacheEntry {
                    game_version: game_version_key,
                    locale: locale_key,
                    roots_signature: roots_key,
                    catalog: Arc::clone(&catalog),
                },
            );
            guard.truncate(MAX_ENTRIES);
        }
        (catalog, false)
    }

    /// Drop every cached catalog. Called whenever project settings, registered locations, or the
    /// selected game version change, so a stale external-root catalog built for a since-changed
    /// root set is never served just because a later call happens to reuse the same map slot.
    pub fn invalidate_all(&self) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.clear();
        }
    }
}

/// A cheap, order-independent identity for `roots` plus a discovery-input fingerprint: each
/// discovered pack's directory (same discovery rule as [`super::loader::load_external_packs`]),
/// fingerprinted by the modification time of every `.json` file anywhere under it -- not just its
/// `schema-pack.json` manifest, so editing a def-type/object-type/patch-operation/locale file
/// (the common schema-pack-authoring edit) changes the signature too, not only editing the
/// manifest itself or adding/removing a pack. This only stats file metadata -- it never reads or
/// parses JSON content -- so it stays fast enough to recompute on every keystroke even though it
/// walks each pack's directory tree.
fn roots_signature(roots: &[PathBuf]) -> String {
    let mut root_strings: Vec<String> = roots
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    root_strings.sort();
    root_strings.dedup();

    let mut stamps: Vec<String> = Vec::new();
    for root in roots {
        for manifest_path in discover_manifest_paths_in_root(root) {
            let pack_dir = manifest_path.parent().unwrap_or(root);
            for json_path in collect_json_files(pack_dir) {
                let modified_ms = std::fs::metadata(&json_path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                stamps.push(format!("{}@{}", json_path.to_string_lossy(), modified_ms));
            }
        }
    }
    stamps.sort();

    let mut signature = root_strings.join("|");
    if !stamps.is_empty() {
        signature.push('#');
        signature.push_str(&stamps.join(","));
    }
    signature
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only convenience mirroring the old built-in-only `get_or_build` entry point.
    fn get_or_build(
        cache: &SchemaCatalogCacheState,
        game_version: Option<&str>,
        locale: Option<&str>,
    ) -> Arc<SchemaCatalog> {
        cache.get_or_build_with_roots(&[], game_version, locale).0
    }

    #[test]
    fn same_resolved_locale_is_a_cache_hit() {
        let cache = SchemaCatalogCacheState::default();
        let first = get_or_build(&cache, Some("1.6"), Some("en"));
        let second = get_or_build(&cache, Some("1.6"), Some("en"));
        assert!(
            Arc::ptr_eq(&first, &second),
            "identical (gameVersion, locale) must reuse the cached catalog"
        );
    }

    #[test]
    fn unsupported_locale_collapses_onto_the_fallback_entry() {
        let cache = SchemaCatalogCacheState::default();
        let en = get_or_build(&cache, Some("1.6"), Some("en"));
        // "xx" is not in `crate::locale::SUPPORTED_LOCALES`, so it resolves to the same fallback
        // locale as "en" and must hit the very same cache entry, not build (and retain) a second
        // one keyed on the raw unsupported string.
        let unsupported = get_or_build(&cache, Some("1.6"), Some("xx"));
        let none = get_or_build(&cache, Some("1.6"), None);
        assert!(Arc::ptr_eq(&en, &unsupported));
        assert!(Arc::ptr_eq(&en, &none));
    }

    #[test]
    fn different_game_version_is_a_cache_miss_even_with_the_same_locale() {
        let cache = SchemaCatalogCacheState::default();
        let v16 = get_or_build(&cache, Some("1.6"), Some("en"));
        let v15 = get_or_build(&cache, Some("1.5"), Some("en"));
        assert!(
            !Arc::ptr_eq(&v16, &v15),
            "distinct game versions must not share a cache entry"
        );
        // Switching back to the first game version reuses its still-cached entry (bounded
        // multi-entry cache), not a stale/contaminated hit from the other game version's entry.
        let v16_again = get_or_build(&cache, Some("1.6"), Some("en"));
        assert!(!Arc::ptr_eq(&v15, &v16_again));
        assert!(Arc::ptr_eq(&v16, &v16_again));
    }

    #[test]
    fn distinct_external_root_sets_get_distinct_cache_entries() {
        let cache = SchemaCatalogCacheState::default();
        let (empty_roots, empty_hit) = cache.get_or_build_with_roots(&[], Some("1.6"), Some("en"));
        assert!(!empty_hit);
        let (with_root, with_root_hit) = cache.get_or_build_with_roots(
            &[PathBuf::from("does/not/exist")],
            Some("1.6"),
            Some("en"),
        );
        assert!(!with_root_hit);
        assert!(
            !Arc::ptr_eq(&empty_roots, &with_root),
            "a project with registered locations must not reuse the built-in-only catalog"
        );

        let (with_root_again, with_root_again_hit) = cache.get_or_build_with_roots(
            &[PathBuf::from("does/not/exist")],
            Some("1.6"),
            Some("en"),
        );
        assert!(with_root_again_hit);
        assert!(Arc::ptr_eq(&with_root, &with_root_again));
    }

    #[test]
    fn root_order_does_not_affect_the_cache_key() {
        let cache = SchemaCatalogCacheState::default();
        let (first, _) = cache.get_or_build_with_roots(
            &[PathBuf::from("a"), PathBuf::from("b")],
            Some("1.6"),
            Some("en"),
        );
        let (second, hit) = cache.get_or_build_with_roots(
            &[PathBuf::from("b"), PathBuf::from("a")],
            Some("1.6"),
            Some("en"),
        );
        assert!(hit, "root order must not change the normalized signature");
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn invalidate_all_forces_every_entry_to_rebuild() {
        let cache = SchemaCatalogCacheState::default();
        let first = get_or_build(&cache, Some("1.6"), Some("en"));
        cache.invalidate_all();
        let second = get_or_build(&cache, Some("1.6"), Some("en"));
        assert!(
            !Arc::ptr_eq(&first, &second),
            "invalidate_all must drop the previously cached catalog"
        );
    }

    /// Content-only edits to a def-type file -- the common schema-pack-authoring edit -- must
    /// bust the cache even though neither the manifest nor the project's registered roots/game
    /// version/locale changed. Explicitly bumps the edited file's mtime into the future rather
    /// than relying on wall-clock granularity between two writes in the same test.
    #[test]
    fn editing_a_def_type_file_without_touching_the_manifest_busts_the_cache() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let manifest = serde_json::json!({
            "formatVersion": 1,
            "packId": "test.cache.contentedit",
            "name": "Cache Content Edit",
            "version": "1.0.0",
            "defTypeDirectories": ["def-types"],
        });
        std::fs::write(tmp.path().join("schema-pack.json"), manifest.to_string()).unwrap();
        let def_types_dir = tmp.path().join("def-types");
        std::fs::create_dir(&def_types_dir).unwrap();
        let def_path = def_types_dir.join("ThingDef.json");
        std::fs::write(
            &def_path,
            r#"{ "defType": "ThingDef", "fields": { "label": { "type": { "kind": "string" } } } }"#,
        )
        .unwrap();

        let cache = SchemaCatalogCacheState::default();
        let roots = vec![tmp.path().to_path_buf()];
        let (first, first_hit) = cache.get_or_build_with_roots(&roots, Some("1.6"), Some("en"));
        assert!(!first_hit);

        // Edit the def-type file's content (add a field) and push its mtime forward so the
        // change is visible regardless of filesystem timestamp resolution.
        std::fs::write(
            &def_path,
            r#"{ "defType": "ThingDef", "fields": { "label": { "type": { "kind": "string" } }, "description": { "type": { "kind": "string" } } } }"#,
        )
        .unwrap();
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&def_path)
            .unwrap();
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(120))
            .expect("set_modified should be supported on the test host");

        let (second, second_hit) = cache.get_or_build_with_roots(&roots, Some("1.6"), Some("en"));
        assert!(
            !second_hit,
            "a content-only edit to a discovered pack's def-type file must not be served from cache"
        );
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(second.def_types["ThingDef"]
            .fields
            .contains_key("description"));
    }
}
