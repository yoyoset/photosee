# W3-PKG: Windows installer, file associations, portable zip (Wave 3)

Read `AGENTS.md` first. Owned paths: see `[W3-PKG]` in `docs/ownership.toml`.

## Why
The user's flow is "double-click a photo → PhotoSee opens it full-screen".
That needs file associations and an installer. The plan's size budget is
**≤15 MB** for the installer, with WebView2 fetched by the online
bootstrapper and not bundled.

## Implement
1. NSIS installer via Tauri's bundler (`bundle.targets = ["nsis"]` in
   `app/tauri.conf.json`). Must have:
   - Per-user install (no admin) by default.
   - WebView2: `downloadBootstrapper` (online). Measure the installer size
     and report it.
   - `web/dist` shipped as a bundle resource. `app/` already locates it next
     to the exe or through the resource dir; verify it from the installed
     location.
   - File associations (`bundle.fileAssociations`) for every extension in
     `core/src/format.rs` `from_extension`. Register them as "Open with
     PhotoSee" capable. Do not force the default handler; Windows asks the
     user, and that is the user's call.
   - The icon: reuse `app/icons`.
2. Portable zip. `scripts/package.ps1` produces both:
   - the installer;
   - `PhotoSee-portable.zip` (exe + `web/`).
   Print both sizes.
3. Smoke test the installed build on this machine:
   - Install silently to a temp user dir if NSIS allows it, otherwise the
     normal per-user path.
   - Launch it with a fixture JPEG as the argument.
   - Check that the window shows the image, using `app/measure.mjs smoke`
     or an equivalent.
   - Uninstall, and check that the file associations are gone.
   - Report every step.

## Acceptance
- `node scripts/gates.mjs --ownership W3-PKG main` passes.
- `powershell -File scripts/check.ps1 -Rust` passes.
- Report:
  - installer size (target ≤15 MB);
  - zip size;
  - the smoke-test result;
  - the uninstall cleanup result.

## Not in scope
Code signing (no certificate), auto-update, Microsoft Store, macOS and
Linux packaging (M3).
