# Dependencies

`scripts/gates.mjs` fails the build if a dependency in `Cargo.toml`
`[workspace.dependencies]` or `web/package.json` has no row here. Versions are
pinned exactly in the manifests. Facts below were checked on crates.io / npm
on 2026-11-03.

## Rust

| crate | version | why | notes |
|---|---|---|---|
| photosee-core | path | workspace crate | |
| image | 0.25.10 | JPEG (zune-jpeg inside) PNG GIF WebP BMP ICO TIFF decode, JPEG/PNG encode | `avif-native` deliberately off (dav1d C build) |
| fast_image_resize | 6.1.0 | SIMD downscaling, which is the main cost of a transcode | |
| jxl-oxide | 0.12.6 | JPEG XL, pure Rust | |
| zune-psd | 0.5.1 | PSD merged image | replaces `psd` 0.3.5: no release since 2024-01, open OOB panic |
| zune-core | 0.5.3 | options types required by zune-psd | |
| moxcms | 0.9.1 | ICC → sRGB, pure Rust | qcms unmaintained since 2024-01; lcms2 is C |
| kamadak-exif | 0.6.1 | EXIF orientation, summary, IFD1 thumbnail | stable but inactive; nom-exif 3.8 is the fallback |
| resvg | 0.49.0 | SVG thumbnails and transcodes | display is passthrough |
| alphanumeric-sort | 1.5.8 | natural sort | natord last released 2015 |
| serde, serde_json | 1.0.229, 1.0.154 | API and config JSON | |
| thiserror | 2.0.21 | error type derive | |
| windows | 0.62.2 | WIC for HEIC on Windows | windows-only target dependency |
| axum | 0.8.9 | HTTP server | |
| tokio | 1.54.1 | async runtime, blocking pool for IO | |
| tower-http | 0.7.1 | static file serving for the web UI | |
| tower | 0.5.3 | `ServiceExt::oneshot` in server tests | dev only |
| tokio-util | 0.7.20 | streaming file bodies | |
| http-body-util | 0.1.5 | body helpers in tests and Range responses | |
| futures-util | 0.3.34 | stream combinators for bodies | |
| mime_guess | 2.0.5 | Content-Type for static files | |
| tempfile | 3.27.0 | temp dirs in tests | dev only |
| rawler | 0.8.0 | **test oracle only**, for RAW preview extraction | not SemVer; mmaps whole files, so never used at runtime |
| tauri | 2.12.x | desktop/mobile shell | added in M1 Wave 2; ≥2.12 for PR #16050 and CVE-2026-42184 |

## Web (devDependencies only; the shipped bundle has zero runtime deps)

| package | version | why |
|---|---|---|
| typescript | 6.0.3 | typescript-eslint 8.72 requires <6.1, so not 7.x |
| vite | 8.3.5 | build and dev server |
| vitest | 5.1.0 | unit tests |
| jsdom | 30.1.3 | DOM for vitest |
| eslint | 10.14.0 | lint, including file and function length limits |
| @eslint/js | 10.0.1 | recommended rules |
| typescript-eslint | 8.72.1 | TS lint rules |
