// Re-export facade: all public items from def_index submodules live here.
// Some re-exports are only referenced by name in test code or appear only in
// function-signature types, so they look unused to cargo check. Allow the lint
// for the entire facade rather than annotating every line.
#![allow(unused_imports)]

mod builder;
mod cache;
mod fingerprint;
mod model;
mod overlay;
mod query;
mod state;

pub(crate) use builder::{apply_file_change, indexed_source_for_location, normalize_relative_path};
pub use builder::{build_def_index, DefIndexBuildOptions};
pub use cache::{
    cache_state_inputs, load_or_rebuild_def_index, rebuild_and_store_def_index,
    store_prebuilt_index, DefIndexCacheError,
};
pub(crate) use fingerprint::settings_fingerprint;
pub use fingerprint::IndexedFileFingerprint;
pub use model::{
    DefDuplicateQueryResult, DefIdentityKey, DefIndex, DefIndexError, DefIndexFacetSummary,
    DefIndexReplacement, DefIndexSearchQuery, DefIndexSummary, DefReferenceResolution,
    DefReferenceSuggestion, DefTypeFacet, IndexedDef, IndexedDefField, IndexedDefSearchResult,
    IndexedDefSource, IndexedSourceKind,
};
pub use overlay::apply_replacement_overlay;
pub use query::{
    get_facet_summary, resolve_def_reference, search_def_results, suggest_def_references,
    summarize_index,
};
pub use state::{DefIndexState, IndexingPhase, IndexingStatus};

#[cfg(test)]
mod tests;
