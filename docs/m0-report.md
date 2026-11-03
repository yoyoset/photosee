# M0 report and G0 decision

Raw data and methods are in `spikes/transport/REPORT.md`, which separates
measured numbers from inferences. To reproduce the tables, run
`node spikes/transport/summarize.mjs`; the orchestrator re-ran it and the
numbers match. Machine: Xeon E5-2698 v3, RTX 3060, 2560×1440 at 59 Hz,
Windows 11 22631, WebView2 156.

## Transport: keep the embedded axum server ✅ (measured)
| 3 MB / 30 MB, `img.src` → `decode()` median | axum 127.0.0.1 | Tauri async custom protocol |
|---|---|---|
| end to end | 76 / 580 ms | 139 / 1141 ms |
| transfer only | 17 / 93 ms | 80 / 623 ms |

The custom protocol is about 2× slower end to end and up to 6.7× slower in
transfer. That is far less dramatic than the 2022 v1 report (175 s vs 1 s),
but it still loses, so the plan's choice stands. We initially assumed the
old 175× gap still applied; measurement shows 2–7×.

## Budgets revised to what users actually see
| budget | old definition | new definition | measured | status |
|---|---|---|---|---|
| prefetched next | swap → next rAF ≤30 ms | swap → browser paint timestamp ≤ 2 frames (≤35 ms at 60 Hz) for a server-resized image | 31.8 ms median, 43.5 p90 | ✅ |
| same, 24 MP passthrough | not specified | must not happen | 102.5 ms median, 221 p90 | **server-side resize is mandatory**; the 1.5× passthrough rule stands |
| idle memory | summed working set ≤150 MB | summed **private** working set ≤150 MB (Task Manager's figure) | 85–99 MiB | ✅ |
| daily cold start → first paint | to be set in M0 | ≤1.2 s for a screen-size image | 1011 ms | ✅ but see below |

Why the first definition was wrong: rAF fires about 0.5 ms after the swap,
before any pixels are composited, so it measured nothing. The summed working
set counts shared WebView2 DLLs once for each of the 7 processes.

## The open issue: cold start is about 1 s, and 645 ms of it is WebView2
Cold-start timeline, in ms since process creation (median of 5 daily launches):

| step | time |
|---|---|
| axum listening | 27 |
| image file read | 35 |
| WebView2 created | 645 |
| first `decode()` | 891 (screen-size image) |
| first paint | 1011 (screen-size image) |

Our own work (server and file read) finishes in under 40 ms. The rest is
WebView2 booting, which we cannot speed up. A native Direct2D viewer would
typically show the first frame in about 150–250 ms. That is an inference,
not measured here.

G0 decision, 2026-11-04: **resident mode**. This was the recommended option;
the user said to continue without choosing. See TASKS.md.

## Not yet measured
- First launch after a reboot, which needs a reboot: `node run.mjs cold 1 photo-screen`.
- SMB numbers. `tools/bench-smb` is ready, but the NAS path is still needed.
- RAW extraction on real files. The extractor has only been tested on
  synthetic containers; the user's own samples are needed.
