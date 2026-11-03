# I: viewer, zoom geometry, prefetch (web)

Read `AGENTS.md` first. Owned paths: see `[I]` in `docs/ownership.toml`.
This is the part of the product the user looks at all the time. "Fast" is
judged here.

## Measured context (`docs/m0-report.md`, `spikes/transport/REPORT.md`)
- A prefetched, server-resized image paints 31.8 ms after the swap; a 24 MP
  passthrough takes 102 ms. So always request the screen box. Never request
  the original unless the user zooms past what was loaded.
- Read how `spikes/transport/` swapped a prefetched image (decoded `Image`
  object vs `src` change) and reuse the faster method. Write the reason in
  a comment.

## Units (use these everywhere)
- `ViewState.zoom.scale` = device pixels per image pixel. `1` = 100% (one
  image pixel per device pixel). A CSS transform therefore uses
  `scale / devicePixelRatio`.
- `zoom.x`, `zoom.y` = pan offset of the image center from the view center,
  in CSS px.
- Image size = the **display** size (after EXIF orientation), then swapped
  for odd `rotation` quarter turns.

## Implement
1. `zoom.ts` (pure, no DOM):
   - `fitScale(img, view, dpr, upscale)`: `view` in CSS px.
     - Without `upscale`, cap at 1. Small images show at 1:1 device pixels.
     - Panorama (`isPanorama`: aspect > `PANORAMA_ASPECT` either way) fits
       the short axis instead.
   - `requestBox(img, view, dpr)`: the device-pixel box to ask `/img` for.
     - It is `ceil(view × dpr)`. For a panorama, the long axis is scaled up
       so that the short axis fits.
     - Before the image size is known, `view × dpr` is correct. The server
       fits inside the box and never upscales, so the first request never
       waits for `/meta`.
   - Add the pure helpers the controller (Wave 2) will call. Choose the names
     yourself and export them:
     - zoom at an anchor point, keeping the image point under the cursor
       fixed. Use the step `ZOOM_STEP`, clamp to [fit, `ZOOM_MAX`].
     - clamp the pan, so the image cannot be dragged fully off screen. A
       side smaller than the view is centered.
     - toggle fit ↔ 100%. Here 100% means scale 1, centered on the anchor.
2. `prefetch.ts`:
   - `prefetchWindow`: `PREFETCH_AHEAD` / `PREFETCH_BEHIND` from
     `constants.ts`. Nearest first, ahead before behind, clipped to
     `[0, count)`, never including `index`.
   - `ImageLru`: keyed by URL. `bytes` ≈ w×h×4. Evict least recently used
     entries beyond `budgetBytes`. An entry larger than the whole budget is
     not cached.
3. `viewer.ts` `mountViewer(stage, ctx)`, subscribed to `ctx.store`:
   - One `<img>` on a black stage, positioned with a CSS `transform`. No
     canvas, no WebGL.
   - On index change:
     - If the LRU has the URL, swap it in.
     - Otherwise show the filmstrip thumbnail (`ctx.src.thumbUrl(path,
       THUMB_EDGE)`) scaled to fit, then the full image once it has
       decoded.
     - A stale response for a previous index must never replace the current
       one. Test this ordering.
     - Then prefetch the window with `prio=prefetch`.
   - `scrubbing` (arrow key held): show thumbnails only. Request the full
     image once `scrubbing` turns false.
   - Loading bar: a thin bar at the top.
     - It shows only after `LOADING_DELAY_MS`, then stays at least
       `LOADING_MIN_VISIBLE_MS`.
     - Put that timing in a pure function with injected time, and test it.
   - DPR change (`matchMedia('(resolution: Xdppx)')`) or stage resize
     (`ResizeObserver`): re-request when the needed box grows. Never request
     again for a shrink.
   - Zoom past the loaded resolution: load the original once (no box).
     Keep showing the scaled one until it has decoded.
   - Failure:
     - An `<img>` error carries no body. Re-fetch that URL once with
       `fetch` to read the `{code, codec, message}` JSON.
     - Then call `renderErrorCard` from `errors.ts` (package L; a stub until
       L merges).
     - The card **replaces** the thumbnail; nothing blurred may stay on
       screen.
     - Tell the store that the image failed, via a callback option or a
       custom DOM event on the stage. Pick one and document it; the
       slideshow skips failed images.
   - Respect `prefers-reduced-motion`. The optional slideshow fade is
     package L's concern, so the viewer only exposes the element or a hook
     for it.

## Tests (vitest + jsdom; images never load in jsdom)
Keep the DOM part thin: inject an image loader (an interface returning a
promise) so viewer logic can be driven by tests. Cover:
- fit and panorama math;
- the zoom anchor (the point under the cursor stays put);
- pan clamping;
- the prefetch window at both ends;
- LRU eviction order and budget;
- stale-response ordering;
- the loading-bar timing;
- error replacing the thumbnail;
- re-request on DPR growth and not on shrink.

## Acceptance
`powershell -File scripts/check.ps1 -Web` and
`node scripts/gates.mjs --ownership I main` pass. The bundle stays under the
gate. Report the gzip delta.

## Not in scope
- Turning keys, wheel and touch into actions (K).
- The overlay and filmstrip (J).
- The controller that maps `Action` → store changes (Wave 2). Your helpers
  are what it will call.
