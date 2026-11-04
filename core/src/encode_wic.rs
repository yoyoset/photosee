//! JPEG encoding through Windows Imaging Component. Package C2.
//!
//! Why: `image`'s encoder takes ~150 ms for a 2160x1440 frame; WIC's takes
//! ~29 ms (release, `core/tests/pipeline_bench.rs`).
//!
//! The pinned `windows` crate gates `IWICBitmapEncoder::CreateNewFrame` and
//! `IWICBitmapFrameEncode::Initialize` (both mention `IPropertyBag2`) behind
//! `Win32_System_Com_StructuredStorage`, which the workspace manifest does
//! not enable (a full-workspace build gets it by feature unification, a
//! `-p photosee-core` build does not). So those two slots are called through
//! the vtable, found relative to their documented neighbours, and the
//! property bag is written through a hand-declared `IPropertyBag2::Write`.
//! All of that lives here; asking the orchestrator to enable the feature
//! would let it all go.
//!
//! Any failure returns an error and `encode::encode` falls back to `image`.

use crate::decode::jpeg_wic::com::{ComGuard, wic};
use crate::error::{Error, Result};
use crate::types::Rgba8;
use std::ffi::c_void;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Com::{IStream, STREAM_SEEK_CUR};
use windows::core::{GUID, HRESULT, IUnknown, Interface};

/// Room for the JPEG header and tables on top of the worst case (raw size).
const SLACK: usize = 64 * 1024;
const VT_R4: u16 = 4;
const PROPBAG2_TYPE_DATA: u32 = 1;

type NewFrameFn =
    unsafe extern "system" fn(*mut c_void, *mut *mut c_void, *mut *mut c_void) -> HRESULT;
type FrameInitFn = unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT;
type BagWriteFn =
    unsafe extern "system" fn(*mut c_void, u32, *mut PropBag2, *mut RawVariant) -> HRESULT;

/// `PROPBAG2` (propbag2.h), declared here because its module is gated.
#[repr(C)]
struct PropBag2 {
    dw_type: u32,
    vt: u16,
    cf_type: u16,
    dw_hint: u32,
    name: *mut u16,
    clsid: GUID,
}

/// `VARIANT` holding a float: the union is 16 bytes on x64 (BRECORD is two
/// pointers) and 8 on x86; 16 covers both, only the first 4 are read.
#[repr(C)]
struct RawVariant {
    vt: u16,
    reserved: [u16; 3],
    value: [u64; 2],
}

/// The vtable entry `back` slots before the field at `anchor`.
///
/// # Safety
/// `anchor` must point at a field of a vtable struct with at least `back`
/// pointer-sized slots before it, all of them function pointers of type `F`.
unsafe fn slot_before<F: Copy>(anchor: *const usize, back: usize) -> F {
    debug_assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<usize>());
    // SAFETY: per the contract; fn pointers and usize have the same size.
    unsafe { std::mem::transmute_copy(&*anchor.sub(back)) }
}

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
    // raw vtable calls match the documented signatures and are given
    // pointers owned by this function.
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
        let (frame, bag) = new_frame(&enc)?;
        set_quality(&bag, q)?;
        let anchor = &frame.vtable().SetSize as *const _ as *const usize;
        let finit: FrameInitFn = slot_before(anchor, 1);
        wic(finit(frame.as_raw(), bag.as_raw()).ok(), "Frame.Initialize")?;
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

/// `CreateNewFrame` through the vtable (gated in the crate). Returns the
/// frame and its encoder-options property bag, both owned.
///
/// # Safety
/// `enc` is a live encoder.
unsafe fn new_frame(enc: &IWICBitmapEncoder) -> Result<(IWICBitmapFrameEncode, IUnknown)> {
    let anchor = &enc.vtable().Commit as *const _ as *const usize;
    let (mut frame, mut bag) = (std::ptr::null_mut(), std::ptr::null_mut());
    // SAFETY: `CreateNewFrame` sits one slot before `Commit`; both out
    // pointers are valid locals, and on success they hold owned interfaces.
    let hr = unsafe {
        let f: NewFrameFn = slot_before(anchor, 1);
        f(enc.as_raw(), &mut frame, &mut bag)
    };
    wic(hr.ok(), "CreateNewFrame")?;
    if frame.is_null() || bag.is_null() {
        return Err(Error::corrupt("WIC CreateNewFrame returned null"));
    }
    // SAFETY: success and non-null: both are owned references.
    let pair = unsafe {
        (
            IWICBitmapFrameEncode::from_raw(frame),
            IUnknown::from_raw(bag),
        )
    };
    Ok(pair)
}

/// Writes `ImageQuality` (a float, 0..1) into the property bag.
///
/// # Safety
/// `bag` is a live `IPropertyBag2`.
unsafe fn set_quality(bag: &IUnknown, quality: u8) -> Result<()> {
    let mut name: Vec<u16> = "ImageQuality\0".encode_utf16().collect();
    let mut prop = PropBag2 {
        dw_type: PROPBAG2_TYPE_DATA,
        vt: VT_R4,
        cf_type: 0,
        dw_hint: 0,
        name: name.as_mut_ptr(),
        clsid: GUID::zeroed(),
    };
    let value = f32::from(quality.clamp(1, 100)) / 100.0;
    let mut var = RawVariant {
        vt: VT_R4,
        reserved: [0; 3],
        value: [u64::from(value.to_bits()), 0],
    };
    // SAFETY: IPropertyBag2 is IUnknown (3 slots) + Read + Write, so Write is
    // slot 4 (propbag2.h); the struct layouts match PROPBAG2 and VARIANT.
    let hr = unsafe {
        let vt = *(bag.as_raw() as *const *const usize);
        let write: BagWriteFn = std::mem::transmute_copy(&*vt.add(4));
        write(bag.as_raw(), 1, &mut prop, &mut var)
    };
    wic(hr.ok(), "IPropertyBag2.Write(ImageQuality)")
}
