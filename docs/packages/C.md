# C: standard decode, orientation, ICC, resize, encode, delivery, pipeline (core)

Read `AGENTS.md` first. Owned paths: see `[C]` in `docs/ownership.toml`.
This is the hot path of every image the user sees that is not passed through.

## Why and budgets (measured in M0, `docs/m0-report.md`)
- Sending a 24 MP JPEG unchanged costs 102 ms in the webview, vs 32 ms for a
  screen-size one. So everything larger than 1.5× the request is transcoded
  here, and this code decides how fast "the next image" is when prefetch
  missed.
- Budget for a non-prefetched 24 MP JPEG → 2560×1440 box: **≤150 ms total**
  (decode + resize + encode) on the dev machine (Xeon E5-2698 v3). Measure it
  and report the split. If it does not fit, report the numbers and what would
  make it fit; do not add dependencies.

## Implement (signatures are fixed; read each stub's doc comment)
1. `format.rs` `sniff(head, ext_guess)`: magic bytes for every `Format`.
   - JPEG `FF D8 FF`, PNG, GIF87a/89a, `RIFF....WEBP`, BMP `BM`,
     ICO `00 00 01 00`, PSD `8BPS`, JXL codestream `FF 0A` or container
     `00 00 00 0C 4A 58 4C 20 0D 0A 87 0A`.
   - ISO-BMFF `ftyp` (major and compatible brands): `avif`/`avis` → Avif;
     `heic`/`heix`/`heim`/`heis`/`hevc`/`hevx` → Heic; `mif1`/`msf1` alone →
     Heic unless an `avif` brand is also listed; `crx ` → Raw(Cr3).
   - RAF `FUJIFILMCCD-RAW`; ORF `IIRO`/`IIRS`/`MMOR`; RW2 `IIU\0`.
   - TIFF `II*\0`/`MM\0*`: CR2 has `CR` at byte 8. Otherwise, if `ext_guess`
     is a TIFF-based RAW (Nef, Arw, Dng), keep it; else Tiff.
   - SVG: after an optional UTF-8 BOM and whitespace, `<?xml` or `<svg`, or
     `<!--`, and the extension says svg. `None` when nothing matches.
2. `orient.rs`: `read` parses the EXIF Orientation with `kamadak-exif`
   (JPEG APP1, PNG eXIf, TIFF IFD0, WebP EXIF chunk). Read at most 256 KiB
   of the head for this; never `read_all`. `apply` implements all 8 cases on
   `Rgba8` (swap w/h for the 4 transposing ones).
3. `color.rs` `to_srgb(img, icc)`: `moxcms` transform from the embedded
   profile to sRGB, in place. A profile that is already sRGB (compare the
   parsed profile, not bytes) or that fails to parse leaves the pixels
   untouched (log nothing, return Ok): a bad profile must never fail a photo.
4. `resize.rs`: `fit` per its doc; `resize` with `fast_image_resize` on RGBA
   with correct alpha (multiply/divide). Pick the filter by measuring:
   Lanczos3 vs CatmullRom vs Bilinear on 24 MP → 2560×1707. Report ms and
   pick the fastest one that has no visible aliasing on the gradient/noise
   test image. Write the choice and numbers in a comment.
5. `encode.rs` `encode`: PNG when any alpha < 255, else JPEG at
   `jpeg_quality` with the `image` encoders. Output must carry no EXIF (so
   no Orientation tag): assert this in a test.
6. `decode/std_formats.rs` `info` / `decode` with `image` 0.25:
   - A `Read + Seek` adapter over `ReadAt` (buffered, 64 KiB) so decoders
     range-read instead of loading whole files where they can.
   - `image::Limits`: max width/height and `max_alloc` derived from
     `MAX_PIXELS`; exceeding them returns `ErrorCode::TooLarge`. Decoder
     errors map to `Corrupt`; unsupported features map to `Unsupported`.
   - `info`: dimensions without decoding pixels; `orientation` from
     `orient::read`; `animated`: GIF with more than one image descriptor,
     WebP with the VP8X animation flag. Parse just enough structure: do not
     decode frames to find out. AVIF → `Unsupported` (see module doc).
   - `decode` returns **sRGB and oriented** (module contract in
     `decode/mod.rs`): decode first frame → `color::to_srgb` with the
     decoder's ICC profile → `orient::apply(orient::read(..))`. 16-bit and
     CMYK TIFF convert to RGBA8. `max` may be ignored (callers resize), but
     say in a comment whether any decoder could use it cheaply.
   - Note: `decode/raw/mod.rs` calls `std_formats::decode(.., Jpeg, ..)` and
     then applies the RAW's own orientation on top; that relies on this
     function applying the JPEG's own tag. One ignored test there
     (`needs package C`) must pass after your change; un-ignore it.
7. `delivery.rs` `choose`: rules in the module doc and `docs/api.md` §img.
   "Native" = `format::always_native(f) || native.contains(&f)`. For RAW,
   compare the preview's display pixels against the request.
8. `pipeline.rs`:
   - `info(fs, path)`: open, read `SNIFF_LEN`, `sniff` (extension as the
     guess), `decode::info`.
   - `render(fs, path, request, native)`: info → for RAW,
     `decode::raw::locate_preview` → `delivery::choose` → for passthrough,
     `Rendered::Passthrough` with the MIME (RAW range = `image/jpeg`); for
     transcode, `decode::decode(src, fmt, Some(target))`, resize to `target`
     when larger, `encode(.., 85)`.

## Tests
Build inputs in memory with the `image` encoders. For EXIF orientation,
write a small test helper that splices an APP1 segment with an IFD0
Orientation tag after SOI (see `tools/gen-fixtures` for a working example).
For ICC, build a Display P3 profile with moxcms if it can serialize one;
otherwise use `fixtures/generated/p3.png` and skip cleanly when it is
missing.
Required cases:
- `sniff`: every magic; `wrong-ext` (JPEG bytes named .png → Jpeg); short
  and empty heads.
- orientation 1–8 via the red top-left marker, end to end through
  `pipeline::render`: the transcoded output decodes with the marker in the
  top-left, and has no Orientation tag (applied exactly once).
- `delivery::choose` table: native small / native large / animated large /
  webp with and without Accept / RAW Normal small / RAW Rotate90 / Tiff.
- limits: a TIFF header claiming 100000×100000 → TooLarge, never an
  allocation; truncated JPEG → Corrupt; garbage → `None` from sniff.
- `core/tests/pipeline_bench.rs` `#[ignore]` test that prints the 24 MP
  timings (run it with `--ignored --nocapture --release` and paste numbers
  in the report).

## Acceptance
`powershell -File scripts/check.ps1 -Rust` and
`node scripts/gates.mjs --ownership C main` pass. Report the 24 MP numbers
(decode / resize / encode, release build) and the filter choice.

## Not in scope
JXL/PSD/SVG (D), HEIC (E), thumbnails (F), anything in `server/`. No
`unsafe`.
