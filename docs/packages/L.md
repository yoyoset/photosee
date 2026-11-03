# L: settings, help, slideshow UI, error surfaces (web)

Read `AGENTS.md` first. Owned paths: see `[L]` in `docs/ownership.toml`.
All user-visible text comes from `web/src/i18n.ts` (`t(key, vars)`). That
file is a contract: if you need a new string, list it in `QUESTIONS.md`
instead of hard-coding it.

## Why
These are the surfaces where the product either explains itself or leaves
the user stuck. The rule behind all of them is "never fail in the
background while looking fine" (`docs/ux-spec.md` §Errors). Settings are
capped at six (§Settings). A seventh needs a non-goals discussion, not code.

## Implement
1. `settings.ts` `mountSettings(root, ctx)`:
   - A modal panel opened by the `open_settings` action. The controller
     toggles visibility; you expose open/close. While open, it registers
     `esc` layer `panel`.
   - Exactly the six settings, with native controls:
     - interval: number input with a range slider, 1–60;
     - `on_end`: radio buttons;
     - shuffle, `start_fullscreen`: checkboxes;
     - transition, wheel: radio buttons.
   - Saving and error handling:
     - Every change calls `ctx.api.saveConfig(full)` debounced by 300 ms,
       then writes the normalized result from the server into
       `store.settings`.
     - On failure, show the error inline next to the controls and keep the
       user's value visible.
   - Accessibility:
     - Keyboard accessible: focus is trapped inside while open and returns
       to the opener on close.
     - Controls have labels.
2. `help.ts` `mountHelp(root, ctx)`: the shortcut table for
   `ctx.session.platform`, matching `docs/ux-spec.md` §Keys (⌘ on macOS).
   Opened by `toggle_help`. Registers `panel` on `esc`.
3. `slideshow-ui.ts` `mountSlideshow(ctx)`, the timer around `step` from
   `slideshow.ts` (package B):
   - Use B's types and behaviour as described in `docs/packages/B.md`. Until
     B merges, `step` is a stub, so test against a fake `step` or wait for
     B's merge and rebase. Say which in the report.
   - Interval from `store.settings.interval_s`. The next tick is scheduled
     only after the current image has displayed (or failed), so slow NAS
     images still get their full interval on screen. Expose a function the
     viewer and controller call for "image shown", "image failed" and
     "folder loaded".
   - `store.slideshow` mirrors the state machine phase.
   - `escape` while playing stops it: register the `slideshow` layer on
     `ctx.esc`.
   - Effects:
     - `load_next_folder` → call a callback supplied at mount; the
       controller does the `/nav` call.
     - `stopped{end}` → a toast `slideshow.ended`.
   - Folder toast: when a new folder starts, show its name for
     `FOLDER_TOAST_MS`.
   - Fade transition:
     - When `transition === 'fade'` and reduced motion is off, cross-fade
       over `SLIDESHOW_FADE_MS`.
     - Do it with a class on the stage, documented for the viewer package.
     - Do not edit viewer files.
4. `errors.ts`:
   - `renderErrorCard(host, err, fileName)`:
     - Shows the file name, the sentence `t('err.<code>')` (for
       `missing_codec`, use `err.missing_codec.<codec>`), and an action.
     - For `heif_ext` the action links to
       `ms-windows-store://pdp/?ProductId=9PMMSR1CGPWG`. For `hevc_ext`
       it links to `ms-windows-store://pdp/?ProductId=9NMZLZ57R3T7`. Mark
       both IDs in a comment as to-be-verified in Wave 3.
     - A collapsed `<details>` holds `err.detail`.
     - Centered on black, readable, never covering the overlay.
   - `showNasBanner(host, dir, retry)`:
     - A top banner `banner.unreachable` with `reason.timeout` /
       `reason.io` and a retry button. Returns a dismiss function.
     - Showing it twice for the same `dir` must not stack.
   - Add an empty-folder notice function (`hint.empty_folder`, with `{key}`
     = `Ctrl+→` or `⌘→`) and a last-image hint (`hint.last`).
5. `panels.css`, imported by your modules: shared panel and toast styles.
   Use a dark surface, not translucency over the image (`docs/non-goals.md`).

## Tests (vitest + jsdom, fake timers)
- Settings:
  - six controls exist;
  - a change → one debounced save;
  - the normalized server value is written back;
  - a save failure shows inline and keeps the value;
  - Esc closes the panel and focus returns.
- Help: the macOS and Windows tables differ in the modifier.
- Slideshow UI:
  - a tick waits for "image shown";
  - the interval is read from settings;
  - pause stops ticks;
  - Esc stops;
  - next-folder callback;
  - the end toast.
- Errors:
  - every `ErrorCode` renders a sentence;
  - both codecs get the right link;
  - the banner does not stack.

## Acceptance
`powershell -File scripts/check.ps1 -Web` and
`node scripts/gates.mjs --ownership L main` pass. Report the gzip delta.

## Not in scope
- The state machine itself (B).
- Key bindings (K).
- The overlay buttons (J).
- The `/nav` call (controller, Wave 2).
- Editing `i18n.ts`.
