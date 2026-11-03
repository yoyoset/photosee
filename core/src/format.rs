//! Format identification. `from_extension` is part of the contract (listing
//! depends on it); `sniff` and `is_browser_native` belong to package C.

use crate::types::{Format, RawKind};

/// Bytes `sniff` needs. CR3/AVIF/HEIC brands sit inside the first ftyp box.
pub const SNIFF_LEN: usize = 64;

/// Guess from the file name. Case-insensitive. `None` = not an image we show.
pub fn from_extension(name: &str) -> Option<Format> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" => Format::Jpeg,
        "png" => Format::Png,
        "gif" => Format::Gif,
        "webp" => Format::Webp,
        "avif" => Format::Avif,
        "bmp" => Format::Bmp,
        "ico" => Format::Ico,
        "svg" => Format::Svg,
        "tif" | "tiff" => Format::Tiff,
        "jxl" => Format::Jxl,
        "psd" => Format::Psd,
        "heic" | "heif" | "hif" => Format::Heic,
        "cr2" => Format::Raw(RawKind::Cr2),
        "cr3" => Format::Raw(RawKind::Cr3),
        "nef" | "nrw" => Format::Raw(RawKind::Nef),
        "arw" | "srf" | "sr2" => Format::Raw(RawKind::Arw),
        "dng" => Format::Raw(RawKind::Dng),
        "orf" => Format::Raw(RawKind::Orf),
        "rw2" => Format::Raw(RawKind::Rw2),
        "raf" => Format::Raw(RawKind::Raf),
        _ => return None,
    })
}

/// Identify by magic bytes; `head` is the first `SNIFF_LEN` bytes (may be
/// shorter for tiny files). Extension decides only when magic is ambiguous
/// (TIFF-based RAWs share the TIFF header).
pub fn sniff(_head: &[u8], _ext_guess: Option<Format>) -> Option<Format> {
    None // package C
}

/// Formats every supported webview decodes natively, regardless of `Accept`.
pub fn always_native(f: Format) -> bool {
    matches!(
        f,
        Format::Jpeg | Format::Png | Format::Gif | Format::Bmp | Format::Ico | Format::Svg
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_case_insensitive_and_strict() {
        assert_eq!(from_extension("A.JPG"), Some(Format::Jpeg));
        assert_eq!(from_extension("x.CR3"), Some(Format::Raw(RawKind::Cr3)));
        assert_eq!(from_extension("notes.txt"), None);
        assert_eq!(from_extension("jpg"), None);
    }
}
