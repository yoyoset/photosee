# M0-R: RAW embedded-preview extractor (Claude sub-agent)

## Why
Camera RAW files embed a full-size JPEG preview, and it is enough for
viewing. `rawler` can extract it, but it mmaps and populates the whole file
(≈0.5s for 50MB over SMB) and returns decoded pixels rather than the JPEG
bytes, so we cannot pass them through. This package finds the largest
embedded JPEG by reading only headers. The server can then stream that byte
range unchanged.

## Owned paths
- `core/src/decode/raw/**`: split by container, e.g. `mod.rs`, `tiff.rs`
  (IFD walking), `bmff.rs` (CR3), `raf.rs`, `jpeg.rs` (SOF size parsing).
- `core/tests/raw_*.rs`.

## Contract (already in `core/src/decode/raw/mod.rs`, keep the signatures)
- `locate_preview(src: &dyn ReadAt, kind: RawKind) -> Result<PreviewLoc>`
- `info(src, kind) -> Result<ImageInfo>`. The size comes from the embedded
  JPEG's SOF marker; the orientation comes from the RAW's own metadata;
  `format: Format::Raw(kind)`.
- `decode(src, kind, max) -> Result<Rgba8>`. Wrap the byte range in a
  sub-range `ReadAt`, decode it with `crate::decode::std_formats::decode(..,
  Format::Jpeg, max)`, then apply orientation with `crate::orient::apply`.
  Those two are stubs owned by package C. Write the call; the test for
  `decode` may be `#[ignore = "needs package C"]`.

Use only `crate::io::read_exact_at` for IO. **Never** call `read_all`. Read at
most a few hundred KB of headers in total.

## Where the preview is (verified from rawler source and format docs)
| kind | location |
|---|---|
| CR2 | IFD0: StripOffsets/StripByteCounts (Compression=6), full size |
| CR3 | ISO-BMFF: `moov` → the `trak` whose sample is the large JPEG ("PreviewBig"; not THMB/PRVW) → `stsz` + `co64`/`stco` |
| NEF | SubIFDs (tag 0x014A) with NewSubFileType=1; take the largest JPEGInterchangeFormatLength |
| ARW | IFD0 JPEGInterchangeFormat/Length (often only 1616 wide; report its real size) |
| DNG | SubIFDs with NewSubFileType=1 and Compression=7 (JPEG). Uncompressed or JXL previews → `Unsupported` |
| ORF | MakerNote → CameraSettings/PreviewImageStart+Length |
| RW2 | tag 0x002E JpegData in IFD0 (an embedded JPEG; offset = value offset) |
| RAF | header: big-endian u32 JPEG offset at byte 84, length at 88 |

Orientation:
- TIFF-based formats: IFD0 tag 0x0112.
- CR3: the CMT1 box's IFD0.
- RAF: the embedded JPEG's EXIF.

Every offset and length must be checked against `src.len()`, and IFD walks
must be capped (≤64 IFDs, ≤1024 entries each) so loops terminate on corrupt
files.

## Acceptance
- Unit tests on synthetic, hand-built byte buffers for:
  - TIFF little- and big-endian;
  - a SubIFD chain;
  - the RAF header;
  - a minimal BMFF tree;
  - truncated and garbage input (must return `Err`, never panic, and allocate
    nothing larger than the file);
  - an IFD loop (an offset pointing back to itself).
- With `PHOTOSEE_PRIVATE_FIXTURES` set: for each file in `raw/`, the
  extracted JPEG decodes with `image` (used directly in the test). Its
  dimensions equal what `rawler::decoders` reports for the preview, and the
  returned range ends inside the file. When the variable is unset, skip
  cleanly and print a note.
- Count and report header bytes read per sample, using a counting `ReadAt`
  wrapper in the test.
- `powershell -File scripts/check.ps1 -Rust` passes, and so does
  `node scripts/gates.mjs --ownership M0-R main`.
- Final report: the formats verified on real samples, the formats verified
  only synthetically, and the bytes read per format.
