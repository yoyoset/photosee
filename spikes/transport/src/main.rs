#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! M0-T spike: axum-on-127.0.0.1 vs custom protocol, cold start, prefetch,
//! idle memory. Usage:
//!   transport-spike --bench <samples-dir> --out <json>
//!   transport-spike <image.jpg> --out <json> [--hold <s>] [--launched-at <epoch-ms>]
mod clock;
mod proto;
mod server;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tauri::{WebviewUrl, WebviewWindowBuilder};
use tokio::sync::OnceCell;

use clock::{Timeline, epoch_ms, process_created_epoch_ms};
use server::{Shared, State};

fn parse_args(t_main: f64) -> Result<State, String> {
    let mut args = std::env::args().skip(1);
    let (mut bench, mut samples, mut first, mut out) = (false, PathBuf::new(), None, None);
    let (mut hold, mut launched_at) = (Duration::from_millis(300), None);
    while let Some(a) = args.next() {
        let mut val = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--bench" => (bench, samples) = (true, PathBuf::from(val()?)),
            "--out" => out = Some(PathBuf::from(val()?)),
            "--hold" => hold = Duration::from_secs_f64(val()?.parse().map_err(|_| "bad --hold")?),
            "--launched-at" => launched_at = Some(val()?.parse().map_err(|_| "bad --launched-at")?),
            _ => first = Some(PathBuf::from(a)),
        }
    }
    let timeline = Timeline::default();
    timeline.mark_at("main", t_main);
    Ok(State {
        bench,
        samples,
        first_path: first,
        first: OnceCell::new(),
        timeline,
        created_epoch: process_created_epoch_ms().unwrap_or(t_main),
        launched_at,
        out: out.ok_or("--out is required")?,
        hold,
        app: OnceLock::new(),
    })
}

/// Binds synchronously (so the window URL is known at once) and serves on
/// Tauri's tokio runtime, in parallel with WebView2 creation.
fn start_server(st: &Shared) -> Result<u16, String> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    std_listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = std_listener.local_addr().map_err(|e| e.to_string())?.port();
    st.timeline.mark("axum_listening");
    let router = server::router(st.clone());
    tauri::async_runtime::spawn(async move {
        if let Ok(l) = tokio::net::TcpListener::from_std(std_listener) {
            let _ = axum::serve(l, router).await;
        }
    });
    if !st.bench {
        // Read the CLI image while WebView2 starts; /first then serves memory.
        let st2 = st.clone();
        tauri::async_runtime::spawn(async move { server::first_bytes(&st2).await });
    }
    Ok(port)
}

fn run(st: Shared, port: u16) -> tauri::Result<()> {
    let st_proto = st.clone();
    let st_setup = st.clone();
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(proto::SCHEME, move |_ctx, req, responder| {
            let st = st_proto.clone();
            tauri::async_runtime::spawn(async move { responder.respond(proto::serve(st, req).await) });
        })
        .setup(move |app| {
            st_setup.timeline.mark("tauri_setup");
            let _ = st_setup.app.set(app.handle().clone());
            let url = format!("http://127.0.0.1:{port}/").parse().map_err(|e| format!("{e}"))?;
            let st_load = st_setup.clone();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("transport-spike")
                .inner_size(1600.0, 1000.0)
                // WebView2 stops rAF in occluded windows; keep it visible.
                .always_on_top(true)
                .on_page_load(move |_w, p| {
                    let name = format!("page_load_{:?}", p.event()).to_lowercase();
                    st_load.timeline.mark(&name);
                })
                .build()?;
            st_setup.timeline.mark("webview_created");
            Ok(())
        })
        .run(tauri::generate_context!())
}

fn main() {
    let t_main = epoch_ms();
    let st = match parse_args(t_main) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("transport-spike: {e}");
            std::process::exit(2);
        }
    };
    let port = match start_server(&st) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("transport-spike: cannot bind: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = run(st, port) {
        eprintln!("transport-spike: {e}");
        std::process::exit(1);
    }
}
