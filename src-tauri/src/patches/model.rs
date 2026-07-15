//! Typed AST for RimWorld `<Patch>` operation files.
//!
//! Field names below (`xpath`, `value`, `order`, `success`, `attribute`, `name`, `mods`,
//! `operations`, `match`, `nomatch`) mirror the private C# fields RimWorld's DirectXmlLoader
//! binds XML child elements to (see `Verse.PatchOperation*` in the decompiled source), not
//! attributes on the `<Operation>` element itself.

use serde::{Deserialize, Serialize};

pub type PatchOperationId = usize;

/// Byte-offset and line/column span of a parsed operation element within its source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSpan {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

/// A preserved XML attribute on an operation element, such as `MayRequire` or `MayRequireAnyOf`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlAttributeModel {
    pub name: String,
    pub value: String,
}

/// `PatchOperation.Success` in the decompiled source: how the operation's real success/failure
/// result should be adjusted before being reported to a containing sequence/conditional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatchSuccessMode {
    #[default]
    Normal,
    Invert,
    Always,
    Never,
}

impl PatchSuccessMode {
    pub fn from_xml_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Normal" => Some(Self::Normal),
            "Invert" => Some(Self::Invert),
            "Always" => Some(Self::Always),
            "Never" => Some(Self::Never),
            _ => None,
        }
    }

    pub fn as_xml_str(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Invert => "Invert",
            Self::Always => "Always",
            Self::Never => "Never",
        }
    }
}

/// `PatchOperationAdd`/`PatchOperationInsert`'s private `Order` enum. `None` on the operation
/// means the field was absent in XML and the built-in default (Append for Add, Prepend for
/// Insert) applies; that default is a preview/apply concern, not stored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatchOrderMode {
    Append,
    Prepend,
}

impl PatchOrderMode {
    pub fn from_xml_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Append" => Some(Self::Append),
            "Prepend" => Some(Self::Prepend),
            _ => None,
        }
    }

    pub fn as_xml_str(self) -> &'static str {
        match self {
            Self::Append => "Append",
            Self::Prepend => "Prepend",
        }
    }
}

// Not `Eq`: `args` can carry a `DiagnosticArgValue::Float`, and `f64` has no `Eq` impl.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchDiagnostic {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    /// Stable code for this diagnostic (see `crate::diagnostics` module docs). `None` for
    /// diagnostics still awaiting migration off the raw parser-library `message` alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::diagnostics::DiagnosticArgs::is_empty"
    )]
    pub args: crate::diagnostics::DiagnosticArgs,
}

impl PatchDiagnostic {
    pub(crate) fn new(
        line: Option<usize>,
        column: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            line,
            column,
            message: message.into(),
            code: None,
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    /// Attaches a stable code (see `crate::diagnostics` module docs), for producers that already
    /// know a normalized condition rather than only having raw parser-library text.
    pub(crate) fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub(crate) fn with_args(mut self, args: crate::diagnostics::DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
    }
}

/// A parsed `<Patch>` XML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchFile {
    pub relative_path: String,
    pub xml_declaration: Option<String>,
    pub operations: Vec<PatchOperationNode>,
    pub diagnostics: Vec<PatchDiagnostic>,
    pub had_fatal_parse_error: bool,
}

/// One `<Operation>`, or an operation-shaped `<li>`/`<match>`/`<nomatch>` element nested inside
/// a `PatchOperationSequence`, `PatchOperationConditional`, or `PatchOperationFindMod`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationNode {
    pub id: PatchOperationId,
    pub class_name: String,
    pub success: PatchSuccessMode,
    /// Attributes on the element other than `Class`, e.g. `MayRequire`, `MayRequireAnyOf`.
    pub attributes: Vec<XmlAttributeModel>,
    pub kind: PatchOperationKind,
    pub span: Option<PatchSpan>,
}

/// Ignores `span`: two nodes parsed from differently-formatted source (e.g. before/after a
/// serialize round trip) are equal if they carry the same operation data.
impl PartialEq for PatchOperationNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.class_name == other.class_name
            && self.success == other.success
            && self.attributes == other.attributes
            && self.kind == other.kind
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathedOperation {
    pub xpath: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathedValueOperation {
    pub xpath: Option<String>,
    /// Raw inner XML of the `<value>` element, preserved byte-for-byte from source.
    pub value_xml: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathedValueOrderOperation {
    pub xpath: Option<String>,
    pub value_xml: Option<String>,
    pub order: Option<PatchOrderMode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeValueOperation {
    pub xpath: Option<String>,
    pub attribute: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeOperation {
    pub xpath: Option<String>,
    pub attribute: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetNameOperation {
    pub xpath: Option<String>,
    pub name: Option<String>,
}

/// A `Class` RimEdit does not recognize as a built-in operation, or a built-in/custom operation
/// still being edited as raw XML. Round-trips via the exact original XML span (the whole
/// `<Operation Class="..." ...>...</Operation>` element, including `Class`/`success`/other
/// attributes) -- `patches::serializer` writes `raw_xml` verbatim for this variant and ignores
/// the containing `PatchOperationNode`'s `class_name`/`success`/`attributes` fields entirely, so
/// editors must treat an `Unknown` node's raw XML as the single source of truth for the whole
/// operation, not just its kind-specific fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnknownPatchOperation {
    pub raw_xml: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PatchOperationKind {
    Add(PathedValueOrderOperation),
    Insert(PathedValueOrderOperation),
    Remove(PathedOperation),
    Replace(PathedValueOperation),
    AttributeAdd(AttributeValueOperation),
    AttributeSet(AttributeValueOperation),
    AttributeRemove(AttributeOperation),
    AddModExtension(PathedValueOperation),
    SetName(SetNameOperation),
    Sequence(Vec<PatchOperationNode>),
    FindMod {
        mods: Vec<String>,
        match_op: Option<Box<PatchOperationNode>>,
        nomatch_op: Option<Box<PatchOperationNode>>,
    },
    Conditional {
        xpath: Option<String>,
        match_op: Option<Box<PatchOperationNode>>,
        nomatch_op: Option<Box<PatchOperationNode>>,
    },
    Test(PathedOperation),
    Unknown(UnknownPatchOperation),
}

pub const BUILT_IN_OPERATION_CLASSES: &[&str] = &[
    "PatchOperationAdd",
    "PatchOperationInsert",
    "PatchOperationRemove",
    "PatchOperationReplace",
    "PatchOperationAttributeAdd",
    "PatchOperationAttributeSet",
    "PatchOperationAttributeRemove",
    "PatchOperationAddModExtension",
    "PatchOperationSetName",
    "PatchOperationSequence",
    "PatchOperationFindMod",
    "PatchOperationConditional",
    "PatchOperationTest",
];

impl PatchOperationNode {
    pub fn is_known_class(&self) -> bool {
        !matches!(self.kind, PatchOperationKind::Unknown(_))
    }
}

#[cfg(test)]
mod diagnostic_ref_wire_tests {
    use super::*;
    use crate::diagnostics::diagnostic_args;

    #[test]
    fn patch_diagnostic_wire_shape_carries_code_and_args() {
        let diag = PatchDiagnostic::new(Some(3), Some(1), "missing required <xpath> field")
            .with_code("patch_missing_required_field")
            .with_args(diagnostic_args([("fieldName", "xpath".into())]));
        let json = serde_json::to_value(&diag).unwrap();
        assert_eq!(json["code"], "patch_missing_required_field");
        assert_eq!(json["args"]["fieldName"], "xpath");
    }

    #[test]
    fn patch_diagnostic_without_code_or_args_omits_both_fields() {
        let diag = PatchDiagnostic::new(None, None, "raw parser text");
        let json = serde_json::to_value(&diag).unwrap();
        assert!(json.get("code").is_none());
        assert!(json.get("args").is_none());
    }
}
