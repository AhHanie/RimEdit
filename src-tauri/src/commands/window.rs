use tauri::{AppHandle, Manager};

/// Reveals the main window once the frontend has painted real content into it (see
/// `src/app/bootstrap.tsx`, which calls this right after its first synchronous render commits).
/// `tauri.conf.json`'s window `visible: false` keeps the window hidden -- but still fully
/// rendering, just not presented to the display -- until this fires. This is what actually
/// eliminates the startup white flash: the `.setup()` background-color override in `lib.rs` only
/// recolors what would otherwise flash, but on Windows the native window and WebView2's own
/// compositor can each briefly show their own default white during initialization regardless of
/// that color. Hiding the window until content is already painted leaves nothing left to flash.
#[tauri::command]
pub fn signal_startup_ready(app: AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("[rimedit] signal_startup_ready: no \"main\" window to show");
        return;
    };
    if let Err(e) = window.show() {
        eprintln!("[rimedit] Failed to show the main window: {e}");
        return;
    }
    let _ = window.set_focus();
}
