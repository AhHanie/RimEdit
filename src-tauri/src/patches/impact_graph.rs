//! Statically infers which Def(s) a patch operation's XPath targets, and builds a queryable
//! graph over an already-built [`super::index::PatchIndex`] connecting operations to those
//! targets.
//!
//! Target inference here is intentionally conservative: it only recognizes
//! `Defs/<DefType>`, `Defs/<DefType>[defName="<Name>"]`, and an OR-only chain of 2+ such
//! `defName="..."` equalities (`Defs/<DefType>[defName="A" or defName="B" or ...]`, a common
//! pattern for one operation targeting several Defs at once), optionally with a leading `/`
//! and/or further child segments, which are ignored. Everything else -- attribute predicates
//! such as `[@Name=...]`/`[@ParentName=...]`, wildcards, `and`-combined or otherwise mixed
//! predicates, XPath functions, axes other than a plain child path -- is reported as
//! [`XPathTarget::Unsupported`] rather than guessed at. This mirrors the plan's XPath
//! autocomplete/target-inference boundary: preview evaluation may use the full backend XML
//! library later, but static impact inference only trusts a conservative subset and says so
//! otherwise.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::def_index::DefIdentityKey;

use super::index::{IndexedPatchOperation, PatchIndex, PatchIndexFile};
use super::model::PatchOperationId;

/// Statically inferred target of a patch operation's XPath.
///
/// `#[serde(rename_all = "camelCase")]` at the enum level only renames the `kind` tag itself
/// (`Def` -> `"def"`, `Defs` -> `"defs"`, ...) -- it does **not** cascade into a struct variant's
/// own field names, so each struct-shaped variant below repeats `rename_all = "camelCase"` at the
/// variant level too (a documented, independent serde attribute), otherwise fields like
/// `def_type`/`def_name` would serialize verbatim as snake_case on the wire despite every
/// Rust->TS type in this codebase otherwise being camelCase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum XPathTarget {
    /// e.g. `Defs/ThingDef[defName="Wall"]` -- targets exactly one Def by identity.
    #[serde(rename_all = "camelCase")]
    Def { def_type: String, def_name: String },
    /// e.g. `Defs/ThingDef` -- targets every Def of a type rather than one specific instance.
    #[serde(rename_all = "camelCase")]
    DefType { def_type: String },
    /// e.g. `Defs/ThingDef[defName="A" or defName="B" or ...]` -- targets 2+ specific Defs of
    /// the same type via an OR-only chain of `defName="..."` equalities. Anything else mixed
    /// into the predicate (`and`, `@Name`, a lone unparseable term) is left `Unsupported` rather
    /// than guessed at, same as the single-name case.
    #[serde(rename_all = "camelCase")]
    Defs {
        def_type: String,
        def_names: Vec<String>,
    },
    /// The XPath is present but doesn't match the conservative patterns above (attribute
    /// predicates, wildcards, functions, multiple predicates, non-`Defs`-rooted paths, ...).
    Unsupported,
    /// The operation has no XPath at all (e.g. `PatchOperationSequence`/`PatchOperationFindMod`
    /// containers, or a pathed operation whose `<xpath>` field failed to parse).
    NoXPath,
}

/// Infer the static [`XPathTarget`] of a patch operation's XPath string.
///
/// Only looks at the `Defs/<DefType>[...]` prefix; further child path segments (e.g.
/// `/statBases`) are ignored since they don't change which Def the operation affects -- *unless*
/// one of them is a parent axis (`..`), which can walk back out of the targeted Def and into a
/// sibling, in which case the whole expression is reported as unsupported rather than trusting
/// the prefix.
pub fn infer_xpath_target(xpath: &str) -> XPathTarget {
    let trimmed = xpath.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return XPathTarget::Unsupported;
    }

    let mut segments = trimmed.split('/');
    let Some(first) = segments.next() else {
        return XPathTarget::Unsupported;
    };
    if first != "Defs" {
        return XPathTarget::Unsupported;
    }

    let target = match segments.next() {
        Some(second) => parse_def_type_segment(second),
        // "Defs" or "Defs/" alone doesn't identify a def type.
        None => return XPathTarget::Unsupported,
    };

    if segments.any(|s| s == "..") {
        return XPathTarget::Unsupported;
    }

    target
}

fn parse_def_type_segment(segment: &str) -> XPathTarget {
    match segment.find('[') {
        None => {
            if is_valid_identifier(segment) {
                XPathTarget::DefType {
                    def_type: segment.to_string(),
                }
            } else {
                XPathTarget::Unsupported
            }
        }
        Some(bracket_start) => {
            let def_type = &segment[..bracket_start];
            if !is_valid_identifier(def_type) {
                return XPathTarget::Unsupported;
            }
            let Some(bracket_end) = segment.rfind(']') else {
                return XPathTarget::Unsupported;
            };
            if bracket_end != segment.len() - 1 {
                // Trailing content after the closing bracket in the same segment (e.g. a
                // second predicate) is beyond the conservative subset.
                return XPathTarget::Unsupported;
            }
            let predicate = &segment[bracket_start + 1..bracket_end];
            match parse_def_name_predicate(predicate) {
                Some(def_name) => XPathTarget::Def {
                    def_type: def_type.to_string(),
                    def_name,
                },
                None => match parse_def_name_or_chain(predicate) {
                    Some(def_names) => XPathTarget::Defs {
                        def_type: def_type.to_string(),
                        def_names,
                    },
                    None => XPathTarget::Unsupported,
                },
            }
        }
    }
}

/// Recognizes only `defName="..."` / `defName='...'` predicates. Anything else -- `@Name`,
/// `@ParentName`, positional predicates, boolean combinators -- is left unsupported: those
/// target inheritance templates or structural positions rather than a concrete `defName`-keyed
/// Def, which is a different (and, for `@Name`/`@ParentName`, autocomplete-only) concern.
fn parse_def_name_predicate(predicate: &str) -> Option<String> {
    let rest = predicate.trim().strip_prefix("defName")?;
    let rest = rest.trim_start().strip_prefix('=')?;
    let rest = rest.trim();
    let value = rest
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| rest.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))?;
    if value.is_empty() || value.contains(['"', '\'']) {
        return None;
    }
    Some(value.to_string())
}

/// Recognizes an OR-only chain of 2+ `defName="..."` equality terms, e.g.
/// `defName="A" or defName="B" or defName="C"` -- a common real-world pattern for one patch
/// operation targeting several Defs at once. Any term that isn't a clean `defName="..."`
/// equality (an `and`, an `@Name`/`@ParentName` term, a malformed value) fails the whole chain
/// closed to `None` rather than guessing, matching [`parse_def_name_predicate`]'s conservatism.
fn parse_def_name_or_chain(predicate: &str) -> Option<Vec<String>> {
    let terms = split_top_level_or_terms(predicate);
    if terms.len() < 2 {
        return None;
    }
    terms
        .into_iter()
        .map(|term| parse_def_name_predicate(term.trim()))
        .collect()
}

/// Splits `predicate` on standalone `or` tokens that are not inside a quoted string (so a
/// `defName` value that happens to contain the literal substring "or", e.g. `MN_NetworkController`
/// or a value like `"A or B"`, is never mistaken for a separator). A candidate `or` is only
/// treated as a separator when the characters immediately before and after it (if any) are not
/// identifier characters, so it can't false-split inside a longer word either.
fn split_top_level_or_terms(predicate: &str) -> Vec<&str> {
    let bytes = predicate.as_bytes();
    let mut terms = Vec::new();
    let mut term_start = 0usize;
    let mut in_quote: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match in_quote {
            Some(q) => {
                if b == q {
                    in_quote = None;
                }
                i += 1;
            }
            None => {
                if b == b'"' || b == b'\'' {
                    in_quote = Some(b);
                    i += 1;
                } else if b == b'o' {
                    let is_or = predicate[i..].starts_with("or");
                    let before_ok = i == 0 || !is_identifier_byte(bytes[i - 1]);
                    let after_idx = i + 2;
                    let after_ok =
                        after_idx >= bytes.len() || !is_identifier_byte(bytes[after_idx]);
                    if is_or && before_ok && after_ok {
                        terms.push(&predicate[term_start..i]);
                        i = after_idx;
                        term_start = i;
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
    }
    terms.push(&predicate[term_start..]);
    terms
}

fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Shared with `patches::xpath`, which needs the same "plain element name" check for completion
/// contexts (def type / field segments) that aren't yet full `XPathTarget`s.
pub(super) fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Identifies one indexed operation by its file and in-file operation id, for graph query
/// results that need to point back at a specific operation without embedding the whole
/// [`IndexedPatchOperation`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchImpactRef {
    pub location_id: String,
    pub relative_path: String,
    pub operation_id: PatchOperationId,
}

/// A queryable graph over a [`PatchIndex`] connecting patch operations to the Defs they may
/// affect, built via static XPath target inference (see [`infer_xpath_target`]).
///
/// Built once from a fully-indexed [`PatchIndex`]; does not evaluate operations against real
/// Def XML documents (that's the preview engine's job in a later patches-editor issue). An
/// operation whose target is [`XPathTarget::Unsupported`] is still retained here -- it is simply
/// absent from `by_def`/`type_wide` and present only in `unsupported`, matching the requirement
/// that complex XPaths be indexed with an unknown static impact rather than dropped.
pub struct PatchImpactGraph {
    /// Operations that name one specific Def (`Defs/<DefType>[defName="<Name>"]`).
    by_def: BTreeMap<DefIdentityKey, Vec<PatchImpactRef>>,
    /// Operations that target an entire Def type with no `defName` predicate
    /// (`Defs/<DefType>`) -- these affect *every* Def of that type, `Wall` included.
    type_wide: HashMap<String, Vec<PatchImpactRef>>,
    /// Operations affecting a Def type in general: the union of `by_def` and `type_wide` for
    /// that type, precomputed for `operations_affecting_def_type`.
    by_def_type_all: HashMap<String, Vec<PatchImpactRef>>,
    unsupported: Vec<PatchImpactRef>,
}

impl PatchImpactGraph {
    pub fn build(index: &PatchIndex) -> Self {
        let mut by_def: BTreeMap<DefIdentityKey, Vec<PatchImpactRef>> = BTreeMap::new();
        let mut type_wide: HashMap<String, Vec<PatchImpactRef>> = HashMap::new();
        let mut by_def_type_all: HashMap<String, Vec<PatchImpactRef>> = HashMap::new();
        let mut unsupported = Vec::new();

        for file in &index.files {
            for op in &file.operations {
                let reference = || PatchImpactRef {
                    location_id: file.source.location_id.clone(),
                    relative_path: file.relative_path.clone(),
                    operation_id: op.id,
                };
                match &op.target {
                    XPathTarget::Def { def_type, def_name } => {
                        by_def
                            .entry(DefIdentityKey {
                                def_type: def_type.clone(),
                                def_name: def_name.clone(),
                            })
                            .or_default()
                            .push(reference());
                        by_def_type_all
                            .entry(def_type.clone())
                            .or_default()
                            .push(reference());
                    }
                    XPathTarget::DefType { def_type } => {
                        type_wide
                            .entry(def_type.clone())
                            .or_default()
                            .push(reference());
                        by_def_type_all
                            .entry(def_type.clone())
                            .or_default()
                            .push(reference());
                    }
                    XPathTarget::Defs {
                        def_type,
                        def_names,
                    } => {
                        for def_name in def_names {
                            by_def
                                .entry(DefIdentityKey {
                                    def_type: def_type.clone(),
                                    def_name: def_name.clone(),
                                })
                                .or_default()
                                .push(reference());
                        }
                        by_def_type_all
                            .entry(def_type.clone())
                            .or_default()
                            .push(reference());
                    }
                    XPathTarget::Unsupported => unsupported.push(reference()),
                    XPathTarget::NoXPath => {}
                }
            }
        }

        Self {
            by_def,
            type_wide,
            by_def_type_all,
            unsupported,
        }
    }

    /// Operations statically known to affect this exact Def: those naming it by `defName`
    /// predicate, plus any operation that targets its whole Def type with no predicate (e.g.
    /// `Defs/ThingDef` affects `ThingDef:Wall` along with every other `ThingDef`).
    pub fn operations_affecting_def(&self, def_type: &str, def_name: &str) -> Vec<PatchImpactRef> {
        let mut matches: Vec<PatchImpactRef> = self
            .by_def
            .get(&DefIdentityKey {
                def_type: def_type.to_string(),
                def_name: def_name.to_string(),
            })
            .cloned()
            .unwrap_or_default();
        if let Some(type_wide) = self.type_wide.get(def_type) {
            matches.extend(type_wide.iter().cloned());
        }
        matches
    }

    /// Operations that target *every* Def of this type with no `defName` predicate
    /// (`Defs/<DefType>`) -- the subset of [`operations_affecting_def`](Self::operations_affecting_def)
    /// that doesn't depend on a specific `defName` string. Used when the selected Def has no
    /// `defName` of its own (an `Abstract="True"` parent template identified only by its `Name`
    /// attribute): `operations_affecting_def`'s `by_def` half is keyed by literal `defName="..."`
    /// predicate strings from patch XPaths, which bear no relationship to a `Name` attribute value,
    /// so it must not be queried with a `Name` value standing in for a `defName`.
    pub fn type_wide_operations(&self, def_type: &str) -> Vec<PatchImpactRef> {
        self.type_wide.get(def_type).cloned().unwrap_or_default()
    }

    /// Operations affecting a Def type in general: the union of operations naming a specific
    /// Def of that type and operations that target the whole type with no predicate.
    pub fn operations_affecting_def_type(&self, def_type: &str) -> &[PatchImpactRef] {
        self.by_def_type_all
            .get(def_type)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Distinct (location, relative_path) patch files with at least one operation statically
    /// known to affect this exact Def.
    pub fn patch_files_affecting_def(
        &self,
        def_type: &str,
        def_name: &str,
    ) -> Vec<(String, String)> {
        let mut seen = std::collections::HashSet::new();
        let mut files = Vec::new();
        for reference in self.operations_affecting_def(def_type, def_name) {
            let key = (reference.location_id, reference.relative_path);
            if seen.insert(key.clone()) {
                files.push(key);
            }
        }
        files
    }

    /// Operations that are conflict *candidates* for this Def: more than one statically-known
    /// operation touching the same exact Def. This does not classify conflict severity or
    /// cause (e.g. `success=Always` masking, sequence short-circuiting) -- that judgment belongs
    /// to patch validation diagnostics, which can use this as a starting point.
    pub fn conflicts_involving_def(&self, def_type: &str, def_name: &str) -> Vec<PatchImpactRef> {
        let matches = self.operations_affecting_def(def_type, def_name);
        if matches.len() > 1 {
            matches
        } else {
            Vec::new()
        }
    }

    /// Operations whose XPath is valid but too complex for static target inference.
    pub fn unsupported_operations(&self) -> &[PatchImpactRef] {
        &self.unsupported
    }
}

/// The statically-inferred target for one specific indexed operation, looked up by file and
/// in-file operation id ("Defs affected by an operation").
pub fn target_for_operation<'a>(
    index: &'a PatchIndex,
    location_id: &str,
    relative_path: &str,
    operation_id: PatchOperationId,
) -> Option<&'a XPathTarget> {
    find_file(index, location_id, relative_path)
        .and_then(|file| find_operation(file, operation_id))
        .map(|op| &op.target)
}

fn find_file<'a>(
    index: &'a PatchIndex,
    location_id: &str,
    relative_path: &str,
) -> Option<&'a PatchIndexFile> {
    index
        .files
        .iter()
        .find(|f| f.source.location_id == location_id && f.relative_path == relative_path)
}

fn find_operation(
    file: &PatchIndexFile,
    operation_id: PatchOperationId,
) -> Option<&IndexedPatchOperation> {
    file.operations.iter().find(|op| op.id == operation_id)
}
