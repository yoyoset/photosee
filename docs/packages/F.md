# F: thumbnails, thumbnail cache, EXIF summary (core)

Read `AGENTS.md` first. Owned paths: see `[F]` in `docs/ownership.toml`.
Package C (decode, orientation, resize, encode) is merged; build on it.

## Why
The filmstrip asks for dozens of thumbnails per folder. Over SMB, a full
decode of each 24 MP file costs about 100 ms of CPU plus the transfer. The
EXIF IFD1 thumbnail costs one small read. The disk cache makes the second
visit to a folder free.

## Implement
1. `thumb.rs` `make(fs, path, edge)`, which returns JPEG bytes (the API
   promises JPEG):
   - Sniff the format like `pipeline::info` does.
   - **Embedded thumbnail first**: JPEG, TIFF and TIFF-based RAW (CR2, NEF,
     ARW, DNG) carry an IFD1 JPEG. Read it with `kamadak-exif` from a bounded
     head read (≤ 1 MiB, never `read_all`).
     - Use it when its long edge is at least `edge / 2`. A typical 160 px
       IFD1 is enough for the 64 px filmstrip at 2× DPR.
     - It is stored unrotated: decode it, apply the *main image's*
       orientation (`orient::read`), resize to fit `edge`, re-encode.
   - Otherwise:
     - RAW: decode the embedded preview through `decode::decode`.
     - Others: `decode::decode(src, fmt, Some(edge×edge))`.
     - Then resize with `resize::fit` (no upscale) and `resize::resize`.
   - Animated images give frame one. `decode` already does this; verify it
     with a test.
   - Alpha: composite onto black (the stage colour), then encode as a JPEG
     at quality 80. Write a small JPEG-only encode path or reuse
     `encode::encode` on opaque pixels.
2. `thumbcache.rs`:
   - `key`: a hex string of a **stable** 64-bit hash (FNV-1a, written
     here; std's `DefaultHasher` is not stable across Rust releases) of
     logical path bytes, size, mtime and edge.
   - Files live at `dir/<first 2 hex>/<key>.jpg`.
   - `put` writes atomically (tmp + rename), creating directories as
     needed.
   - `get` returns the bytes and bumps the file's mtime with
     `File::set_modified`, so eviction is least-recently-used without an
     index file.
   - Budget:
     - Keep a running byte total behind a `Mutex`, initialized lazily by one
       directory scan.
     - When a `put` pushes it over `budget_bytes`, delete the oldest files
       by mtime until it is at 90% of the budget.
     - `budget_bytes == 0` means caching is disabled: get/put are no-ops.
   - Every IO failure is swallowed (and counted, if useful): a cache must
     never break a thumbnail.
3. `meta.rs` `exif_summary(fs, path)`:
   - Read with `kamadak-exif` from a bounded head (≤ 1 MiB). It understands
     JPEG, TIFF, HEIF, PNG and WebP containers.
   - Field mapping:
     - camera = Model, prefixed by Make unless Model already starts with it
       (`Canon Canon EOS R6` → `Canon EOS R6`);
     - lens = LensModel;
     - iso = PhotographicSensitivity;
     - exposure: "1/250" for times under 1 s, "2.5" s otherwise;
     - f_number, focal_mm;
     - taken = DateTimeOriginal as stored.
   - Missing or unreadable EXIF → `ExifSummary::default()`, never an error.
     A missing file is still `NotFound`.

## Tests (`core/tests/thumb_*.rs`; MemFs is available there)
- Build JPEGs in memory, some with an IFD1 thumbnail. See
  `tools/gen-fixtures` for APP1 building; extend it in the test helper.
- An IFD1 thumbnail is used: count bytes read with a counting `ReadAt`.
  Compare with the decode path.
- Orientation 6 photo → thumbnail is portrait with the marker top-left.
- PNG with alpha → JPEG on black.
- Cache:
  - round trip;
  - eviction order after `get` bumps an old entry;
  - budget 0;
  - an unwritable directory does not error.
- EXIF:
  - Make/Model dedupe;
  - exposure formatting;
  - a JPEG without EXIF → default.

## Acceptance
`powershell -File scripts/check.ps1 -Rust` and
`node scripts/gates.mjs --ownership F main` pass. Report, for a 24 MP JPEG
with IFD1 and for one without, the thumbnail ms and bytes read.

## Not in scope
The `/thumb` route and its priority (package G).
