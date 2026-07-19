use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::{AppError, AppResult};

pub fn verify_chunk(data: &[u8], expected: &[u8; 32]) -> bool {
    blake3::hash(data).as_bytes() == expected
}

pub fn verify_file(path: &Path, expected_root: &[u8; 32]) -> AppResult<bool> {
    let mut file = File::open(path)
        .map_err(|e| AppError::new(format!("failed to open {}: {e}", path.display())))?;

    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| AppError::new(format!("failed to read {}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(hasher.finalize().as_bytes() == expected_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_corrupted_chunk() {
        let good = b"the quick brown fox";
        let expected = *blake3::hash(good).as_bytes();

        assert!(verify_chunk(good, &expected));
        assert!(!verify_chunk(b"the quick brown FOX", &expected));
    }

    #[test]
    fn detects_a_corrupted_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.bin");
        std::fs::write(&path, b"original content").unwrap();

        let root = *blake3::hash(b"original content").as_bytes();
        assert!(verify_file(&path, &root).unwrap());

        std::fs::write(&path, b"tampered content").unwrap();
        assert!(!verify_file(&path, &root).unwrap());
    }
}
