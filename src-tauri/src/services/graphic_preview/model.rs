use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphicPreviewAssetResult {
    pub tex_path: String,
    pub graphic_class: String,
    pub variants: Vec<GraphicPreviewVariant>,
    pub warnings: Vec<GraphicPreviewWarning>,
}

/// A non-fatal graphic-preview condition (e.g. a missing Textures directory, a texture that
/// couldn't be located, a DDS file the browser preview can't render). Carries the same
/// `code`/`message`/`args` shape as every other diagnostic family (see `crate::diagnostics` module
/// docs) so the frontend renders it through `renderDiagnostic` instead of displaying `message`
/// (English-only, backend-assembled) verbatim.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphicPreviewWarning {
    pub code: String,
    pub message: String,
    #[serde(
        default,
        skip_serializing_if = "crate::diagnostics::DiagnosticArgs::is_empty"
    )]
    pub args: crate::diagnostics::DiagnosticArgs,
}

impl GraphicPreviewWarning {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    /// Attaches typed args for `code`. Additive on top of the still-English `message`.
    pub fn with_args(mut self, args: crate::diagnostics::DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
    }
}

// Known role values: "single", "north", "east", "south", "west", "variant", "appearance"
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphicPreviewVariant {
    pub id: String,
    /// Structured, translatable variant label -- never an assembled English string (see
    /// `GraphicPreviewLabel` docs). The frontend renders this through its own translation catalog
    /// rather than displaying anything from the backend directly.
    pub label: GraphicPreviewLabel,
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

/// One of the four cardinal RimWorld sprite directions. A small closed set, translated on the
/// frontend via `t()` (mirrors how `renderDiagnosticSeverity` translates a closed backend-sourced
/// string) -- never assembled into English text on the backend.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Direction {
    North,
    East,
    South,
    West,
}

/// Which of RimWorld's three stack-count sprite slots a variant represents.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StackSlot {
    Single,
    Partial,
    Full,
}

/// A `GraphicPreviewVariant`'s display label, as a translatable discriminant plus typed
/// literal/numeric args -- never pre-assembled English text (Plan.md: backend-originated UI text
/// must be `code`/`args`-shaped, translated by the frontend, not raw English). `index` values are
/// 1-based, matching what a user should see (`"Variant 1"`, not `"Variant 0"`).
///
/// `AppearanceNamed`'s `suffix` is the one exception carrying free text: it's derived directly from
/// an actual texture file name on disk (e.g. `"Damaged"` from `Blocks_Damaged.png`), not a fixed UI
/// vocabulary entry, so there is nothing to look up in a translation catalog -- the frontend must
/// interpolate it verbatim, the same way a diagnostic's literal `args` (a field name, a def name) are
/// interpolated rather than translated.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum GraphicPreviewLabel {
    Single,
    Direction {
        direction: Direction,
    },
    Variant {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        direction: Option<Direction>,
    },
    Stack {
        stack: StackSlot,
        #[serde(skip_serializing_if = "Option::is_none")]
        direction: Option<Direction>,
    },
    Appearance {
        index: usize,
    },
    AppearanceNamed {
        suffix: String,
    },
}

impl GraphicPreviewLabel {
    /// Attaches a directional suffix to a `Variant`/`Stack` label (e.g. a folder-scanned texture
    /// set that also carries a `_north`/`_east`/... suffix). A no-op for the other kinds, which
    /// never combine with an extra direction in this resolver's output.
    pub(crate) fn with_direction(self, direction: Option<Direction>) -> Self {
        match self {
            GraphicPreviewLabel::Variant { index, .. } => {
                GraphicPreviewLabel::Variant { index, direction }
            }
            GraphicPreviewLabel::Stack { stack, .. } => {
                GraphicPreviewLabel::Stack { stack, direction }
            }
            other => other,
        }
    }
}
