//! Test-only synthetic files, one per RAW kind, each shaped like the real
//! layout (decoy thumbnails included) so the locator has to pick right.

use super::build::{IFD, LONG, SHORT, TiffB, UNDEF, bx, cat, full, jpeg, jpeg_with_orientation};
use crate::types::RawKind;

/// A synthetic file and what the locator must find in it.
pub struct Sample {
    pub data: Vec<u8>,
    pub off: u64,
    pub len: u64,
    pub w: u32,
    pub h: u32,
    /// Expected `PreviewLoc.orientation` (EXIF value): what is still to
    /// apply on top of the JPEG's own tag.
    pub orientation: u16,
}

fn done(t: TiffB, off: u32, j: &[u8], w: u32, h: u32, o: u16) -> Sample {
    let len = j.len() as u64;
    Sample {
        data: t.buf,
        off: u64::from(off),
        len,
        w,
        h,
        orientation: o,
    }
}

pub fn cr2() -> Sample {
    let (big, thumb) = (jpeg(64, 48), jpeg(16, 12));
    let mut t = TiffB::new(true, 42);
    let (b, s) = (t.blob(&big), t.blob(&thumb));
    let ifd1 = t.ifd(
        &[(0x0201, LONG, 1, s), (0x0202, LONG, 1, thumb.len() as u32)],
        0,
    );
    let ifd0 = t.ifd(
        &[
            (0x0103, SHORT, 1, 6),
            (0x0111, LONG, 1, b),
            (0x0112, SHORT, 1, 8),
            (0x0117, LONG, 1, big.len() as u32),
        ],
        ifd1,
    );
    t.set_ifd0(ifd0);
    done(t, b, &big, 64, 48, 8)
}

pub fn arw() -> Sample {
    let j = jpeg(40, 24);
    let mut t = TiffB::new(true, 42);
    let b = t.blob(&j);
    let ifd0 = t.ifd(
        &[(0x0201, LONG, 1, b), (0x0202, LONG, 1, j.len() as u32)],
        0,
    );
    t.set_ifd0(ifd0);
    done(t, b, &j, 40, 24, 1)
}

/// Big-endian, three SubIFDs; the biggest preview hides behind a SubIFD's
/// next pointer (a SubIFD chain), the raw is NewSubFileType=0.
pub fn nef() -> Sample {
    let (big, mid, raw) = (jpeg(72, 40), jpeg(24, 16), jpeg(80, 48));
    let mut t = TiffB::new(false, 42);
    let (bo, mo, ro) = (t.blob(&big), t.blob(&mid), t.blob(&raw));
    let jif = |t: &mut TiffB, nsft: u32, off: u32, len: usize, next: u32| {
        let e = [
            (0x00FE, LONG, 1, nsft),
            (0x0201, LONG, 1, off),
            (0x0202, LONG, 1, len as u32),
        ];
        t.ifd(&e, next)
    };
    let s_big = jif(&mut t, 1, bo, big.len(), 0);
    let s_mid = jif(&mut t, 1, mo, mid.len(), s_big);
    let s_raw = jif(&mut t, 0, ro, raw.len(), 0);
    let arr = t.u32s(&[s_mid, s_raw]);
    let ifd0 = t.ifd(&[(0x0112, SHORT, 1, 6), (0x014A, LONG, 2, arr)], 0);
    t.set_ifd0(ifd0);
    done(t, bo, &big, 72, 40, 6)
}

pub fn dng() -> Sample {
    let (prev, raw) = (jpeg(56, 32), jpeg(80, 48));
    let mut t = TiffB::new(true, 42);
    let (po, ro) = (t.blob(&prev), t.blob(&raw));
    let strip = |t: &mut TiffB, nsft: u32, comp: u32, off: u32, len: usize| {
        let e = [
            (0x00FE, LONG, 1, nsft),
            (0x0103, SHORT, 1, comp),
            (0x0111, LONG, 1, off),
            (0x0117, LONG, 1, len as u32),
        ];
        t.ifd(&e, 0)
    };
    let s_raw = strip(&mut t, 0, 7, ro, raw.len());
    let s_prev = strip(&mut t, 1, 7, po, prev.len());
    let arr = t.u32s(&[s_raw, s_prev]);
    // IFD0 itself is the uncompressed thumbnail, as in Adobe's converter.
    let ifd0 = t.ifd(
        &[
            (0x00FE, LONG, 1, 1),
            (0x0103, SHORT, 1, 1),
            (0x0112, SHORT, 1, 3),
            (0x014A, LONG, 2, arr),
        ],
        0,
    );
    t.set_ifd0(ifd0);
    done(t, po, &prev, 56, 32, 3)
}

pub fn rw2() -> Sample {
    let j = jpeg(48, 32);
    let mut t = TiffB::new(true, 0x55);
    let b = t.blob(&j);
    let ifd0 = t.ifd(
        &[(0x002E, UNDEF, j.len() as u32, b), (0x0112, SHORT, 1, 1)],
        0,
    );
    t.set_ifd0(ifd0);
    done(t, b, &j, 48, 32, 1)
}

/// Olympus "OLYMPUS\0II" MakerNote; offsets inside it are MakerNote-relative.
pub fn orf() -> Sample {
    let j = jpeg(40, 32);
    let mut mn = TiffB::raw(true, b"OLYMPUS\0II\x03\x00");
    let main = mn.ifd(&[(0x2020, IFD, 1, 0)], 0);
    let jo = mn.blob(&j);
    let cs = mn.ifd(
        &[(0x0101, LONG, 1, jo), (0x0102, LONG, 1, j.len() as u32)],
        0,
    );
    mn.patch32(TiffB::value_at(main, 0), cs);
    let mut t = TiffB::new(true, 0x4F52);
    let mo = t.blob(&mn.buf);
    let exif = t.ifd(&[(0x927C, UNDEF, mn.buf.len() as u32, mo)], 0);
    let ifd0 = t.ifd(&[(0x0112, SHORT, 1, 6), (0x8769, LONG, 1, exif)], 0);
    t.set_ifd0(ifd0);
    done(t, mo + jo, &j, 40, 32, 6)
}

pub fn raf() -> Sample {
    let j = jpeg_with_orientation(32, 24, 6);
    let mut d = b"FUJIFILMCCD-RAW 0201FF383501".to_vec();
    d.resize(84, 0);
    let off = 160u32;
    d.extend_from_slice(&off.to_be_bytes());
    d.extend_from_slice(&(j.len() as u32).to_be_bytes());
    d.resize(off as usize, 0);
    d.extend_from_slice(&j);
    d.extend_from_slice(&[0u8; 64]); // CFA header/data would follow
    Sample {
        data: d,
        off: u64::from(off),
        len: j.len() as u64,
        w: 32,
        h: 24,
        orientation: 1, // the JPEG's own tag (6) carries it
    }
}

pub const CANON_UUID: [u8; 16] = [
    0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a, 0x48,
];

fn trak(off: u64, len: usize) -> Vec<u8> {
    let stsz = full(
        b"stsz",
        &cat(&[
            &0u32.to_be_bytes(),
            &1u32.to_be_bytes(),
            &(len as u32).to_be_bytes(),
        ]),
    );
    let co64 = full(b"co64", &cat(&[&1u32.to_be_bytes(), &off.to_be_bytes()]));
    let stbl = bx(
        b"stbl",
        &cat(&[&full(b"stsd", &[0, 0, 0, 0]), &stsz, &co64]),
    );
    bx(b"trak", &bx(b"mdia", &bx(b"minf", &stbl)))
}

/// CR3 with a decoy trak 1 (small JPEG) and PreviewBig in trak 2 per CCDT;
/// `ccdt: false` leaves CCTP empty to exercise the trak-1 fallback.
pub fn cr3_with(ccdt: bool) -> Sample {
    let (small, big) = (jpeg(20, 12), jpeg(64, 40));
    let canon = canon_box(ccdt);
    let ftyp = bx(b"ftyp", b"crx \0\0\0\x01crx isom");
    let moov_for = |a: u64, b: u64| {
        bx(
            b"moov",
            &cat(&[&canon, &trak(a, small.len()), &trak(b, big.len())]),
        )
    };
    let data_at = (ftyp.len() + moov_for(0, 0).len() + 8) as u64;
    let big_at = data_at + small.len() as u64;
    let moov = moov_for(data_at, big_at);
    let data = cat(&[&ftyp, &moov, &bx(b"mdat", &cat(&[&small, &big]))]);
    let (off, len, w, h) = if ccdt {
        (big_at, big.len(), 64, 40)
    } else {
        (data_at, small.len(), 20, 12)
    };
    Sample {
        data,
        off,
        len: len as u64,
        w,
        h,
        orientation: 8,
    }
}

/// Canon uuid box: CNCV, CCTP (PreviewBig in trak 2 when `ccdt`) and a
/// CMT1 TIFF carrying Orientation=8.
fn canon_box(ccdt: bool) -> Vec<u8> {
    let mut cmt1 = TiffB::new(true, 42);
    let i0 = cmt1.ifd(&[(0x0112, SHORT, 1, 8)], 0);
    cmt1.set_ifd0(i0);
    let rec = |ty: u64, n: u32| {
        bx(
            b"CCDT",
            &cat(&[&ty.to_be_bytes(), &0u32.to_be_bytes(), &n.to_be_bytes()]),
        )
    };
    let recs = if ccdt {
        cat(&[&rec(0, 3), &rec(2, 2)])
    } else {
        Vec::new()
    };
    let cctp = full(
        b"CCTP",
        &cat(&[&0u32.to_be_bytes(), &2u32.to_be_bytes(), &recs]),
    );
    bx(
        b"uuid",
        &cat(&[
            &CANON_UUID,
            &bx(b"CNCV", b"CanonCR3_001"),
            &cctp,
            &bx(b"CMT1", &cmt1.buf),
        ]),
    )
}

pub fn sample(kind: RawKind) -> Sample {
    match kind {
        RawKind::Cr2 => cr2(),
        RawKind::Cr3 => cr3_with(true),
        RawKind::Nef => nef(),
        RawKind::Arw => arw(),
        RawKind::Dng => dng(),
        RawKind::Orf => orf(),
        RawKind::Rw2 => rw2(),
        RawKind::Raf => raf(),
    }
}

pub const ALL: [RawKind; 8] = [
    RawKind::Cr2,
    RawKind::Cr3,
    RawKind::Nef,
    RawKind::Arw,
    RawKind::Dng,
    RawKind::Orf,
    RawKind::Rw2,
    RawKind::Raf,
];
