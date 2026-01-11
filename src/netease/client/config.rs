use super::error::NeteaseError;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct NeteaseClientConfig {
    pub domain: String,
    pub api_domain: String,
    pub data_dir: PathBuf,
}

impl Default for NeteaseClientConfig {
    fn default() -> Self {
        let data_dir = ProjectDirs::from("dev", "netease", "netease-ratui")
            .map(|p| p.data_local_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("netease-ratui"));
        Self {
            domain: "https://music.163.com".to_owned(),
            api_domain: "https://interface.music.163.com".to_owned(),
            data_dir,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClientState {
    pub cookies: HashMap<String, String>,
    pub device_id: Option<String>,
}

pub fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join("netease_state.json")
}

pub fn load_state(data_dir: &Path) -> Result<ClientState, NeteaseError> {
    let p = state_path(data_dir);
    if !p.exists() {
        return Ok(ClientState::default());
    }
    let bytes = fs::read(p).map_err(NeteaseError::Io)?;
    serde_json::from_slice(&bytes).map_err(NeteaseError::Serde)
}

pub fn save_state(data_dir: &Path, state: &ClientState) -> Result<(), NeteaseError> {
    let p = state_path(data_dir);
    let bytes = serde_json::to_vec_pretty(state).map_err(NeteaseError::Serde)?;
    fs::write(p, bytes).map_err(NeteaseError::Io)
}
