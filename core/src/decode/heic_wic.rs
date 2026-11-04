//! HEIC through Windows Imaging Component. Package E. WIC needs both the
//! "HEIF Image Extensions" (container) and the "HEVC Video Extensions"
//! (codec); when one is missing, `classify` maps the HRESULT to
//! MissingCodec(HeifExt|HevcExt) so the UI can say exactly what to install.
//!
//! Orientation, measured (HEIF ext 1.2.62.0, HEVC ext 2.4.149.0): WIC's HEIF
//! decoder already applies `irot` and `imir` to both `GetSize` and the pixels
//! (tests/heic_orient.rs patches them into an encoded file), so `info`
//! reports `Normal` and `decode` does nothing more. That matches what
//! orient.rs assumed. Pitfall: WIC reads `imir` axis 0 as a top-bottom flip
//! and 1 as left-right; the spec's "vertical axis" wording is read both ways.
//!
//! Speed, measured: releasing the last live HEIF decoder in the process
//! blocks ~255 ms (the extension tears down shared state; a new decoder waits
//! for it), so a 64x48 decode took ~340 ms. `keep_loaded` holds one unused
//! decoder forever: ~17 ms. CoCreateInstance(factory) is ~8.5 us: no cache.
//! Known leak, not ours: each HEIF CopyPixels leaks ~1 handle in the extension
//! (bare WIC, 300 calls: +307; no CopyPixels: +0). See tests/heic_leak.rs.

use crate::color;
use crate::decode::std_formats::MAX_PIXELS;
use crate::error::{Codec, Error, ErrorCode, Result};
use crate::io::{self, ReadAt};
use crate::resize;
use crate::types::{Format, ImageInfo, Orientation, Rgba8, Size};
use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, WINCODEC_ERR_COMPONENTNOTFOUND};
// Globs: this file is a thin layer over these two APIs and uses ~30 names.
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;
use windows::core::Interface;

/// iPhone HEICs are 1-5 MB; anything this big is not a photo we can hold.
const MAX_FILE: u64 = 256 * 1024 * 1024;
/// Real ICC profiles are a few KB; a huge one is hostile.
const MAX_ICC: u32 = 4 * 1024 * 1024;
/// HEIF files must list mif1 or msf1, but older encoders only wrote the
/// codec brand. AVIF is HEIF too, and the HEIF extension reads it.
const HEIF_BRANDS: [&[u8; 4]; 6] = [b"mif1", b"msf1", b"heic", b"heix", b"hevc", b"avif"];

// The next two HRESULTs come from forum reports about a missing HEVC codec,
// not from documentation, and with both extensions installed they could not
// be triggered here. Observed instead (all Corrupt): bad metadata or garbage
// after ftyp -> 0xC00D36BE at CreateDecoderFromStream; truncated mdat ->
// 0x80070057 or 0xC00D36BE, zeroed HEVC data -> 0xC00D3E8C, both at
// CopyPixels; an `av01` item without the AV1 extension -> 0x88982F81 at
// GetFrame; a non-HEIF brand -> 0x88982F50 at Open, hence `is_heif`.
const MF_E_TOPO_CODEC_NOT_FOUND: i32 = 0xC00D_5212_u32 as i32;
/// HRESULT_FROM_WIN32(ERROR_NOT_FOUND = 1168).
const HR_ERROR_NOT_FOUND: i32 = 0x8007_0490_u32 as i32;

/// Where a WIC call failed, since the same HRESULT means different things at
/// different stages: `Setup` (COM, factory, stream: not the file's fault),
/// `Open` (CreateDecoderFromStream picks a container decoder), `Decode`
/// (after a decoder exists: frames and pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Setup,
    Open,
    Decode,
}

/// Maps a failed WIC call to the error the UI shows. Pure, so the table is
/// testable without uninstalling Store packages.
pub fn classify(stage: Stage, hr: i32) -> ErrorCode {
    let not_found = hr == WINCODEC_ERR_COMPONENTNOTFOUND.0;
    let codec_gone = not_found || hr == MF_E_TOPO_CODEC_NOT_FOUND || hr == HR_ERROR_NOT_FOUND;
    match stage {
        Stage::Setup => ErrorCode::Unsupported,
        Stage::Open if not_found => ErrorCode::MissingCodec(Codec::HeifExt),
        Stage::Decode if codec_gone => ErrorCode::MissingCodec(Codec::HevcExt),
        Stage::Open | Stage::Decode => ErrorCode::Corrupt,
    }
}

/// True when the file starts with an `ftyp` box naming a HEIF brand. Without
/// it, an MP4 or garbage file reaches WIC, finds no decoder, and would be
/// reported as "install the HEIF extension".
pub fn is_heif(bytes: &[u8]) -> bool {
    if bytes.get(4..8) != Some(b"ftyp".as_slice()) {
        return false;
    }
    let size = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let ftyp = bytes.get(8..size.min(bytes.len())).unwrap_or_default();
    // Major brand, minor version (not a brand), then compatible brands.
    let mut brands = ftyp.chunks_exact(4).enumerate().filter(|(i, _)| *i != 1);
    brands.any(|(_, b)| HEIF_BRANDS.iter().any(|h| h.as_slice() == b))
}

/// WIC failure -> our error, with the HRESULT in hex for the "details" line.
trait At<T> {
    fn at(self, stage: Stage, what: &str) -> Result<T>;
}

impl<T> At<T> for windows::core::Result<T> {
    fn at(self, stage: Stage, what: &str) -> Result<T> {
        self.map_err(|e| {
            let hr = e.code().0;
            let msg = format!("WIC {what}: HRESULT 0x{:08X} {}", hr as u32, e.message());
            Error::new(classify(stage, hr), msg)
        })
    }
}

/// Balances `CoInitializeEx`. `S_OK` and `S_FALSE` both need a matching
/// `CoUninitialize`; `RPC_E_CHANGED_MODE` means the thread is already an STA,
/// which WIC works in too, but that init is not ours to undo. It only lives
/// in `Session`, which COM interfaces make !Send, so it drops on its thread.
struct ComGuard {
    owned: bool,
}

impl ComGuard {
    fn init() -> Result<Self> {
        // SAFETY: no reserved pointer; COINIT_MULTITHREADED is a valid flag.
        // Balanced in Drop only when it succeeded.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            hr.ok().at(Stage::Setup, "CoInitializeEx")?;
        }
        Ok(Self { owned: hr.is_ok() })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: balances our successful init, on its thread (type doc).
            unsafe { CoUninitialize() };
        }
    }
}

/// Once per process, from an MTA thread: pins the MTA and leaks one unused
/// HEIF decoder in it, so the extension never pays its ~255 ms teardown.
fn keep_loaded(factory: &IWICImagingFactory, com: &ComGuard) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    // SAFETY: COM is initialized (MTA) on this thread. The usage cookie is
    // never released and the decoder never used or released, on purpose.
    let pin = || unsafe {
        if CoIncrementMTAUsage().is_ok() {
            std::mem::forget(factory.CreateDecoder(&GUID_ContainerFormatHeif, std::ptr::null()));
        }
    };
    if com.owned {
        ONCE.call_once(pin);
    }
}

/// One opened HEIC. Field order is drop order and it matters: the COM
/// objects go before `bytes` (the stream reads it without copying), and
/// everything goes before `com` (no COM calls after CoUninitialize).
struct Session {
    frame: IWICBitmapFrameDecode,
    factory: IWICImagingFactory,
    _keep: (IWICBitmapDecoder, IWICStream, Vec<u8>, ComGuard),
}

impl Session {
    fn open(src: &dyn ReadAt) -> Result<Self> {
        let len = src.len()?;
        if len > MAX_FILE {
            let msg = format!("HEIC file is {len} bytes (limit {MAX_FILE})");
            return Err(Error::new(ErrorCode::TooLarge, msg));
        }
        let bytes = io::read_all(src)?;
        if !is_heif(&bytes) {
            return Err(Error::corrupt("not HEIF: no ftyp box with a HEIF brand"));
        }
        let com = ComGuard::init()?;
        // SAFETY: COM is initialized on this thread by `com`.
        let factory: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
                .at(Stage::Setup, "CoCreateInstance(WICImagingFactory)")?;
        keep_loaded(&factory, &com);
        // SAFETY: `bytes` ends up in `_keep` after the stream and decoder, so
        // it outlives every reader; moving a Vec does not move its buffer. On
        // early returns the COM locals are declared later, so dropped first.
        let (stream, decoder, frame) = unsafe {
            let stream = factory.CreateStream().at(Stage::Setup, "CreateStream")?;
            let what = "InitializeFromMemory";
            stream.InitializeFromMemory(&bytes).at(Stage::Setup, what)?;
            let cache = WICDecodeMetadataCacheOnDemand;
            let decoder = factory
                .CreateDecoderFromStream(&stream, std::ptr::null(), cache)
                .at(Stage::Open, "CreateDecoderFromStream")?;
            let frame = decoder.GetFrame(0).at(Stage::Decode, "GetFrame")?;
            (stream, decoder, frame)
        };
        let _keep = (decoder, stream, bytes, com);
        Ok(Self {
            frame,
            factory,
            _keep,
        })
    }

    fn size(&self) -> Result<Size> {
        let (mut w, mut h) = (0u32, 0u32);
        // SAFETY: both out-pointers are valid, live u32 locals.
        unsafe { self.frame.GetSize(&mut w, &mut h) }.at(Stage::Decode, "GetSize")?;
        if w == 0 || h == 0 {
            return Err(Error::corrupt(format!("HEIC reports size {w}x{h}")));
        }
        Ok(Size::new(w, h))
    }

    /// The first ICC profile among the frame's color contexts. Color is a
    /// nicety, so any failure here means "no profile", never a failed decode.
    fn icc(&self) -> Option<Vec<u8>> {
        let mut count = 0u32;
        // SAFETY: an empty slice asks only for the count, written to a live u32.
        unsafe { self.frame.GetColorContexts(&mut [], &mut count) }.ok()?;
        let mut ctxs: Vec<Option<IWICColorContext>> = Vec::new();
        for _ in 0..count.min(8) {
            // SAFETY: plain factory call; the result is an owned interface.
            ctxs.push(Some(unsafe { self.factory.CreateColorContext() }.ok()?));
        }
        // SAFETY: every slot holds a live, factory-created context, which is
        // what GetColorContexts fills; the wrapper passes the slice length.
        unsafe { self.frame.GetColorContexts(&mut ctxs, &mut count) }.ok()?;
        let mut filled = ctxs.iter().take(count as usize).flatten();
        filled.find_map(profile_bytes)
    }
}

fn profile_bytes(ctx: &IWICColorContext) -> Option<Vec<u8>> {
    // SAFETY: `ctx` is a live context filled by GetColorContexts.
    if unsafe { ctx.GetType() }.ok()? != WICColorContextProfile {
        return None;
    }
    let mut len = 0u32;
    // SAFETY: an empty buffer asks only for the length, written to a live u32.
    unsafe { ctx.GetProfileBytes(&mut [], &mut len) }.ok()?;
    if len == 0 || len > MAX_ICC {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    // SAFETY: the wrapper passes `buf`'s real length, so WIC cannot overrun.
    unsafe { ctx.GetProfileBytes(&mut buf, &mut len) }.ok()?;
    buf.truncate(len as usize);
    Some(buf)
}

pub fn info(src: &dyn ReadAt) -> Result<ImageInfo> {
    let s = Session::open(src)?;
    Ok(ImageInfo {
        format: Format::Heic,
        size: s.size()?,
        orientation: Orientation::Normal,
        animated: false,
    })
}

pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    let s = Session::open(src)?;
    let full = s.size()?;
    let target = max.map_or(full, |m| resize::fit(full, m, false));
    if target.pixels() > MAX_PIXELS {
        let msg = format!("HEIC is {}x{} (limit {MAX_PIXELS} px)", target.w, target.h);
        return Err(Error::new(ErrorCode::TooLarge, msg));
    }
    let mut img = pixels(&s, full, target)?;
    if let Some(icc) = s.icc() {
        color::to_srgb(&mut img, &icc)?;
    }
    Ok(img)
}

/// The frame (through a Fant scaler when `size` is smaller: a cheap
/// reduction, the caller still resizes) converted to RGBA and copied out.
fn pixels(s: &Session, full: Size, size: Size) -> Result<Rgba8> {
    let too_big = || Error::new(ErrorCode::TooLarge, "HEIC pixel buffer overflows");
    let stride = size.w.checked_mul(4).ok_or_else(too_big)?;
    let len = usize::try_from(u64::from(stride) * u64::from(size.h)).map_err(|_| too_big())?;
    let mut data = vec![0u8; len];
    let (rgba, dither, alpha) = (&GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone, 0.0);
    let (none, pal) = (None::<&IWICPalette>, WICBitmapPaletteTypeCustom);
    // SAFETY: plain factory calls; the scaler and converter hold their own
    // references to their sources. A null rect means the whole source, whose
    // size is `size` (scaler target or frame size); `data` holds exactly
    // stride * height bytes and the wrapper passes that length.
    unsafe {
        let src: IWICBitmapSource = if size == full {
            s.frame.cast().at(Stage::Decode, "QueryInterface")?
        } else {
            let scaler = s.factory.CreateBitmapScaler();
            let scaler = scaler.at(Stage::Setup, "CreateBitmapScaler")?;
            let init = scaler.Initialize(&s.frame, size.w, size.h, WICBitmapInterpolationModeFant);
            init.at(Stage::Decode, "IWICBitmapScaler::Initialize")?;
            scaler.cast().at(Stage::Decode, "QueryInterface")?
        };
        let conv = s.factory.CreateFormatConverter();
        let conv = conv.at(Stage::Setup, "CreateFormatConverter")?;
        let init = conv.Initialize(&src, rgba, dither, none, alpha, pal);
        init.at(Stage::Decode, "IWICFormatConverter::Initialize")?;
        let copy = conv.CopyPixels(std::ptr::null(), stride, &mut data);
        copy.at(Stage::Decode, "CopyPixels")?;
    }
    Ok(Rgba8 { size, data })
}
