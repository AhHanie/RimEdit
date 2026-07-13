//! Tauri-facing glue for the patch preview engine (see
//! `docs/patches-editor/07-preview-engine.md`). Combines every indexable Def XML file into one
//! document (in RimWorld load order), applies every patch operation from every patch file
//! (preview-only enable/disable and reorder are scoped to the operations that affect the
//! requested Def, but application always runs the full patch stream -- exactly like RimWorld
//! itself, which has no notion of "only apply patches touching Def X"), resolves XML inheritance,
//! and returns the requested Def's final XML plus diagnostics.
//!
//! The heavy lifting (`compute_def_preview`) takes plain data and does not touch `AppHandle`, so
//! it is directly unit-testable; [`preview_def_for_project`] is the thin Tauri-aware wrapper that
//! loads settings/schema catalog/patch index and reads Def files from disk.

mod conflicts;
mod model;
mod operation_lookup;
mod preview;
mod reorder;
mod scan;
mod selection;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use model::{
    PatchPreviewConflictDiagnostic, PatchPreviewImpactSummary, PatchPreviewOperationSummary,
    PatchPreviewRequest, PatchPreviewResult, PatchPreviewTarget, PreviewInputs,
};
#[allow(unused_imports)]
pub use preview::{compute_def_preview, preview_def_for_project};
