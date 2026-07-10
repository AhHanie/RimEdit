use crate::instrumentation;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentationConfigDto {
    pub available: bool,
    pub enabled: bool,
    pub sink: String,
}

#[tauri::command]
pub fn get_instrumentation_config(app: AppHandle) -> InstrumentationConfigDto {
    #[cfg(debug_assertions)]
    {
        InstrumentationConfigDto {
            available: true,
            enabled: instrumentation::is_enabled(&app),
            sink: "console".to_string(),
        }
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = app;
        InstrumentationConfigDto {
            available: false,
            enabled: false,
            sink: "console".to_string(),
        }
    }
}

#[tauri::command]
pub fn set_instrumentation_enabled(app: AppHandle, enabled: bool) -> InstrumentationConfigDto {
    instrumentation::set_enabled(&app, enabled);
    get_instrumentation_config(app)
}
