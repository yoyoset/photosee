# D: JPEG XL, PSD and SVG decoders (core)

Read `AGENTS.md` first. Owned paths: see `[D]` in `docs/ownership.toml`.

## Why
These three are not shown natively by every webview, so the server decodes
them for display, thumbnails or both. Each module implements the
`decode/mod.rs` contract: `info(src)` without pixel decode, and
`decode(src, max)` returning RGBA8 that is **sRGB and oriented**.

## Implement
1. `decode/jxl.rs` with `jxl-oxide` 0.12:
   - `info`: dimensions and orientation from the image header only. Report
     the size *before* orientation, like every other format (`ImageInfo`
     doc). Animated = the header's animation flag.
   - `decode`: first frame (keyframe 0). Ask jxl-oxide for sRGB output
     (`request_color_encoding` or equivalent) so no ICC step is needed. Check
     whether jxl-oxide's render already applies the header orientation; it
     must end up applied exactly once. Write which it was in a comment.
   - Feed it a buffered reader over `ReadAt` (whole-file read is acceptable
     for JXL; say so in a comment), and cap pixels at
     `std_formats::MAX_PIXELS` → `ErrorCode::TooLarge` *before* rendering.
2. `decode/psd.rs` with `zune-psd` 0.5: the merged composite image only.
   - `info`: parse the 26-byte header yourself (signature, version 1,
     channels, height, width, depth, color mode). Version 2 (PSB) →
     `Unsupported` (non-goal).
   - `decode`: RGB/RGBA and grayscale, 8 and 16 bit. Other color modes
     (CMYK, Lab, indexed, bitmap) → `Unsupported` with the mode named in the
     message. Find out what zune-psd does with ZIP-compressed image data and
     map it to `Unsupported`, not a panic or garbage.
   - Orientation is always `Normal`. Embedded ICC (image resource 1039) is
     out of scope: note it in the module doc as a known gap.
3. `decode/svg.rs` with `resvg` 0.49 (default features off, `raster-images`
   on; no text feature, so text is not rendered: note it).
   - Security: SVG is attacker-controlled. Set `usvg::Options` so it cannot
     read the filesystem (no `resources_dir`, and an image href resolver
     that only accepts `data:` URLs). Cap the file at 16 MiB → `TooLarge`.
   - `info`: the tree's size, rounded up, at least 1×1.
   - `decode`: rasterize so the result fits `max` (or the native size when
     `None`, capped at 8192 on the long edge), aspect preserved, transparent
     background. resvg renders premultiplied RGBA; convert to straight
     alpha.

## Tests (`core/tests/codecs_*.rs`)
- PSD: build minimal PSD files in the test (header, empty color-mode /
  resources / layer sections, raw-compressed and RLE image data) for RGB 8,
  gray 16, and a CMYK one that must return Unsupported; a PSB header.
- SVG: an inline SVG string with a known red rectangle → pixel check; one
  with `<image href="file:///C:/Windows/win.ini">` and one with an external
  `http://` href → renders without loading anything; a 17 MiB file →
  TooLarge.
- JXL: jxl-oxide cannot encode. Small sample files may be committed under
  `core/tests/data/codecs/` (≤ 16 KiB each, with source and license in a
  `README.md` there). Check the jxl-oxide crate source in the cargo
  registry, or the libjxl conformance set, for small permissively licensed
  samples. Cover at least one plain image and one with a non-Normal
  orientation if one exists; otherwise say so.
- Every decoder: truncated input and garbage → `Err`, never panic.

## Acceptance
`powershell -File scripts/check.ps1 -Rust` and
`node scripts/gates.mjs --ownership D main` pass. Report the decode time of
each sample and whether jxl-oxide applied the orientation itself.

## Not in scope
Format sniffing and the pipeline (package C), HEIC (E), dependency changes.
