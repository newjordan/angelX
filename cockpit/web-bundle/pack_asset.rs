//! Reuse a packed asset only after decoding and comparing every current byte.
//! No timestamp, digest or sidecar is a freshness authority: nested Cargo still
//! builds first, and a missing, corrupt or different packed file is regenerated.

use std::{fs, io::{self, Read}, path::Path};

use crate::bundle_bytes;

fn previous_encoding(path: &Path, plain_len: usize) -> Option<Vec<u8>> {
    // OUT_DIR is Cargo-owned and builds serialize its writers. Reject known
    // non-regular slots before open, and bound the read even if a file grows.
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() >= plain_len as u64 {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() >= plain_len as u64 {
        return None;
    }
    let mut stored = Vec::new();
    file.take(plain_len as u64).read_to_end(&mut stored).ok()?;
    (stored.len() < plain_len).then_some(stored)
}

/// Store a beneficial zlib encoding, or keep the caller's plain asset. The
/// returned length is the existing runtime decoder's exact output bound.
pub fn pack_asset(plain: &[u8], packed_path: &Path) -> io::Result<Option<usize>> {
    if plain.len() < 4096 {
        return Ok(None);
    }
    if let Some(stored) = previous_encoding(packed_path, plain.len()) {
        if bundle_bytes::unpack(&stored, Some(plain.len())).is_ok_and(|decoded| decoded == plain)
        {
            return Ok(Some(plain.len()));
        }
    }
    let (stored, original_len) = bundle_bytes::pack(plain);
    if let Some(len) = original_len {
        // A damaged slot must not turn the fallback write into a FIFO open or
        // a write through a symlink. OUT_DIR itself is Cargo-owned, not hostile.
        if fs::symlink_metadata(packed_path).is_ok_and(|metadata| !metadata.is_file()) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "packed browser asset slot is not a regular file"));
        }
        let decoded = bundle_bytes::unpack(&stored, Some(len))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if decoded != plain {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "packed browser asset differs from its input"));
        }
        fs::write(packed_path, stored)?;
    }
    Ok(original_len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("angel-pack-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn slot(&self) -> std::path::PathBuf { self.0.join("0.zlib") }
    }
    impl Drop for Scratch {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn unchanged_asset_reuses_exact_verified_storage_without_writing() {
        let dir = Scratch::new();
        let path = dir.slot();
        let plain = b"unchanged WASM and sound bytes".repeat(4096);
        // A valid alternative encoding proves reuse rather than recompression
        // at pack()'s level 1. A fixed old mtime proves no write took place.
        let stored = miniz_oxide::deflate::compress_to_vec_zlib(&plain, 9);
        assert_ne!(stored, bundle_bytes::pack(&plain).0);
        fs::write(&path, &stored).unwrap();
        let old = UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
        assert_eq!(pack_asset(&plain, &path).unwrap(), Some(plain.len()));
        assert_eq!(fs::read(&path).unwrap(), stored);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
    }

    #[test]
    fn missing_changed_reordered_corrupt_and_truncated_slots_regenerate() {
        let dir = Scratch::new();
        let path = dir.slot();
        let plain = b"a".repeat(32768);
        let other = b"b".repeat(plain.len()); // same length is not freshness
        let old = bundle_bytes::pack(&other).0;
        for previous in [None, Some(old.clone()), Some(b"invalid zlib".to_vec()), Some(old[..old.len() / 2].to_vec())] {
            if let Some(previous) = previous {
                fs::write(&path, previous).unwrap();
            }
            assert_eq!(pack_asset(&plain, &path).unwrap(), Some(plain.len()));
            assert_eq!(bundle_bytes::unpack(&fs::read(&path).unwrap(), Some(plain.len())).unwrap(), plain);
            fs::remove_file(&path).unwrap();
        }
    }

    #[test]
    fn larger_old_asset_is_bounded_and_replaced() {
        let dir = Scratch::new();
        let path = dir.slot();
        fs::write(&path, bundle_bytes::pack(&b"z".repeat(1_000_000)).0).unwrap();
        let plain = b"x".repeat(4096);
        assert_eq!(pack_asset(&plain, &path).unwrap(), Some(plain.len()));
        assert_eq!(bundle_bytes::unpack(&fs::read(path).unwrap(), Some(plain.len())).unwrap(), plain);
    }

    #[test]
    fn oversized_stored_file_is_not_read_or_reused() {
        let dir = Scratch::new();
        let path = dir.slot();
        fs::File::create(&path).unwrap().set_len(128 * 1024 * 1024).unwrap();
        let plain = b"x".repeat(4096);
        assert!(previous_encoding(&path, plain.len()).is_none());
        assert_eq!(pack_asset(&plain, &path).unwrap(), Some(plain.len()));
        assert_eq!(bundle_bytes::unpack(&fs::read(path).unwrap(), Some(plain.len())).unwrap(), plain);
        assert!(previous_encoding(&dir.0, plain.len()).is_none());
    }

    #[test]
    fn small_empty_and_incompressible_inputs_keep_plain_bytes() {
        let dir = Scratch::new();
        let path = dir.slot();
        let mut state = 0x1234_5678_u32;
        let random: Vec<u8> = (0..32768).map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        }).collect();
        for plain in [b"".as_slice(), b"shell", &random] {
            assert_eq!(pack_asset(plain, &path).unwrap(), None);
            assert!(!path.exists());
        }
    }

    #[test]
    fn write_failure_is_not_silently_accepted() {
        let dir = Scratch::new();
        assert!(pack_asset(&b"x".repeat(8192), &dir.0.join("missing/0.zlib")).is_err());
        assert!(pack_asset(&b"x".repeat(4096), &dir.0).is_err());
        #[cfg(unix)] {
            let real = dir.0.join("real");
            fs::write(&real, b"unchanged symlink target").unwrap();
            std::os::unix::fs::symlink(&real, dir.slot()).unwrap();
            assert!(pack_asset(&b"x".repeat(4096), &dir.slot()).is_err());
            assert_eq!(fs::read(real).unwrap(), b"unchanged symlink target");
        }
    }
}
