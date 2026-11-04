//! JPEG encoding through Windows Imaging Component. Package C2.
//!
//! Why: `image`'s encoder takes ~150 ms for a 2160x1440 frame; WIC's takes
//! ~29 ms (release, `core/tests/pipeline_bench.rs`).
//!
//! Any failure returns an error and `encode::encode` falls back to `image`.

use crate::decode::jpeg_wic::com::{ComGuard, wic};
use crate::error::{Error, Result};
use crate::types::Rgba8;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::StructuredStorage::{IPropertyBag2, PROPBAG2};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Com::{IStream, STREAM_SEEK_CUR};
use windows::Win32::System::Ole::PROPBAG2_TYPE_DATA;
use windows::Win32::System::Variant::{VARIANT, VT_R4};
use windows::core::{Interface, PWSTR};

/// Room for the JPEG header and tables on top of the worst case (raw size).
const SLACK: usize = 64 * 1024;

/// RGBA (opaque, checked by the caller) -> tightly packed BGR, the JPEG
/// codec's native layout.
fn rgba_to_bgr(img: &Rgba8) -> Vec<u8> {
    let mut out = vec![0u8; img.data.len() / 4 * 3];
    for (d, s) in out.chunks_exact_mut(3).zip(img.data.chunks_exact(4)) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
    }
    out
}

/// Opaque RGBA to JPEG bytes at `quality` (1..=100).
pub fn jpeg(img: &Rgba8, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (img.size.w, img.size.h);
    let stride = w
        .checked_mul(3)
        .ok_or_else(|| Error::corrupt("encode: width overflows"))?;
    let _com = ComGuard::init()?;
    let bgr = rgba_to_bgr(img);
    // Never written through a Rust reference: WIC gets the raw pointer.
    let mut out = vec![0u8; bgr.len() + SLACK];
    let len = u32::try_from(out.len()).map_err(|_| Error::corrupt("encode: output too large"))?;
    // SAFETY: COM is initialized by `_com`; `out` outlives every COM object
    // made inside `write` (they are dropped when it returns) and is only
    // touched again after that.
    let written = unsafe { write(out.as_mut_ptr(), len, (w, h, stride), quality, &bgr)? };
    out.truncate(written);
    Ok(out)
}

/// # Safety
/// COM initialized on this thread; `buf` points at `len` writable bytes that
/// stay valid and untouched by Rust until this returns.
unsafe fn write(buf: *mut u8, len: u32, dim: (u32, u32, u32), q: u8, bgr: &[u8]) -> Result<usize> {
    let (w, h, stride) = dim;
    // SAFETY (whole body): plain factory/interface calls on live objects; the
    // stream gets the raw buffer pointer (see `jpeg`).
    unsafe {
        let factory = CoCreateInstance::<_, IWICImagingFactory>(
            &CLSID_WICImagingFactory,
            None,
            CLSCTX_INPROC_SERVER,
        );
        let factory = wic(factory, "CoCreateInstance")?;
        let stream = wic(factory.CreateStream(), "CreateStream")?;
        let init = (stream.vtable().InitializeFromMemory)(stream.as_raw(), buf, len);
        wic(init.ok(), "InitializeFromMemory")?;
        let enc = factory.CreateEncoder(&GUID_ContainerFormatJpeg, std::ptr::null());
        let enc = wic(enc, "CreateEncoder")?;
        let istream: IStream = wic(stream.cast(), "QueryInterface")?;
        wic(
            enc.Initialize(&istream, WICBitmapEncoderNoCache),
            "Encoder.Initialize",
        )?;
        let (mut frame, mut bag) = (None, None);
        wic(enc.CreateNewFrame(&mut frame, &mut bag), "CreateNewFrame")?;
        let (Some(frame), Some(bag)) = (frame, bag) else {
            return Err(Error::corrupt("WIC CreateNewFrame returned null"));
        };
        set_quality(&bag, q)?;
        wic(frame.Initialize(&bag), "Frame.Initialize")?;
        wic(frame.SetSize(w, h), "SetSize")?;
        let mut fmt = GUID_WICPixelFormat24bppBGR;
        wic(frame.SetPixelFormat(&mut fmt), "SetPixelFormat")?;
        if fmt != GUID_WICPixelFormat24bppBGR {
            return Err(Error::unsupported(
                "WIC JPEG encoder wants another pixel format",
            ));
        }
        wic(frame.WritePixels(h, stride, bgr), "WritePixels")?;
        wic(frame.Commit(), "Frame.Commit")?;
        wic(enc.Commit(), "Encoder.Commit")?;
        let mut pos = 0u64;
        wic(istream.Seek(0, STREAM_SEEK_CUR, Some(&mut pos)), "Seek")?;
        usize::try_from(pos)
            .ok()
            .filter(|&p| p > 0 && p <= len as usize)
            .ok_or_else(|| Error::corrupt("WIC JPEG: bad output length"))
    }
}

/// Writes `ImageQuality` (a float, 0..1) into the encoder-options bag.
fn set_quality(bag: &IPropertyBag2, quality: u8) -> Result<()> {
    let mut name: Vec<u16> = "ImageQuality\0".encode_utf16().collect();
    let prop = PROPBAG2 {
        dwType: PROPBAG2_TYPE_DATA.0 as u32,
        vt: VT_R4,
        pstrName: PWSTR(name.as_mut_ptr()),
        ..Default::default()
    };
    let value = VARIANT::from(f32::from(quality.clamp(1, 100)) / 100.0);
    // SAFETY: one property and one value, both live locals; `name` outlives
    // the call.
    wic(
        unsafe { bag.Write(1, &prop, &value) },
        "IPropertyBag2.Write(ImageQuality)",
    )
}
