//! Structured `<value>` XML fragment support for the patch value editor (issue 06).
//!
//! `PatchOperationAdd`/`Insert`/`Replace`/`AddModExtension` (and metadata-defined custom
//! operations with an `xmlValue` field) store their value payload as a raw XML string --
//! `patches::model::PathedValueOperation::value_xml` / `PathedValueOrderOperation::value_xml`.
//! This module lets the frontend read that string as a schema-shape-classified tree and write an
//! edited structured value back to XML text, without inventing new wire types:
//!
//! - [`parse_value_fragment`] reuses `xml_document::model::build_child_view` -- the exact same
//!   shape classification (`XmlChildShape::Element`/`Object`/`ListOfLi`) the Def form editor
//!   already renders from -- by wrapping the fragment in a synthetic root element and parsing it
//!   with the existing, already-tested `xml_document::parser`. The frontend then reuses
//!   `xml-editor`'s `buildObjectFieldValue` (which already knows how to turn an
//!   `XmlChildView`/`XmlNestedChildView` into an `ObjectFieldValue` tree for editing) instead of a
//!   second, parallel implementation of shape classification.
//! - [`serialize_initial_elements`] reuses `xml_document::edit::InitialElement` -- the same
//!   recursive tree `xml-editor`'s object-list "insert item" edits already build client-side from
//!   an edited `ObjectFieldValue` (see `formValues.ts`'s `objectFieldValueToInitialElement`) -- and
//!   writes it out as plain XML text using the same escaping/indentation conventions as
//!   `patches::serializer`/`patches::custom_metadata`.

use crate::xml_document::model::{build_child_view, XmlChildView, XmlNodeKind};
use crate::xml_document::{parse_to_document, InitialElement, NameValuePair, ParseDiagnostic};

use super::serializer::{escape_attr, escape_text, indent};

/// Synthetic wrapper element name used to parse a `<value>` fragment (which may have zero, one,
/// or several top-level elements/text) as a well-formed document. Chosen to be exceedingly
/// unlikely to collide with a real RimWorld element name.
const WRAPPER_TAG: &str = "RimEditPatchValueFragment";

/// Placeholder relative path passed to the parser -- this fragment is never a real file, so
/// diagnostics referencing a path are not meaningful and are discarded by the caller.
const FRAGMENT_SOURCE_LABEL: &str = "<patch-value-fragment>";

/// Parse a patch operation's raw `<value>` inner XML into shape-classified child views, one per
/// top-level element. A well-formed single-field payload (the common case: `<statBases>...`,
/// `<li Class="...">...`) yields exactly one entry; zero entries means the fragment is empty or
/// contains no element (just text/whitespace); more than one entry means the payload adds several
/// sibling elements at once, which the frontend treats as an ambiguous shape and falls back to raw
/// XML editing for (see `docs/patches-editor/06-structured-patch-value-editor.md`).
///
/// Returns `Err` when the wrapped fragment fails to parse at all (`had_fatal_parse_error`) --
/// callers must not offer structured editing for a value that isn't well-formed XML in the first
/// place, since `quick_xml`'s recovery can still emit a partial tree for badly malformed input,
/// which would otherwise let the frontend silently rewrite (and lose) the original malformed text.
///
/// The `Err` is the underlying [`ParseDiagnostic`] the shared XML parser already produced (same
/// `parse_xml_syntax_error`/`parse_unexpected_eof`/etc. codes `parse_xml_document` surfaces for a
/// real file), not a bare `String` -- so the caller can build a translatable, catalog-backed
/// `AppError` (`code` + `args`) instead of showing raw parser text as the primary message. Its
/// `relativePath`/`line`/`column` refer to the synthetic wrapper document, not a real file, and are
/// not meaningful to callers.
pub fn parse_value_fragment(value_xml: &str) -> Result<Vec<XmlChildView>, Box<ParseDiagnostic>> {
    let wrapped = format!("<{WRAPPER_TAG}>{value_xml}</{WRAPPER_TAG}>");
    let doc = parse_to_document(FRAGMENT_SOURCE_LABEL, &wrapped);

    if doc.had_fatal_parse_error {
        let diagnostic = doc.parse_diagnostics.first().cloned().unwrap_or_else(|| {
            ParseDiagnostic::new(
                FRAGMENT_SOURCE_LABEL,
                None,
                None,
                None,
                "parse_xml_syntax_error",
                "The value is not well-formed XML.",
            )
        });
        return Err(Box::new(diagnostic));
    }

    let Some(&root_id) = doc
        .top_level_nodes
        .iter()
        .find(|&&id| matches!(doc.nodes[id].kind, XmlNodeKind::Element(_)))
    else {
        return Ok(Vec::new());
    };

    let mut children = Vec::new();
    let mut order = 0usize;
    for &child_id in &doc.nodes[root_id].children {
        if let XmlNodeKind::Element(_) = doc.nodes[child_id].kind {
            children.push(build_child_view(&doc, child_id, order));
            order += 1;
        }
    }
    Ok(children)
}

/// Serialize a structured value edit (built client-side from an edited `ObjectFieldValue`, the
/// same tree shape `xml-editor`'s object-list item insertion already sends over IPC) into XML
/// text suitable for `PathedValueOperation::value_xml`/`PathedValueOrderOperation::value_xml`.
/// Writes each element in `elements` at indent level 0; callers pass a single-element slice for
/// the normal one-field-payload case.
pub fn serialize_initial_elements(elements: &[InitialElement]) -> String {
    let mut out = String::new();
    for el in elements {
        write_initial_element(&mut out, el, 0);
    }
    out
}

fn write_initial_element(out: &mut String, el: &InitialElement, level: usize) {
    indent(out, level);
    out.push('<');
    out.push_str(&el.name);
    for attr in &el.attributes {
        out.push(' ');
        out.push_str(&attr.name);
        out.push_str("=\"");
        out.push_str(&escape_attr(&attr.value));
        out.push('"');
    }

    let text = el.value.as_deref().filter(|v| !v.is_empty());
    let has_nested = !el.children.is_empty() || !el.li_items.is_empty();

    if text.is_none() && !has_nested {
        out.push_str(" />\n");
        return;
    }

    out.push('>');
    if let Some(text) = text {
        out.push_str(&escape_text(text));
    } else {
        out.push('\n');
        for child in el.children.iter().chain(el.li_items.iter()) {
            write_initial_element(out, child, level + 1);
        }
        indent(out, level);
    }
    out.push_str("</");
    out.push_str(&el.name);
    out.push_str(">\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml_document::model::XmlChildShape;

    #[test]
    fn parses_empty_fragment_as_no_children() {
        assert!(parse_value_fragment("").unwrap().is_empty());
        assert!(parse_value_fragment("   ").unwrap().is_empty());
    }

    #[test]
    fn parses_scalar_element_field() {
        let views = parse_value_fragment("<label>Wall</label>").unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].name, "label");
        assert_eq!(views[0].text_value.as_deref(), Some("Wall"));
        assert!(matches!(views[0].xml_shape, XmlChildShape::Element));
    }

    #[test]
    fn parses_object_field_with_attribute() {
        let views = parse_value_fragment(
            r#"<statBases><MaxHitPoints Attr="1">300</MaxHitPoints></statBases>"#,
        )
        .unwrap();
        assert_eq!(views.len(), 1);
        assert!(matches!(views[0].xml_shape, XmlChildShape::Object));
        let children = views[0].children.as_deref().unwrap_or_default();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "MaxHitPoints");
        assert_eq!(children[0].text_value.as_deref(), Some("300"));
    }

    #[test]
    fn parses_list_of_li_field() {
        let views = parse_value_fragment("<comps><li>A</li><li>B</li></comps>").unwrap();
        assert_eq!(views.len(), 1);
        assert!(matches!(views[0].xml_shape, XmlChildShape::ListOfLi));
        assert_eq!(views[0].list_items, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn parses_li_items_with_class_discriminator() {
        let views = parse_value_fragment(
            r#"<modExtensions><li Class="MyMod.ExtA"><foo>1</foo></li></modExtensions>"#,
        )
        .unwrap();
        assert_eq!(views.len(), 1);
        let li_items = &views[0].li_items;
        assert_eq!(li_items.len(), 1);
        assert_eq!(
            li_items[0]
                .attributes
                .iter()
                .find(|a| a.name == "Class")
                .map(|a| a.value.as_str()),
            Some("MyMod.ExtA")
        );
    }

    #[test]
    fn multiple_top_level_elements_are_reported_as_ambiguous() {
        let views = parse_value_fragment("<statBases /><comps />").unwrap();
        assert_eq!(views.len(), 2);
    }

    #[test]
    fn fatally_malformed_fragment_is_reported_as_an_error_instead_of_a_partial_tree() {
        // An unclosed tag with no matching close anywhere is unrecoverable -- quick_xml's
        // `had_fatal_parse_error` should fire, and callers must not silently accept whatever
        // partial tree the recovery path produced (which would let the frontend rewrite this
        // malformed text as different, "valid" XML without ever surfacing that it was broken).
        assert!(parse_value_fragment("<statBases><MaxHitPoints>300</statBases>").is_err());
    }

    #[test]
    fn malformed_fragment_error_carries_a_catalog_backed_code_not_just_raw_parser_text() {
        // The `Err` must be the shared parser's own structured `ParseDiagnostic` (same
        // `parse_xml_syntax_error`/`parse_unexpected_eof`/etc. codes a real file's parse failure
        // gets) -- not a bare, uncatalogued `String` -- so the command layer can build a
        // translatable `AppError` instead of showing raw English parser text as the primary
        // message. See `commands::patches::parse_patch_value_xml`.
        let err = match parse_value_fragment("<statBases><MaxHitPoints>300</statBases>") {
            Err(e) => e,
            Ok(_) => panic!("expected a parse error"),
        };
        assert!(
            matches!(
                err.code.as_str(),
                "parse_xml_syntax_error" | "parse_unexpected_eof"
            ),
            "expected a known catalog-backed parse error code, got: {}",
            err.code
        );
        // The raw parser message is still attached, but only as optional technical detail --
        // never the sole signal a caller has to render.
        assert!(!err.message.is_empty());
    }

    #[test]
    fn serializes_scalar_element() {
        let xml = serialize_initial_elements(&[InitialElement {
            name: "label".to_string(),
            value: Some("Wall".to_string()),
            attributes: Vec::new(),
            children: Vec::new(),
            li_items: Vec::new(),
        }]);
        assert_eq!(xml, "<label>Wall</label>\n");
    }

    #[test]
    fn serializes_empty_element_as_self_closing() {
        let xml = serialize_initial_elements(&[InitialElement {
            name: "modExtensions".to_string(),
            value: None,
            attributes: Vec::new(),
            children: Vec::new(),
            li_items: Vec::new(),
        }]);
        assert_eq!(xml, "<modExtensions />\n");
    }

    #[test]
    fn serializes_nested_object_with_attribute_and_escaping() {
        let xml = serialize_initial_elements(&[InitialElement {
            name: "statBases".to_string(),
            value: None,
            attributes: Vec::new(),
            children: vec![InitialElement {
                name: "MaxHitPoints".to_string(),
                value: Some("A & B".to_string()),
                attributes: vec![NameValuePair {
                    name: "Attr".to_string(),
                    value: "1".to_string(),
                }],
                children: Vec::new(),
                li_items: Vec::new(),
            }],
            li_items: Vec::new(),
        }]);
        assert!(xml.contains("<statBases>\n"));
        assert!(xml.contains(r#"<MaxHitPoints Attr="1">A &amp; B</MaxHitPoints>"#));
        assert!(xml.contains("</statBases>\n"));
    }

    #[test]
    fn round_trips_object_list_li_items_with_class_attribute() {
        let element = InitialElement {
            name: "modExtensions".to_string(),
            value: None,
            attributes: Vec::new(),
            children: Vec::new(),
            li_items: vec![InitialElement {
                name: "li".to_string(),
                value: None,
                attributes: vec![NameValuePair {
                    name: "Class".to_string(),
                    value: "MyMod.ExtA".to_string(),
                }],
                children: vec![InitialElement {
                    name: "foo".to_string(),
                    value: Some("1".to_string()),
                    attributes: Vec::new(),
                    children: Vec::new(),
                    li_items: Vec::new(),
                }],
                li_items: Vec::new(),
            }],
        };
        let xml = serialize_initial_elements(&[element]);
        let reparsed = parse_value_fragment(&xml).unwrap();
        assert_eq!(reparsed.len(), 1);
        assert_eq!(reparsed[0].li_items.len(), 1);
        assert_eq!(
            reparsed[0].li_items[0]
                .attributes
                .iter()
                .find(|a| a.name == "Class")
                .map(|a| a.value.as_str()),
            Some("MyMod.ExtA")
        );
    }
}
