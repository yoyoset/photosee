# PhotoSee UX spec

The goal is the Picasa Photo Viewer feeling: black, instant, nothing in the
way. All numbers live in `web/src/constants.ts`; this file explains them.

## Look
- Background pure black. Translucent backgrounds are a non-goal: WebView2
  loses its GPU fast path with them, and transparent Tauri regions swallow
  clicks.
- Small images are not upscaled by default. They show at 1 image pixel = 1
  *device* pixel.
- Panoramas (aspect > `PANORAMA_ASPECT` = 3, either direction) fit the short
  axis. The wheel then scrolls along the long axis.
- Image changes are instant. The slideshow can optionally fade
  (`SLIDESHOW_FADE_MS`). Under `prefers-reduced-motion` nothing animates.
- Loading indicator: a thin top progress bar. It appears only after
  `LOADING_DELAY_MS` (450) and then stays at least `LOADING_MIN_VISIBLE_MS`
  (500).
  - Reason: Nielsen's limits. Under ~0.1s needs no feedback, and an indicator
    that flashes briefly is worse than none. On SMB, 150ms flickered
    constantly.
  - Never a spinner covering the image.

## Overlay
- Fades after `OVERLAY_HIDE_MS` (2.8s) of no input. It never hides while
  hovered or while it holds keyboard focus.
- It reappears on any pointer move, or when the pointer enters the bottom
  `OVERLAY_REVEAL_ZONE_PX`.
- Top-left shows `file name · 23 / 412 · folder`.
- Bottom has the filmstrip (`FILMSTRIP_THUMB_PX` = 64, current item centered,
  virtualized, static thumbnails) and a row of buttons:
  - prev / slideshow (play-pause) / next / rotate ccw / rotate cw / fit-100% / fullscreen / settings.
- Every button has an `aria-label` and a tooltip with its shortcut.
- `#live` (aria-live) announces "23 / 412, name.jpg" after each image change.
- On touch devices, buttons are at least `TOUCH_TARGET_PX` (44) and the
  overlay respects `env(safe-area-inset-*)`.

## Keys
| Action | Windows / Linux | macOS |
|---|---|---|
| next / prev | → ← Space Backspace PgDn PgUp | same |
| first / last | Home End | same |
| next / prev folder | Ctrl+→ / Ctrl+←, `]` / `[` | Cmd+→ / Cmd+←, `]` / `[` |
| fullscreen | F, F11, Enter | F, Ctrl+Cmd+F, Enter |
| slideshow | S | S |
| rotate cw / ccw (display only) | R / Shift+R | same |
| fit / 100% | 0 / 1 | same |
| zoom in / out | + / − | same |
| info / help | I / ? | same |
| Esc | layered, see below | same |

Notes:
- macOS reserves Ctrl+arrows (Spaces) and F11 (show desktop), hence the Cmd
  bindings.
- Esc unwinds one layer per press: panel/help → stop slideshow → exit zoom →
  exit fullscreen → close window. Android Back follows the same order.
- **Holding an arrow key** sets `scrubbing`. While it is set, only thumbnails
  are shown; the full image for the landing index is decoded once keys have
  been quiet for `KEY_REPEAT_SETTLE_MS`. This avoids ImageGlass #1754, where
  the wrong image is left on screen after a held key.
- In the browser (Docker) build, Esc and F11 belong to the browser and
  fullscreen needs a user gesture. Show a one-time hint to that effect;
  "start fullscreen" has no effect there.

## Mouse and trackpad
- Wheel turns pages by default. Settings can switch it to zoom.
- Trackpad wheel streams are accumulated: one page turn per
  `WHEEL_PAGE_DELTA` (100), then `WHEEL_COOLDOWN_MS` (250).
- Ctrl+wheel and trackpad pinch (Chromium reports pinch as Ctrl+wheel) zoom
  around the cursor by `ZOOM_STEP` (1.25). This needs `passive: false` and
  `preventDefault`.
- Double-click toggles fit ↔ 100%. Drag pans when zoomed.
- Clicking the outer `EDGE_CLICK_FRACTION` (15%) of either side turns the
  page, but only in fit mode. Edge zones ignore double-click.
- Leaving 100% or custom zoom happens only by explicit action. Turning the
  page always resets to fit and resets display rotation (R) to 0: rotation
  is a look at this one image, not a mode.
- Double-click to 100% keeps the image point under the cursor where it is
  (like every zoom step), rather than moving it to the center.

## Touch
- Horizontal swipe turns the page when it covers `SWIPE_PAGE_FRACTION` (30%)
  of the width or moves faster than `SWIPE_FLICK_PX_PER_MS`. The image
  follows the finger.
- Pull down to close: past `PULL_CLOSE_PX` (100) or faster than
  `PULL_CLOSE_PX_PER_MS` (0.11). The background fades with the drag.
- Swipe and pull work only in fit mode. When zoomed, a drag pans first and
  turns the page only once the image edge is reached.
- Single tap toggles the overlay, after `DOUBLE_TAP_MS` (300) to rule out a
  double tap. Double tap zooms. Pinch zooms around the midpoint.

## Settings (six, no more)
1. Slideshow interval, 1–60 s (default 5)
2. At slideshow end: loop / next folder (default) / stop
3. Shuffle (within the current folder only)
4. Transition: none (default) / fade
5. Start fullscreen (default on)
6. Wheel: navigate (default) / zoom

Language follows the system (zh / en). There is no translucency setting, no
"include subfolders", and no configurable end folder: the folder the user
opened is the implicit root.

## Folders
"Next folder" (used by Ctrl+→ and by the slideshow end action):
- Order is pre-order depth-first over the opened root.
- Siblings go in natural order. When siblings run out, climb a level and
  continue.
- Folders without images are skipped.
- The traversal never leaves the root. The root itself is the first folder
  when it has images.
- Golden cases: `contracts/nav.json`.

Entering a folder shows a toast with the folder name for
`FOLDER_TOAST_MS` (1.5 s). At the last image there is no wrap-around by
default; a hint says "已是最后一张 · Ctrl+→ 下一个文件夹".

## Slideshow
- Plays from the current image. The visible pause control is the overlay
  button (WCAG 2.2.2).
- A failed image is skipped automatically and counts as shown.
- The end of the folder follows setting 2. "Next folder" loads it and
  continues from its first image. When no next folder exists, the slideshow
  stops and shows the end hint.
- Shuffle is a permutation of the current folder, drawn once at start.

## Errors (never "failed in the background, looks fine")
- **Broken image, or a codec that cannot be used**: a card replaces the image.
  It shows the file name, a localized sentence for `code`, an action, and a
  collapsible technical detail line.
  - Missing codec: the card says which one is missing. For `heif_ext` /
    `hevc_ext` it links to the Microsoft Store page.
- If the thumbnail was showing and the full image then fails, the card
  **replaces** the thumbnail. It never stays blurred.
- **NAS unreachable** (`timeout` / `io` on list or image): a top banner reads
  "无法访问 \\nas\photos（超时 5s）[重试]". Images already cached keep working.
- **Empty folder**: "此文件夹没有图片 · Ctrl+→ 下一个文件夹".

## Rendering details
- Request size is fit box × `devicePixelRatio`. When DPR changes (the window
  moves to another monitor), re-request the image.
- EXIF orientation is applied exactly once: by the webview for passthrough,
  by core for transcodes. Display rotation (R) is added on top, and the fit
  calculation swaps width and height for odd quarter turns.
- Animated GIF / WebP / AVIF are always passed through, so animation
  survives.
