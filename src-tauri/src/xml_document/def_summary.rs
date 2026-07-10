use super::model::{DefSummary, XmlDocument, XmlNodeId, XmlNodeKind};

fn element_text_value(doc: &XmlDocument, elem_id: XmlNodeId) -> Option<String> {
    let node = &doc.nodes[elem_id];
    let mut buf = String::new();
    for &child_id in &node.children {
        match &doc.nodes[child_id].kind {
            XmlNodeKind::Text(t) => buf.push_str(&t.value),
            XmlNodeKind::CData(t) => buf.push_str(&t.value),
            _ => {}
        }
    }
    let trimmed = buf.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub(crate) fn extract_def_summaries(doc: &XmlDocument) -> Vec<DefSummary> {
    let def_candidates: Vec<XmlNodeId> = {
        let root_elem_id = doc
            .top_level_nodes
            .iter()
            .find(|&&id| matches!(doc.nodes[id].kind, XmlNodeKind::Element(_)));

        match root_elem_id {
            Some(&root_id) => {
                if let XmlNodeKind::Element(ref root_el) = doc.nodes[root_id].kind {
                    if root_el.name == "Defs" || root_el.name.ends_with(":Defs") {
                        doc.nodes[root_id]
                            .children
                            .iter()
                            .filter(|&&id| matches!(doc.nodes[id].kind, XmlNodeKind::Element(_)))
                            .copied()
                            .collect()
                    } else if root_el.name == "Patch" {
                        // Patch files are handled by the separate `patches` subsystem
                        // with its own model and diagnostics; they are not Defs.
                        vec![]
                    } else if root_el.name == "ModMetaData" {
                        // About.xml is handled by the `about` module with its own view
                        // and validation; it must not surface as an "Unknown Def type".
                        vec![]
                    } else {
                        vec![root_id]
                    }
                } else {
                    vec![]
                }
            }
            None => vec![],
        }
    };

    def_candidates
        .into_iter()
        .map(|def_id| {
            let def_node = &doc.nodes[def_id];
            let def_el = match &def_node.kind {
                XmlNodeKind::Element(e) => e,
                _ => unreachable!(),
            };

            let def_type = def_el.name.clone();
            let parent_name = def_el
                .attributes
                .iter()
                .find(|a| a.name == "ParentName")
                .map(|a| a.value.clone());
            let xml_name = def_el
                .attributes
                .iter()
                .find(|a| a.name == "Name")
                .map(|a| a.value.clone());

            let mut def_name: Option<String> = None;
            let mut label: Option<String> = None;

            for &child_id in &def_node.children {
                if let XmlNodeKind::Element(ref child_el) = doc.nodes[child_id].kind {
                    match child_el.name.as_str() {
                        "defName" if def_name.is_none() => {
                            def_name = element_text_value(doc, child_id);
                        }
                        "label" if label.is_none() => {
                            label = element_text_value(doc, child_id);
                        }
                        _ => {}
                    }
                }
            }

            DefSummary {
                node_id: def_node.id,
                def_type,
                def_name,
                label,
                parent_name,
                xml_name,
                line: Some(def_node.span.line),
                column: Some(def_node.span.column),
            }
        })
        .collect()
}
