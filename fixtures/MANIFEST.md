# Test fixtures

## Generated (`fixtures/generated/`, gitignored)
Produced by `cargo run --release --manifest-path tools/gen-fixtures/Cargo.toml`.
The generator is deterministic: the same inputs always produce the same bytes.

| file | purpose |
|---|---|
| `orient/1.jpg` … `orient/8.jpg` | 300×200 JPEG with an EXIF Orientation tag of 1..8. Content is a marker image whose top-left quadrant is red, so a test can tell which way it was rotated or flipped |
| `p3.png` | 64×64 PNG with an embedded Display P3 ICC profile, filled with P3 pure red |
| `anim.gif` | 3-frame 32×32 animated GIF |
| `anim.webp` | 3-frame animated WebP, if the generator can write it; otherwise listed as missing |
| `pano.jpg` | 6000×1000 panorama |
| `tall.png` | 400×4000 |
| `tiny.png` | 16×16 (no upscaling test) |
| `big24mp.jpg` | 6000×4000 noise plus gradients, quality 90 (bench) |
| `alpha.png` | RGBA with transparent pixels (forces PNG output) |
| `truncated.jpg` | a valid JPEG cut at 60% |
| `garbage.jpg` | random bytes with a .jpg name |
| `huge-header.tif` | TIFF header claiming 100000×100000 with no data (pixel limit test) |
| `wrong-ext.png` | a JPEG named .png (magic beats extension) |
| `tree/` | folder tree mirroring the `docs/api.md` golden tree, with real tiny JPEGs |

## Private (`fixtures-private/`, gitignored, user-provided)
Set `PHOTOSEE_PRIVATE_FIXTURES=F:\my_ai\photosee\fixtures-private`.

| folder | contents |
|---|---|
| `raw/` | one file per format the user has: cr2 cr3 nef arw dng orf rw2 raf |
| `heic/` | iPhone HEIC, at least one rotated and one Live Photo still |
| `psd/`, `jxl/` | optional samples |

Public RAW samples (raw.pixls.us, CC0) are downloaded only with the user's
explicit approval.
