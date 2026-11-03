# J: overlay, filmstrip, info panel (web)

Read `AGENTS.md` first. Owned paths: see `[J]` in `docs/ownership.toml`.

## Why
The overlay is the only chrome the viewer has. It must get out of the way
completely (Picasa's black stage) and come back the moment the user reaches
for it. Every number is in `constants.ts`, and the reasons are in
`docs/ux-spec.md` §Overlay.

## Implement
1. `overlay.ts` `mountOverlay(root, ctx)`:
   - Title line, top left: `name · 23 / 412 · folder`, from
     `ctx.store.current()` and `listing`. Long names are ellipsized in the
     middle so the extension stays visible. Do it in CSS where possible,
     otherwise with a pure helper.
   - Bottom bar holds the filmstrip and buttons in this order:
     prev, play/pause, next, rotate ccw, rotate cw, fit/100%, fullscreen,
     settings.
     - Each button dispatches one `Action`.
     - Each has an `aria-label` from `t()` (`i18n.ts` keys `btn.*`) and a
       `title` of `label (shortcut)`.
     - Shortcuts depend on the platform: `ctx.session.platform === 'macos'`
       uses ⌘ where others use Ctrl. Use inline SVG icons, no icon font.
   - Auto-hide:
     - Sets `store.overlayVisible`.
     - Hides after `OVERLAY_HIDE_MS` without input, fading over
       `OVERLAY_FADE_MS`.
     - Never hides while hovered or while focus is inside it.
     - Any pointer move, or entering the bottom `OVERLAY_REVEAL_ZONE_PX`,
       shows it.
     - The cursor hides in fullscreen after `CURSOR_HIDE_MS`.
     - Put the timer logic in a pure, time-injected class and unit-test it.
   - `#live` region (already in `index.html`): after each index change, write
     `t('live.position', {n, total, name})`.
   - Touch: buttons at least `TOUCH_TARGET_PX`. Pad with
     `env(safe-area-inset-*)`.
   - Register on `ctx.esc` nothing; the overlay is not an Esc layer.
2. `filmstrip.ts`:
   - `visibleRange` (pure).
   - A mount function you define, used by `mountOverlay`.
   - Virtualized: only visible thumbs plus overscan exist in the DOM. Thumbs
     are `<img loading="lazy" decoding="async">` from
     `ctx.src.thumbUrl(path, THUMB_EDGE)`, `FILMSTRIP_THUMB_PX` tall.
   - The current item is centered and highlighted.
   - Click dispatches `goto`. Thumbnails of animated images are static,
     which the server guarantees.
   - Must stay smooth with 10 000 items. Test the DOM node count.
3. `info.ts` `mountInfo(root, ctx)` (key I, via `toggle_info`; the
   controller decides visibility):
   - A panel with name, dimensions, file size, and the EXIF summary from
     `ctx.api.meta(path)`. Use `info.*` keys.
   - Omit missing fields; show no empty rows.
   - Registers an `esc` layer `panel` while open.
   - Fetch only while visible. Cancel or ignore responses for a previous
     image.
4. `overlay.css`, imported from `overlay.ts`:
   - Bars use a dark gradient behind the controls only. The stage stays
     pure black; never a translucent full-window layer.
   - System font stack, 13–14 px, high-contrast focus ring
     (`:focus-visible`).
   - No animation under `prefers-reduced-motion`.

## Tests (vitest + jsdom)
- Hide-timer class:
  - hides after the timeout;
  - hover or focus blocks hiding;
  - the reveal zone shows it again.
- Title formatting and middle ellipsis helper.
- `visibleRange` edges: start, end, `count` < viewport, overscan.
- Mounting with 10 000 items creates fewer than 100 thumb nodes.
- Buttons dispatch the right actions and carry `aria-label`s, on both
  `windows` and `macos`.
- The live region text after an index change.
- Info panel:
  - drops missing EXIF fields;
  - ignores a stale `meta` response.

## Acceptance
`powershell -File scripts/check.ps1 -Web` and
`node scripts/gates.mjs --ownership J main` pass. Report the gzip delta.

## Not in scope
- Settings and help panels, error cards, slideshow timer (L).
- Key and gesture handling (K).
- Image display (I).
