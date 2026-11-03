# M0-T report: transport and cold start (Tauri 2.12.5 + WebView2)

Status: measured on 2026-11-04. Every number below comes from
`results/*.json`, produced by `run.mjs` and tabulated by `summarize.mjs`.
Sections are labelled **Measured** or **Inference**, and an inference is never
presented as a measurement.

## Machine

| | |
|---|---|
| CPU | Intel Xeon E5-2698 v3 @ 2.30 GHz, 16 cores / 32 threads (Haswell-EP, 2014) |
| RAM | 64 GB |
| GPU | NVIDIA GeForce RTX 3060, driver 32.0.16.1664. A "GameViewer Virtual Display Adapter" (remote-desktop software) is also installed |
| Display | 2560x1440 @ 59 Hz, 150 % scaling (page sees DPR 1.5) |
| Windows | Windows 11 Pro 23H2, build 22631.6199 |
| WebView2 | Evergreen runtime 156.0.4314.58 (UA `Edg/156.0.0.0`) |
| Stack | tauri 2.12.5 (wry 0.57.1, webview2-com 0.39.1), tauri-build 2.7.4, axum 0.8.9, tokio 1.54.1, Rust 1.92, release profile (thin LTO, codegen-units 1) |
| Last boot | 2026-11-03 11:00 +08:00, so all runs are warm-OS-cache runs |

## What the spike is

- One release binary, `transport-spike.exe`. The window loads `index.html`
  from the embedded axum server on `127.0.0.1:<random port>`. `tauri://` is
  never used.
- Transport (a) is `GET /file?n=...` on axum: `tokio::fs::File`, then
  `ReaderStream` with 256 KiB chunks, then `Body::from_stream`.
- Transport (b) is `register_asynchronous_uri_scheme_protocol("ps")`, reached
  as `http://ps.localhost/file?n=...`. The handler calls `tokio::fs::read` and
  responds with one complete `Vec<u8>`. The API has no streaming option.
- The test JPEGs come from `gen-jpegs` (image 0.25.10, q90):

  | file | pixels | bytes |
  |---|---|---|
  | `noise-3mb` | 1460x1100 RGB noise | 3,014,610 |
  | `noise-30mb` | 4900x3270 RGB noise | 29,947,698 |
  | `photo-24mp` | 6000x4000, gradients and light grain | 5.32 MB |
  | `photo-screen` | 2560x1440, same style | 0.84 MB |

- **Process start** is the kernel creation time of the process, read with
  `GetProcessTimes`. All cold-start numbers are milliseconds after that
  point. The driver also stamps the time just before `spawn()`. The gap
  between the two ("spawn→created") is 1.4 to 4.1 ms, which cross-checks the
  clock.
- In cold-start mode, axum binds synchronously in `main` before Tauri is
  built. It serves on Tauri's tokio runtime, and a task reads the CLI image
  into memory **in parallel with WebView2 creation**. `/first` then serves
  the image from memory. The page HTML contains `<img src="/first">`, so the
  request starts while the document is still being parsed.
- **Memory** is taken 10.0 s after `decode()` resolves. It is the whole
  process tree under `transport-spike.exe`: the app, the WebView2 browser,
  GPU, renderer, network, storage and crashpad processes, 7 in total. It is
  reported two ways. The first is the sum of `WorkingSetSize`, the brief's
  metric. The second is the sum of "Working Set - Private"
  (`Win32_PerfFormattedData_PerfProc_Process`), which is what Task
  Manager's default Memory column shows.

## Measured

### 1. Transport: `img.src = …` → `img.decode()` resolved (ms)

These are 3 bench launches × 10 cache-busted runs each, so n = 30 per row.
Within each run the two transports alternate order. "Fetch" is the same URL
read with `fetch().arrayBuffer()` and no decode, which isolates transfer
cost.

| file | transport | decode median | decode p90 | fetch median | fetch p90 |
|---|---|---|---|---|---|
| noise-3mb | axum 127.0.0.1 | **76.3** | 93.1 | 17.2 | 18.8 |
| noise-3mb | custom protocol | **139.4** | 148.6 | 79.6 | 87.3 |
| noise-30mb | axum 127.0.0.1 | **580.3** | 619.4 | 92.7 | 100.4 |
| noise-30mb | custom protocol | **1140.9** | 1182.1 | 623.4 | 664.0 |

The per-launch decode medians are stable. Axum 3 MB gave 76.6, 75.5 and
76.8. Custom protocol 30 MB gave 1138.6, 1146.2 and 1136.6.

### 2. Cold start: daily (warm OS cache, existing WebView2 profile), 5 launches

The first image is passed as a CLI argument. Columns are ms after process
creation, shown as median (max). The full per-launch rows are in the
`summarize.mjs` output.

| milestone | 24MP JPEG (5.3 MB) | screen-size JPEG (0.84 MB) |
|---|---|---|
| `main()` entered | 25.0 (32.4) | 28.6 (32.2) |
| **axum listening** | **27.4 (36.7)** | 31.1 (36.5) |
| image file in memory | 35.1 (45.9) | 34.4 (40.8) |
| Tauri `setup` | 42.8 (55.4) | 46.0 (56.9) |
| page navigation start (`timeOrigin`) | 624.3 (642.9) | 656.5 (691.3) |
| **WebView2 created** (`WebviewWindowBuilder::build()` returned) | **644.5 (662.8)** | 681.5 (715.8) |
| `/first` requested by the page | 662.8 (680.2) | 696.8 (738.2) |
| image fully received (`responseEnd`) | 677.9 (698.8) | 699.1 (740.3) |
| **first `decode()` resolved** | **1043.3 (1068.3)** | **891.4 (927.6)** |
| first paint (Element Timing `renderTime`) | 1461.1 (1492.3) | 1011.3 (1032.5) |

`build()` returns about 20 ms after navigation has already started.

Variant: **fresh WebView2 profile**, with
`%LOCALAPPDATA%\dev.photosee.transport-spike` deleted before each launch.
There were 3 launches with the 24MP JPEG. WebView2 created: median 693.1 ms
(max 710.5). `decode()` resolved: 1022.3 ms (1063.0). Painted: 1419.1 ms
(1480.1). Profile creation adds about 50 ms.

Single observation: on the **very first launch of a freshly linked exe**,
spawn→created was 491 ms, against 1 to 4 ms afterwards. WebView2 was created
at 903 ms and `decode()` resolved at 1292 ms. This happened once and was not
repeated.

### 3. Prefetched next: swap → next `requestAnimationFrame` (ms)

Three images are kept decoded ahead with `new Image()` + `decode()`. The
visible `<img>` src is then swapped to the prefetched URL after a 250 ms
dwell. That gives 20 swaps per launch, 3 launches, n = 60. "Paint" is the
Element Timing `renderTime` of the swapped image minus the swap time, which
is the frame that actually showed it.

| image | rAF median | rAF p90 | rAF max | 2nd rAF median | paint median | paint p90 |
|---|---|---|---|---|---|---|
| photo-screen (2560x1440) | **0.5** | 0.7 | 8.3 | 9.9 | **31.8** | 43.5 |
| photo-24mp (6000x4000) | **0.5** | 0.6 | 0.7 | 10.7 | **102.5** | 221.0 |

Per-launch paint medians: screen 32.0, 31.3 and 31.9; 24MP 103.5, 102.5 and
100.8. The prefetch had always finished before the swap: the median wait was
0.2 ms.

### 4. Idle memory, 10 s after showing one image (7 processes)

| first image | Σ working set | Σ private working set |
|---|---|---|
| photo-screen, 5 launches | median **349.2 MiB** (max 351.3) | median **84.6 MiB** (max 86.9) |
| photo-24mp, 5 launches | median **436.1 MiB** (max 438.3) | median **98.8 MiB** (max 98.9) |
| photo-24mp, fresh profile, 3 launches | 445.9 MiB | 103.4 MiB |

The per-process split, from the last launch, is shown as WS / private WS in
MiB:

| process | screen image | 24MP image |
|---|---|---|
| transport-spike.exe | 23.9 / 4.8 | 28.1 / 9.1 |
| WebView2 browser | 106.8 / 25.5 | 109.3 / 25.9 |
| GPU process | 75.8 / 31.8 | 85.6 / 41.5 |
| network utility | 37.4 / 7.4 | 37.5 / 7.4 |
| storage utility | 21.2 / 2.7 | 21.1 / 2.6 |
| renderer | 72.3 / 10.8 | 145.0 / 10.9 |
| crashpad | 11.7 / 1.4 | 11.7 / 1.4 |

The process roles come from each process's `--type=` command line, checked
on a separate launch.

## Verdict per budget

| budget | measured | verdict |
|---|---|---|
| Prefetched next ≤ 30 ms, metric as written (swap → next rAF) | median 0.5 ms, max 8.3 ms (screen and 24MP) | **PASS** |
| Prefetched next ≤ 30 ms, judged by when the image is on screen (Element Timing) | screen-size: median 31.8 ms, p90 43.5 ms. 24MP: median 102.5 ms, p90 221.0 ms | screen-size: **borderline FAIL** (1.8 ms over at the median). 24MP passthrough: **FAIL** |
| Idle memory ≤ 150 MB (143 MiB), metric as written (Σ working set) | 349 to 446 MiB | **FAIL** |
| Idle memory ≤ 150 MB, Task Manager metric (Σ private WS) | 85 to 103 MiB | **PASS** |
| Cold start (proposed, see below) | daily: decode() 891 ms and paint 1011 ms (screen-size image) | proposal |

**Proposed cold-start budget** (daily start, on this reference machine):
process start → first image painted **≤ 1.2 s**, and first `decode()`
**≤ 1.0 s**, for a screen-size image. The measured values are 1011 ms (max
1033) and 891 ms (max 928). About 645 ms of that is WebView2 creation,
which we do not control. The ~250 ms after it is our share: request,
decode, paint. Re-check the budget after a reboot (see "Could not verify").

## Inference (not measured)

- **The 2022 result (custom protocol 175 s vs localhost 1 s) does not hold
  for Tauri 2 / WebView2 156 at that scale.** It does hold in direction.
  The custom protocol transfers 30 MB 6.7× slower (623 vs 93 ms) and 3 MB
  4.6× slower. End to end with decode it is about 2× slower. Likely cause:
  the whole body is buffered and then copied into a COM `IStream` that the
  browser process reads, at about 48 MB/s, while axum streams over loopback.
  The axum route wins on both reasons the plan gives: shared HTTP semantics
  and speed.
- **The rAF metric is too weak to decide the gate.** rAF fires about 0.5 ms
  after the swap even though the display runs at 59 Hz, where a random swap
  should wait 8 ms on average. This fits Chromium sending a "missed"
  BeginFrame immediately when the page goes from idle to wanting a frame. So
  rAF measures "main thread ready", not "pixels visible". The Element Timing
  number includes the vsync wait (~8 ms average at 59 Hz) and very probably
  one DWM composition frame (~17 ms). A native Direct2D swap chain that goes
  through DWM pays the same ~25 ms. So the screen-size result of 31.8 ms is
  about 7 ms of real work above a floor that native shares, unless native
  uses independent flip.
- **24MP passthrough cannot meet any 30 ms budget, even prefetched.**
  `decode()` on a detached `new Image()` does not make the visible swap
  cheap: it still costs 100 to 220 ms to paint, consistent with
  uploading/rescaling a 96 MB bitmap at raster time. The API's rule that
  images above 1.5× the screen's pixels are resized server-side is
  therefore necessary, not optional. The screen-size numbers are the ones
  that matter for the app.
- **Cold start is bounded by WebView2, and the parallel start works.**
  axum listens at about 27 ms and the image is in memory at about 35 ms,
  roughly 600 ms before the webview can ask for it. The page receives the
  image 15 to 25 ms after navigation starts. A server-side 24MP → screen
  resize (an estimated 150 to 300 ms, not measured here) fits inside that
  600 ms window, so with the app's resize rule a 24MP file should cold-start
  like the screen-size row rather than the 24MP row.
- **On memory, neither sum is the "true" footprint.** Σ WS counts the shared
  WebView2 DLL pages once per process (6 processes map `msedge.dll`), so it
  overstates the footprint. Σ private WS understates it: the renderer's WS
  grows by 73 MiB with the 24MP image while its private WS does not move, so
  the decoded bitmap lives in shared memory. The honest number lies between
  the two, closer to private WS plus the image's decoded size. GPU VRAM is
  not included in either. The orchestrator should pick the metric before
  G0. With Task Manager's metric, the budget passes with ~45 MiB of headroom.
- The 491 ms spawn delay on the first launch of a new exe is most likely a
  Defender scan. Users see it once per install or update.

## Could not verify

- **First launch after reboot.** Not measured: rebooting is outside what this
  agent can do. All cold-start rows are daily cold starts with a warm OS file
  cache, including the WebView2 runtime DLLs. The fresh-profile variant
  simulates first-launch-after-install, not after-reboot. To measure it,
  reboot and run `node run.mjs cold 1 photo-screen` first thing.
- Whether native Direct2D would beat 31.8 ms. That needs a native spike,
  and the DWM-floor argument above is an inference.
- Whether Element Timing `renderTime` is the true presentation time or a
  paint time with coarsening. It is used as the best in-page proxy available.
- GPU memory (dedicated/shared VRAM) of the GPU process.
- Other machines: there was only one, and it is an old high-core Xeon with a
  fast GPU. Single-thread speed is lower than a current laptop.
- The window is 1600x1000 logical (2400x1500 physical) on a 1440-pixel-high
  screen, so its bottom edge is off-screen. It is always-on-top so that
  WebView2 occlusion tracking cannot pause rAF. The off-screen edge did not
  affect the numbers, as far as the per-launch spread shows.

## Reproduce

Run from `spikes/transport/` (Windows, Node ≥ 20, Rust 1.92 MSVC). Leave the
machine idle and the window visible while the driver runs. The app exits by
itself after each launch.

```
cargo build --release                           # ~4 min from clean (thin LTO)
node run.mjs gen                                # samples/*.jpg
node run.mjs machine                            # results/machine.json
node run.mjs bench 3                            # results/bench-{1..3}.json, ~50 s each
node run.mjs cold 5 photo-24mp                  # results/cold-photo-24mp-*.json
node run.mjs cold 5 photo-screen                # results/cold-photo-screen-*.json
node run.mjs cold 3 photo-24mp --fresh-profile  # deletes the spike's WebView2 profile first
node summarize.mjs                              # the tables above
```

`node make-icon.mjs` regenerates the placeholder `icons/icon.ico`, which
tauri-build requires on Windows. The output is committed.

## Pitfalls hit

- `ReaderStream`'s default 4 KiB chunk would send about 7,500 chunks for
  30 MB, so the spike uses 256 KiB chunks.
- `Win32_PerfFormattedData_PerfProc_Process` sometimes answered from a
  process list that predated the just-launched tree, so every private-WS
  lookup missed on the first trial. The driver now re-queries until every
  pid is present.
- WebView2 stops rAF in occluded windows, so the bench window is
  always-on-top.
- Without the `custom-protocol` feature on `tauri`, a `cargo build --release`
  is still a Tauri "dev" build. The spike enables it, as the CLI does for
  production builds.
