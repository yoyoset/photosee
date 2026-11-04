#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! PhotoSee desktop shell: Tauri window + the embedded axum server.
//!
//! Background mode (G0's resident mode) is opt-in from the tray menu
//! (tray.rs). Off by default: the close button quits. On: closing hides the
//! window and later opens reuse the warm WebView2. The tray icon is always
//! visible, so a running process can always be seen and quit. That replaced
//! the first version's invisible resident process after user feedback.
mod args;
mod host;
mod perf;
mod places;
mod serve;
mod token;
mod tray;
mod window;

use axum::Router;
use photosee_server::AppState;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Manager, WebviewUrl, WindowEvent};

struct Plan {
    listener: TcpListener,
    port: u16,
    token: String,
    state: AppState,
    web_dir: PathBuf,
}

fn prepare(start: Option<String>) -> Result<Plan, String> {
    let token = token::session_token()?;
    let (listener, port) = serve::bind()?;
    let state = serve::build_state(
        token.clone(),
        serve::allowed_hosts(port),
        start,
        &serve::data_dir(),
        places::places(),
    );
    Ok(Plan {
        listener,
        port,
        token,
        state,
        web_dir: serve::find_web_dir()?,
    })
}

/// A second launch lands here, in the first process.
fn on_second_launch(app: &AppHandle, start: &RwLock<Option<String>>, argv: &[String], cwd: &str) {
    let path = args::start_from(argv.get(1..).unwrap_or_default(), Path::new(cwd));
    if let Some(p) = path {
        // A poisoned lock still holds a usable value; overwrite it.
        *start.write().unwrap_or_else(|e| e.into_inner()) = Some(p);
        if let Some(w) = app.get_webview_window(window::MAIN) {
            // The page boots from /session, which now reports the new start.
            let _ = w.eval("location.reload()");
        }
    }
    window::show(app);
}

fn run(plan: Plan) -> i32 {
    let Plan {
        listener,
        port,
        token,
        state,
        web_dir,
    } = plan;
    let start = state.start.clone();
    let url = format!("http://127.0.0.1:{port}/?t={token}");
    let res = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(move |app, argv, cwd| {
            on_second_launch(app, &start, &argv, &cwd);
        }))
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|w, ev| {
            if let WindowEvent::CloseRequested { api, .. } = ev {
                // Background mode hides; otherwise the close quits (tray.rs).
                if tray::on_close(w.app_handle()) {
                    api.prevent_close();
                }
            }
        })
        .setup(move |app| {
            perf::mark("setup");
            tray::load();
            tray::install(app)?;
            let ctl = Arc::new(window::TauriWin(app.handle().clone()));
            let router: Router = photosee_server::router(state.clone(), Some(web_dir))
                .merge(host::routes(ctl, state, perf::out_path()));
            serve::spawn(listener, router);
            let mut script = host::BRIDGE_JS.to_owned();
            if perf::out_path().is_some() {
                script.push_str(perf::PERF_JS);
            }
            let parsed = url.parse().map_err(|e| format!("bad window url: {e}"))?;
            window::create(app, WebviewUrl::External(parsed), &script)?;
            perf::mark("window_created");
            Ok(())
        })
        .run(tauri::generate_context!());
    match res {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("photosee: {e}");
            1
        }
    }
}

fn main() {
    perf::mark("main");
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    let code = match prepare(args::start_from(&argv, &cwd)) {
        Ok(plan) => run(plan),
        Err(msg) => window::run_error(&msg),
    };
    std::process::exit(code);
}
