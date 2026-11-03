//! HTTP Range support for passthrough responses (`bytes=a-b` single range
//! only; multi-range answers 200 with the full body). Package G.

/// Parses a Range header against a body of `len` bytes. `None` = serve all.
pub fn parse(header: &str, len: u64) -> Option<(u64, u64)> {
    let _ = (header, len);
    None // package G
}
