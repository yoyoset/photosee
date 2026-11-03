# E: HEIC through Windows Imaging Component (core, Windows only)

Read `AGENTS.md` first. Owned paths: see `[E]` in `docs/ownership.toml`.
This package is `unsafe` FFI; the orchestrator reviews every `unsafe` block.

## Why
Every iPhone photo is HEIC, and WebView2 cannot show it. Windows decodes it
through WIC only when **two** Store packages are present: "HEIF Image
Extensions" (the container) and "HEVC Video Extensions" (the codec). Users
regularly have one and not the other. When one is missing, the UI has to say
which one (`ErrorCode::MissingCodec(Codec::HeifExt | Codec::HevcExt)`).
"Couldn't open file" is the failure we are replacing.

On this dev machine both are installed (`Microsoft.HEIFImageExtension`
1.2.62.0, `Microsoft.HEVCVideoExtension` 2.4.149.0), so real decoding can be
tested here.

## Implement `core/src/decode/heic_wic.rs` (windows crate 0.62, features already enabled)
- COM: call `CoInitializeEx(None, COINIT_MULTITHREADED)` on entry. Server
  jobs run on tokio blocking threads that may be reused. Treat `S_FALSE` and
  `RPC_E_CHANGED_MODE` as fine, and balance `CoUninitialize` only for calls
  that succeeded. Use a small RAII guard.
- Create a `CLSID_WICImagingFactory`. Caching the factory per thread is
  allowed if you measure that it matters; report the number.
- Input: read the file with `crate::io::read_all`, then
  `IWICStream::InitializeFromMemory`. HEIC files are a few MB, and a
  hand-written `IStream` over `ReadAt` is not worth the unsafe surface
  now. Cap the file at 256 MiB → `TooLarge`.
- `info`: `CreateDecoderFromStream` → frame 0 → `GetSize`. Orientation: find
  out by experiment whether WIC's HEIF decoder already applies `irot`/`imir`
  to the pixels and size it returns.
  - If it does: report `Normal`.
  - If it does not: read `System.Photo.Orientation` through the metadata
    query reader, and apply it in `decode`.
  Either way, write the observed behaviour in the module doc ("assumed A,
  measured B" if they differ).
- `decode`: frame 0 → optionally `IWICBitmapScaler` to fit `max` with
  `WICBitmapInterpolationModeFant`. That is a cheap reduction, and the
  caller still resizes. Then `IWICFormatConverter` to
  `GUID_WICPixelFormat32bppRGBA` → `CopyPixels` into `Rgba8`. Color: get the
  frame's `IWICColorContext` and, when it is an ICC profile, its bytes, and
  call `crate::color::to_srgb` (package C; until C merges it is a no-op
  stub, which is fine).
- Missing-codec detection, as a **pure function**
  `fn classify(stage, hresult) -> ErrorCode` so it is unit-testable without
  uninstalling anything:
  - `CreateDecoderFromStream` → `WINCODEC_ERR_COMPONENTNOTFOUND`
    (0x88982F50) means no HEIF container decoder → `HeifExt`.
  - Decoder created but `GetFrame`/`CopyPixels` fails with
    0x88982F50 / `MF_E_TOPO_CODEC_NOT_FOUND` (0xC00D5212) /
    `HRESULT_FROM_WIN32(ERROR_NOT_FOUND)` → `HevcExt`.
  - Anything else → `Corrupt` with the HRESULT in hex in the message.
  These HRESULTs come from forum reports, not documentation. Mark them as
  such in a comment. If you can trigger any of them for real (for example
  an AV1-coded HEIF, which is `.avif` content in a `.heic` name), record
  what you observed.

## Tests (`core/tests/heic_*.rs`, `#[cfg(windows)]`)
- Make a HEIC at test time with the WIC encoder
  (`GUID_ContainerFormatHeif`, available when the HEVC extension is
  installed). Use a 64×48 image with a red top-left quadrant, so that
  `info` and `decode` can be checked (size, marker position, RGBA). If the
  encoder is unavailable, skip with a printed note.
- If `PHOTOSEE_PRIVATE_FIXTURES` is set, decode every file in `heic/`, and
  print its size, orientation and decode ms.
- `classify` table test.
- Garbage and truncated bytes → `Err`, never a panic, and no COM leak.
  Run 200 decodes in a loop and check that the process handle count did not
  keep growing (`GetProcessHandleCount`, in the windows crate's
  `Win32_System_Threading` feature: if that feature is not enabled, say so
  and skip this check rather than editing `Cargo.toml`).

## Acceptance
`powershell -File scripts/check.ps1 -Rust` and
`node scripts/gates.mjs --ownership E main` pass. Every `unsafe` block has a
`// SAFETY:` comment. Report:
- decode ms for a 12 MP HEIC, if you can make or find one;
- the orientation finding;
- which HRESULTs you actually observed.

## Not in scope
libheif (M2), Apple ImageIO (M3), non-Windows code paths (`heic.rs` is a
contract file), HEIC sequences and Live Photo video.
