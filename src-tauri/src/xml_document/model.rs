use serde::{Deserialize, Serialize};

use super::diagnostics::{ParseDiagnostic, ValidationDiagnostic, XmlSpan};

pub type XmlNodeId = usize;

pub struct XmlDocument {
    pub source: String,
    pub relative_path: String,
    pub nodes: Vec<XmlNode>,
    pub top_level_nodes: Vec<XmlNodeId>,
    pub def_summaries: Vec<DefSummary>,
    pub profile: XmlDocumentProfile,
    pub parse_diagnostics: Vec<ParseDiagnostic>,
    pub validation_diagnostics: Vec<ValidationDiagnostic>,
    pub had_fatal_parse_error: bool,
}

/// Identifies which specialized editor/validation path a parsed document should use.
/// Detected from the root element name and (for `About`) the file's relative path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlDocumentProfile {
    Defs,
    Patch,
    About,
    GenericXml,
}

/// Detects the document profile from the parsed tree and its relative path.
///
/// `About` is detected from either signal (path or root element) so that a
/// `ModMetaData` document opened from a nonstandard location is still routed
/// correctly, and so that `About/About.xml` files with an unexpected root still
/// surface a clear "root is not ModMetaData" diagnostic instead of falling through
/// to the generic Def-candidate path.
pub(crate) fn detect_profile(doc: &XmlDocument) -> XmlDocumentProfile {
    let root_name = doc.top_level_nodes.iter().find_map(|&id| {
        if let XmlNodeKind::Element(ref el) = doc.nodes[id].kind {
            Some(el.name.as_str())
        } else {
            None
        }
    });

    if root_name == Some("ModMetaData") || is_about_relative_path(&doc.relative_path) {
        return XmlDocumentProfile::About;
    }

    match root_name {
        Some("Patch") => XmlDocumentProfile::Patch,
        Some(_) => XmlDocumentProfile::Defs,
        None => XmlDocumentProfile::GenericXml,
    }
}

fn is_about_relative_path(relative_path: &str) -> bool {
    relative_path
        .replace('\\', "/")
        .eq_ignore_ascii_case("About/About.xml")
}

pub(crate) struct XmlNode {
    pub id: XmlNodeId,
    pub parent: Option<XmlNodeId>,
    pub kind: XmlNodeKind,
    pub children: Vec<XmlNodeId>,
    pub span: XmlSpan,
    pub dirty: bool,
}

pub(crate) enum XmlNodeKind {
    Element(XmlElement),
    Text(XmlText),
    Comment(XmlText),
    CData(XmlText),
    ProcessingInstruction(XmlText),
    DocType(XmlText),
}

pub(crate) struct XmlElement {
    pub name: String,
    pub attributes: Vec<XmlAttribute>,
    pub start_tag_span: XmlSpan,
    pub end_tag_span: Option<XmlSpan>,
    pub self_closing: bool,
}

pub(crate) struct XmlAttribute {
    pub name: String,
    pub value: String,
    // Exact attribute spans will back validation/edit diagnostics once the parser computes them.
    #[allow(dead_code)]
    pub span: XmlSpan,
}

pub(crate) struct XmlText {
    pub value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlDocumentLoadResult {
    pub project_id: String,
    pub relative_path: String,
    pub raw_xml: String,
    pub document: Option<XmlDocumentView>,
    pub parse_diagnostics: Vec<ParseDiagnostic>,
    pub validation_diagnostics: Vec<ValidationDiagnostic>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlDocumentView {
    pub node_count: usize,
    pub root_element: Option<String>,
    pub profile: XmlDocumentProfile,
    pub defs: Vec<DefSummary>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefSummary {
    pub node_id: XmlNodeId,
    pub def_type: String,
    pub def_name: Option<String>,
    pub label: Option<String>,
    pub parent_name: Option<String>,
    /// Value of the `Name` XML attribute - the XML inheritance template identifier.
    /// Present only on abstract or template nodes. Not the same as `def_name`.
    pub xml_name: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlChildShape {
    Element,
    Object,
    ListOfLi,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlAttributeView {
    pub name: String,
    pub value: String,
    pub known: bool,
}

/// Rich per-`<li>` view for object-list fields. Preserves the item's node id,
/// attributes (including `Class`), element children, and structural state so
/// the frontend can render editable comp forms.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlListItemView {
    pub node_id: XmlNodeId,
    pub text_value: Option<String>,
    pub attributes: Vec<XmlAttributeView>,
    pub children: Vec<XmlNestedChildView>,
    pub order: usize,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub self_closing: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlNestedChildView {
    pub node_id: XmlNodeId,
    pub name: String,
    pub text_value: Option<String>,
    pub list_items: Vec<String>,
    pub xml_shape: XmlChildShape,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<XmlNestedChildView>>,
    pub order: usize,
    pub line: Option<usize>,
    pub column: Option<usize>,
    /// Attributes on this element (e.g. `Class` on `<li>` items).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<XmlAttributeView>,
    /// For `ListOfLi` shape: one entry per `<li>`, containing the element children of
    /// that item when it has object structure. Empty inner vec for scalar `<li>` items.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub li_object_items: Vec<Vec<XmlNestedChildView>>,
    /// Rich per-`<li>` view for object-list fields with attributes and full item state.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub li_items: Vec<XmlListItemView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlChildView {
    pub node_id: XmlNodeId,
    pub name: String,
    pub text_value: Option<String>,
    pub list_items: Vec<String>,
    pub xml_shape: XmlChildShape,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<XmlNestedChildView>>,
    pub order: usize,
    pub known: bool,
    pub line: Option<usize>,
    pub column: Option<usize>,
    /// Attributes on this element (e.g. `Class` discriminator on object-typed children).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<XmlAttributeView>,
    /// For `ListOfLi` shape: one entry per `<li>`, containing the element children of
    /// that item when it has object structure. Empty inner vec for scalar `<li>` items.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub li_object_items: Vec<Vec<XmlNestedChildView>>,
    /// Rich per-`<li>` view for object-list fields with attributes and full item state.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub li_items: Vec<XmlListItemView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefEditorView {
    pub node_id: XmlNodeId,
    pub def_type: String,
    pub def_name: Option<String>,
    pub label: Option<String>,
    pub parent_name: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub attributes: Vec<XmlAttributeView>,
    pub children: Vec<XmlChildView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlEditorDocumentView {
    pub node_count: usize,
    pub root_element: Option<String>,
    pub profile: XmlDocumentProfile,
    pub defs: Vec<DefEditorView>,
    pub about: Option<super::about::AboutMetadataView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlEditorDocumentLoadResult {
    pub project_id: String,
    pub relative_path: String,
    pub raw_xml: String,
    pub document: Option<XmlEditorDocumentView>,
    pub parse_diagnostics: Vec<ParseDiagnostic>,
    pub validation_diagnostics: Vec<ValidationDiagnostic>,
}

const MAX_NESTED_CHILD_DEPTH: usize = 32;

pub fn build_editor_view(doc: &XmlDocument) -> XmlEditorDocumentView {
    let root_element = doc.top_level_nodes.iter().find_map(|&id| {
        if let XmlNodeKind::Element(ref el) = doc.nodes[id].kind {
            Some(el.name.clone())
        } else {
            None
        }
    });

    let defs = doc
        .def_summaries
        .iter()
        .filter_map(|summary| build_def_editor_view(doc, summary))
        .collect();

    let about = if doc.profile == XmlDocumentProfile::About {
        super::about::build_about_metadata_view(doc)
    } else {
        None
    };

    XmlEditorDocumentView {
        node_count: doc.nodes.len(),
        root_element,
        profile: doc.profile,
        defs,
        about,
    }
}

fn build_def_editor_view(doc: &XmlDocument, summary: &DefSummary) -> Option<DefEditorView> {
    let def_node = doc.nodes.get(summary.node_id)?;
    let def_el = match &def_node.kind {
        XmlNodeKind::Element(e) => e,
        _ => return None,
    };

    let attributes = def_el
        .attributes
        .iter()
        .map(|attr| XmlAttributeView {
            name: attr.name.clone(),
            value: attr.value.clone(),
            known: false,
        })
        .collect();

    let mut children = Vec::new();
    let mut order: usize = 0;
    for &child_id in &def_node.children {
        if let XmlNodeKind::Element(_) = doc.nodes[child_id].kind {
            children.push(build_child_view(doc, child_id, order));
            order += 1;
        }
    }

    Some(DefEditorView {
        node_id: summary.node_id,
        def_type: summary.def_type.clone(),
        def_name: summary.def_name.clone(),
        label: summary.label.clone(),
        parent_name: summary.parent_name.clone(),
        line: summary.line,
        column: summary.column,
        attributes,
        children,
    })
}

/// Build a shape-classified view of a single top-level element child. `pub(crate)` (rather than
/// private) so `patches::value_xml` can reuse the exact same shape classification the Def form
/// editor uses when turning a patch operation's raw `<value>` XML into a structured field view --
/// see `docs/patches-editor/06-structured-patch-value-editor.md`.
pub(crate) fn build_child_view(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    order: usize,
) -> XmlChildView {
    let (name, attributes) = match &doc.nodes[elem_id].kind {
        XmlNodeKind::Element(el) => {
            let attrs = el
                .attributes
                .iter()
                .map(|attr| XmlAttributeView {
                    name: attr.name.clone(),
                    value: attr.value.clone(),
                    known: false,
                })
                .collect();
            (el.name.clone(), attrs)
        }
        _ => (String::new(), vec![]),
    };
    let (text_value, xml_shape) = classify_child_shape(doc, elem_id);
    let list_items = if matches!(xml_shape, XmlChildShape::ListOfLi) {
        collect_scalar_list_items(doc, elem_id)
    } else {
        vec![]
    };
    let children = if matches!(xml_shape, XmlChildShape::Object) {
        collect_nested_element_children(doc, elem_id, 0)
    } else {
        None
    };
    let li_object_items = if matches!(xml_shape, XmlChildShape::ListOfLi) {
        collect_li_object_children(doc, elem_id, 0)
    } else {
        vec![]
    };
    let li_items = if matches!(xml_shape, XmlChildShape::ListOfLi) {
        collect_li_items(doc, elem_id, 0)
    } else {
        vec![]
    };
    XmlChildView {
        node_id: doc.nodes[elem_id].id,
        name,
        text_value,
        list_items,
        xml_shape,
        children,
        attributes,
        li_object_items,
        li_items,
        order,
        known: false,
        line: Some(doc.nodes[elem_id].span.line),
        column: Some(doc.nodes[elem_id].span.column),
    }
}

fn build_nested_child_view(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    order: usize,
    depth: usize,
) -> XmlNestedChildView {
    let (name, attributes) = match &doc.nodes[elem_id].kind {
        XmlNodeKind::Element(el) => {
            let attrs = el
                .attributes
                .iter()
                .map(|attr| XmlAttributeView {
                    name: attr.name.clone(),
                    value: attr.value.clone(),
                    known: false,
                })
                .collect();
            (el.name.clone(), attrs)
        }
        _ => (String::new(), vec![]),
    };
    let (text_value, xml_shape) = classify_child_shape(doc, elem_id);
    let list_items = if matches!(xml_shape, XmlChildShape::ListOfLi) {
        collect_scalar_list_items(doc, elem_id)
    } else {
        vec![]
    };
    let children = if matches!(xml_shape, XmlChildShape::Object) {
        collect_nested_element_children(doc, elem_id, depth + 1)
    } else {
        None
    };
    let li_object_items = if matches!(xml_shape, XmlChildShape::ListOfLi) {
        collect_li_object_children(doc, elem_id, depth + 1)
    } else {
        vec![]
    };
    let li_items = if matches!(xml_shape, XmlChildShape::ListOfLi) {
        collect_li_items(doc, elem_id, depth + 1)
    } else {
        vec![]
    };
    XmlNestedChildView {
        node_id: doc.nodes[elem_id].id,
        name,
        text_value,
        list_items,
        xml_shape,
        children,
        li_object_items,
        li_items,
        attributes,
        order,
        line: Some(doc.nodes[elem_id].span.line),
        column: Some(doc.nodes[elem_id].span.column),
    }
}

/// For each `<li>` child of `elem_id`, collect the element children of that item.
/// Returns one inner vec per `<li>`: non-empty for object-structured items, empty for scalars.
fn collect_li_object_children(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    depth: usize,
) -> Vec<Vec<XmlNestedChildView>> {
    doc.nodes[elem_id]
        .children
        .iter()
        .filter_map(|&li_id| {
            if let XmlNodeKind::Element(ref li_el) = doc.nodes[li_id].kind {
                if li_el.name == "li" {
                    let has_element_children = doc.nodes[li_id]
                        .children
                        .iter()
                        .any(|&c| matches!(doc.nodes[c].kind, XmlNodeKind::Element(_)));
                    if has_element_children {
                        Some(collect_nested_element_children(doc, li_id, depth).unwrap_or_default())
                    } else {
                        Some(vec![]) // scalar <li>
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

/// Build a rich `XmlListItemView` for each `<li>` child of `elem_id`.
///
/// Each item captures the node id, attributes (including `Class`), self-closing
/// state, scalar text, and element children so the frontend can render an
/// editable comp form per item.
fn collect_li_items(doc: &XmlDocument, elem_id: XmlNodeId, depth: usize) -> Vec<XmlListItemView> {
    let mut items = Vec::new();
    let mut order: usize = 0;
    for &li_id in &doc.nodes[elem_id].children {
        let li_el = match &doc.nodes[li_id].kind {
            XmlNodeKind::Element(e) => e,
            _ => continue,
        };
        if li_el.name != "li" {
            continue;
        }

        let attributes = li_el
            .attributes
            .iter()
            .map(|attr| XmlAttributeView {
                name: attr.name.clone(),
                value: attr.value.clone(),
                known: false,
            })
            .collect();

        let self_closing = li_el.self_closing;

        let has_element_children = doc.nodes[li_id]
            .children
            .iter()
            .any(|&c| matches!(doc.nodes[c].kind, XmlNodeKind::Element(_)));

        let text_value = if !has_element_children {
            let mut parts = Vec::new();
            for &child_id in &doc.nodes[li_id].children {
                match &doc.nodes[child_id].kind {
                    XmlNodeKind::Text(t) | XmlNodeKind::CData(t) => {
                        let trimmed = t.value.trim();
                        if !trimmed.is_empty() {
                            parts.push(trimmed.to_string());
                        }
                    }
                    _ => {}
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(""))
            }
        } else {
            None
        };

        let children = if has_element_children && depth < MAX_NESTED_CHILD_DEPTH {
            collect_nested_element_children(doc, li_id, depth).unwrap_or_default()
        } else {
            vec![]
        };

        items.push(XmlListItemView {
            node_id: li_id,
            text_value,
            attributes,
            children,
            order,
            line: Some(doc.nodes[li_id].span.line),
            column: Some(doc.nodes[li_id].span.column),
            self_closing,
        });
        order += 1;
    }
    items
}

fn collect_scalar_list_items(doc: &XmlDocument, elem_id: XmlNodeId) -> Vec<String> {
    doc.nodes[elem_id]
        .children
        .iter()
        .filter_map(|&li_id| {
            if let XmlNodeKind::Element(ref li_el) = doc.nodes[li_id].kind {
                if li_el.name == "li" {
                    let text: String = doc.nodes[li_id]
                        .children
                        .iter()
                        .filter_map(|&tid| {
                            if let XmlNodeKind::Text(ref t) = doc.nodes[tid].kind {
                                let s = t.value.trim();
                                if !s.is_empty() {
                                    Some(s.to_string())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    Some(text)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

fn collect_nested_element_children(
    doc: &XmlDocument,
    elem_id: XmlNodeId,
    depth: usize,
) -> Option<Vec<XmlNestedChildView>> {
    if depth >= MAX_NESTED_CHILD_DEPTH {
        return None;
    }
    let mut nested = Vec::new();
    let mut order: usize = 0;
    for &child_id in &doc.nodes[elem_id].children {
        if let XmlNodeKind::Element(_) = doc.nodes[child_id].kind {
            nested.push(build_nested_child_view(doc, child_id, order, depth));
            order += 1;
        }
    }
    if nested.is_empty() {
        None
    } else {
        Some(nested)
    }
}

fn classify_child_shape(doc: &XmlDocument, elem_id: XmlNodeId) -> (Option<String>, XmlChildShape) {
    let node = &doc.nodes[elem_id];
    let mut text_parts: Vec<String> = Vec::new();
    let mut element_child_names: Vec<String> = Vec::new();

    for &child_id in &node.children {
        match &doc.nodes[child_id].kind {
            XmlNodeKind::Text(t) => {
                let trimmed = t.value.trim();
                if !trimmed.is_empty() {
                    text_parts.push(trimmed.to_string());
                }
            }
            XmlNodeKind::CData(t) => {
                text_parts.push(t.value.clone());
            }
            XmlNodeKind::Element(ref el) => {
                element_child_names.push(el.name.clone());
            }
            _ => {}
        }
    }

    let xml_shape = if !element_child_names.is_empty() {
        if element_child_names.iter().all(|n| n == "li") {
            XmlChildShape::ListOfLi
        } else {
            XmlChildShape::Object
        }
    } else {
        XmlChildShape::Element
    };

    let text_value = match xml_shape {
        XmlChildShape::Element => {
            if text_parts.is_empty() {
                None
            } else {
                Some(text_parts.join(""))
            }
        }
        _ => None,
    };

    (text_value, xml_shape)
}
