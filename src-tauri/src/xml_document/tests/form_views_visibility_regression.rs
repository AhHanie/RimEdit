use super::*;

// A generic Rust-side regression proof, NOT a Form-View-specific mechanism: `xml_document` has
// no concept of "hidden"/"visibility" anywhere. Form View filtering happens entirely in the
// frontend (`src/features/form-views`, `src/features/xml-editor/lib/formDescriptors.ts`), which
// simply never builds a descriptor/edit for a field it doesn't render. So the only thing this
// crate can be shown to do "correctly" for a hidden field is what it already does for ANY field
// an edit doesn't target: leave its subtree completely untouched through an edit + serialize +
// re-validate cycle. These tests prove exactly that generic guarantee -- using field shapes
// (object root, list root, attribute, blocking-diagnostic scalar) that Plan.md section 5 calls
// out as the ones a Form View would hide -- and prove it byte-exact for the untouched
// object/list roots (their complete original source span, not a handful of hand-picked
// substrings that would miss reordered attributes or reformatted whitespace anywhere inside
// that range), matching Plan.md section 13's "hidden known fields survive parse/edit/serialize
// round trips and still yield diagnostics".
//
// What is NOT claimed: there is no "view" type, parameter, or branch anywhere in `XmlEdit`
// (`xml_document::edit::api`), `ValidationContext`/`validate_document`, or
// `serialize_xml_document` for these tests to disable or bypass -- that absence is a property of
// the type signatures themselves, not something a runtime test can additionally demonstrate
// beyond "no such parameter/branch exists to read".

const HIDDEN_FIELDS_FIXTURE_XML: &str = r#"<Defs>
  <ThingDef Name="GunBase" Abstract="True">
    <defName>Gun_Old</defName>
    <label>old gun</label>
    <size>1, 1</size>
    <graphicData>
      <texPath>Things/Gun_Old</texPath>
      <graphicClass>Graphic_Single</graphicClass>
    </graphicData>
    <recipes>
      <li>MakeGun_Old</li>
      <li>RepairGun_Old</li>
    </recipes>
  </ThingDef>
</Defs>"#;

#[test]
fn an_edit_to_one_field_leaves_every_other_subtree_byte_exact_including_object_and_list_roots() {
    let mut doc = parse_to_document("test.xml", HIDDEN_FIELDS_FIXTURE_XML);
    assert!(doc.parse_diagnostics.is_empty());
    let def_id = doc.def_summaries[0].node_id;

    // Capture the exact original source span for the two structurally interesting untouched
    // nodes (an object root and a list root) BEFORE the edit. `doc.source` is the immutable
    // original parse buffer for this document's whole lifetime -- `apply_xml_edit` only flips
    // `dirty` flags / rewrites the specific node(s) it targets, it never mutates `doc.source`
    // itself -- so these spans, and the exact text they point at, stay meaningful after the edit
    // below (see `xml_document::serializer::is_subtree_clean`, which copies
    // `doc.source[node.span.start..node.span.end]` verbatim into the output for any node whose
    // whole subtree remains clean, rather than reconstructing it element-by-element -- so any
    // reordered attribute, changed indentation, or reformatted `<li>` layout would break the
    // byte-exact assertions below, not just a `.contains()` of a short hand-picked fragment).
    let view = build_editor_view(&doc);
    let def_view = &view.defs[0];
    let graphic_data_id = def_view
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .expect("fixture has graphicData")
        .node_id;
    let recipes_id = def_view
        .children
        .iter()
        .find(|c| c.name == "recipes")
        .expect("fixture has recipes")
        .node_id;
    let original_graphic_data_text =
        doc.source[doc.nodes[graphic_data_id].span.start..doc.nodes[graphic_data_id].span.end]
            .to_string();
    let original_recipes_text =
        doc.source[doc.nodes[recipes_id].span.start..doc.nodes[recipes_id].span.end].to_string();
    // Sanity: the captured spans actually contain real content, not empty/degenerate ranges --
    // otherwise the byte-exact assertions below would trivially pass for the wrong reason.
    assert!(original_graphic_data_text.contains("Things/Gun_Old"));
    assert!(original_graphic_data_text.contains("Graphic_Single"));
    assert!(original_recipes_text.contains("MakeGun_Old"));
    assert!(original_recipes_text.contains("RepairGun_Old"));

    // Only `label` is edited. `graphicData`, `recipes`, the `Name`/`Abstract` attributes (part
    // of the Def element's own start tag), and `defName` are never referenced by any edit here.
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "label".to_string(),
            value: "new gun".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    // Both nodes are still clean (the edit didn't target them or any descendant), so their span
    // text is provably unchanged even before serialization runs.
    assert_eq!(
        doc.source[doc.nodes[graphic_data_id].span.start..doc.nodes[graphic_data_id].span.end],
        original_graphic_data_text
    );
    assert_eq!(
        doc.source[doc.nodes[recipes_id].span.start..doc.nodes[recipes_id].span.end],
        original_recipes_text
    );

    let out = serialize_xml_document(&doc);

    // The edited field changed...
    assert!(out.contains("<label>new gun</label>"));
    assert!(!out.contains("<label>old gun</label>"));

    // ...and the untouched object root / list root each appear in the output as ONE exact,
    // contiguous byte range identical to their complete original source text.
    assert!(out.contains(&original_graphic_data_text));
    assert!(out.contains(&original_recipes_text));
    assert!(out.contains(r#"Name="GunBase""#));
    assert!(out.contains(r#"Abstract="True""#));
    assert!(out.contains("<defName>Gun_Old</defName>"));

    // Re-parsing the serialized output reproduces the identical structured shape/values a form
    // would read back, not merely "the same text somewhere in the file".
    let reparsed = parse_to_document("test.xml", &out);
    let reparsed_view = build_editor_view(&reparsed);
    let reparsed_def = &reparsed_view.defs[0];
    let gfx = reparsed_def
        .children
        .iter()
        .find(|c| c.name == "graphicData")
        .expect("graphicData survives round trip");
    let gfx_children = gfx.children.as_ref().expect("graphicData has children");
    assert_eq!(
        gfx_children
            .iter()
            .find(|c| c.name == "texPath")
            .and_then(|c| c.text_value.as_deref()),
        Some("Things/Gun_Old")
    );
    let recipes = reparsed_def
        .children
        .iter()
        .find(|c| c.name == "recipes")
        .expect("recipes survives round trip");
    assert_eq!(recipes.list_items, vec!["MakeGun_Old", "RepairGun_Old"]);
}

#[test]
fn a_field_left_untouched_by_an_edit_still_produces_its_validation_diagnostic() {
    let mut doc = parse_to_document("test.xml", HIDDEN_FIELDS_FIXTURE_XML);
    let def_id = doc.def_summaries[0].node_id;

    // Edit only `defName` -- `size` (the deliberately invalid, blocking field) is never
    // referenced by this or any other edit, standing in for a top-level root a Form View hid.
    apply_xml_edit(
        &mut doc,
        XmlEdit::SetChildElementText {
            parent_node_id: def_id,
            child_name: "defName".to_string(),
            value: "Gun_New".to_string(),
        },
        &XmlEditContext::default(),
    )
    .unwrap();

    let out = serialize_xml_document(&doc);
    let diagnostics = validate_test_xml(&out, &empty_def_index());

    // `size` ("1, 1" is missing the required parenthesized-tuple shape for a vector2 field --
    // see `validation_vector_requires_parenthesized_tuple` in validation_core.rs for the same
    // rule exercised directly) still produces its blocking diagnostic. Full-document validation
    // has no notion of "this field was hidden/unedited" to skip it: it walks the whole parsed
    // tree every time, independent of which fields any caller rendered or edited.
    let size_diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "validation_field_type_mismatch" && d.field_path.as_deref() == Some("size"))
        .expect("the untouched `size` field still produces its diagnostic");
    assert!(
        size_diagnostic.blocking,
        "the diagnostic on the untouched field remains blocking"
    );

    // The edit itself is also reflected, proving both facts hold simultaneously in one document.
    assert!(out.contains("<defName>Gun_New</defName>"));
}
