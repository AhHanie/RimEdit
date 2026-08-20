//! Schema navigation: the [`SchemaCursor`] state machine over [`SchemaCatalog`], direct-vs-
//! inherited Def field lookup, and [`TraversalState`], which wraps a segment-by-segment walk for
//! `mod.rs`'s orchestrator. Independent of raw XPath scanning and of completion presentation --
//! callers pass already-trimmed field-step text in and get back traversal/resolution state, never
//! a serialized completion result.

use std::collections::HashSet;

use crate::patches::impact_graph::is_valid_identifier;
use crate::schema_pack::{
    lookup_object_field_with_alias, DefTypeSchema, FieldSchema, FieldTypeKind, SchemaCatalog,
    XmlFieldShape,
};

use super::scan::find_matching_close;
use super::types::{XPathDiagnostic, XPathResolvedField};

/// Where the segment walk is positioned in the schema after the Def type/predicate segment.
/// Contains only catalog-derived identifiers and this small state enum -- never project XML --
/// so completion stays a pure, fast, deterministic function of the typed text and the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SchemaCursor {
    /// Direct XML children of a Def type. RimWorld applies patches before Def XML inheritance, so
    /// only fields declared directly on `def_type` are completed/resolved here -- an
    /// ancestor-only field surfaces the existing `xpath_autocomplete_inherited_field` diagnostic
    /// instead (see [`resolve_field`]/[`is_inherited_only_field`]).
    DefFields { def_type: String },
    /// Fields of an object-type schema, resolved through its `inherits` chain and XML aliases
    /// (`schema_pack::lookup_object_field_with_alias`/`collect_object_fields_ordered`). Unlike
    /// `DefFields`, inherited fields are completed/resolved directly: object-type inheritance is
    /// ordinary schema inheritance already resolved on one XML element, not RimWorld's
    /// before-patches Def inheritance.
    ObjectFields { schema_ref: String },
    /// A `listOfLi` field whose items are objects: the next segment must be `li` or `li[n]` (see
    /// [`parse_list_item_step`]) before entering `schema_ref`'s object fields. For a
    /// discriminator-based item type, only the declared base `schema_ref` is traversed --
    /// narrowing to one `Class="..."` variant's own members would require predicate parsing beyond
    /// the plain positional index, which stays outside this conservative grammar (see `xpath`
    /// module docs).
    ListItem { schema_ref: String },
    /// A `keyedObjectMap` field: the next segment must be `li` or `li[n]` before entering a map
    /// entry (see [`SchemaCursor::KeyedMapEntry`]).
    KeyedMapLi { schema_ref: String },
    /// Inside a `keyedObjectMap` `<li>`: the next segment is either `key` (a scalar terminal) or
    /// `value` (enters `schema_ref`'s object fields).
    KeyedMapEntry { schema_ref: String },
    /// A `keyedObjectList` field: the next segment is a data-dependent key (the item's own XML
    /// element name, e.g. a `defName`) rather than a schema-known name. RimEdit has no index of
    /// these keys, so no suggestions are offered at an empty segment; any non-empty, well-formed
    /// segment is accepted as the key and transitions into `schema_ref`'s object fields.
    DynamicKey { schema_ref: String },
    /// A scalar field, or an XML shape with no statically known descendants (scalar lists,
    /// `namedChildrenMap`, `keyedValueList`, `typedReferenceList`, attributes/text/flags, or an
    /// object/list field whose `schemaRef` doesn't resolve in the catalog).
    Terminal,
}

/// Choose the next [`SchemaCursor`] after `field` resolves, from both its `field_type.kind` and
/// `xml` shape together. Falls back to [`SchemaCursor::Terminal`] for any shape with no
/// statically known descendants, including an
/// object/list field whose declared `schemaRef` isn't present in `catalog` -- an unknown ref stays
/// editable but uncompleted rather than guessed at.
fn cursor_after_field(catalog: &SchemaCatalog, field: &FieldSchema) -> SchemaCursor {
    match field.xml {
        // Some object fields use `xml: element` rather than `xml: object` (mirrors
        // `xml_document::validation::fields::validate_object_children`'s own
        // `matches!(.., XmlFieldShape::Object | XmlFieldShape::Element)` check).
        XmlFieldShape::Object | XmlFieldShape::Element
            if field.field_type.kind == FieldTypeKind::Object =>
        {
            object_schema_cursor(catalog, field.field_type.schema_ref.as_deref())
        }
        XmlFieldShape::ListOfLi if field.field_type.kind == FieldTypeKind::List => {
            match object_item_schema_ref(catalog, field) {
                Some(schema_ref) => SchemaCursor::ListItem {
                    schema_ref: schema_ref.to_string(),
                },
                None => SchemaCursor::Terminal,
            }
        }
        XmlFieldShape::KeyedObjectMap => match object_item_schema_ref(catalog, field) {
            Some(schema_ref) => SchemaCursor::KeyedMapLi {
                schema_ref: schema_ref.to_string(),
            },
            None => SchemaCursor::Terminal,
        },
        XmlFieldShape::KeyedObjectList => match object_item_schema_ref(catalog, field) {
            Some(schema_ref) => SchemaCursor::DynamicKey {
                schema_ref: schema_ref.to_string(),
            },
            None => SchemaCursor::Terminal,
        },
        // Scalar lists, NamedChildrenMap, KeyedValueList, TypedReferenceList, Attribute, Text,
        // FlagsText: no statically known child schema.
        _ => SchemaCursor::Terminal,
    }
}

fn object_schema_cursor(catalog: &SchemaCatalog, schema_ref: Option<&str>) -> SchemaCursor {
    match schema_ref {
        Some(schema_ref) if catalog.object_types.contains_key(schema_ref) => {
            SchemaCursor::ObjectFields {
                schema_ref: schema_ref.to_string(),
            }
        }
        _ => SchemaCursor::Terminal,
    }
}

/// `field.items.schemaRef` when `items.kind` is `object` and it resolves in `catalog`.
fn object_item_schema_ref<'a>(catalog: &SchemaCatalog, field: &'a FieldSchema) -> Option<&'a str> {
    let items = field.items.as_ref()?;
    if items.kind != FieldTypeKind::Object {
        return None;
    }
    let schema_ref = items.schema_ref.as_deref()?;
    catalog
        .object_types
        .contains_key(schema_ref)
        .then_some(schema_ref)
}

/// The three shapes a `listOfLi`/`keyedObjectMap` entry step (`li`, `li[n]`, ...) can take.
/// Deliberately independent of any RimWorld schema or project data -- see [`parse_list_item_step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ListItemStep {
    /// Exactly `li`, or `li[n]` for a positive decimal `n` with no trailing content.
    Valid,
    /// A trailing, unclosed positional predicate (`li[`, `li[12`) -- still being typed.
    Incomplete,
    /// Anything else: a non-`li` step, a malformed/unsupported predicate (`li[0]`, `li[-1]`,
    /// `li[1.5]`, `li[last()]`, `li[@Class="..."]`), multiple predicates (`li[1][2]`), or trailing
    /// content after the closing bracket (`li[1]extra`).
    Invalid,
}

/// Classify a single fully-typed list-entry step's text against the grammar `"li" | "li"
/// "[" positive-integer "]"` (see `xpath` module docs). Reuses [`find_matching_close`] so the
/// closing bracket must belong to the first (and only) opening bracket, which naturally rejects a
/// second adjacent predicate and any trailing text. The index is validated lexically (leading digit
/// 1-9, remaining digits 0-9) rather than parsed to an integer, so an arbitrarily long decimal
/// index isn't rejected by an arbitrary overflow limit.
pub(super) fn parse_list_item_step(text: &str) -> ListItemStep {
    let Some(rest) = text.strip_prefix("li") else {
        return ListItemStep::Invalid;
    };
    if rest.is_empty() {
        return ListItemStep::Valid;
    }
    if !rest.starts_with('[') {
        return ListItemStep::Invalid;
    }
    match find_matching_close(rest, 0) {
        None => ListItemStep::Incomplete,
        Some(close_pos) if close_pos != rest.len() - 1 => ListItemStep::Invalid,
        Some(close_pos) => {
            let content = &rest[1..close_pos];
            if is_positive_integer(content) {
                ListItemStep::Valid
            } else {
                ListItemStep::Invalid
            }
        }
    }
}

/// A non-empty decimal literal with no leading zero, e.g. `"1"`, `"10"` -- rejects `"0"`, `"01"`,
/// `"-1"`, `"1.5"`, and non-numeric content alike.
fn is_positive_integer(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() && c != '0' => chars.all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

/// Resolve one fully-typed segment against `cursor`, mutating it in place to the next position.
/// Returns the field just resolved (for [`XPathResolvedField`] accumulation) when this segment
/// named a real field, `Ok(None)` for a structural transition (`li`/`value`) that doesn't itself
/// name a field, or `Err` with the existing `xpath_autocomplete_unsupported_pattern` diagnostic
/// when `cursor` cannot make sense of `text` at all -- callers stop the walk on `Err` rather than
/// guessing. A [`SchemaCursor::Terminal`] cursor always succeeds with `Ok(None)` and no diagnostic:
/// typing past a scalar field is simply not completed, not an error (see `xpath` module docs).
fn resolve_and_transition(
    catalog: &SchemaCatalog,
    cursor: &mut SchemaCursor,
    def_type: &str,
    text: &str,
) -> Result<Option<XPathResolvedField>, XPathDiagnostic> {
    match cursor.clone() {
        SchemaCursor::DefFields { def_type: dt } => {
            if let Some((canonical, field)) = direct_field(catalog, &dt, text) {
                let resolved = XPathResolvedField {
                    def_type: dt.clone(),
                    field_name: canonical.to_string(),
                    field: field.clone(),
                };
                *cursor = cursor_after_field(catalog, field);
                Ok(Some(resolved))
            } else if is_inherited_only_field(catalog, &dt, text) {
                Err(inherited_field_diagnostic(text, &dt))
            } else {
                Err(unsupported_child_segment())
            }
        }
        SchemaCursor::ObjectFields { schema_ref } => {
            if let Some((canonical, field)) =
                lookup_object_field_with_alias(catalog, &schema_ref, text)
            {
                let resolved = XPathResolvedField {
                    def_type: def_type.to_string(),
                    field_name: canonical.to_string(),
                    field: field.clone(),
                };
                *cursor = cursor_after_field(catalog, field);
                Ok(Some(resolved))
            } else {
                Err(unsupported_child_segment())
            }
        }
        SchemaCursor::ListItem { schema_ref } => match parse_list_item_step(text) {
            ListItemStep::Valid => {
                *cursor = SchemaCursor::ObjectFields { schema_ref };
                Ok(None)
            }
            // Structurally can't happen for a fully-typed (non-final) segment -- an unclosed `[`
            // keeps bracket depth > 0, so `split_segments` can't have split a `/` after it.
            ListItemStep::Incomplete => Ok(None),
            ListItemStep::Invalid => Err(unsupported_child_segment()),
        },
        SchemaCursor::KeyedMapLi { schema_ref } => match parse_list_item_step(text) {
            ListItemStep::Valid => {
                *cursor = SchemaCursor::KeyedMapEntry { schema_ref };
                Ok(None)
            }
            ListItemStep::Incomplete => Ok(None),
            ListItemStep::Invalid => Err(unsupported_child_segment()),
        },
        SchemaCursor::KeyedMapEntry { schema_ref } => match text {
            "key" => {
                *cursor = SchemaCursor::Terminal;
                Ok(None)
            }
            "value" => {
                *cursor = SchemaCursor::ObjectFields { schema_ref };
                Ok(None)
            }
            _ => Err(unsupported_child_segment()),
        },
        SchemaCursor::DynamicKey { schema_ref } => {
            if is_valid_identifier(text) {
                *cursor = SchemaCursor::ObjectFields { schema_ref };
                Ok(None)
            } else {
                Err(unsupported_child_segment())
            }
        }
        // Nothing more to resolve past a scalar/unresolvable field -- stay silent, not an error.
        SchemaCursor::Terminal => Ok(None),
    }
}

/// Wraps a segment-by-segment schema walk: the current [`SchemaCursor`], the root Def type (needed
/// by `ObjectFields` resolution to stamp [`XPathResolvedField::def_type`]), and the last field
/// successfully resolved along the way, kept as the best-effort resolved field for in-progress
/// typing (see `xpath` module docs on `resolved_field`).
pub(super) struct TraversalState {
    def_type: String,
    cursor: SchemaCursor,
    last_resolved: Option<XPathResolvedField>,
}

impl TraversalState {
    pub(super) fn new(def_type: String) -> Self {
        let cursor = SchemaCursor::DefFields {
            def_type: def_type.clone(),
        };
        Self {
            def_type,
            cursor,
            last_resolved: None,
        }
    }

    /// Resolve one fully-typed segment, advancing `cursor` and, when it named a real field,
    /// updating `last_resolved`. Returns the diagnostic on an unsupported segment; the caller stops
    /// the walk rather than continuing to transition.
    pub(super) fn transition(
        &mut self,
        catalog: &SchemaCatalog,
        text: &str,
    ) -> Result<(), XPathDiagnostic> {
        if let Some(field) =
            resolve_and_transition(catalog, &mut self.cursor, &self.def_type, text)?
        {
            self.last_resolved = Some(field);
        }
        Ok(())
    }

    pub(super) fn cursor(&self) -> &SchemaCursor {
        &self.cursor
    }

    pub(super) fn last_resolved(&self) -> Option<XPathResolvedField> {
        self.last_resolved.clone()
    }

    pub(super) fn into_last_resolved(self) -> Option<XPathResolvedField> {
        self.last_resolved
    }
}

/// Resolve a fully-typed field name (or alias) against a Def type's *direct* fields only.
/// Returns an `xpath_autocomplete_inherited_field` diagnostic when `name` is a real field but only
/// reachable through the schema's `inherits` chain -- RimWorld applies patches before XML
/// inheritance, so such a field cannot be targeted unless it is physically present in the XML
/// being patched (which this module, having no access to the live combined document, can't check).
pub(super) fn resolve_field(
    catalog: &SchemaCatalog,
    def_type: &str,
    name: &str,
) -> (Option<XPathResolvedField>, Vec<XPathDiagnostic>) {
    if name.is_empty() {
        return (None, Vec::new());
    }
    if let Some((canonical, field)) = direct_field(catalog, def_type, name) {
        return (
            Some(XPathResolvedField {
                def_type: def_type.to_string(),
                field_name: canonical.to_string(),
                field: field.clone(),
            }),
            Vec::new(),
        );
    }
    if is_inherited_only_field(catalog, def_type, name) {
        return (None, vec![inherited_field_diagnostic(name, def_type)]);
    }
    (None, Vec::new())
}

fn field_or_alias(schema: &DefTypeSchema, name: &str) -> bool {
    schema.fields.contains_key(name)
        || schema
            .fields
            .values()
            .any(|f| f.xml_aliases.iter().any(|a| a == name))
}

fn direct_field<'a>(
    catalog: &'a SchemaCatalog,
    def_type: &str,
    name: &str,
) -> Option<(&'a str, &'a FieldSchema)> {
    let schema = catalog.def_types.get(def_type)?;
    if let Some((canonical, field)) = schema.fields.get_key_value(name) {
        return Some((canonical.as_str(), field));
    }
    schema.fields.iter().find_map(|(canonical, field)| {
        field
            .xml_aliases
            .iter()
            .any(|a| a == name)
            .then_some((canonical.as_str(), field))
    })
}

/// Whether `name` is a field known to `def_type` only via its schema `inherits` chain (i.e. absent
/// from `def_type`'s own direct fields). Walks parents depth-first with a cycle guard, mirroring
/// `schema_pack::lookup::collect_all_object_inherited_fields`'s object-type equivalent -- there is
/// no Def-type version of that helper today because nothing needed "is this field inherited-only"
/// as a yes/no question before this module.
fn is_inherited_only_field(catalog: &SchemaCatalog, def_type: &str, name: &str) -> bool {
    let Some(schema) = catalog.def_types.get(def_type) else {
        return false;
    };
    if field_or_alias(schema, name) {
        return false;
    }
    let mut visited: HashSet<String> = HashSet::from([def_type.to_string()]);
    let mut stack: Vec<String> = schema.inherits.clone();
    while let Some(parent) = stack.pop() {
        if !visited.insert(parent.clone()) {
            continue;
        }
        if let Some(parent_schema) = catalog.def_types.get(&parent) {
            if field_or_alias(parent_schema, name) {
                return true;
            }
            stack.extend(parent_schema.inherits.iter().cloned());
        }
    }
    false
}

pub(super) fn unsupported_child_segment() -> XPathDiagnostic {
    XPathDiagnostic::warning(
        "xpath_autocomplete_unsupported_pattern",
        "This path segment does not match a known schema field or structural entry, so deeper autocomplete stops here.",
    )
}

fn inherited_field_diagnostic(name: &str, def_type: &str) -> XPathDiagnostic {
    XPathDiagnostic::warning(
        "xpath_autocomplete_inherited_field",
        format!(
            "'{name}' is declared on a schema parent of '{def_type}', not on '{def_type}' itself. RimWorld applies patches before XML inheritance, so this field can only be targeted if it is physically present in the XML being patched."
        ),
    )
    .with_args(crate::diagnostics::diagnostic_args([
        ("fieldName", name.into()),
        ("defType", def_type.into()),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patches::xpath::test_support::{
        catalog, def_type_schema, object_field, object_type_schema, scalar_field,
    };

    fn catalog_with(def_type: &str, fields: &[(&str, FieldSchema)]) -> SchemaCatalog {
        catalog(&[(def_type, def_type_schema(&[], fields))], &[])
    }

    #[test]
    fn parse_list_item_step_accepts_bare_li_and_positive_index() {
        assert_eq!(parse_list_item_step("li"), ListItemStep::Valid);
        assert_eq!(parse_list_item_step("li[1]"), ListItemStep::Valid);
        assert_eq!(parse_list_item_step("li[42]"), ListItemStep::Valid);
    }

    #[test]
    fn parse_list_item_step_rejects_zero_leading_zero_and_negative() {
        assert_eq!(parse_list_item_step("li[0]"), ListItemStep::Invalid);
        assert_eq!(parse_list_item_step("li[01]"), ListItemStep::Invalid);
        assert_eq!(parse_list_item_step("li[-1]"), ListItemStep::Invalid);
    }

    #[test]
    fn parse_list_item_step_rejects_class_predicate_and_trailing_content() {
        assert_eq!(
            parse_list_item_step("li[@Class=\"Foo\"]"),
            ListItemStep::Invalid
        );
        assert_eq!(parse_list_item_step("li[1]extra"), ListItemStep::Invalid);
        assert_eq!(parse_list_item_step("li[1][2]"), ListItemStep::Invalid);
    }

    #[test]
    fn parse_list_item_step_incomplete_for_unclosed_bracket() {
        assert_eq!(parse_list_item_step("li["), ListItemStep::Incomplete);
        assert_eq!(parse_list_item_step("li[12"), ListItemStep::Incomplete);
    }

    #[test]
    fn traversal_state_transitions_through_def_field_into_terminal() {
        let catalog = catalog_with("ThingDef", &[("label", scalar_field())]);
        let mut state = TraversalState::new("ThingDef".to_string());
        state.transition(&catalog, "label").unwrap();
        assert_eq!(state.cursor(), &SchemaCursor::Terminal);
        assert_eq!(
            state.last_resolved().map(|f| f.field_name),
            Some("label".to_string())
        );
    }

    #[test]
    fn traversal_state_transitions_into_object_fields() {
        let catalog = catalog(
            &[(
                "ThingDef",
                def_type_schema(&[], &[("graphicData", object_field("GraphicData"))]),
            )],
            &[(
                "GraphicData",
                object_type_schema(&[("texPath", scalar_field())]),
            )],
        );

        let mut state = TraversalState::new("ThingDef".to_string());
        state.transition(&catalog, "graphicData").unwrap();
        assert_eq!(
            state.cursor(),
            &SchemaCursor::ObjectFields {
                schema_ref: "GraphicData".to_string()
            }
        );
        state.transition(&catalog, "texPath").unwrap();
        assert_eq!(
            state.last_resolved().map(|f| f.field_name),
            Some("texPath".to_string())
        );
    }

    #[test]
    fn traversal_state_errors_on_unknown_segment() {
        let catalog = catalog_with("ThingDef", &[("label", scalar_field())]);
        let mut state = TraversalState::new("ThingDef".to_string());
        let err = state.transition(&catalog, "notAField").unwrap_err();
        assert_eq!(err.code, "xpath_autocomplete_unsupported_pattern");
    }

    #[test]
    fn traversal_state_reports_inherited_only_field_diagnostic() {
        let catalog = catalog(
            &[
                ("Building", def_type_schema(&["ThingDef"], &[])),
                (
                    "ThingDef",
                    def_type_schema(&[], &[("label", scalar_field())]),
                ),
            ],
            &[],
        );
        let mut state = TraversalState::new("Building".to_string());
        let err = state.transition(&catalog, "label").unwrap_err();
        assert_eq!(err.code, "xpath_autocomplete_inherited_field");
    }
}
