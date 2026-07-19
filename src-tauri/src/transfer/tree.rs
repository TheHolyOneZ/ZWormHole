use std::path::Path;

use serde::Serialize;

use crate::error::{AppError, AppResult};


#[derive(Debug, Clone, Serialize)]
pub struct FolderNode {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,

    pub size: u64,
    pub children: Vec<FolderNode>,
}


const MAX_NODES: usize = 20_000;


pub fn read_tree(root: &Path) -> AppResult<FolderNode> {
    let meta = std::fs::metadata(root)
        .map_err(|e| AppError::new(format!("failed to read {}: {e}", root.display())))?;
    if !meta.is_dir() {
        return Err(AppError::new("not a folder"));
    }
    let mut count = 0usize;
    build_node(root, root, &mut count)
}

fn build_node(path: &Path, root: &Path, count: &mut usize) -> AppResult<FolderNode> {
    *count += 1;
    if *count > MAX_NODES {
        return Err(AppError::new(
            "this folder has too many files to preview — send it without picking, or choose a smaller folder",
        ));
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    let meta = std::fs::metadata(path)
        .map_err(|e| AppError::new(format!("failed to read {}: {e}", path.display())))?;

    if !meta.is_dir() {
        return Ok(FolderNode {
            name,
            rel_path,
            is_dir: false,
            size: meta.len(),
            children: Vec::new(),
        });
    }

    let mut entries: Vec<_> = std::fs::read_dir(path)
        .map_err(|e| AppError::new(format!("failed to list {}: {e}", path.display())))?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
    });

    let mut children = Vec::with_capacity(entries.len());
    let mut total = 0u64;
    for entry in entries {


        let child = match build_node(&entry.path(), root, count) {
            Ok(node) => node,
            Err(e) if e.to_string().contains("too many files") => return Err(e),
            Err(_) => continue,
        };
        total += child.size;
        children.push(child);
    }

    Ok(FolderNode {
        name,
        rel_path,
        is_dir: true,
        size: total,
        children,
    })
}
