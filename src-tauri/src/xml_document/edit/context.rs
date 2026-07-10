use std::collections::HashMap;

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlEditContext {
    #[serde(default)]
    pub field_order: Vec<String>,
    #[serde(default)]
    pub nested_field_orders: HashMap<String, Vec<String>>,
}

impl XmlEditContext {
    /// Returns the field order for inserting a child at the given ancestor path.
    ///
    /// Empty path → top-level `field_order`; non-empty → `nested_field_orders[path.join(".")]`.
    pub(crate) fn field_order_for_path<'a>(&'a self, path: &[String]) -> &'a [String] {
        if path.is_empty() {
            &self.field_order
        } else {
            self.nested_field_orders
                .get(&path.join("."))
                .map(|v| v.as_slice())
                .unwrap_or(&[])
        }
    }
}
