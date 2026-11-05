//! IO abstraction. Everything reads through `ReadAt` so RAW previews, EXIF
//! thumbnails and format sniffing touch only the bytes they need — on SMB a
//! whole-file read of a 50MB RAW costs ~0.5s. Contract file: orchestrator only.

use crate::error::{Error, ErrorCode, Result};
use crate::types::{Entry, EntryKind};
use std::path::Path;

/// Positional reads; implementations must be safe to call from many threads.
#[allow(clippy::len_without_is_empty)] // `len` is fallible IO, not a container size
pub trait ReadAt: Send + Sync {
    fn len(&self) -> Result<u64>;
    /// Reads up to `buf.len()` bytes at `offset`; returns bytes read (0 = EOF).
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize>;
}

/// Reads exactly `len` bytes at `offset`, or fails with `Corrupt` if the file
/// is shorter (a truncated file is a broken image, not an IO fault).
pub fn read_exact_at(src: &dyn ReadAt, offset: u64, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    let mut done = 0;
    while done < len {
        let n = src.read_at(offset + done as u64, &mut buf[done..])?;
        if n == 0 {
            return Err(Error::corrupt(format!(
                "unexpected end of file at {} (wanted {len} bytes at {offset})",
                offset + done as u64
            )));
        }
        done += n;
    }
    Ok(buf)
}

/// Reads the whole source. Only for formats whose decoders need it.
pub fn read_all(src: &dyn ReadAt) -> Result<Vec<u8>> {
    let len = usize::try_from(src.len()?)
        .map_err(|_| Error::new(ErrorCode::TooLarge, "file larger than address space"))?;
    read_exact_at(src, 0, len)
}

impl ReadAt for [u8] {
    fn len(&self) -> Result<u64> {
        Ok(<[u8]>::len(self) as u64)
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(<[u8]>::len(self));
        let n = buf.len().min(<[u8]>::len(self) - start);
        buf[..n].copy_from_slice(&self[start..start + n]);
        Ok(n)
    }
}

impl ReadAt for Vec<u8> {
    fn len(&self) -> Result<u64> {
        ReadAt::len(self.as_slice())
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.as_slice().read_at(offset, buf)
    }
}

impl ReadAt for std::fs::File {
    fn len(&self) -> Result<u64> {
        Ok(self.metadata()?.len())
    }
    #[cfg(windows)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        // seek_read moves the shared cursor, but every caller passes an
        // explicit offset, so concurrent use stays correct.
        Ok(std::os::windows::fs::FileExt::seek_read(self, buf, offset)?)
    }
    #[cfg(unix)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        Ok(std::os::unix::fs::FileExt::read_at(self, buf, offset)?)
    }
}

/// Filesystem access. Desktop/Docker use `LocalFs` (SMB shares appear as
/// normal paths); mobile will add an SMB client backend in M4.
pub trait FsBackend: Send + Sync {
    /// Raw, unsorted, unfiltered entries of `dir`. Images are recognized by
    /// extension only; `scan` filters and sorts.
    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>>;
    fn open(&self, path: &Path) -> Result<Box<dyn ReadAt>>;
    fn stat(&self, path: &Path) -> Result<Entry>;
}

pub struct LocalFs;

impl FsBackend for LocalFs {
    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        for item in std::fs::read_dir(dir)? {
            let item = item?;
            // On Windows/SMB, DirEntry::metadata comes from the directory
            // listing itself — no extra round trip per file.
            let Ok(meta) = item.metadata() else { continue };
            let name = item.file_name().to_string_lossy().into_owned();
            if let Some(e) = entry_from(name, &meta) {
                out.push(e);
            }
        }
        Ok(out)
    }

    fn open(&self, path: &Path) -> Result<Box<dyn ReadAt>> {
        Ok(Box::new(std::fs::File::open(path)?))
    }

    fn stat(&self, path: &Path) -> Result<Entry> {
        let meta = std::fs::metadata(path)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        entry_from(name, &meta).ok_or_else(|| {
            Error::unsupported(format!("not an image or directory: {}", path.display()))
        })
    }
}

fn entry_from(name: String, meta: &std::fs::Metadata) -> Option<Entry> {
    // A comic archive is listed as a folder; `archive::ArchiveFs` opens it.
    // Its size stays the file length so the index cache can key on it.
    let kind = if meta.is_dir() || crate::archive::is_archive_name(&name) {
        EntryKind::Dir
    } else {
        EntryKind::Image(crate::format::from_extension(&name)?)
    };
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    Some(Entry {
        name,
        kind,
        size: if meta.is_dir() { 0 } else { meta.len() },
        mtime_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_exact_at_reports_truncation_as_corrupt() {
        let data = vec![1u8, 2, 3];
        assert_eq!(read_exact_at(&data, 1, 2).unwrap(), vec![2, 3]);
        let err = read_exact_at(&data, 2, 5).unwrap_err();
        assert_eq!(err.code, ErrorCode::Corrupt);
    }

    #[test]
    fn slice_read_past_end_returns_zero() {
        let data = vec![1u8, 2, 3];
        let mut buf = [0u8; 4];
        assert_eq!(data.read_at(10, &mut buf).unwrap(), 0);
    }
}
