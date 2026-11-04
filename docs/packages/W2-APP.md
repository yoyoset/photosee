# W2-APP: Tauri desktop shell, resident mode (Wave 2)

Read `AGENTS.md` first. Owned paths: see `[W2-APP]` in `docs/ownership.toml`.
`spikes/transport/` is a working Tauri 2.12.5 + axum setup measured in M0
(`REPORT.md`). Reuse its versions and build setup. Do not re-measure it.

## Why
The user double-clicks a photo and expects it on screen at once. M0 measured
a ~1 s cold start, 645 ms of which is WebView2 creation, so G0 chose
**resident mode**:
- the first open of the day pays the 1 s;
- closing the window only hides it;
- every later open reuses the warm webview.

## Implement `app/` (a workspace member, *not* in `default-members`)
1. `app/Cargo.toml`:
   - `tauri =2.12.5` with `custom-protocol`, and `tauri-build =2.7.4`, as in
     the spike;
   - `tauri-plugin-single-instance`: the newest release compatible with
     tauri 2.12. Pin it exactly and record why in `docs/deps.md`;
   - `photosee-server`, `photosee-core`, `tokio`.
   - Add `app` to the root `members` (you may edit the root `Cargo.toml` and
     `Cargo.lock` for this only).
2. Startup, in this order (the M0 timeline is why):
   1. Parse argv: one optional path, a file or a folder.
   2. Start the server on `127.0.0.1:0` on the tokio runtime, *in parallel
      with* the webview creation:
      - `Policy::AnyLocal`;
      - `token` = 32 random bytes, hex encoded. Use OS randomness:
        `getrandom` is already in the lockfile through tauri, or use the
        `windows` crate's `BCryptGenRandom`. Never time-seeded;
      - `allowed_hosts` = `["127.0.0.1:<port>", "localhost:<port>"]`;
      - `start` = the logical path of the argument;
      - `web_dir` = the built `web/dist`, located next to the exe or found
        through Tauri's resource dir.
   3. Create the window: black background, no white flash, so set the
      window background colour before the first paint. Navigate it to
      `http://127.0.0.1:<port>/?t=<token>`. The guard turns that into an
      HttpOnly cookie and redirects (`server/src/guard.rs`).
3. Resident mode:
   - The window close button hides the window and the process stays up.
   - Single instance: a second launch with a path sends it to the running
     instance, which:
     1. sets `*state.start.write() = Some(path)`;
     2. reloads the page (`location.reload()` via `eval`);
     3. shows the window and focuses it.
   - Quitting: a tray icon is out of scope. Add an "Exit" path that ends
     the process: Esc on the last layer calls `host.close()`, which hides;
     `Ctrl+Q` (`Cmd+Q` on macOS) quits. Write down what you chose.
4. Host bridge (`web/src/host.ts` defines the `Host` interface the UI wants
   as `window.__PHOTOSEE_HOST__`):
   - **First measure** whether the HTML Fullscreen API
     (`requestFullscreen`) inside WebView2 makes the *Tauri window*
     fullscreen. Record the result ("assumed A, measured B").
   - If it does not, add host routes to the router *in app*:
     - `POST /api/v1/host/fullscreen {on}`, `GET` for its state, and
       `POST /api/v1/host/close`;
     - put them behind the same guard layer;
     - inject an initialization script that defines `__PHOTOSEE_HOST__`
       with `fetch` calls.
   - Do **not** enable Tauri IPC for the remote `http://127.0.0.1` origin.
     The page is served over HTTP, and a capability granting it IPC would
     widen the attack surface.
5. `scripts/build-app.ps1`:
   - build `web` (`npm run build` → `web/dist`);
   - `cargo build -p photosee-app --release`;
   - put `web/dist` next to the exe (or as a Tauri resource);
   - print the exe path and its size.
   - No installer yet; NSIS comes in Wave 3.

## Acceptance
- `powershell -File scripts/check.ps1 -Rust` passes. It builds the default
  members; also run `cargo clippy -p photosee-app -- -D warnings`.
- `node scripts/gates.mjs --ownership W2-APP main` passes.
- Measured with the release build on this machine, reported as numbers:
  - Launching with `fixtures/generated/big24mp.jpg`: process start → first
    paint. Reuse the spike's timing approach, or a `performance.now`
    report posted to a debug-only route.
  - A second launch with another file while resident → paint.
  - Private working set while resident and idle.
- Manual check, described in the report: open a folder, page with the arrow
  keys, F for fullscreen, Esc unwinds the layers, the window close hides,
  and the second launch reuses the window.

## Not in scope
Installer and file associations (Wave 3), tray icon, macOS/Linux packaging
(M3), auto-update.
