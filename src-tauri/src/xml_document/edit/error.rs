use serde::Serialize;

use super::super::model::XmlNodeId;

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlEditError {
    #[error("Node {0} not found")]
    NodeNotFound(XmlNodeId),
    #[error("Node {0} is not an element")]
    NotAnElement(XmlNodeId),
    #[error("Object path must not be empty")]
    EmptyObjectPath,
    #[error("Invalid XML element name: '{0}'")]
    InvalidElementName(String),
    #[error("Duplicate map key: '{0}'")]
    DuplicateMapKey(String),
}
