//! Test helper: builds HEIC files at test time with the WIC HEIF encoder
//! (present when the HEVC Video Extensions are installed). Other heic_*.rs
//! tests pull it in with `#[path]`; its own test keeps every item used.
//!
//! Pitfall: `IWICBitmapEncoder::CreateNewFrame` and
//! `IWICBitmapFrameEncode::Initialize` take an `IPropertyBag2`, so the
//! windows crate only exposes them with the `Win32_System_Com_StructuredStorage`
//! feature, which the workspace does not enable (see QUESTIONS.md). Until it
//! does, both are called through their vtable slots, and each slot is
//! cross-checked against its public neighbours before use.
#![cfg(windows)]

use std::ffi::c_void;
use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatHeif, GUID_WICPixelFormat32bppBGRA,
    IWICBitmapEncoder, IWICBitmapFrameEncode, IWICImagingFactory, WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::{HRESULT, HSTRING, Interface};

/// Keeps COM initialized for the test thread's encoder calls.
pub struct Com(bool);

impl Com {
    pub fn init() -> Self {
        // SAFETY: no reserved pointer; balanced in Drop when it succeeded.
        Self(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: matches the successful CoInitializeEx in `init`.
            unsafe { CoUninitialize() };
        }
    }
}

/// 64x48 BGRA test image: red top-left quadrant, blue elsewhere. Strong
/// colours survive HEVC's 4:2:0 chroma subsampling and lossy coding.
pub fn quadrant_bgra(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let red = x < w / 2 && y < h / 2;
            px.extend_from_slice(if red {
                &[0, 0, 255, 255]
            } else {
                &[255, 0, 0, 255]
            });
        }
    }
    px
}

type NewFrameFn =
    unsafe extern "system" fn(*mut c_void, *mut *mut c_void, *mut *mut c_void) -> HRESULT;
type FrameInitFn = unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT;

/// Reads vtable slot `i` of a COM object.
///
/// # Safety
/// `obj` must be a live COM interface whose vtable has more than `i` slots.
unsafe fn slot(obj: &impl Interface, i: usize) -> usize {
    // SAFETY: a COM object pointer points at its vtable pointer, and the
    // caller guarantees the slot exists.
    unsafe { *(*(obj.as_raw() as *const *const usize)).add(i) }
}

fn new_frame(enc: &IWICBitmapEncoder) -> windows::core::Result<IWICBitmapFrameEncode> {
    // SAFETY: wincodec.h orders IWICBitmapEncoder as IUnknown (0-2),
    // Initialize, GetContainerFormat, GetEncoderInfo, SetColorContexts,
    // SetPalette, SetThumbnail, SetPreview (9), CreateNewFrame (10), Commit
    // (11). The asserts prove 9 and 11 are where we think, so 10 is
    // CreateNewFrame. A null property-bag out-pointer is allowed.
    unsafe {
        assert_eq!(slot(enc, 9), enc.vtable().SetPreview as usize);
        assert_eq!(slot(enc, 11), enc.vtable().Commit as usize);
        let f: NewFrameFn = std::mem::transmute(slot(enc, 10));
        let mut raw = std::ptr::null_mut();
        f(enc.as_raw(), &mut raw, std::ptr::null_mut()).ok()?;
        Ok(IWICBitmapFrameEncode::from_raw(raw))
    }
}

fn init_frame(frame: &IWICBitmapFrameEncode) -> windows::core::Result<()> {
    // SAFETY: IWICBitmapFrameEncode is IUnknown (0-2), Initialize (3),
    // SetSize (4). The asserts pin slots 2 and 4. A null IPropertyBag2
    // means default encoder options.
    unsafe {
        assert_eq!(slot(frame, 2), frame.vtable().base__.Release as usize);
        assert_eq!(slot(frame, 4), frame.vtable().SetSize as usize);
        let f: FrameInitFn = std::mem::transmute(slot(frame, 3));
        f(frame.as_raw(), std::ptr::null_mut()).ok()
    }
}

/// Encodes BGRA pixels as HEIC. `Err` usually means no HEIF encoder (the
/// HEVC extension is missing); callers skip with a note.
pub fn encode_heic(w: u32, h: u32, bgra: &[u8]) -> windows::core::Result<Vec<u8>> {
    let _com = Com::init();
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("t.heic");
    // SAFETY: COM is initialized; every pointer argument is a live local
    // or null where the API allows it; `bgra` holds w * h * 4 bytes.
    unsafe {
        let f: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = f.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), GENERIC_WRITE.0)?;
        let enc = f.CreateEncoder(&GUID_ContainerFormatHeif, std::ptr::null())?;
        enc.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let frame = new_frame(&enc)?;
        init_frame(&frame)?;
        frame.SetSize(w, h)?;
        let src = f.CreateBitmapFromMemory(w, h, &GUID_WICPixelFormat32bppBGRA, w * 4, bgra)?;
        frame.WriteSource(&src, std::ptr::null())?;
        frame.Commit()?;
        enc.Commit()?;
    }
    Ok(std::fs::read(&path).expect("read encoded HEIC"))
}

fn be32(b: &[u8], at: usize) -> Option<usize> {
    let s = b.get(at..at + 4)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

/// (offset, size) of the first child box of type `t` in `b[from..to]`.
fn find_box(b: &[u8], from: usize, to: usize, t: &[u8; 4]) -> Option<(usize, usize)> {
    let mut at = from;
    while at + 8 <= to {
        let size = be32(b, at)?;
        if size < 8 {
            return None;
        }
        if b.get(at + 4..at + 8)? == t {
            return Some((at, size));
        }
        at += size;
    }
    None
}

fn grow(b: &mut [u8], at: usize, by: usize) {
    let size = be32(b, at).expect("box size") + by;
    b[at..at + 4].copy_from_slice(&(size as u32).to_be_bytes());
}

/// Adds transformative properties (`irot`, `imir`; 1-byte payload each) to
/// the primary item, the way an iPhone records rotation. Works on the WIC
/// encoder's layout: one item, ipma v0 with 1-byte entries, and a `free`
/// box between `meta` and `mdat` that we shrink by exactly what we add, so
/// `iloc`'s absolute offsets into `mdat` stay valid.
pub fn add_transforms(heic: &[u8], props: &[(&[u8; 4], u8)]) -> Option<Vec<u8>> {
    let (meta, meta_len) = find_box(heic, 0, heic.len(), b"meta")?;
    let (iprp, iprp_len) = find_box(heic, meta + 12, meta + meta_len, b"iprp")?;
    let (ipco, ipco_len) = find_box(heic, iprp + 8, iprp + iprp_len, b"ipco")?;
    let (ipma, ipma_len) = find_box(heic, iprp + 8, iprp + iprp_len, b"ipma")?;
    let (free, free_len) = find_box(heic, meta + meta_len, heic.len(), b"free")?;
    let added = props.len() * 10;
    let ok = ipma == ipco + ipco_len && be32(heic, ipma + 12)? == 1 && heic[ipma + 11] & 1 == 0;
    if !ok || free != meta + meta_len || free_len < added + 8 {
        return None;
    }
    let first_new = ipco_box_count(heic, ipco, ipco_len)? + 1;
    let mut out = heic[..ipco + ipco_len].to_vec();
    for (t, v) in props {
        out.extend_from_slice(&[0, 0, 0, 9]);
        out.extend_from_slice(*t);
        out.push(*v);
    }
    let count_at = ipma + 18;
    out.extend_from_slice(&heic[ipma..ipma + ipma_len]);
    for i in 0..props.len() {
        out.push(0x80 | (first_new + i) as u8); // essential, as transforms must be
    }
    let shift = props.len() * 9;
    out[count_at + shift] += props.len() as u8;
    grow(&mut out, ipma + shift, props.len());
    grow(&mut out, ipco, shift);
    grow(&mut out, iprp, added);
    grow(&mut out, meta, added);
    out.extend_from_slice(&((free_len - added) as u32).to_be_bytes());
    out.extend_from_slice(b"free");
    out.extend_from_slice(&heic[free + 8 + added..]);
    Some(out)
}

fn ipco_box_count(b: &[u8], ipco: usize, len: usize) -> Option<usize> {
    let (mut at, mut n) = (ipco + 8, 0);
    while at < ipco + len {
        at += be32(b, at)?.max(8);
        n += 1;
    }
    Some(n)
}

#[test]
fn encoder_and_patcher_work() {
    let bytes = match encode_heic(64, 48, &quadrant_bgra(64, 48)) {
        Ok(b) => b,
        Err(e) => return println!("skipped: no WIC HEIF encoder ({e})"),
    };
    assert_eq!(&bytes[4..8], b"ftyp");
    println!("WIC HEIF encoder: 64x48 -> {} bytes", bytes.len());
    let patched = add_transforms(&bytes, &[(b"irot", 1), (b"imir", 0)]).expect("WIC layout");
    // Same length and an untouched mdat: iloc offsets are still right.
    assert_eq!(patched.len(), bytes.len());
    let mdat = find_box(&bytes, 0, bytes.len(), b"mdat").expect("mdat");
    assert_eq!(find_box(&patched, 0, patched.len(), b"mdat"), Some(mdat));
    assert!(patched.windows(5).any(|w| w == b"irot"));
}
