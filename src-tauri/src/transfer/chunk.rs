use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::{AppError, AppResult};


pub fn read_chunk_from(file: &mut File, chunk_index: u32, chunk_size: u32) -> AppResult<Vec<u8>> {
    let offset = chunk_index as u64 * chunk_size as u64;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| AppError::new(format!("failed to seek source file: {e}")))?;
    let mut buf = vec![0u8; chunk_size as usize];
    let n = file
        .read(&mut buf)
        .map_err(|e| AppError::new(format!("failed to read chunk: {e}")))?;
    buf.truncate(n);
    Ok(buf)
}


pub fn open_staging_file(staging_path: &Path, total_size: u64) -> AppResult<File> {
    if let Some(parent) = staging_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::new(format!("failed to create destination dir: {e}")))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(staging_path)
        .map_err(|e| AppError::new(format!("failed to open staging file: {e}")))?;
    file.set_len(total_size)
        .map_err(|e| AppError::new(format!("failed to size staging file: {e}")))?;
    Ok(file)
}


pub fn write_chunk_to(
    file: &mut File,
    chunk_index: u32,
    chunk_size: u32,
    data: &[u8],
) -> AppResult<()> {
    let offset = chunk_index as u64 * chunk_size as u64;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| AppError::new(format!("failed to seek in staging file: {e}")))?;
    file.write_all(data)
        .map_err(|e| AppError::new(format!("failed to write chunk to staging file: {e}")))
}

#[cfg(test)]
pub fn write_chunk_at(
    staging_path: &Path,
    chunk_index: u32,
    chunk_size: u32,
    total_size: u64,
    data: &[u8],
) -> AppResult<()> {
    if let Some(parent) = staging_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::new(format!("failed to create destination dir: {e}")))?;
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(staging_path)
        .map_err(|e| AppError::new(format!("failed to open staging file: {e}")))?;

    file.set_len(total_size)
        .map_err(|e| AppError::new(format!("failed to size staging file: {e}")))?;

    let offset = chunk_index as u64 * chunk_size as u64;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| AppError::new(format!("failed to seek in staging file: {e}")))?;
    file.write_all(data)
        .map_err(|e| AppError::new(format!("failed to write chunk to staging file: {e}")))
}
