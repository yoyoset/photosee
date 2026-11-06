# PhotoSee UX spec

The goal is the Picasa Photo Viewer feeling: black, instant, nothing in the
way. All numbers live in `web/src/constants.ts`; this file explains them.

## Look
- Background pure black. Translucent backgrounds are a non-goal: WebView2
  loses its GPU fast path with them, and transparent Tauri regions swallow
  clicks.
- "Fit" has three modes, kept per user in ui-state (`fit`): **whole image**
  (default), **width** and **height**. The fit button and W step through
  them, with a toast naming the new one.
  - Small images are enlarged to fit, in every mode. Until 2026-11-06 they
    stayed at 1:1; on a phone a small comic scan then filled "only a little
    bit" of the screen (user).
  - A fitted page bigger than the view (fit width on a tall page) opens at
    its top-left. A drag pans it, and so does the wheel, which turns the page
    only once that edge is reached. Taps on the sides still turn pages.
- Panoramas (aspect > `PANORAMA_ASPECT` = 3, either direction) fit the short
  axis in whole-image mode. The wheel then scrolls along the long axis.
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
  - prev / slideshow (play-pause) / next / rotate ccw / rotate cw / fit mode / fullscreen / settings.
- Top bar: back (left, Esc) and image info (right, I). Phones have no
  keyboard, so every viewer key with no gesture has a button; the help panel
  is reachable from settings ("操作说明") and lists the gestures too.
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
| fit mode: whole → width → height | W | same |
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
- Double-click toggles fit ↔ 100%. An image fitted above 100% (a small one
  enlarged) doubles instead. Drag pans when zoomed or when the fitted image
  is bigger than the view.
- Clicking the outer `EDGE_CLICK_FRACTION` (15%) of either side turns the
  page, but only in fit mode. Edge zones ignore double-click.
- Leaving 100% or custom zoom happens only by explicit action. Turning the
  page always resets to fit and resets display rotation (R) to 0: rotation
  is a look at this one image, not a mode.
- Double-click to 100% keeps the image point under the cursor where it is
  (like every zoom step), rather than moving it to the center.

## Touch
- A tap on the left or right `TAP_TURN_FRACTION` (30%) of the width turns
  the page at once (not while zoomed). There is no swipe page turn: on
  phones the sideways swipe from the edge is the system's Back, and pages
  kept turning by accident (user, 2026-11-06). A sideways drag does nothing.
- Pull down to close: past `PULL_CLOSE_PX` (100) or faster than
  `PULL_CLOSE_PX_PER_MS` (0.11). The background fades with the drag. Only
  when the image fits the view; otherwise a drag pans.
- A tap in the middle toggles the overlay, after `DOUBLE_TAP_MS` (300) to
  rule out a double tap. Double tap zooms. Pinch zooms around the midpoint.
- Browser: a tap on a file or folder opens it (a mouse click only selects).
  A second tap within `DOUBLE_TAP_MS` is ignored. Long-press opens the item
  menu. Tree rows and toolbar buttons are 36 px under `pointer: coarse`.
- Back (the phone's gesture, a mouse back button) in the folder view closes
  the drawer, else opens the previous folder, else the parent; only from the
  top does it leave PhotoSee (browser-back.ts). Before 2026-11-06 it left
  PhotoSee from any folder.
- Double tap zooms around the tapped spot. Long-press opens the item menu on
  iOS too (`LONG_PRESS_MS`, 500; iOS sends no `contextmenu`).
- Under `pointer: coarse` the details rows are 44 px, the Recent row × and
  "Clear" are always shown, and the settings controls are 22–44 px.
- A tap on the picture in the single view opens the viewer.
- Folder tiles show the folder's first page (`/cover`), with the folder icon
  as a corner badge; no cover keeps the plain icon.
- Phones on their side (height ≤ 480 px) drop the filmstrip from the viewer's
  bottom bar: the bars took 57% of a 375 px height.
- Browser build: "Add to Home Screen" opens PhotoSee without browser bars
  (manifest, `display: fullscreen`). Without a fullscreen API (iPhone Safari)
  the fullscreen button is hidden. During a slideshow the screen is kept on:
  Wake Lock on HTTPS, a muted 16×16 looping video on plain HTTP.
- Browser at or below `NARROW_MAX_PX` (640) wide: the tree is a drawer over
  the files, closed by default. Picking a folder or tapping the dimmed area
  closes it. Its state is per page, never the shared `tree.visible`.

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
`FOLDER_TOAST_MS` (1.5 s). At the last image there is no wrap-around. A
turn past it shows "已是最后一张 · 再往后翻一次进入下一个文件夹"; another
turn while that hint is up (`HINT_TOAST_MS`) opens the next folder. The same
holds backwards at the first image. Phones have no Ctrl+→, and "tap next
again" is what the user expected (2026-11-06). A held arrow key and the
slideshow never chain into the next folder this way.

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
- **Empty folder**: "此文件夹没有图片 · 往后翻进入下一个文件夹"; the first
  turn goes on.

## Rendering details
- Request size is fit box × `devicePixelRatio`. When DPR changes (the window
  moves to another monitor), re-request the image.
- EXIF orientation is applied exactly once: by the webview for passthrough,
  by core for transcodes. Display rotation (R) is added on top, and the fit
  calculation swaps width and height for odd quarter turns.
- Animated GIF / WebP / AVIF are always passed through, so animation
  survives.
