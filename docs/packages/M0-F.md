# M0-F: fixture generator (flashgate)

## Goal
A deterministic generator that writes every file listed in
`fixtures/MANIFEST.md` → "Generated" into `fixtures/generated/`.
Deterministic means fixed seeds, so the bytes are identical on every run.

## Owned paths
- `tools/gen-fixtures/**`: a standalone Rust binary. Its own `Cargo.toml` has
  an empty `[workspace]` table, and it has its own `Cargo.lock`.
- `fixtures/generated/.gitkeep`

## Dependencies (pin exactly)
- `image = { version = "=0.25.10", default-features = false, features = ["jpeg", "png", "gif", "webp", "tiff"] }`
- `moxcms = "=0.9.1"`, used to produce the Display P3 ICC profile bytes. If
  moxcms cannot serialize a profile, embed a minimal P3 profile as a byte
  constant generated once, and note this in the report.
- Nothing else. Write the random generator yourself (xorshift64, seed
  0x5EED).

## Exact outputs
| file | how |
|---|---|
| `orient/{1..8}.jpg` | 300×200 RGB: top-left quadrant red (255,0,0), top-right green, bottom-left blue, bottom-right white. Encode JPEG q90, then insert an APP1 EXIF segment right after SOI: `Exif\0\0` + TIFF header `II*\0` + IFD0 with one entry, tag 0x0112 SHORT count 1 value N, next-IFD 0. Write the segment bytes by hand |
| `p3.png` | 64×64 RGBA (255,0,0,255), with an `iCCP` chunk carrying the Display P3 profile (zlib via `image`'s PNG encoder, if it supports ICC; otherwise write the chunk manually with the `flate2` that `image` already depends on, no new direct dep). If impossible, report it and skip |
| `anim.gif` | 32×32, 3 frames (red, green, blue), 100ms delay, infinite loop |
| `anim.webp` | skip if `image` can't encode animated WebP; list it in the report |
| `pano.jpg` | 6000×1000 horizontal gradient, q85 |
| `tall.png` | 400×4000 vertical gradient |
| `tiny.png` | 16×16 checkerboard |
| `big24mp.jpg` | 6000×4000: gradient + xorshift noise ±16, q90 |
| `alpha.png` | 128×128 RGBA; left half alpha 0, right half opaque |
| `truncated.jpg` | `pano.jpg` bytes cut at 60% of length |
| `garbage.jpg` | 4096 xorshift bytes |
| `huge-header.tif` | hand-written little-endian TIFF: IFD0 with ImageWidth=100000, ImageLength=100000, BitsPerSample=8, SamplesPerPixel=3, Compression=1, PhotometricInterpretation=2, StripOffsets=8 + IFD size, StripByteCounts=16, RowsPerStrip=1, then 16 zero bytes |
| `wrong-ext.png` | the bytes of `orient/1.jpg` |
| `tree/` | `a10.jpg`, `a2.jpg`, `.hidden.jpg`, `notes.txt` ("x"), `2023/b.png`, `2024/` (empty dir), `2025/c.jpg`, where each image is a real 8×8 file of that format |

## Acceptance
- `cargo run --release` in `tools/gen-fixtures` writes all of the files
  above, and a second run produces byte-identical files. Add a `--check` flag
  that regenerates into a temp dir and compares the two.
- `cargo test` covers the hand-written EXIF segment: parse it back and assert
  orientation N. It also covers the TIFF header: parse width and height back.
- Files ≤300 lines, functions ≤50 lines.
- `node scripts/gates.mjs --ownership M0-F main` passes.
- Report: every file written, plus anything skipped and why.
