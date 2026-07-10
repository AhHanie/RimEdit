use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphicPreviewAssetResult {
    pub tex_path: String,
    pub graphic_class: String,
    pub variants: Vec<GraphicPreviewVariant>,
    pub warnings: Vec<String>,
}

// Known role values: "single", "north", "east", "south", "west", "variant", "appearance"
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphicPreviewVariant {
    pub id: String,
    pub label: String,
    pub role: String,
    pub source_location_id: String,
    pub source_location_name: String,
    pub relative_texture_path: String,
    pub asset_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<bool>,
}
