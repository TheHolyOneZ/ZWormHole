use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};


pub const CHUNK_SIZE: u32 = 60 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifest {
    pub relative_path: String,
    pub size: u64,
    pub chunk_size: u32,

    pub chunk_hashes: Vec<[u8; 32]>,
    pub root_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferManifest {


    pub manifest_id: String,
    pub files: Vec<FileManifest>,


    pub folder_archive_name: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireFileManifest {
    pub relative_path: String,
    pub size: u64,
    pub chunk_size: u32,
    pub root_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireTransferManifest {
    pub manifest_id: String,
    pub files: Vec<WireFileManifest>,
    #[serde(default)]
    pub folder_archive_name: Option<String>,


    #[serde(default)]
    pub transfer_id: Option<String>,
}

impl From<&TransferManifest> for WireTransferManifest {
    fn from(m: &TransferManifest) -> Self {
        Self {
            manifest_id: m.manifest_id.clone(),
            files: m
                .files
                .iter()
                .map(|f| WireFileManifest {
                    relative_path: f.relative_path.clone(),
                    size: f.size,
                    chunk_size: f.chunk_size,
                    root_hash: f.root_hash,
                })
                .collect(),
            folder_archive_name: m.folder_archive_name.clone(),


            transfer_id: None,
        }
    }
}

pub fn chunk_count(size: u64, chunk_size: u32) -> u32 {
    if size == 0 {
        return 0;
    }
    ((size + chunk_size as u64 - 1) / chunk_size as u64) as u32
}

fn hash_file(path: &Path, chunk_size: u32) -> AppResult<(Vec<[u8; 32]>, [u8; 32])> {
    let mut file = File::open(path)
        .map_err(|e| AppError::new(format!("failed to open {}: {e}", path.display())))?;

    let mut whole_file_hasher = blake3::Hasher::new();
    let mut chunk_hashes = Vec::new();
    let mut buf = vec![0u8; chunk_size as usize];

    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| AppError::new(format!("failed to read {}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        whole_file_hasher.update(&buf[..n]);
        chunk_hashes.push(*blake3::hash(&buf[..n]).as_bytes());
    }

    Ok((chunk_hashes, *whole_file_hasher.finalize().as_bytes()))
}


pub fn build_manifest(base_dir: &Path, paths: &[PathBuf]) -> AppResult<TransferManifest> {
    let mut files = Vec::with_capacity(paths.len());

    for path in paths {
        let metadata = std::fs::metadata(path)
            .map_err(|e| AppError::new(format!("failed to stat {}: {e}", path.display())))?;
        if metadata.is_dir() {
            return Err(AppError::new(format!(
                "{} is a folder — folder transfer isn't supported yet, select individual files",
                path.display()
            )));
        }
        let size = metadata.len();

        let relative_path = path
            .strip_prefix(base_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let (chunk_hashes, root_hash) = hash_file(path, CHUNK_SIZE)?;

        files.push(FileManifest {
            relative_path,
            size,
            chunk_size: CHUNK_SIZE,
            chunk_hashes,
            root_hash,
        });
    }

    let manifest_id = compute_manifest_id(&files);

    Ok(TransferManifest {
        manifest_id,
        files,
        folder_archive_name: None,
    })
}

fn compute_manifest_id(files: &[FileManifest]) -> String {
    let mut hasher = blake3::Hasher::new();
    for f in files {
        hasher.update(f.relative_path.as_bytes());
        hasher.update(&f.size.to_le_bytes());
        hasher.update(f.root_hash.as_slice());
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn same_content_yields_same_manifest_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(&vec![1u8; CHUNK_SIZE as usize * 2 + 100])
            .unwrap();

        let m1 = build_manifest(dir.path(), &[path.clone()]).unwrap();
        let m2 = build_manifest(dir.path(), &[path]).unwrap();

        assert_eq!(m1.manifest_id, m2.manifest_id);
        assert_eq!(m1.files[0].chunk_hashes.len(), 3);
    }

    #[test]
    fn changed_content_yields_different_manifest_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");

        std::fs::write(&path, b"version one").unwrap();
        let m1 = build_manifest(dir.path(), &[path.clone()]).unwrap();

        std::fs::write(&path, b"version two, different").unwrap();
        let m2 = build_manifest(dir.path(), &[path]).unwrap();

        assert_ne!(m1.manifest_id, m2.manifest_id);
    }
}
