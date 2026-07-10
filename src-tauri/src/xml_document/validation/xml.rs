use crate::xml_document::model::{XmlDocument, XmlNodeId, XmlNodeKind};

pub(super) fn scalar_text(doc: &XmlDocument, node_id: XmlNodeId) -> Option<String> {
    let node = doc.nodes.get(node_id)?;
    let mut parts = Vec::new();
    for &child_id in &node.children {
        match &doc.nodes[child_id].kind {
            XmlNodeKind::Text(t) | XmlNodeKind::CData(t) => parts.push(t.value.as_str()),
            _ => {}
        }
    }
    Some(parts.join(""))
}

pub(super) fn element_child_names(doc: &XmlDocument, node_id: XmlNodeId) -> Vec<String> {
    doc.nodes
        .get(node_id)
        .map(|node| {
            node.children
                .iter()
                .filter_map(|&child_id| {
                    if let XmlNodeKind::Element(el) = &doc.nodes[child_id].kind {
                        Some(el.name.clone())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}
