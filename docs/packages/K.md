# K: keyboard map, wheel, mouse and touch gestures (web)

Read `AGENTS.md` first. Owned paths: see `[K]` in `docs/ownership.toml`.

## Why
Input decides whether the viewer feels precise or sloppy. A trackpad that
skips three pages, a held arrow key that leaves the wrong image on screen
(ImageGlass #1754), or a pinch that zooms the whole webview are each enough
to make users give up on a viewer. Input modules only *emit* `Action`s
(`actions.ts`); the controller (Wave 2) applies them.

## Implement
1. `keys.ts`
   - `keyAction(e, platform)` is pure. It implements the `docs/ux-spec.md`
     §Keys table exactly, including:
     - macOS: Cmd+arrows for folder navigation, Ctrl+Cmd+F for fullscreen.
     - Modifiers that are not in the table → `null`. For example, Ctrl+R
       must not rotate, and Alt+← stays browser back.
     - `+` arrives as `=` with Shift on most layouts; check `e.key`, not
       `code`.
     - Esc → `escape`.
   - `bindKeys(target, platform, dispatch)`:
     - Calls `preventDefault` only for handled keys.
     - Ignores events from `input`, `select`, `textarea` and
       contenteditable elements, so the settings panel still works.
     - Auto-repeat of ← / → (`e.repeat`) emits `scrub {active:true}` once
       and the page turns as usual. `keyup`, or no repeat for
       `KEY_REPEAT_SETTLE_MS`, emits `scrub {active:false}`.
2. `gestures.ts`
   - `WheelAccumulator.feed(deltaY, now)` is pure:
     - Accumulate until |sum| ≥ `WHEEL_PAGE_DELTA`, emit ±1, reset, then
       ignore everything for `WHEEL_COOLDOWN_MS`.
     - A direction change resets the sum.
     - A mouse wheel notch (100+) turns exactly one page.
   - `bindGestures(el, ctx)`:
     - Wheel:
       - When `ctx.store.get().settings?.wheel === 'zoom'`, or with Ctrl
         (pinch), emit `zoom_by` with the cursor anchor and factor
         `ZOOM_STEP` per notch. This listener needs `passive: false` and
         `preventDefault`, or the webview zooms itself.
       - When zoomed (mode ≠ fit), the wheel pans instead of turning
         pages.
     - Mouse:
       - Double-click → `zoom_toggle` with an anchor.
       - Drag when zoomed → `pan_by`.
       - A click in the outer `EDGE_CLICK_FRACTION` turns the page, but
         only in fit mode, and must not fire for the first click of a
         double-click.
     - Touch, via Pointer Events and `touch-action: none` on the stage:
       - Horizontal swipe, past `SWIPE_PAGE_FRACTION` × width or faster
         than `SWIPE_FLICK_PX_PER_MS` → `next`/`prev`.
       - Pull down past `PULL_CLOSE_PX` or `PULL_CLOSE_PX_PER_MS` →
         `escape`.
       - Both only in fit mode. When zoomed, a drag pans.
       - Single tap toggles the overlay after `DOUBLE_TAP_MS`.
       - Double tap → `zoom_toggle`.
       - Pinch → `zoom_by` around the midpoint.
     - Implement the swipe/pull/tap classification as a pure function
       over a list of pointer samples with timestamps, and test that
       function. Keep the DOM binding thin.
     - Live feedback during a drag (the image following the finger,
       background fading on pull):
       - Expose the in-progress offset through a callback or CSS custom
         property on `el`, e.g. `--drag-x` / `--pull`.
       - Document which one you chose.
       - The viewer (package I) applies it later; you do not edit viewer
         files.
     - Overlay toggling: a tap only sets `ctx.store.set({ overlayVisible:
       !visible })`. Nothing else in the store is touched directly.

## Tests (vitest)
- The full key table on `windows` and `macos`, with negative cases
  (unhandled modifiers, typing in an input).
- Wheel:
  - trackpad stream (30 events of 8);
  - mouse notches;
  - direction change;
  - cooldown.
- Gesture classifier:
  - swipe by distance and by velocity;
  - pull close;
  - tap vs double tap;
  - pinch factor and midpoint;
  - nothing in zoomed mode.
- Scrub on / off sequence with fake timers.

## Acceptance
`powershell -File scripts/check.ps1 -Web` and
`node scripts/gates.mjs --ownership K main` pass. Report the gzip delta.

## Not in scope
- Applying actions to state (controller, Wave 2).
- Image transforms (I).
- Android Back (M4).
