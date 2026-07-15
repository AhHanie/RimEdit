//! Shared trace/diagnostic construction for missing required fields and XPath evaluation
//! outcomes. Used by both `super::mutations` and `super::control_flow` so every XPath-backed
//! operation reports the same stable diagnostic codes and wording.

use crate::diagnostics::diagnostic_args;

use super::{ApplyDiagnostic, PatchOperationKey};

pub(super) fn missing_field_diagnostic(
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
    field: &str,
) {
    diagnostics.push(
        ApplyDiagnostic::error(
            "patch_apply_missing_field",
            format!("Operation is missing its required '{}' field", field),
            Some(key.clone()),
        )
        .with_args(diagnostic_args([("fieldName", field.into())])),
    );
}

pub(super) fn xpath_error_diagnostic(
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
    xpath: &str,
    error: &str,
) {
    diagnostics.push(
        ApplyDiagnostic::warning(
            "patch_apply_xpath_error",
            format!("XPath \"{}\" failed to evaluate: {}", xpath, error),
            Some(key.clone()),
        )
        .with_args(diagnostic_args([
            ("xpath", xpath.into()),
            ("error", error.into()),
        ])),
    );
}

/// A well-formed, successfully-evaluated XPath matched zero nodes. Distinct from
/// `xpath_error_diagnostic` (a genuine evaluation failure) -- this is the common "nothing to act
/// on" case for a mutating operation (Add/Insert/Remove/Replace/attribute ops/AddModExtension/
/// SetName), which today silently no-ops with no explanation. Not raised for `Test`/`Conditional`,
/// where "zero matches" is itself the operation's intended outcome, not a failure to explain.
pub(super) fn xpath_no_match_diagnostic(
    diagnostics: &mut Vec<ApplyDiagnostic>,
    key: &PatchOperationKey,
    xpath: &str,
) {
    diagnostics.push(
        ApplyDiagnostic::warning(
            "patch_apply_xpath_no_match",
            format!("XPath \"{}\" did not match any node", xpath),
            Some(key.clone()),
        )
        .with_args(diagnostic_args([("xpath", xpath.into())])),
    );
}
