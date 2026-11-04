//! The one window, and the glue between Tauri and the host routes.

use crate::host::WindowCtl;
use tauri::window::Color;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";

pub struct TauriWin(pub AppHandle);

impl TauriWin {
    fn win(&self) -> Option<WebviewWindow> {
        self.0.get_webview_window(MAIN)
    }
}

impl WindowCtl for TauriWin {
    fn is_fullscreen(&self) -> bool {
        self.win()
            .and_then(|w| w.is_fullscreen().ok())
            .unwrap_or(false)
    }

    fn set_fullscreen(&self, on: bool) -> bool {
        if let Some(w) = self.win() {
            let _ = w.set_fullscreen(on);
        }
        self.is_fullscreen()
    }

    fn hide(&self) {
        // Leave fullscreen first, so the next show is a normal window and
        // not a fullscreen one the user did not ask for.
        self.set_fullscreen(false);
        if let Some(w) = self.win() {
            let _ = w.hide();
        }
    }

    fn quit(&self) {
        self.0.exit(0);
    }
}

/// Black before the first paint: WebView2's default is white, which flashes
/// while the page loads.
pub fn create(app: &tauri::App, url: WebviewUrl, script: &str) -> tauri::Result<()> {
    let mut b = WebviewWindowBuilder::new(app, MAIN, url);
    // WebView2 keeps its profile (cache, cookies) under the user's AppData
    // by default; a portable copy keeps it in its own folder instead.
    if crate::serve::portable_dir().is_some() {
        b = b.data_directory(crate::serve::data_dir().join("webview"));
    }
    b.title("PhotoSee")
        .inner_size(1280.0, 800.0)
        .center()
        .background_color(Color(0, 0, 0, 255))
        .initialization_script(script)
        .build()?;
    Ok(())
}

pub fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Start-up failed (no web UI, no randomness, ...). Without a dialog plugin
/// the least surprising place to say so is the window itself; double-clicking
/// a photo and seeing nothing would be worse.
pub fn run_error(message: &str) -> i32 {
    let script = format!(
        "window.__PHOTOSEE_ERROR__ = {};",
        serde_json::Value::from(message)
    );
    let res = tauri::Builder::default()
        .setup(move |app| Ok(create(app, WebviewUrl::App("index.html".into()), &script)?))
        .run(tauri::generate_context!());
    if res.is_err() { 1 } else { 0 }
}
