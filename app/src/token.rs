//! Session token: 32 bytes from the OS CSPRNG, hex encoded.

/// Never time-seeded: the token is the only thing between a web page in the
/// user's browser and their photos (docs: server/src/guard.rs threat model).
pub fn session_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("OS randomness unavailable: {e}"))?;
    Ok(hex(&bytes))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}"); // writing to a String cannot fail
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_64_hex_chars_and_differs_per_call() {
        let (a, b) = (session_token().unwrap(), session_token().unwrap());
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn hex_is_zero_padded_lowercase() {
        assert_eq!(hex(&[0, 1, 0xab, 0xff]), "0001abff");
    }
}
