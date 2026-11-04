//! Loopback benchmarks for package G's acceptance numbers. Ignored by
//! default; run in release:
//!   cargo test --release -p photosee-server --test api_bench -- --ignored --nocapture
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "api_support.rs"]
mod support;

use photosee_core::io::LocalFs;
use photosee_server::paths::Policy;
use photosee_server::queue::IoQueue;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::*;

/// Serves a folder with a 3 MB BMP and a 400x300 BMP on a random port.
async fn serve(dir: &Path) -> SocketAddr {
    std::fs::write(dir.join("big.bmp"), bmp(1000, 1000)).unwrap();
    std::fs::write(dir.join("t.bmp"), bmp(400, 300)).unwrap();
    let (state, tmp) = state_with(Arc::new(LocalFs), Policy::AnyLocal, IoQueue::new(4));
    std::mem::forget(tmp); // the cache dir must outlive the server task
    let app = photosee_server::router(state, None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn enc(p: &Path) -> String {
    p.to_str()
        .unwrap()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Keep-alive HTTP/1.1 GET; returns the body length.
fn fetch(conn: &mut BufReader<TcpStream>, path_and_query: &str) -> usize {
    let req = format!("GET {path_and_query} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    conn.get_mut().write_all(req.as_bytes()).unwrap();
    let mut len = 0;
    let mut status = String::new();
    conn.read_line(&mut status).unwrap();
    assert!(status.contains(" 200 "), "{status}");
    loop {
        let mut line = String::new();
        conn.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap();
        }
    }
    let mut body = vec![0u8; len];
    conn.read_exact(&mut body).unwrap();
    len
}

fn connect(addr: SocketAddr) -> BufReader<TcpStream> {
    let s = TcpStream::connect(addr).unwrap();
    s.set_nodelay(true).unwrap();
    BufReader::new(s)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "benchmark; run in release with --ignored"]
async fn bench_thumb_hits_and_img_passthrough() {
    let dir = tempfile::tempdir().unwrap();
    let addr = serve(dir.path()).await;
    let thumb = format!("/api/v1/thumb?p={}", enc(&dir.path().join("t.bmp")));
    let img = format!(
        "/api/v1/img?p={}&w=1000&h=1000",
        enc(&dir.path().join("big.bmp"))
    );
    let (thumb2, img2) = (thumb.clone(), img.clone());
    let report = tokio::task::spawn_blocking(move || {
        let mut c = connect(addr);
        let first = fetch(&mut c, &thumb2); // miss: generates and caches
        let rps = |clients: usize| {
            let start = Instant::now();
            let handles: Vec<_> = (0..clients)
                .map(|_| {
                    let t = thumb2.clone();
                    std::thread::spawn(move || {
                        let mut c = connect(addr);
                        let mut n = 0u32;
                        while start.elapsed() < Duration::from_secs(3) {
                            fetch(&mut c, &t);
                            n += 1;
                        }
                        n
                    })
                })
                .collect();
            let total: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
            f64::from(total) / start.elapsed().as_secs_f64()
        };
        let (one, eight) = (rps(1), rps(8));
        let mut times: Vec<Duration> = (0..300)
            .map(|_| {
                let t = Instant::now();
                assert_eq!(fetch(&mut c, &img2), 3_000_054);
                t.elapsed()
            })
            .collect();
        times.sort();
        (first, one, eight, times[150], times[285])
    })
    .await
    .unwrap();
    println!(
        "thumb {} bytes; /thumb cache hits: {:.0} req/s (1 conn), {:.0} req/s (8 conn); \
         /img passthrough 3 MB: p50 {:?}, p95 {:?}",
        report.0, report.1, report.2, report.3, report.4
    );
}
