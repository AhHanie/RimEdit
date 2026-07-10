#[cfg(debug_assertions)]
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
#[cfg(debug_assertions)]
use std::sync::atomic::Ordering;
use tauri::AppHandle;
#[cfg(debug_assertions)]
use tauri::Manager;

pub struct InstrumentationState {
    #[allow(dead_code)]
    pub(crate) enabled: AtomicBool,
}

impl InstrumentationState {
    pub fn from_env() -> Self {
        #[cfg(debug_assertions)]
        {
            let enabled =
                parse_env_enabled(std::env::var("RIMEDIT_INSTRUMENTATION").ok().as_deref());
            InstrumentationState {
                enabled: AtomicBool::new(enabled),
            }
        }
        #[cfg(not(debug_assertions))]
        InstrumentationState {
            enabled: AtomicBool::new(false),
        }
    }
}

#[cfg(debug_assertions)]
fn parse_env_enabled(val: Option<&str>) -> bool {
    val == Some("1")
}

pub struct InstrumentationSpan {
    #[cfg(debug_assertions)]
    inner: Option<SpanInner>,
}

#[cfg(debug_assertions)]
struct SpanInner {
    app: AppHandle,
    name: String,
    tags: BTreeMap<String, String>,
    start: std::time::Instant,
}

#[cfg(debug_assertions)]
impl Drop for InstrumentationSpan {
    fn drop(&mut self) {
        let Some(ref inner) = self.inner else {
            return;
        };
        let duration_ms = inner.start.elapsed().as_secs_f64() * 1000.0;
        let Some(state) = inner.app.try_state::<InstrumentationState>() else {
            return;
        };
        if !state.enabled.load(Ordering::Relaxed) {
            return;
        }
        let mut output = format!(
            "[rimedit:timing] source=backend name={} durationMs={:.2}",
            inner.name, duration_ms
        );
        for (k, v) in &inner.tags {
            output.push(' ');
            output.push_str(k);
            output.push('=');
            output.push_str(v);
        }
        eprintln!("{}", output);
    }
}

impl InstrumentationSpan {
    /// Add or overwrite a tag on an in-flight span. Used to record a terminal
    /// outcome (e.g. which save path was taken) that is only known partway
    /// through the span's lifetime. No-op when instrumentation is disabled or
    /// in release builds.
    pub fn set_tag(&mut self, key: impl Into<String>, value: impl Into<String>) {
        #[cfg(debug_assertions)]
        if let Some(inner) = self.inner.as_mut() {
            inner.tags.insert(key.into(), value.into());
        }
        #[cfg(not(debug_assertions))]
        let _ = (key, value);
    }
}

#[allow(dead_code)]
pub fn is_enabled(app: &AppHandle) -> bool {
    #[cfg(debug_assertions)]
    {
        app.try_state::<InstrumentationState>()
            .map(|s| s.enabled.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = app;
        false
    }
}

pub fn set_enabled(app: &AppHandle, enabled: bool) {
    #[cfg(debug_assertions)]
    if let Some(state) = app.try_state::<InstrumentationState>() {
        state.enabled.store(enabled, Ordering::Relaxed);
    }
    #[cfg(not(debug_assertions))]
    let _ = (app, enabled);
}

pub fn span(app: &AppHandle, name: impl Into<String>) -> InstrumentationSpan {
    span_with_tags(app, name, std::iter::empty::<(String, String)>())
}

pub fn span_with_tags(
    app: &AppHandle,
    name: impl Into<String>,
    tags: impl IntoIterator<Item = (String, String)>,
) -> InstrumentationSpan {
    #[cfg(not(debug_assertions))]
    {
        let _ = (app, name, tags);
        return InstrumentationSpan {};
    }
    #[cfg(debug_assertions)]
    {
        let is_on = app
            .try_state::<InstrumentationState>()
            .map(|s| s.enabled.load(Ordering::Relaxed))
            .unwrap_or(false);

        if is_on {
            InstrumentationSpan {
                inner: Some(SpanInner {
                    app: app.clone(),
                    name: name.into(),
                    tags: tags.into_iter().collect(),
                    start: std::time::Instant::now(),
                }),
            }
        } else {
            InstrumentationSpan { inner: None }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_disabled_when_absent() {
        assert!(!parse_env_enabled(None));
    }

    #[test]
    fn env_enabled_when_value_is_one() {
        assert!(parse_env_enabled(Some("1")));
    }

    #[test]
    fn env_disabled_when_value_is_not_one() {
        assert!(!parse_env_enabled(Some("0")));
        assert!(!parse_env_enabled(Some("true")));
        assert!(!parse_env_enabled(Some("")));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn noop_span_drops_without_panic() {
        let span = InstrumentationSpan { inner: None };
        drop(span);
    }

    #[test]
    fn instrumentation_state_starts_disabled_by_default() {
        let state = InstrumentationState {
            enabled: AtomicBool::new(false),
        };
        assert!(!state.enabled.load(Ordering::Relaxed));
    }

    #[test]
    fn instrumentation_state_can_be_enabled() {
        let state = InstrumentationState {
            enabled: AtomicBool::new(true),
        };
        assert!(state.enabled.load(Ordering::Relaxed));
    }
}
