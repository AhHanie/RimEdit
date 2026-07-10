use crate::schema_pack::ReferenceScope;

use super::model::{
    DefIndex, DefIndexFacetSummary, DefIndexSearchQuery, DefIndexSummary, DefReferenceResolution,
    DefReferenceSuggestion, DefTypeFacet, IndexedDef, IndexedDefSearchResult, IndexedSourceKind,
};

const RANK_EMPTY_OR_EXACT_NAME: usize = 1;
const RANK_PREFIX_NAME: usize = 2;
const RANK_PARTIAL_NAME: usize = 3;
const RANK_EXACT_LABEL: usize = 4;
const RANK_PREFIX_LABEL: usize = 5;
const RANK_PARTIAL_LABEL: usize = 6;
const RANK_EXACT_DEF_TYPE: usize = 7;
const RANK_PREFIX_DEF_TYPE: usize = 8;
const RANK_PARTIAL_DEF_TYPE: usize = 9;
const RANK_PARTIAL_PATH: usize = 10;

fn ranked_search<'a>(
    index: &'a DefIndex,
    query: &DefIndexSearchQuery,
) -> Vec<(&'a IndexedDef, usize)> {
    let needle = query.query.trim().to_lowercase();
    let mut results = Vec::new();

    for def in &index.defs {
        if !query.include_sources && def.source.source_kind == IndexedSourceKind::Source {
            continue;
        }
        if query.def_type.as_deref().is_some_and(|t| t != def.def_type) {
            continue;
        }

        let def_name_owned;
        let def_name = if !def.def_name_lower.is_empty() || def.def_name.is_empty() {
            def.def_name_lower.as_str()
        } else {
            def_name_owned = def.def_name.to_lowercase();
            def_name_owned.as_str()
        };
        let label_owned;
        let label = if !def.label_lower.is_empty() || def.label.is_none() {
            def.label_lower.as_str()
        } else {
            label_owned = def.label.as_deref().unwrap_or("").to_lowercase();
            label_owned.as_str()
        };
        let def_type = def.def_type.to_lowercase();
        let path = def.relative_path.to_lowercase();

        let rank = if needle.is_empty() || def_name == needle {
            RANK_EMPTY_OR_EXACT_NAME
        } else if def_name.starts_with(&needle) {
            RANK_PREFIX_NAME
        } else if def_name.contains(&needle) {
            RANK_PARTIAL_NAME
        } else if !label.is_empty() && label == needle {
            RANK_EXACT_LABEL
        } else if !label.is_empty() && label.starts_with(&needle) {
            RANK_PREFIX_LABEL
        } else if !label.is_empty() && label.contains(&needle) {
            RANK_PARTIAL_LABEL
        } else if def_type == needle {
            RANK_EXACT_DEF_TYPE
        } else if def_type.starts_with(&needle) {
            RANK_PREFIX_DEF_TYPE
        } else if def_type.contains(&needle) {
            RANK_PARTIAL_DEF_TYPE
        } else if path.contains(&needle) {
            RANK_PARTIAL_PATH
        } else {
            continue;
        };
        results.push((def, rank));
    }

    results.sort_by(|(a, ar), (b, br)| {
        ar.cmp(br)
            .then_with(|| {
                let ap = matches!(a.source.source_kind, IndexedSourceKind::Project);
                let bp = matches!(b.source.source_kind, IndexedSourceKind::Project);
                bp.cmp(&ap)
            })
            .then_with(|| a.def_type.cmp(&b.def_type))
            .then_with(|| a.def_name.cmp(&b.def_name))
            .then_with(|| a.source.location_name.cmp(&b.source.location_name))
            .then_with(|| a.relative_path.cmp(&b.relative_path))
            .then_with(|| {
                a.line
                    .unwrap_or(usize::MAX)
                    .cmp(&b.line.unwrap_or(usize::MAX))
            })
    });
    if let Some(limit) = query.limit {
        results.truncate(limit);
    }
    results
}

pub fn search_def_results(
    index: &DefIndex,
    query: &DefIndexSearchQuery,
) -> Vec<IndexedDefSearchResult> {
    ranked_search(index, query)
        .into_iter()
        .map(|(def, rank)| IndexedDefSearchResult {
            def: def.clone(),
            rank,
        })
        .collect()
}

pub fn summarize_index(index: &DefIndex) -> DefIndexSummary {
    DefIndexSummary {
        indexed_defs: index.defs.len(),
        project_defs: index
            .defs
            .iter()
            .filter(|d| d.source.source_kind == IndexedSourceKind::Project)
            .count(),
        source_defs: index
            .defs
            .iter()
            .filter(|d| d.source.source_kind == IndexedSourceKind::Source)
            .count(),
        errors: index.errors.len(),
        built_at_unix_ms: index.built_at_unix_ms,
    }
}

pub fn get_facet_summary(index: &DefIndex, include_sources: bool) -> DefIndexFacetSummary {
    let mut type_map: std::collections::HashMap<String, (usize, usize)> = Default::default();
    for def in &index.defs {
        if !include_sources && def.source.source_kind == IndexedSourceKind::Source {
            continue;
        }
        let e = type_map.entry(def.def_type.clone()).or_default();
        match def.source.source_kind {
            IndexedSourceKind::Project => e.0 += 1,
            IndexedSourceKind::Source => e.1 += 1,
        }
    }
    let mut def_types: Vec<DefTypeFacet> = type_map
        .into_iter()
        .map(|(def_type, (pc, sc))| DefTypeFacet {
            total_count: pc + sc,
            def_type,
            project_count: pc,
            source_count: sc,
        })
        .collect();
    def_types.sort_by(|a, b| {
        b.total_count
            .cmp(&a.total_count)
            .then_with(|| a.def_type.cmp(&b.def_type))
    });
    DefIndexFacetSummary {
        project_defs: index
            .defs
            .iter()
            .filter(|d| d.source.source_kind == IndexedSourceKind::Project)
            .count(),
        source_defs: if include_sources {
            index
                .defs
                .iter()
                .filter(|d| d.source.source_kind == IndexedSourceKind::Source)
                .count()
        } else {
            0
        },
        errors: index.errors.len(),
        def_types,
    }
}

fn cmp_suggestion_results(
    (a, ar): &(&IndexedDef, usize),
    (b, br): &(&IndexedDef, usize),
) -> std::cmp::Ordering {
    ar.cmp(br)
        .then_with(|| {
            let ap = matches!(a.source.source_kind, IndexedSourceKind::Project);
            let bp = matches!(b.source.source_kind, IndexedSourceKind::Project);
            bp.cmp(&ap)
        })
        .then_with(|| a.def_name.cmp(&b.def_name))
        .then_with(|| a.source.location_name.cmp(&b.source.location_name))
}

pub fn suggest_def_references(
    index: &DefIndex,
    target_def_types: &[&str],
    query: &str,
    scope: &ReferenceScope,
    limit: usize,
) -> Vec<DefReferenceSuggestion> {
    let needle = query.trim().to_lowercase();
    let mut results: Vec<(&IndexedDef, usize)> = Vec::new();

    // Use the pre-built by_type map to skip irrelevant defs; fall back to full scan
    // when the map is empty (e.g. freshly deserialised overlays in tests).
    let candidates: Vec<&IndexedDef> = if index.by_type.is_empty() {
        index
            .defs
            .iter()
            .filter(|d| target_def_types.contains(&d.def_type.as_str()))
            .collect()
    } else {
        target_def_types
            .iter()
            .flat_map(|t| index.by_type.get(*t).map(|v| v.as_slice()).unwrap_or(&[]))
            .map(|&i| &index.defs[i])
            .collect()
    };

    for def in candidates {
        if *scope == ReferenceScope::ProjectOnly && def.source.read_only {
            continue;
        }

        let def_name_owned;
        let def_name = if !def.def_name_lower.is_empty() || def.def_name.is_empty() {
            def.def_name_lower.as_str()
        } else {
            def_name_owned = def.def_name.to_lowercase();
            def_name_owned.as_str()
        };
        let label_owned;
        let label = if !def.label_lower.is_empty() || def.label.is_none() {
            def.label_lower.as_str()
        } else {
            label_owned = def.label.as_deref().unwrap_or("").to_lowercase();
            label_owned.as_str()
        };

        let rank = if needle.is_empty() || def_name == needle {
            RANK_EMPTY_OR_EXACT_NAME
        } else if def_name.starts_with(&needle) {
            RANK_PREFIX_NAME
        } else if def_name.contains(&needle) {
            RANK_PARTIAL_NAME
        } else if !label.is_empty() && label == needle {
            RANK_EXACT_LABEL
        } else if !label.is_empty() && label.starts_with(&needle) {
            RANK_PREFIX_LABEL
        } else if !label.is_empty() && label.contains(&needle) {
            RANK_PARTIAL_LABEL
        } else {
            continue;
        };
        results.push((def, rank));
    }

    // Partial sort: only pay O(n log k) when results exceed the limit.
    if results.len() > limit {
        results.select_nth_unstable_by(limit - 1, cmp_suggestion_results);
        results.truncate(limit);
    }
    results.sort_unstable_by(cmp_suggestion_results);

    results
        .into_iter()
        .map(|(def, rank)| DefReferenceSuggestion {
            def_name: def.def_name.clone(),
            def_type: def.def_type.clone(),
            label: def.label.clone(),
            relative_path: def.relative_path.clone(),
            node_id: def.node_id,
            line: def.line,
            column: def.column,
            location_id: def.source.location_id.clone(),
            location_name: def.source.location_name.clone(),
            read_only: def.source.read_only,
            rank,
        })
        .collect()
}

pub fn resolve_def_reference(
    index: &DefIndex,
    target_def_types: &[&str],
    def_name: &str,
    scope: &ReferenceScope,
) -> DefReferenceResolution {
    let mut project_matches: Vec<&IndexedDef> = Vec::new();
    let mut source_matches: Vec<&IndexedDef> = Vec::new();

    for def_type in target_def_types {
        for def in index.find_by_key(def_type, def_name) {
            if def.source.read_only {
                // ProjectOnly scope: read-only source defs are out of scope.
                if *scope != ReferenceScope::ProjectOnly {
                    source_matches.push(def);
                }
            } else {
                project_matches.push(def);
            }
        }
    }

    if !project_matches.is_empty() {
        if project_matches.len() > 1 {
            return DefReferenceResolution::Ambiguous;
        }
        let def = project_matches[0];
        return DefReferenceResolution::EditableProjectDef {
            relative_path: def.relative_path.clone(),
            node_id: def.node_id,
        };
    }

    if source_matches.len() > 1 {
        return DefReferenceResolution::Ambiguous;
    }

    if let Some(def) = source_matches.into_iter().next() {
        return DefReferenceResolution::ReadOnlySourceDef {
            location_id: def.source.location_id.clone(),
            relative_path: def.relative_path.clone(),
            node_id: def.node_id,
        };
    }

    DefReferenceResolution::Missing
}

#[allow(dead_code)]
impl DefIndex {
    pub fn search(&self, query: &DefIndexSearchQuery) -> Vec<&IndexedDef> {
        ranked_search(self, query)
            .into_iter()
            .map(|(def, _)| def)
            .collect()
    }
}
