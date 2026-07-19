

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use snow::Builder;

use crate::error::{AppError, AppResult};

pub const IK_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredIdentity {
    private_key_hex: String,
    public_key_hex: String,
}

#[derive(Debug, Clone)]
pub struct DeviceIdentity {
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl DeviceIdentity {
    pub fn fingerprint(&self) -> String {
        hex::encode(&self.public_key)
    }
}

fn generate() -> AppResult<DeviceIdentity> {
    let params = IK_PATTERN
        .parse()
        .map_err(|e| AppError::new(format!("invalid noise pattern: {e}")))?;
    let keypair = Builder::new(params)
        .generate_keypair()
        .map_err(|e| AppError::new(format!("failed to generate device identity: {e}")))?;
    Ok(DeviceIdentity {
        private_key: keypair.private,
        public_key: keypair.public,
    })
}

fn load_from_disk(path: &Path) -> Option<DeviceIdentity> {
    let raw = fs::read_to_string(path).ok()?;
    let stored: StoredIdentity = serde_json::from_str(&raw).ok()?;
    Some(DeviceIdentity {
        private_key: hex::decode(stored.private_key_hex).ok()?,
        public_key: hex::decode(stored.public_key_hex).ok()?,
    })
}

fn save_to_disk(path: &Path, identity: &DeviceIdentity) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::new(format!("failed to create app data dir: {e}")))?;
    }
    let stored = StoredIdentity {
        private_key_hex: hex::encode(&identity.private_key),
        public_key_hex: hex::encode(&identity.public_key),
    };
    let raw = serde_json::to_string_pretty(&stored)
        .map_err(|e| AppError::new(format!("failed to serialize device identity: {e}")))?;
    fs::write(path, raw).map_err(|e| AppError::new(format!("failed to write device identity: {e}")))
}


pub fn load_or_create(app_data_dir: &Path) -> AppResult<DeviceIdentity> {
    let path = default_identity_path(app_data_dir);
    if let Some(identity) = load_from_disk(&path) {
        return Ok(identity);
    }
    let identity = generate()?;
    save_to_disk(&path, &identity)?;
    Ok(identity)
}

pub fn default_identity_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("identity.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_reloads_the_same_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = default_identity_path(dir.path());

        let first = load_or_create(dir.path()).unwrap();
        let reloaded = load_from_disk(&path).unwrap();

        assert_eq!(first.public_key, reloaded.public_key);
        assert_eq!(first.private_key, reloaded.private_key);
    }

    #[test]
    fn fingerprint_is_stable_across_loads() {
        let dir = tempfile::tempdir().unwrap();
        let a = load_or_create(dir.path()).unwrap();
        let b = load_or_create(dir.path()).unwrap();
        assert_eq!(a.fingerprint(), b.fingerprint());
    }
}
