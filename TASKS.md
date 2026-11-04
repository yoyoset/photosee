# TASKS (orchestrator-owned)

Status: `todo` · `running` · `review` · `merged` · `blocked`

## C0 contracts
| step | status | note |
|---|---|---|
| MSVC toolchain | merged | No install needed: rustc and cc-rs locate VS 18 Insiders via vswhere from both PowerShell and Git Bash (probe crate compiled C and linked). The plan assumed `cl` missing from PATH meant Build Tools were required; that was wrong. |
| repo, contracts, skeleton, gates | merged | `scripts/check.ps1` green. Gates verified to fail on an undocumented dependency and on a 306-line file. |

## M0 spike (gate G0)
| WP | owner | status | branch |
|---|---|---|---|
| M0-T transport + cold start | Claude sub-agent | merged 73b807c; axum 2x faster than custom protocol; cold start ~1.0s (WebView2 645ms) -> G0 question to user | wp/m0-t |
| M0-R RAW preview extractor | Claude sub-agent | merged df56390; 38 tests, synthetic only (no real samples yet) | wp/m0-r |
| M0-S SMB bench tool | sonnet (stand-in for progate) | merged cfc8e06, awaiting NAS run | wp/m0-s |
| M0-F fixture generator | sonnet (stand-in for flashgate) | merged; orientations re-verified with GDI+ | wp/m0-f |

Blocked on the user: NAS share path (to run M0-S), private RAW/HEIC samples
(for M0-R's real-file test).

## G0 decision (2026-11-04)
**Resident mode.** The user was offered resident mode vs a native Windows
shell and replied "continue" without choosing, so the recommended option
stands. Closing the window hides it; a second launch hands its argument
to the running instance. That leaves ~1 s for the first open of the day
and near instant afterwards, at ~90 MB private memory while resident. It
only shapes `app/` (Wave 2); Wave 1 is unaffected. Revisit if the first
open after a reboot turns out much worse than 1 s.

## M1 Wave 1
Batch 1 runs now. Batch 2 (F thumbnails, G routes) starts once A and C are
merged, because both call into them. T (test top-up) runs last.

| WP | owner | status | branch |
|---|---|---|---|
| A scan/sort/nav | sonnet | merged 1af9ab7; 23 tests; next from a leaf = 3 read_dir, flat in sibling count | wp/a |
| B slideshow.ts | sonnet | merged 50635fc; 49 tests. A folder_loaded that arrives while paused is dropped, so the controller must not switch folders while paused | wp/b |
| C std decode + pipeline | Claude (opus) | merged 6dd8442; correct, but 24 MP end to end is 681 ms (budget 150), see C2 | wp/c |
| D jxl/psd/svg | sonnet | merged 8d022b8; jxl-oxide applies orientation itself (measured) | wp/d |
| E HEIC via WIC | Claude (opus) | merged b540b34; WIC applies irot/imir (measured); extension leaks ~1 handle per CopyPixels (not ours) | wp/e |
| F thumb/cache/meta | sonnet | merged f80e2bd; 30 tests; thumbnail with IFD1 2 ms / 131 KB read, without it 388 ms (C2) | wp/f |
| G routes/queue/range/cache | sonnet | running | wp/g |
| H paths/guard | orchestrator | merged 1d4e0c0; 12 tests incl. a real junction escape | wp/h |
| I viewer/zoom/prefetch | Claude (opus) | merged 68186af; 220 web tests; drag feedback and error-card removal applied | wp/i |
| J overlay/filmstrip/info | sonnet | merged 3cf30d2; 49 tests; overlay z-index 10; info is a PanelHandle on L's Modal | wp/j |
| K keys/gestures | sonnet | merged 7e671b5 + review fix; 59 tests; edge clicks changed to immediate (brief had them wait 300 ms); drag feedback via --drag-x/--drag-y/--pull | wp/k |
| L settings/help/slideshow-ui/errors | sonnet | merged 645da34; 43 tests against the real step(); handles: panel {open,close,toggle}, slideshow {imageShown,imageFailed,folderLoaded,noNextFolder}, fade class ps-fade on #stage | wp/l |

### C2: transcode speed (open, measured 2026-11-04)
Budget for a non-prefetched 24 MP JPEG to 2560x1440 is ≤150 ms. Measured, release build,
6000x4000 q92: decode 411 ms, resize 86 ms (CatmullRom), encode 170 ms, total 681 ms.
On loopback, passing the file through costs 102 ms to paint (M0), so for big JPEGs
transcoding is currently *slower* than passthrough. Options to measure, cheapest first:
- WIC JPEG decode with DCT scaling (`IWICBitmapSourceTransform`, SIMD libjpeg-class),
  Windows only;
- the `jpeg-decoder` crate's DCT `scale()` to the smallest size ≥ target;
- the `jpeg-encoder` crate (SIMD) instead of image's encoder;
- meanwhile, prefer passthrough for native JPEG on desktop loopback (a delivery rule
  change, so it goes through the contract).

Execution mode since 2026-11-04 (user: parallel Claude agents cost too many tokens):
one sonnet worker at a time, and the orchestrator commits finished work itself.
Pi/DeepSeek through the OpenCode Go subscription takes priority once
OPENCODE_KEY_YOYOSET / OPENCODE_KEY_YOYOSET2 are set.

Pi workers (progate/flashgate) are still unavailable (no OPENCODE key in
the environment), so sonnet sub-agents stand in.

## Decisions changed during C0 (vs. the approved plan)
- **Slideshow state machine moved from `core/` to `web/src/slideshow.ts`.**
  The timer runs in the UI, and a round trip per tick buys nothing, so the
  pure state machine lives in TS (package B, still pure and tested). Folder
  resolution stays in core (`/nav`).
- **`/list` returns one JSON document, not NDJSON.** Natural sort needs every
  entry before the first line can be emitted, so streaming would buy nothing.
  The first image never waits for the listing anyway.
- **`i18n.ts` became a contract file** (it was package L's). Every UI package
  needs the same keys, and one table avoids four packages inventing
  strings. Package L got `help.ts` and package J got `info.ts`; the plan had
  left the help and info panels unassigned.
- **AVIF is passthrough-only.** `image`'s `avif-native` needs a dav1d C build.
  It is recorded in docs/non-goals.md.
