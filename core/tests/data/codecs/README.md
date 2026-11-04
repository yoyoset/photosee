# Codec samples (package D)

Only files that were already on disk (nothing was downloaded), each <= 16 KiB.

| File | Bytes | What | Source | License |
|---|---|---|---|---|
| `jxl_oxide_doc_240x135.jxl` | 42 | 240x135 lossy JXL, no orientation, no alpha | hex literal in the crate docs of `jxl-oxide` 0.12.6 (`src/lib.rs`, "Decoding an image"), copied out of the cargo registry | MIT OR Apache-2.0 (jxl-oxide) |
| `mimetype_jxl.jxl` | 117 | 1000x1000 JXL, 8-bit RGB | `testdata/jxl.jxl` of `github.com/gabriel-vasile/mimetype` v1.4.2 (Go module cache) | MIT, (c) 2018-2020 Gabriel Vasile |

Neither sample has a non-Normal orientation, and jxl-oxide cannot encode.
`codecs_jxl.rs` therefore derives all eight orientations from the 42-byte
sample by rewriting its image header (see the comment there) and checks the
decoded pixels against a hand-written rotation/flip. These derived files are
built in memory and are not stored here.

PSD and SVG tests build their inputs in the test code; no binary samples.
No PSD sample fits in 16 KiB (the only permissively licensed one found,
mimetype's `psd.psd`, is 29 KB). Real Photoshop files were used only
locally to validate the PSD decoder (see the package report), not committed.
