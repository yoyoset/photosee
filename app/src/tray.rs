//! Notification-area icon and the "keep running in the background" choice.
//!
//! User feedback, 2026-11-04: a resident process with no visible trace and no
//! way to quit was unacceptable. So:
//! - The tray icon is always shown while the process runs. Left click opens
//!   the window; the menu offers Open, Keep in background, and Quit.
//! - Background mode is opt-in (off by default). When it is off, closing the
//!   window quits. When it is on, closing hides the window, and the tray
//!   icon stays visible as the way back and the way out.
//!
//! The choice is stored in `<data dir>/app.json`, which belongs to the shell,
//! not to the UI settings (six, docs/ux-spec.md).

use crate::window;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

static RESIDENT: AtomicBool = AtomicBool::new(false);

/// True when closing the window should only hide it.
pub fn resident() -> bool {
    RESIDENT.load(Ordering::Relaxed)
}

fn prefs_file() -> PathBuf {
    crate::serve::data_dir().join("app.json")
}

/// Reads the stored choice once at start-up. A missing or broken file means
/// the default: quit on close.
pub fn load() {
    let on = std::fs::read(prefs_file())
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("resident")?.as_bool())
        .unwrap_or(false);
    RESIDENT.store(on, Ordering::Relaxed);
}

fn save(on: bool) {
    RESIDENT.store(on, Ordering::Relaxed);
    let body = serde_json::json!({ "resident": on }).to_string();
    // Best effort: losing this choice only costs one extra cold start.
    let _ = photosee_core::config::write_atomic(&prefs_file(), body.as_bytes());
}

/// Chinese when the Windows UI language is Chinese, English otherwise; the
/// web UI follows the same rule through navigator.language.
fn zh() -> bool {
    #[cfg(windows)]
    {
        // SAFETY: no arguments; returns the user's UI LANGID.
        let lang = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        lang & 0x3ff == 0x04 // PRIMARYLANGID == LANG_CHINESE
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let (open, keep, quit) = if zh() {
        ("打开 PhotoSee", "后台常驻（下次打开更快）", "退出")
    } else {
        (
            "Open PhotoSee",
            "Keep running in background (faster next open)",
            "Quit",
        )
    };
    let open = MenuItem::with_id(app, "open", open, true, None::<&str>)?;
    let keep = CheckMenuItem::with_id(app, "resident", keep, true, resident(), None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &keep, &sep, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("PhotoSee")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, ev| match ev.id().as_ref() {
            "open" => window::show(app),
            "resident" => save(keep.is_checked().unwrap_or(false)),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                window::show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Window close button: hide in background mode, otherwise quit.
pub fn on_close(app: &AppHandle) -> bool {
    if resident() {
        if let Some(w) = app.get_webview_window(window::MAIN) {
            let _ = w.set_fullscreen(false);
            let _ = w.hide();
        }
        true
    } else {
        app.exit(0);
        false
    }
}
