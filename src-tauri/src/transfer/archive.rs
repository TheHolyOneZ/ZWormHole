use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::error::{AppError, AppResult};


pub fn zip_folder(folder: &Path) -> AppResult<PathBuf> {
    zip_folder_inner(folder, None)
}


pub fn zip_folder_selection(
    folder: &Path,
    selected: &std::collections::HashSet<String>,
) -> AppResult<PathBuf> {
    zip_folder_inner(folder, Some(selected))
}

fn zip_folder_inner(
    folder: &Path,
    selected: Option<&std::collections::HashSet<String>>,
) -> AppResult<PathBuf> {
    let folder_name = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());

    let zip_path = std::env::temp_dir().join(format!(
        "zwormhole-{}-{}.zip",
        folder_name,
        uuid::Uuid::new_v4()
    ));

    let zip_file = File::create(&zip_path)
        .map_err(|e| AppError::new(format!("failed to create archive: {e}")))?;
    let mut writer = ZipWriter::new(zip_file);


    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for entry in walkdir::WalkDir::new(folder)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(folder)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");


        if let Some(sel) = selected {
            if !sel.contains(&relative) {
                continue;
            }
        }

        writer
            .start_file(&relative, options)
            .map_err(|e| AppError::new(format!("failed to add {relative} to archive: {e}")))?;

        let mut file = File::open(path)
            .map_err(|e| AppError::new(format!("failed to read {}: {e}", path.display())))?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = file
                .read(&mut buf)
                .map_err(|e| AppError::new(format!("failed to read {}: {e}", path.display())))?;
            if n == 0 {
                break;
            }
            writer
                .write_all(&buf[..n])
                .map_err(|e| AppError::new(format!("failed to write to archive: {e}")))?;
        }
    }

    writer
        .finish()
        .map_err(|e| AppError::new(format!("failed to finalize archive: {e}")))?;

    Ok(zip_path)
}


pub fn extract_zip(zip_path: &Path, dest_dir: &Path, folder_name: &str) -> AppResult<PathBuf> {
    let file = File::open(zip_path)
        .map_err(|e| AppError::new(format!("failed to open archive: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::new(format!("failed to read archive: {e}")))?;


    let mut target = dest_dir.join(folder_name);
    let mut suffix = 1;
    while target.exists() {
        target = dest_dir.join(format!("{folder_name} ({suffix})"));
        suffix += 1;
    }

    archive
        .extract(&target)
        .map_err(|e| AppError::new(format!("failed to extract archive: {e}")))?;

    Ok(target)
}


pub fn rename_to_clean_zip_name(
    zip_path: &Path,
    dest_dir: &Path,
    folder_name: &str,
) -> AppResult<PathBuf> {
    let mut target = dest_dir.join(format!("{folder_name}.zip"));
    let mut suffix = 1;
    while target.exists() {
        target = dest_dir.join(format!("{folder_name} ({suffix}).zip"));
        suffix += 1;
    }
    std::fs::rename(zip_path, &target)
        .map_err(|e| AppError::new(format!("failed to rename archive: {e}")))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zips_nested_files_and_preserves_structure() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("top.txt"), b"top level").unwrap();
        std::fs::create_dir(src.path().join("nested")).unwrap();
        std::fs::write(src.path().join("nested/inner.txt"), b"nested content").unwrap();

        let zip_path = zip_folder(src.path()).unwrap();
        assert!(zip_path.exists());

        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 2);

        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();
        assert_eq!(names, vec!["nested/inner.txt", "top.txt"]);

        std::fs::remove_file(&zip_path).unwrap();
    }

    #[test]
    fn zip_selection_includes_only_picked_files() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a.txt"), b"a").unwrap();
        std::fs::write(src.path().join("b.txt"), b"b").unwrap();
        std::fs::create_dir(src.path().join("sub")).unwrap();
        std::fs::write(src.path().join("sub/c.txt"), b"c").unwrap();

        let mut sel = std::collections::HashSet::new();
        sel.insert("a.txt".to_string());
        sel.insert("sub/c.txt".to_string());

        let zip_path = zip_folder_selection(src.path(), &sel).unwrap();
        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();
        assert_eq!(names, vec!["a.txt", "sub/c.txt"]);

        std::fs::remove_file(&zip_path).unwrap();
    }

    #[test]
    fn zip_and_extract_round_trips_the_folder() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a.txt"), b"hello").unwrap();
        std::fs::create_dir(src.path().join("sub")).unwrap();
        std::fs::write(src.path().join("sub/b.txt"), b"world").unwrap();

        let zip_path = zip_folder(src.path()).unwrap();

        let dest = tempfile::tempdir().unwrap();
        let extracted = extract_zip(&zip_path, dest.path(), "myfolder").unwrap();

        assert_eq!(extracted, dest.path().join("myfolder"));
        assert_eq!(std::fs::read(extracted.join("a.txt")).unwrap(), b"hello");
        assert_eq!(std::fs::read(extracted.join("sub/b.txt")).unwrap(), b"world");

        std::fs::remove_file(&zip_path).unwrap();
    }

    #[test]
    fn extract_picks_a_free_name_when_the_folder_already_exists() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a.txt"), b"hello").unwrap();
        let zip_path = zip_folder(src.path()).unwrap();

        let dest = tempfile::tempdir().unwrap();
        std::fs::create_dir(dest.path().join("myfolder")).unwrap();

        let extracted = extract_zip(&zip_path, dest.path(), "myfolder").unwrap();
        assert_eq!(extracted, dest.path().join("myfolder (1)"));

        std::fs::remove_file(&zip_path).unwrap();
    }
}
