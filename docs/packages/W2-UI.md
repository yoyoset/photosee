# W2-UI: controller and boot (web, Wave 2)

Read `AGENTS.md` first. Owned paths: see `[W2-UI]` in `docs/ownership.toml`.
Every Wave 1 module is merged and tested in isolation. This package wires
them into a working viewer. Read each module's **header comment and exports
only**; do not re-read their tests or the Wave 1 briefs.

## Module handles you wire (all in `web/src/`)
| module | what you call |
|---|---|
| `viewer.ts` | `mountViewer(stage, ctx, { onShown(index), onFailed(index, err) })` → `{ element, geometry(), dispose() }` |
| `zoom.ts` | `zoomAt`, `zoomStep`, `toggleZoom`, `zoomActual`, `panBy`, `FIT`. Each takes `(store.zoom, viewer.geometry(), …)` and returns the next zoom state |
| `overlay.ts` | `mountOverlay(#overlay, ctx)` (hosts the filmstrip) |
| `info.ts`, `settings.ts`, `help.ts` | `mount*(root, ctx)` → `PanelHandle {open, close, toggle, isOpen, dispose}`. Opening one closes the others |
| `slideshow-ui.ts` | `mountSlideshow(ctx, { loadNextFolder })` → handle with `toggle/start/stop/imageShown/imageFailed/folderLoaded(count, name)/noNextFolder` |
| `keys.ts` / `gestures.ts` | `bindKeys(window, platform, dispatch)`, `bindGestures(stage, ctx)` |
| `errors.ts` | `showNasBanner(host, dir, retry, reason)`, `showToast`, `showEmptyFolder`, `showLastHint` |
| `state.ts` | `Store`, `EscStack` (layers: panel → slideshow → zoom → fullscreen → window) |

## Implement
1. `controller.ts`: `createDispatch(getCtx)`, the `Action` → state mapping.
   Split it into `controller-*.ts` files by concern: navigation, zoom and
   panels.
   - `next`/`prev`/`first`/`last`/`goto`:
     - Clamp at the ends; there is no wrap. At the last image, show
       `showLastHint`.
     - Every page turn resets zoom to `FIT` and rotation to 0
       (`docs/ux-spec.md`).
   - `next_folder`/`prev_folder`:
     - Call `api.nav(dir, root, d)`, then `api.list`, then go to index 0
       and toast the folder name.
     - `null` → `hint.no_next_folder` toast.
     - Do not switch folders while the slideshow is *paused*. B's state
       machine drops `folder_loaded` in that phase.
   - Zoom actions: compute from `viewer.geometry()`, or ignore the action
     when it is null.
   - `rotate`: quarter turns, 0..3.
   - `toggle_info` / `toggle_help` / `open_settings`: the panel handles.
   - `toggle_slideshow`: the slideshow handle.
   - `scrub` → `store.scrubbing`.
   - `escape` → `esc.handle()`. Register the `zoom` layer (not fit → fit),
     the `fullscreen` layer (exit) and the `window` layer (`host.close()`).
2. `Host` interface (in your files): `setFullscreen(on)`, `isFullscreen()`,
   `close()`.
   - The browser implementation uses the Fullscreen API.
   - The desktop shell (`app/`, later) provides a Tauri-backed one through
     `window.__PHOTOSEE_HOST__` when present.
3. `main.ts` boot:
   - `api.session()` and `api.config()` in parallel.
   - Mount everything.
   - Start:
     - `session.start` with a file: show that file **before** `/list`
       returns. Put a one-item provisional listing in the store, then
       replace it with the real listing at the file's index.
     - With only a dir: list it, index 0.
     - Neither: an empty state.
   - `start_fullscreen` → `host.setFullscreen(true)`. In browser mode
     (`session.mode === 'server'`) show `hint.browser_fullscreen` once
     instead.
   - Errors:
     - `timeout` / `io` on `/list` → NAS banner with a retry.
     - An empty folder → `showEmptyFolder`.
4. Wire viewer → slideshow: `onShown` → `imageShown`, `onFailed` →
   `imageFailed`. The slideshow's `loadNextFolder` → the controller's
   next-folder path, which then calls `folderLoaded(count, name)` or
   `noNextFolder()`.

## Tests (`web/test/controller*.test.ts`, jsdom, fake `Api` from `contracts/*.json`)
- Page-turn clamping and the last-image hint.
- Zoom and rotation reset on a page turn.
- Next folder: nav + list → index 0 + toast; null → hint; blocked while
  paused.
- Esc order: panel open → closes the panel; then zoomed → fit; then
  fullscreen → exit; then `host.close()`.
- Boot:
  - a start file paints before `/list` resolves;
  - list failure → banner;
  - an empty folder → notice.
- Slideshow end with `next_folder` loads the next folder and continues.

## Acceptance
`powershell -File scripts/check.ps1 -Web` and
`node scripts/gates.mjs --ownership W2-UI main` pass. The bundle stays
≤ 60 KB gzip; report the number.

## Not in scope
- `app/` (the Tauri shell).
- Editing Wave 1 modules: if one needs a change, list it in your report
  with the exact reason.
