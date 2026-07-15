//! App locale registry and validation.
//!
//! Mirrors the supported-locale list in `src/i18n/locale.ts`. Rust does not
//! import frontend code, so the two lists are kept in sync by hand; both
//! currently contain only `"en"`. Add a locale to both places together when a
//! second locale ships (see `docs/i18n/issues/02-global-locale-settings.md`).

use crate::project_model::StoreError;

pub const SUPPORTED_LOCALES: &[&str] = &["en"];
pub const FALLBACK_LOCALE: &str = "en";

pub fn is_supported_locale(code: &str) -> bool {
    SUPPORTED_LOCALES.contains(&code)
}

/// Resolve any locale string to a supported code, falling back to `en` for
/// unknown or legacy values.
pub fn resolve_locale(code: &str) -> String {
    if is_supported_locale(code) {
        code.to_string()
    } else {
        FALLBACK_LOCALE.to_string()
    }
}

/// Validate a locale string for use as the persisted app locale. Rejects
/// anything outside `SUPPORTED_LOCALES`.
pub fn validate_locale(code: &str) -> Result<(), StoreError> {
    if is_supported_locale(code) {
        Ok(())
    } else {
        Err(StoreError::UnsupportedLocale(code.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_is_supported() {
        assert!(is_supported_locale("en"));
    }

    #[test]
    fn unknown_locale_is_not_supported() {
        assert!(!is_supported_locale("fr"));
        assert!(!is_supported_locale(""));
    }

    #[test]
    fn resolve_locale_falls_back_for_unknown() {
        assert_eq!(resolve_locale("fr"), "en");
        assert_eq!(resolve_locale(""), "en");
    }

    #[test]
    fn resolve_locale_keeps_supported() {
        assert_eq!(resolve_locale("en"), "en");
    }

    #[test]
    fn validate_locale_accepts_supported() {
        assert!(validate_locale("en").is_ok());
    }

    #[test]
    fn validate_locale_rejects_unsupported() {
        assert!(validate_locale("fr").is_err());
        // Exact match only; no case folding.
        assert!(validate_locale("EN").is_err());
    }
}
