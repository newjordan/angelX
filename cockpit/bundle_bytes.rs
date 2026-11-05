//! Lossless storage for embedded browser assets, decoded only on demand.

/// `None` means the bytes are already plain; `Some(n)` bounds decoded output.
// The build script encodes; the runtime decodes. Each compiles this source.
#[allow(dead_code)]
pub fn pack(plain: &[u8]) -> (Vec<u8>, Option<usize>) {
    if plain.len() >= 4096 {
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(plain, 1);
        if packed.len() < plain.len() {
            return (packed, Some(plain.len()));
        }
    }
    (plain.to_vec(), None)
}

#[allow(dead_code)]
pub fn unpack(stored: &[u8], original_len: Option<usize>) -> Result<Vec<u8>, String> {
    let Some(original_len) = original_len else {
        return Ok(stored.to_vec());
    };
    let plain = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(stored, original_len)
        .map_err(|error| format!("invalid packed browser asset: {error:?}"))?;
    if plain.len() != original_len {
        return Err(format!(
            "browser asset length mismatch: {} != {original_len}",
            plain.len()
        ));
    }
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressible_assets_shrink_and_round_trip_exactly() {
        let plain = b"\0asm:the same browser page and executable bytes".repeat(8192);
        let (stored, original_len) = pack(&plain);
        assert!(
            stored.len() < plain.len() / 2,
            "large repeated input must shrink"
        );
        assert_eq!(original_len, Some(plain.len()));
        assert_eq!(unpack(&stored, original_len).unwrap(), plain);
    }

    #[test]
    fn small_and_empty_assets_stay_plain() {
        for plain in [b"".as_slice(), b"<html>same page</html>"] {
            let (stored, original_len) = pack(plain);
            assert_eq!(original_len, None);
            assert_eq!(unpack(&stored, original_len).unwrap(), plain);
        }
    }

    #[test]
    fn unhelpful_compression_keeps_original_bytes() {
        let mut state = 0x1234_5678_u32;
        let plain: Vec<u8> = (0..32768)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        let (stored, original_len) = pack(&plain);
        assert_eq!(original_len, None);
        assert_eq!(stored, plain);
    }

    #[test]
    fn corrupt_truncated_and_wrong_length_assets_are_errors() {
        let plain = b"lossless browser bundle".repeat(4096);
        let (stored, original_len) = pack(&plain);
        assert!(unpack(b"not zlib", Some(100)).is_err());
        assert!(unpack(&stored[..stored.len() / 2], original_len).is_err());
        assert!(unpack(&stored, Some(plain.len() - 1)).is_err());
        assert!(unpack(&stored, Some(plain.len() + 1)).is_err());
    }
}
