use crate::schema_pack::FieldTypeKind;

/// Check a successfully-parsed numeric value against optional inclusive min/max bounds.
///
/// Returns `None` when the value is in range or when `kind` is not numeric.
/// Returns `Some(message)` when the value falls outside the configured bounds.
///
/// Callers must only invoke this after the value has already been confirmed to parse
/// as the expected numeric type -- it returns `None` for unparsable input so that a
/// type-mismatch diagnostic and an out-of-range diagnostic are never both emitted.
pub(super) fn check_numeric_bounds(
    value: &str,
    kind: &FieldTypeKind,
    min: Option<f64>,
    max: Option<f64>,
) -> Option<String> {
    let as_f64 = match kind {
        FieldTypeKind::Integer => value.parse::<i64>().ok().map(|n| n as f64)?,
        FieldTypeKind::Float => value.parse::<f64>().ok()?,
        _ => return None,
    };
    if let Some(lo) = min {
        if as_f64 < lo {
            return Some(format!(
                "value {value} is below the configured minimum {lo}"
            ));
        }
    }
    if let Some(hi) = max {
        if as_f64 > hi {
            return Some(format!(
                "value {value} is above the configured maximum {hi}"
            ));
        }
    }
    None
}

pub(super) fn valid_vector(value: &str, expected_len: usize) -> bool {
    let Some(inner) = value.strip_prefix('(').and_then(|s| s.strip_suffix(')')) else {
        return false;
    };
    let parts = inner.split(',').map(str::trim).collect::<Vec<_>>();
    parts.len() == expected_len
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.parse::<f64>().is_ok())
}

/// Validate a `UnityEngine.Color` tuple string: `(r, g, b)` or `(r, g, b, a)`.
/// Checks format only (parens, 3–4 comma-separated numeric tokens); does not enforce range.
pub(super) fn valid_color(value: &str) -> bool {
    let Some(inner) = value.strip_prefix('(').and_then(|s| s.strip_suffix(')')) else {
        return false;
    };
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() < 3 || parts.len() > 4 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.parse::<f64>().is_ok())
}

pub(super) fn is_valid_scalar_value(value: &str, kind: &FieldTypeKind) -> bool {
    match kind {
        FieldTypeKind::String
        | FieldTypeKind::LocalizedString
        | FieldTypeKind::TypeName
        | FieldTypeKind::DefReference
        | FieldTypeKind::Enum
        | FieldTypeKind::Unknown
        | FieldTypeKind::Unrecognized => true,
        FieldTypeKind::Integer => !value.is_empty() && value.parse::<i64>().is_ok(),
        FieldTypeKind::Float => !value.is_empty() && value.parse::<f64>().is_ok(),
        FieldTypeKind::Boolean => {
            matches!(value, "true" | "false" | "True" | "False" | "1" | "0")
        }
        FieldTypeKind::Vector2 => valid_vector(value, 2),
        FieldTypeKind::Vector3 => valid_vector(value, 3),
        FieldTypeKind::Color => valid_color(value),
        FieldTypeKind::IntRange => {
            // "min~max" or a single integer (RimWorld treats it as min == max)
            if let Some((a, b)) = value.split_once('~') {
                a.trim().parse::<i64>().is_ok() && b.trim().parse::<i64>().is_ok()
            } else {
                value.parse::<i64>().is_ok()
            }
        }
        FieldTypeKind::FloatRange => {
            if let Some((a, b)) = value.split_once('~') {
                a.trim().parse::<f64>().is_ok() && b.trim().parse::<f64>().is_ok()
            } else {
                false
            }
        }
        FieldTypeKind::List | FieldTypeKind::Object | FieldTypeKind::StatMap => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- check_numeric_bounds ---

    #[test]
    fn numeric_bounds_accepts_exact_inclusive_minimum_integer() {
        assert!(check_numeric_bounds("0", &FieldTypeKind::Integer, Some(0.0), None).is_none());
        assert!(check_numeric_bounds("5", &FieldTypeKind::Integer, Some(5.0), None).is_none());
    }

    #[test]
    fn numeric_bounds_accepts_exact_inclusive_maximum_integer() {
        assert!(check_numeric_bounds("100", &FieldTypeKind::Integer, None, Some(100.0)).is_none());
    }

    #[test]
    fn numeric_bounds_accepts_exact_inclusive_minimum_float() {
        assert!(check_numeric_bounds("0.0", &FieldTypeKind::Float, Some(0.0), None).is_none());
    }

    #[test]
    fn numeric_bounds_accepts_exact_inclusive_maximum_float() {
        assert!(check_numeric_bounds("1.5", &FieldTypeKind::Float, None, Some(1.5)).is_none());
    }

    #[test]
    fn numeric_bounds_rejects_integer_below_minimum() {
        assert!(check_numeric_bounds("-1", &FieldTypeKind::Integer, Some(0.0), None).is_some());
    }

    #[test]
    fn numeric_bounds_rejects_integer_above_maximum() {
        assert!(check_numeric_bounds("101", &FieldTypeKind::Integer, None, Some(100.0)).is_some());
    }

    #[test]
    fn numeric_bounds_rejects_float_below_minimum() {
        assert!(check_numeric_bounds("-0.1", &FieldTypeKind::Float, Some(0.0), None).is_some());
    }

    #[test]
    fn numeric_bounds_rejects_float_above_maximum() {
        assert!(check_numeric_bounds("1.6", &FieldTypeKind::Float, None, Some(1.5)).is_some());
    }

    #[test]
    fn numeric_bounds_returns_none_for_unparsable_value() {
        // Unparsable input must not produce a bounds diagnostic; only type-mismatch fires.
        assert!(
            check_numeric_bounds("not-a-number", &FieldTypeKind::Integer, Some(0.0), None)
                .is_none()
        );
        assert!(check_numeric_bounds("", &FieldTypeKind::Float, Some(0.0), None).is_none());
    }

    #[test]
    fn numeric_bounds_accepts_value_within_two_sided_range() {
        assert!(
            check_numeric_bounds("50", &FieldTypeKind::Integer, Some(0.0), Some(100.0)).is_none()
        );
    }

    #[test]
    fn numeric_bounds_ignores_non_numeric_kinds() {
        assert!(check_numeric_bounds("hello", &FieldTypeKind::String, Some(0.0), None).is_none());
    }

    // --- existing tests ---

    #[test]
    fn int_range_scalar_accepts_min_tilde_max() {
        assert!(is_valid_scalar_value("0~100", &FieldTypeKind::IntRange));
        assert!(is_valid_scalar_value(
            "1200000~1200000",
            &FieldTypeKind::IntRange
        ));
        assert!(is_valid_scalar_value("-5~5", &FieldTypeKind::IntRange));
    }

    #[test]
    fn int_range_scalar_accepts_single_integer() {
        assert!(is_valid_scalar_value("1200000", &FieldTypeKind::IntRange));
        assert!(is_valid_scalar_value("0", &FieldTypeKind::IntRange));
        assert!(is_valid_scalar_value("-1", &FieldTypeKind::IntRange));
    }

    #[test]
    fn int_range_scalar_rejects_non_numeric() {
        assert!(!is_valid_scalar_value("fast", &FieldTypeKind::IntRange));
        assert!(!is_valid_scalar_value("1.5", &FieldTypeKind::IntRange));
        assert!(!is_valid_scalar_value("", &FieldTypeKind::IntRange));
    }
}
