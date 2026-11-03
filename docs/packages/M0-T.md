# M0-T: transport and cold-start spike (Claude sub-agent)

## Why
The architecture serves images to the webview from an embedded axum server
on 127.0.0.1. The main reason is one HTTP semantics shared with the Docker
build. Performance is the secondary claim, and it rests on 2022 Tauri v1
data (tauri#4197: custom protocol 175s vs localhost 1s for the same file).
This spike measures the claim on the real stack and decides the G0 gate: if
the web route misses the budgets, Windows goes native (Rust + Direct2D + WIC).

## Owned paths
`spikes/transport/**` only. It is a standalone Tauri 2 app. Its own
`Cargo.toml` has an empty `[workspace]` table, and it has its own lockfile,
so the root manifest is not touched. Pin `tauri = "=2.12.5"`
and `tauri-build = "=2.7.4"` (crates.io latest stable on 2026-11-03), plus
axum `=0.8.9` and tokio `=1.54.1` to match the root workspace. Use the Tauri
CLI through `npx @tauri-apps/cli@2` or `cargo tauri` if installed; if neither
works offline, use `cargo run` with a hand-written `tauri.conf.json`.

## Build
A window loads `index.html` from the embedded axum server; do not use
`tauri://`.

1. **Transport.** Two transports for the same file:
   - (a) `http://127.0.0.1:<random port>/file?n=...`, served by axum with
     streaming `tokio::fs` reads.
   - (b) an asynchronous custom protocol registered with
     `register_asynchronous_uri_scheme_protocol` (on Windows this is
     reachable as `http://<scheme>.localhost/...`).

   Generate the test JPEGs yourself in the spike, with the `image` crate as a
   spike-only dependency (≈3MB and ≈30MB; noise compresses badly, so noise
   gives large files). For each size × transport, run 10 cold-ish runs, each
   with a cache-busting query. Time from `img.src = ...` to `img.decode()`
   resolving, using `performance.now()`. Report median and p90.
2. **Cold start.** Run 5 launches of the release build and record
   timestamps. Measure two cases separately:
   - first launch after reboot (if you can't reboot, say so and measure only
     "daily" cold start);
   - daily cold start.

   For each launch, record:
   - process start → axum listening
   - process start → WebView2 created (Tauri `setup` / window-created event)
   - process start → first image `decode()` resolved, with a 24MP JPEG
     passed as a CLI argument

   Start axum and the first decode in parallel with WebView2 init, as the
   plan says.
3. **Prefetched next.** Prefetch 3 images with `new Image()` +
   `decode()`, then swap the visible `<img>` src to the prefetched URL. Time
   until the next `requestAnimationFrame` after the swap. Report median over
   20 swaps.
4. **Idle memory.** Working set of the whole process tree (app + WebView2
   children) 10 s after showing one image.

## Acceptance
- `spikes/transport/REPORT.md` contains:
  - the machine (CPU, GPU, Windows build, WebView2 version);
  - a table with every number above;
  - the exact commands to reproduce;
  - a verdict per budget: prefetched next ≤30ms, idle memory ≤150MB, and a
    proposed cold-start budget.
- The report clearly separates measured numbers from inferences.
- The spike builds with `cargo build --release` inside `spikes/transport`.
- `node scripts/gates.mjs --ownership M0-T main` passes. Files must be ≤300
  lines.

## Out of scope
No UI polish and no reuse of `web/`. The spike is throwaway; only the report
matters.
