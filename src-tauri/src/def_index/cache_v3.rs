//! Compact on-disk def-index cache codec.
//!
//! Replaces the pretty-printed JSON `DefIndex` (a 158 MB real-world file for a large Steam
//! Workshop collection) with a versioned, deduplicated, compressed encoding:
//!
//! - CBOR (via `ciborium`), not `bincode`: several types reachable from `DefIndex` rely on
//!   serde behavior that only self-describing formats support -- `DiagnosticArgValue` is
//!   `#[serde(untagged)]` (bincode cannot deserialize untagged enums at all -- it has no
//!   `deserialize_any`), and `DefIndexError.args` is `skip_serializing_if`-omitted when empty
//!   (safe for any self-describing format, but silently misaligns bincode's fixed positional
//!   field sequence when the field is actually skipped). CBOR is self-describing exactly like
//!   JSON, so both patterns round-trip correctly, while still being a compact binary encoding
//!   with none of JSON's text/whitespace overhead.
//! - zstd compresses the CBOR bytes. Level 3 (zstd's own documented default) is used rather than
//!   a maximum-ratio level: this cache is read back on every launch/interactive hydration and
//!   written comparatively rarely (after a full rebuild or an incremental persist), so decode
//!   speed on the read path matters far more than shaving a few extra percent off disk usage.
//! - `DefIndexCacheDtoV3` drops `IndexedDef.key` (recomputed from `def_type`/`def_name` in
//!   [`from_dto`]) and normalizes the two things repeated on every single indexed Def --
//!   `IndexedDefSource` (constant per location) and `relative_path` (constant per file) -- into
//!   deduplicated tables, storing a compact index into each per Def instead. The path table isn't
//!   a fresh dedup table: it reuses `file_fingerprints`, which already carries a
//!   `(location_id, relative_path)` pair per scanned file, so a Def's path is usually just an
//!   index into that existing table (`CompactDefDto::path_in_fingerprints`). A small
//!   `path_overflow` table is the fallback for the rare Def whose path isn't present there.
//!
//! This module only encodes/decodes the DTO shape; cache-file I/O (path, atomic write, version/
//! settings-fingerprint checks) stays in `cache.rs`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::fingerprint::IndexedFileFingerprint;
use super::model::{
    DefIdentityKey, DefIndex, DefIndexError, IndexedDef, IndexedDefField, IndexedDefSource,
};
use crate::xml_document::model::XmlNodeId;

pub(super) const CACHE_VERSION_V3: u16 = 3;

/// Safety backstop for a rebuilt cache's compressed size -- see `encode`'s doc comment. Chosen
/// well above any realistic Steam Workshop-sized collection's compressed size: the JSON
/// predecessor this replaces was already 158 MB *uncompressed and unverified against any known
/// upper bound*, so 512 MiB of compressed CBOR leaves generous headroom before this ever fires
/// for a genuinely oversized collection rather than a reasonable one.
pub(super) const MAX_CACHE_BYTES: usize = 512 * 1024 * 1024;

/// Safety backstop for a rebuilt cache's Def count, checked independently of `MAX_CACHE_BYTES` --
/// a compact-but-pathological record shape (e.g. many Defs with near-empty fields) could stay
/// under the byte limit while still being too many records to be a reasonable single collection.
/// Chosen well above the ~120k Defs the 158 MB v2 cache this replaces actually contained.
pub(super) const MAX_CACHE_DEFS: usize = 2_000_000;

/// zstd's own documented default level. See this module's doc comment for why decode speed is
/// prioritized over maximum compression ratio here.
const ZSTD_COMPRESSION_LEVEL: i32 = 3;

#[derive(Serialize, Deserialize)]
pub(super) struct DefIndexCacheDtoV3 {
    pub(super) version: u16,
    pub(super) settings_fingerprint: String,
    pub(super) file_fingerprints: Vec<IndexedFileFingerprint>,
    sources: Vec<IndexedDefSource>,
    /// Overflow table for the rare Def whose `(location_id, relative_path)` isn't present in
    /// `file_fingerprints` (e.g. a settings/fingerprint mismatch mid-rebuild). See
    /// `CompactDefDto::path_in_fingerprints`.
    path_overflow: Vec<String>,
    defs: Vec<CompactDefDto>,
    errors: Vec<DefIndexError>,
    built_at_unix_ms: i64,
}

#[derive(Serialize, Deserialize)]
struct CompactDefDto {
    def_type: String,
    def_name: String,
    label: Option<String>,
    parent_name: Option<String>,
    /// When `true`, `path_index` indexes into `DefIndexCacheDtoV3::file_fingerprints` (the common
    /// case: the fingerprint table already carries this exact `(location_id, relative_path)`
    /// pair, so no separate path table entry is needed). When `false`, `path_index` indexes into
    /// `DefIndexCacheDtoV3::path_overflow` instead.
    path_in_fingerprints: bool,
    path_index: u32,
    /// Index into `DefIndexCacheDtoV3::sources`; reconstructs `IndexedDef::source` (and its own
    /// `location_id`).
    source_index: u32,
    node_id: Option<XmlNodeId>,
    line: Option<usize>,
    column: Option<usize>,
    fields: Vec<IndexedDefField>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum CacheCodecError {
    #[error("cache encode failed: {0}")]
    Encode(String),
    #[error("cache compressed size ({actual} bytes) exceeds the {limit}-byte safety limit")]
    TooLarge { actual: usize, limit: usize },
    #[error("cache record count ({actual}) exceeds the {limit}-record safety limit")]
    TooManyRecords { actual: usize, limit: usize },
}

/// Builds the deduplicated DTO from a runtime `DefIndex`. `sources` are deduped by value equality
/// against what's already been collected so far -- cheap in practice, since the number of
/// distinct locations is always far smaller than the number of Defs. Paths are resolved against
/// `file_fingerprints` first (see `CompactDefDto::path_in_fingerprints`) and only fall back to the
/// small `path_overflow` dedup table for a `(location_id, relative_path)` pair that isn't present
/// there.
pub(super) fn to_dto(
    settings_fingerprint: String,
    file_fingerprints: Vec<IndexedFileFingerprint>,
    index: &DefIndex,
) -> DefIndexCacheDtoV3 {
    let fingerprint_lookup: HashMap<(&str, &str), u32> = file_fingerprints
        .iter()
        .enumerate()
        .map(|(i, fp)| {
            (
                (fp.location_id.as_str(), fp.relative_path.as_str()),
                i as u32,
            )
        })
        .collect();

    let mut sources: Vec<IndexedDefSource> = Vec::new();
    let mut path_overflow: Vec<String> = Vec::new();
    let mut overflow_lookup: HashMap<(String, String), u32> = HashMap::new();
    let mut defs = Vec::with_capacity(index.defs.len());

    for def in &index.defs {
        let source_index = match sources.iter().position(|s| *s == def.source) {
            Some(i) => i as u32,
            None => {
                sources.push(def.source.clone());
                (sources.len() - 1) as u32
            }
        };
        let (path_in_fingerprints, path_index) = match fingerprint_lookup
            .get(&(def.source.location_id.as_str(), def.relative_path.as_str()))
        {
            Some(&i) => (true, i),
            None => {
                let path_key = (def.source.location_id.clone(), def.relative_path.clone());
                let i = match overflow_lookup.get(&path_key) {
                    Some(&i) => i,
                    None => {
                        let i = path_overflow.len() as u32;
                        path_overflow.push(path_key.1.clone());
                        overflow_lookup.insert(path_key, i);
                        i
                    }
                };
                (false, i)
            }
        };
        defs.push(CompactDefDto {
            def_type: def.def_type.clone(),
            def_name: def.def_name.clone(),
            label: def.label.clone(),
            parent_name: def.parent_name.clone(),
            path_in_fingerprints,
            path_index,
            source_index,
            node_id: def.node_id,
            line: def.line,
            column: def.column,
            fields: def.fields.clone(),
        });
    }

    DefIndexCacheDtoV3 {
        version: CACHE_VERSION_V3,
        settings_fingerprint,
        file_fingerprints,
        sources,
        path_overflow,
        defs,
        errors: index.errors.clone(),
        built_at_unix_ms: index.built_at_unix_ms,
    }
}

/// Reconstructs `(settings_fingerprint, file_fingerprints, DefIndex)` from a decoded DTO.
/// `def_name_lower`/`label_lower` are left empty -- `#[serde(skip)]` computed fields, always
/// rebuilt by `DefIndex::rebuild_computed_fields` after hydration, exactly like the v2 cache.
///
/// A `path_index`/`source_index` out of range (never produced by `to_dto`, but a hand-corrupted
/// or bit-flipped cache file could still decode successfully at the CBOR/zstd framing level
/// while having invalid indices) drops just that one Def rather than failing the whole cache --
/// consistent with every other "corrupt cache is a benign miss" guarantee elsewhere in this
/// module, just scoped to a single record instead of the whole file.
pub(super) fn from_dto(dto: DefIndexCacheDtoV3) -> (String, Vec<IndexedFileFingerprint>, DefIndex) {
    let defs = dto
        .defs
        .into_iter()
        .filter_map(|c| {
            let relative_path = if c.path_in_fingerprints {
                dto.file_fingerprints
                    .get(c.path_index as usize)?
                    .relative_path
                    .clone()
            } else {
                dto.path_overflow.get(c.path_index as usize)?.clone()
            };
            let source = dto.sources.get(c.source_index as usize)?.clone();
            Some(IndexedDef {
                key: DefIdentityKey {
                    def_type: c.def_type.clone(),
                    def_name: c.def_name.clone(),
                },
                def_type: c.def_type,
                def_name: c.def_name,
                label: c.label,
                parent_name: c.parent_name,
                relative_path,
                node_id: c.node_id,
                line: c.line,
                column: c.column,
                source,
                fields: c.fields,
                def_name_lower: String::new(),
                label_lower: String::new(),
            })
        })
        .collect();
    let index = DefIndex {
        defs,
        errors: dto.errors,
        built_at_unix_ms: dto.built_at_unix_ms,
        by_type: Default::default(),
    };
    (dto.settings_fingerprint, dto.file_fingerprints, index)
}

/// Encodes and compresses a DTO. Returns `CacheCodecError::TooManyRecords` or `TooLarge` (never
/// panics or truncates) if the Def count or compressed size exceeds their respective safety
/// limits -- callers must not write the returned bytes to disk in that case; see `write_cache`'s
/// handling.
pub(super) fn encode(dto: &DefIndexCacheDtoV3) -> Result<Vec<u8>, CacheCodecError> {
    encode_with_limits(dto, MAX_CACHE_DEFS, MAX_CACHE_BYTES)
}

/// `encode`'s implementation, parameterized on both safety limits so tests can force each
/// guardrail to fire deterministically without constructing an actually-oversized fixture.
fn encode_with_limits(
    dto: &DefIndexCacheDtoV3,
    def_limit: usize,
    byte_limit: usize,
) -> Result<Vec<u8>, CacheCodecError> {
    if dto.defs.len() > def_limit {
        return Err(CacheCodecError::TooManyRecords {
            actual: dto.defs.len(),
            limit: def_limit,
        });
    }
    let mut cbor = Vec::new();
    ciborium::into_writer(dto, &mut cbor).map_err(|e| CacheCodecError::Encode(e.to_string()))?;
    let compressed = zstd::stream::encode_all(&cbor[..], ZSTD_COMPRESSION_LEVEL)
        .map_err(|e| CacheCodecError::Encode(e.to_string()))?;
    if compressed.len() > byte_limit {
        return Err(CacheCodecError::TooLarge {
            actual: compressed.len(),
            limit: byte_limit,
        });
    }
    Ok(compressed)
}

/// Decompresses and decodes cache bytes. Returns `None` for any decompression or decode error --
/// a corrupt, truncated, or foreign-format file is always a benign miss, never a hard error.
pub(super) fn decode(bytes: &[u8]) -> Option<DefIndexCacheDtoV3> {
    let decompressed = zstd::stream::decode_all(bytes).ok()?;
    ciborium::from_reader(&decompressed[..]).ok()
}

/// Test-only: builds valid v3 cache bytes but with `version` overridden -- used by
/// `def_index::tests::cache` to exercise the real `read_cache_file`/`load_cached_index_only`
/// integration path against an "incompatible version" cache, without needing a public way to
/// mutate a `DefIndexCacheDtoV3`'s otherwise-private fields from outside this module.
#[cfg(test)]
pub(super) fn encode_with_version_for_test(
    settings_fingerprint: String,
    file_fingerprints: Vec<IndexedFileFingerprint>,
    index: &DefIndex,
    version: u16,
) -> Vec<u8> {
    let mut dto = to_dto(settings_fingerprint, file_fingerprints, index);
    dto.version = version;
    encode(&dto).expect("test fixture must encode")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def_index::model::IndexedSourceKind;
    use crate::project_model::SourceType;

    fn source(location_id: &str) -> IndexedDefSource {
        IndexedDefSource {
            location_id: location_id.to_string(),
            location_name: format!("{} display", location_id),
            source_kind: IndexedSourceKind::Project,
            source_type: SourceType::Folder,
            read_only: false,
            mod_id: None,
            game_version: None,
            expansion_name: None,
        }
    }

    fn def(def_type: &str, def_name: &str, location_id: &str, relative_path: &str) -> IndexedDef {
        IndexedDef {
            key: DefIdentityKey {
                def_type: def_type.to_string(),
                def_name: def_name.to_string(),
            },
            def_type: def_type.to_string(),
            def_name: def_name.to_string(),
            label: Some(format!("{} label", def_name)),
            parent_name: None,
            relative_path: relative_path.to_string(),
            node_id: None,
            line: Some(3),
            column: Some(5),
            source: source(location_id),
            fields: vec![],
            def_name_lower: String::new(),
            label_lower: String::new(),
        }
    }

    #[test]
    fn to_dto_dedupes_identical_sources_and_overflow_paths_across_defs() {
        // No `file_fingerprints` supplied, so every Def's path misses the fingerprint table and
        // falls back to `path_overflow` -- this test exercises that fallback table's own
        // deduplication, independent of the (separately tested) fingerprint-reuse path.
        let index = DefIndex {
            defs: vec![
                def("ThingDef", "Steel", "loc-a", "Defs/a.xml"),
                def("ThingDef", "Wood", "loc-a", "Defs/a.xml"),
                def("ThingDef", "Plastic", "loc-a", "Defs/b.xml"),
                def("ThingDef", "Cloth", "loc-b", "Defs/a.xml"),
            ],
            errors: vec![],
            built_at_unix_ms: 1234,
            by_type: Default::default(),
        };

        let dto = to_dto("fp".to_string(), vec![], &index);

        assert_eq!(dto.sources.len(), 2, "one entry per distinct location");
        // `loc-a` + `Defs/a.xml` and `loc-b` + `Defs/a.xml` are distinct (location, path) pairs
        // even though the relative path string is identical.
        assert_eq!(dto.path_overflow.len(), 3);
        assert_eq!(dto.defs.len(), 4);
        assert!(dto.defs.iter().all(|d| !d.path_in_fingerprints));
    }

    #[test]
    fn to_dto_reuses_the_file_fingerprint_table_instead_of_a_separate_path_entry() {
        // The common case: every Def's (location, path) is already present in `file_fingerprints`
        // (as it always is for a Def parsed out of a file that was also fingerprinted during the
        // same scan), so `path_overflow` should stay empty entirely.
        let fingerprints = vec![
            IndexedFileFingerprint {
                location_id: "loc-a".to_string(),
                relative_path: "Defs/a.xml".to_string(),
                byte_len: 1,
                modified_unix_ms: None,
                content_hash: "h1".to_string(),
            },
            IndexedFileFingerprint {
                location_id: "loc-a".to_string(),
                relative_path: "Defs/b.xml".to_string(),
                byte_len: 2,
                modified_unix_ms: None,
                content_hash: "h2".to_string(),
            },
        ];
        let index = DefIndex {
            defs: vec![
                def("ThingDef", "Steel", "loc-a", "Defs/a.xml"),
                def("ThingDef", "Wood", "loc-a", "Defs/a.xml"),
                def("ThingDef", "Plastic", "loc-a", "Defs/b.xml"),
            ],
            errors: vec![],
            built_at_unix_ms: 1,
            by_type: Default::default(),
        };

        let dto = to_dto("fp".to_string(), fingerprints, &index);

        assert!(
            dto.path_overflow.is_empty(),
            "every path is already covered by file_fingerprints"
        );
        assert!(dto.defs.iter().all(|d| d.path_in_fingerprints));
    }

    #[test]
    fn to_dto_falls_back_to_overflow_for_a_def_missing_from_the_fingerprint_table() {
        // A Def whose (location, path) isn't in `file_fingerprints` (e.g. captured from a
        // slightly different scan snapshot) must not be dropped -- it falls back to the overflow
        // table rather than requiring an exact fingerprint-table hit.
        let fingerprints = vec![IndexedFileFingerprint {
            location_id: "loc-a".to_string(),
            relative_path: "Defs/a.xml".to_string(),
            byte_len: 1,
            modified_unix_ms: None,
            content_hash: "h1".to_string(),
        }];
        let index = DefIndex {
            defs: vec![def("ThingDef", "Steel", "loc-a", "Defs/missing.xml")],
            errors: vec![],
            built_at_unix_ms: 1,
            by_type: Default::default(),
        };

        let dto = to_dto("fp".to_string(), fingerprints, &index);

        assert_eq!(dto.path_overflow, vec!["Defs/missing.xml".to_string()]);
        assert!(!dto.defs[0].path_in_fingerprints);

        let bytes = encode(&dto).unwrap();
        let decoded = decode(&bytes).unwrap();
        let (_, _, rebuilt) = from_dto(decoded);
        assert_eq!(rebuilt.defs[0].relative_path, "Defs/missing.xml");
    }

    #[test]
    fn round_trip_preserves_every_indexed_def_field_and_fingerprint() {
        let fingerprints = vec![IndexedFileFingerprint {
            location_id: "loc-a".to_string(),
            relative_path: "Defs/a.xml".to_string(),
            byte_len: 42,
            modified_unix_ms: Some(999),
            content_hash: "abc123".to_string(),
        }];
        let mut d = def("ThingDef", "Steel", "loc-a", "Defs/a.xml");
        d.parent_name = Some("BaseThing".to_string());
        d.node_id = None;
        d.fields = vec![IndexedDefField {
            name: "statBases".to_string(),
            text_value: Some("<Mass>1.0</Mass>".to_string()),
            line: Some(7),
            column: Some(2),
        }];
        let index = DefIndex {
            defs: vec![d.clone()],
            errors: vec![DefIndexError {
                location_id: "loc-a".to_string(),
                location_name: "Loc A".to_string(),
                source_kind: IndexedSourceKind::Project,
                relative_path: Some("Defs/broken.xml".to_string()),
                code: "parse_error".to_string(),
                message: "boom".to_string(),
                line: Some(1),
                column: Some(1),
                args: crate::diagnostics::DiagnosticArgs::new(),
            }],
            built_at_unix_ms: 5555,
            by_type: Default::default(),
        };

        let dto = to_dto("fp1".to_string(), fingerprints.clone(), &index);
        let bytes = encode(&dto).expect("encode must succeed for a small fixture");
        let decoded = decode(&bytes).expect("decode must succeed for freshly encoded bytes");
        let (settings_fp, file_fps, rebuilt) = from_dto(decoded);

        assert_eq!(settings_fp, "fp1");
        assert_eq!(file_fps, fingerprints);
        assert_eq!(rebuilt.built_at_unix_ms, 5555);
        assert_eq!(rebuilt.defs.len(), 1);
        let round_tripped = &rebuilt.defs[0];
        assert_eq!(round_tripped.key, d.key);
        assert_eq!(round_tripped.def_type, d.def_type);
        assert_eq!(round_tripped.def_name, d.def_name);
        assert_eq!(round_tripped.label, d.label);
        assert_eq!(round_tripped.parent_name, d.parent_name);
        assert_eq!(round_tripped.relative_path, d.relative_path);
        assert_eq!(round_tripped.line, d.line);
        assert_eq!(round_tripped.column, d.column);
        assert_eq!(round_tripped.source, d.source);
        assert_eq!(round_tripped.fields.len(), 1);
        assert_eq!(round_tripped.fields[0].name, "statBases");
        assert_eq!(
            round_tripped.fields[0].text_value.as_deref(),
            Some("<Mass>1.0</Mass>")
        );
        // Computed fields are never persisted -- must come back empty until
        // `rebuild_computed_fields` restores them, exactly like the v2 cache.
        assert_eq!(round_tripped.def_name_lower, "");
        assert_eq!(round_tripped.label_lower, "");

        assert_eq!(rebuilt.errors.len(), 1);
        assert_eq!(rebuilt.errors[0].code, "parse_error");
    }

    #[test]
    fn round_trip_preserves_an_error_with_empty_diagnostic_args() {
        // Regression: `DefIndexError.args` is `skip_serializing_if`-omitted when empty. Bincode
        // (rejected for exactly this reason -- see this module's doc comment) would misalign its
        // positional field sequence in this exact case; CBOR must not.
        let index = DefIndex {
            defs: vec![],
            errors: vec![DefIndexError {
                location_id: "loc-a".to_string(),
                location_name: "Loc A".to_string(),
                source_kind: IndexedSourceKind::Source,
                relative_path: None,
                code: "scan_failed".to_string(),
                message: "boom".to_string(),
                line: None,
                column: None,
                args: crate::diagnostics::DiagnosticArgs::new(),
            }],
            built_at_unix_ms: 1,
            by_type: Default::default(),
        };
        let dto = to_dto("fp".to_string(), vec![], &index);
        let bytes = encode(&dto).unwrap();
        let decoded = decode(&bytes).unwrap();
        let (_, _, rebuilt) = from_dto(decoded);

        assert_eq!(rebuilt.errors.len(), 1);
        assert_eq!(rebuilt.errors[0].code, "scan_failed");
        assert!(rebuilt.errors[0].args.is_empty());
    }

    #[test]
    fn decode_rejects_corrupt_bytes_as_a_benign_miss() {
        assert!(decode(b"not a valid cache file").is_none());
    }

    #[test]
    fn encode_reports_too_large_instead_of_writing_an_oversized_cache() {
        let index = DefIndex {
            defs: vec![def("ThingDef", "Steel", "loc-a", "Defs/a.xml")],
            errors: vec![],
            built_at_unix_ms: 1,
            by_type: Default::default(),
        };
        let dto = to_dto("fp".to_string(), vec![], &index);

        let err = encode_with_limits(&dto, MAX_CACHE_DEFS, 1)
            .expect_err("a 1-byte limit must always be exceeded");
        match err {
            CacheCodecError::TooLarge { actual, limit } => {
                assert!(actual > 1);
                assert_eq!(limit, 1);
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }

        // The real (generous) limit must not reject the same tiny fixture.
        assert!(encode(&dto).is_ok());
    }

    #[test]
    fn encode_reports_too_many_records_instead_of_writing_an_oversized_cache() {
        let index = DefIndex {
            defs: vec![
                def("ThingDef", "Steel", "loc-a", "Defs/a.xml"),
                def("ThingDef", "Wood", "loc-a", "Defs/a.xml"),
            ],
            errors: vec![],
            built_at_unix_ms: 1,
            by_type: Default::default(),
        };
        let dto = to_dto("fp".to_string(), vec![], &index);

        let err = encode_with_limits(&dto, 1, MAX_CACHE_BYTES)
            .expect_err("a 1-record limit must always be exceeded by 2 defs");
        match err {
            CacheCodecError::TooManyRecords { actual, limit } => {
                assert_eq!(actual, 2);
                assert_eq!(limit, 1);
            }
            other => panic!("expected TooManyRecords, got {other:?}"),
        }

        // The real (generous) limit must not reject the same tiny fixture.
        assert!(encode(&dto).is_ok());
    }

    #[test]
    fn v3_encoding_is_meaningfully_smaller_than_pretty_json_for_a_representative_collection() {
        // A synthetic stand-in for a large real-world collection: many Defs sharing a small
        // number of distinct locations/files, which is exactly the shape source/path
        // deduplication targets -- see this module's doc comment.
        let mut defs = Vec::new();
        for i in 0..2000 {
            let location_id = format!("loc-{}", i % 20);
            let relative_path = format!("Defs/Things/file{}.xml", i % 200);
            let mut d = def("ThingDef", &format!("Def{i}"), &location_id, &relative_path);
            d.fields = vec![IndexedDefField {
                name: "statBases".to_string(),
                text_value: Some("<Mass>1.0</Mass><MarketValue>10</MarketValue>".to_string()),
                line: Some(10),
                column: Some(4),
            }];
            defs.push(d);
        }
        let index = DefIndex {
            defs,
            errors: vec![],
            built_at_unix_ms: 1,
            by_type: Default::default(),
        };

        // Stand-in for the old v2 cache format: a full `DefIndex` (with every Def's `key` and
        // full `IndexedDefSource` duplicated inline) serialized as pretty JSON.
        let json = serde_json::to_string_pretty(&index).unwrap();

        let dto = to_dto("fp".to_string(), vec![], &index);
        let compressed = encode(&dto).unwrap();

        assert!(
            compressed.len() < json.len() / 3,
            "expected the compact+compressed v3 cache ({} bytes) to be well under a third of \
             the pretty-JSON size ({} bytes) for a collection with heavy source/path duplication",
            compressed.len(),
            json.len()
        );
    }
}
