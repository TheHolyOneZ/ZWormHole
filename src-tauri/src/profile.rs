

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};


const AVATAR_DIM: u32 = 128;


pub const AVATAR_MAX_BYTES: usize = 256 * 1024;


#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileWire {
    pub display_name: String,
    #[serde(default)]
    pub avatar_png: Option<Vec<u8>>,


    #[serde(default)]
    pub updated_at: u64,
}


#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MyProfileMeta {

    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    updated_at: u64,
}


pub struct MyProfileStore {
    meta_path: PathBuf,
    avatar_path: PathBuf,
    meta: RwLock<MyProfileMeta>,
}

impl MyProfileStore {
    pub fn new(app_data_dir: &Path) -> Self {
        let meta_path = app_data_dir.join("profile.json");
        let avatar_path = app_data_dir.join("my_avatar.png");
        let meta = read_json(&meta_path).unwrap_or_default();
        Self {
            meta_path,
            avatar_path,
            meta: RwLock::new(meta),
        }
    }

    pub fn display_name_override(&self) -> Option<String> {
        self.meta.read().unwrap().display_name.clone()
    }

    pub fn avatar_bytes(&self) -> Option<Vec<u8>> {
        fs::read(&self.avatar_path).ok()
    }


    pub fn wire(&self, fallback: &str) -> ProfileWire {
        let meta = self.meta.read().unwrap();
        ProfileWire {
            display_name: meta
                .display_name
                .clone()
                .unwrap_or_else(|| fallback.to_string()),
            avatar_png: self.avatar_bytes(),
            updated_at: meta.updated_at,
        }
    }

    fn bump_and_write(&self, meta: &mut MyProfileMeta) -> AppResult<()> {
        meta.updated_at = crate::net::now_unix_secs();
        write_json(&self.meta_path, meta)
    }

    pub fn set_display_name(&self, name: Option<String>) -> AppResult<()> {
        let mut meta = self.meta.write().unwrap();
        meta.display_name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        self.bump_and_write(&mut meta)
    }


    pub fn set_avatar(&self, image_bytes: &[u8]) -> AppResult<()> {
        let png = normalize_avatar(image_bytes)?;
        if let Some(parent) = self.avatar_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::new(format!("failed to create app data dir: {e}")))?;
        }
        fs::write(&self.avatar_path, &png)
            .map_err(|e| AppError::new(format!("failed to write avatar: {e}")))?;
        let mut meta = self.meta.write().unwrap();
        self.bump_and_write(&mut meta)
    }

    pub fn clear_avatar(&self) -> AppResult<()> {
        let _ = fs::remove_file(&self.avatar_path);
        let mut meta = self.meta.write().unwrap();
        self.bump_and_write(&mut meta)
    }
}


#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachedProfile {
    pub display_name: String,
    #[serde(default)]
    pub avatar_png: Option<Vec<u8>>,
    #[serde(default)]
    pub updated_at: u64,
}


pub struct RemoteProfileStore {
    dir: PathBuf,
}

impl RemoteProfileStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path_for(&self, fingerprint: &str) -> PathBuf {
        self.dir.join(format!("{fingerprint}.json"))
    }


    pub fn cache(&self, fingerprint: &str, profile: ProfileWire) -> AppResult<()> {
        if let Some(existing) = self.get(fingerprint) {
            if existing.updated_at > profile.updated_at {
                return Ok(());
            }
        }
        fs::create_dir_all(&self.dir)
            .map_err(|e| AppError::new(format!("failed to create profiles dir: {e}")))?;
        let cached = CachedProfile {
            display_name: profile.display_name,
            avatar_png: profile.avatar_png.filter(|a| a.len() <= AVATAR_MAX_BYTES),
            updated_at: profile.updated_at,
        };
        write_json(&self.path_for(fingerprint), &cached)
    }

    pub fn get(&self, fingerprint: &str) -> Option<CachedProfile> {
        read_json(&self.path_for(fingerprint))
    }

    pub fn list(&self) -> Vec<(String, CachedProfile)> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(fp) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Some(profile) = read_json::<CachedProfile>(&path) {
                out.push((fp.to_string(), profile));
            }
        }
        out
    }
}


fn normalize_avatar(bytes: &[u8]) -> AppResult<Vec<u8>> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| AppError::new(format!("couldn't read that image: {e}")))?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(AppError::new("that image has no pixels"));
    }
    let side = w.min(h);
    let x = (w - side) / 2;
    let y = (h - side) / 2;
    let square = img.crop_imm(x, y, side, side);
    let thumb = square.resize_exact(AVATAR_DIM, AVATAR_DIM, image::imageops::FilterType::Lanczos3);

    let mut out = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| AppError::new(format!("couldn't encode avatar: {e}")))?;
    Ok(out)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let raw = serde_json::to_string_pretty(value)
        .map_err(|e| AppError::new(format!("failed to serialize profile: {e}")))?;
    fs::write(path, raw).map_err(|e| AppError::new(format!("failed to write profile: {e}")))
}

pub fn remote_profiles_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("remote_profiles")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_profile_display_name_override_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let store = MyProfileStore::new(dir.path());
        assert_eq!(store.display_name_override(), None);
        assert_eq!(store.wire("host").display_name, "host");

        store.set_display_name(Some("  Etienne  ".into())).unwrap();
        assert_eq!(store.display_name_override().as_deref(), Some("Etienne"));
        assert_eq!(store.wire("host").display_name, "Etienne");

        store.set_display_name(Some("   ".into())).unwrap();
        assert_eq!(store.display_name_override(), None);
    }

    #[test]
    fn remote_cache_keeps_newest_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = RemoteProfileStore::new(remote_profiles_dir(dir.path()));
        store
            .cache(
                "abc",
                ProfileWire {
                    display_name: "New".into(),
                    avatar_png: None,
                    updated_at: 100,
                },
            )
            .unwrap();

        store
            .cache(
                "abc",
                ProfileWire {
                    display_name: "Old".into(),
                    avatar_png: None,
                    updated_at: 50,
                },
            )
            .unwrap();
        assert_eq!(store.get("abc").unwrap().display_name, "New");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn oversized_avatar_is_dropped_but_name_kept() {
        let dir = tempfile::tempdir().unwrap();
        let store = RemoteProfileStore::new(remote_profiles_dir(dir.path()));
        store
            .cache(
                "abc",
                ProfileWire {
                    display_name: "Huge".into(),
                    avatar_png: Some(vec![0u8; AVATAR_MAX_BYTES + 1]),
                    updated_at: 1,
                },
            )
            .unwrap();
        let cached = store.get("abc").unwrap();
        assert_eq!(cached.display_name, "Huge");
        assert!(cached.avatar_png.is_none());
    }
}
