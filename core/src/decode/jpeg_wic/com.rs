//! COM plumbing for the WIC JPEG path: guard, opened frame, color context.
//! Same drop-order and SAFETY rules as `heic_wic.rs` (not shared: that file
//! belongs to another package; fold the two together later).

use crate::error::{Error, ErrorCode, Result};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;

/// Real ICC profiles are a few KB; a huge one is hostile.
const MAX_ICC: u32 = 4 * 1024 * 1024;

/// WIC failure -> our error, HRESULT in hex for the "details" line. `Setup`
/// failures (COM, factory, stream) are not the file's fault: Unsupported.
/// Everything else is Corrupt, but the caller falls back to `image` anyway.
pub(crate) fn wic<T>(r: windows::core::Result<T>, what: &str) -> Result<T> {
    r.map_err(|e| {
        let code = match what {
            "CoInitializeEx" | "CoCreateInstance" | "CreateStream" | "InitializeFromMemory" => {
                ErrorCode::Unsupported
            }
            _ => ErrorCode::Corrupt,
        };
        let msg = format!(
            "WIC {what}: HRESULT 0x{:08X} {}",
            e.code().0 as u32,
            e.message()
        );
        Error::new(code, msg)
    })
}

/// Balances `CoInitializeEx` (see heic_wic.rs: `S_FALSE` also needs the
/// matching uninit; `RPC_E_CHANGED_MODE` means an STA that is not ours).
pub(crate) struct ComGuard {
    owned: bool,
}

impl ComGuard {
    pub(crate) fn init() -> Result<Self> {
        // SAFETY: no reserved pointer; COINIT_MULTITHREADED is a valid flag.
        // Balanced in Drop only when it succeeded.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            wic(hr.ok(), "CoInitializeEx")?;
        }
        if hr.is_ok() {
            static ONCE: std::sync::Once = std::sync::Once::new();
            // SAFETY: this thread is in the MTA. The usage cookie is leaked
            // on purpose: it keeps the MTA (and windowscodecs with it) alive
            // between requests instead of tearing down on every CoUninitialize.
            ONCE.call_once(|| {
                let _ = unsafe { CoIncrementMTAUsage() };
            });
        }
        Ok(Self { owned: hr.is_ok() })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: balances our successful init, on its thread (COM
            // interfaces make `Session` !Send, so it drops where it was made).
            unsafe { CoUninitialize() };
        }
    }
}

/// One opened JPEG. Field order is drop order: COM objects go before the
/// bytes the stream reads without copying, and everything before `com`.
pub(super) struct Session {
    pub(super) frame: IWICBitmapFrameDecode,
    pub(super) factory: IWICImagingFactory,
    _keep: (IWICBitmapDecoder, IWICStream, Vec<u8>, ComGuard),
}

impl Session {
    pub(super) fn open(bytes: Vec<u8>) -> Result<Self> {
        let com = ComGuard::init()?;
        // SAFETY: COM is initialized on this thread by `com`.
        let factory = unsafe {
            CoCreateInstance::<_, IWICImagingFactory>(
                &CLSID_WICImagingFactory,
                None,
                CLSCTX_INPROC_SERVER,
            )
        };
        let factory = wic(factory, "CoCreateInstance")?;
        // SAFETY: `bytes` ends up in `_keep` after the stream and decoder, so
        // it outlives every reader; moving a Vec does not move its buffer. On
        // early returns the COM locals are declared later, so dropped first.
        let (stream, decoder, frame) = unsafe {
            let stream: IWICStream = wic(factory.CreateStream(), "CreateStream")?;
            wic(stream.InitializeFromMemory(&bytes), "InitializeFromMemory")?;
            let cache = WICDecodeMetadataCacheOnDemand;
            let decoder = factory.CreateDecoderFromStream(&stream, std::ptr::null(), cache);
            let decoder: IWICBitmapDecoder = wic(decoder, "CreateDecoderFromStream")?;
            let frame = wic(decoder.GetFrame(0), "GetFrame")?;
            (stream, decoder, frame)
        };
        let _keep = (decoder, stream, bytes, com);
        Ok(Self {
            frame,
            factory,
            _keep,
        })
    }

    pub(super) fn size(&self) -> Result<(u32, u32)> {
        let (mut w, mut h) = (0u32, 0u32);
        // SAFETY: both out-pointers are valid, live u32 locals.
        wic(unsafe { self.frame.GetSize(&mut w, &mut h) }, "GetSize")?;
        Ok((w, h))
    }

    /// The first ICC profile among the frame's color contexts. Color is a
    /// nicety, so any failure here means "no profile", never a failed decode.
    pub(super) fn icc(&self) -> Option<Vec<u8>> {
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
