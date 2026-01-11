use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

use super::download::{clear_dir_files, download_to_file, now_ms};

#[derive(Debug, Serialize, Deserialize, Default)]
pub(super) struct CacheIndex {
    #[serde(default)]
    version: u32,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    file_name: String,
    size_bytes: u64,
    last_access_ms: u64,
}

pub(super) enum ResolvedAudio {
    Path(PathBuf),
    Temp(NamedTempFile),
}

pub struct AudioCache {
    dir: Option<PathBuf>,
    index_path: Option<PathBuf>,
    index: CacheIndex,
    max_bytes: u64,
}

impl AudioCache {
    pub fn new(data_dir: &Path) -> Self {
        const INDEX_VERSION: u32 = 2;

        let max_bytes = env::var("NETEASE_AUDIO_CACHE_MAX_MB")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2048)
            .saturating_mul(1024)
            .saturating_mul(1024);

        let dir = data_dir.join("audio_cache");
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!(dir = %dir.display(), err = %e, "创建音频缓存目录失败，将禁用缓存");
            return Self {
                dir: None,
                index_path: None,
                index: CacheIndex::default(),
                max_bytes,
            };
        }

        let index_path = dir.join("index.json");
        let mut index = fs::read(&index_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<CacheIndex>(&b).ok())
            .unwrap_or_default();

        if index.version != INDEX_VERSION {
            // 废弃旧索引/旧命名规则：直接清空缓存目录
            let _ = clear_dir_files(&dir, None);
            index = CacheIndex {
                version: INDEX_VERSION,
                entries: HashMap::new(),
            };
            let bytes = serde_json::to_vec_pretty(&index).unwrap_or_default();
            if let Err(e) = fs::write(&index_path, bytes) {
                tracing::warn!(path = %index_path.display(), err = %e, "写入音频缓存索引失败");
            }
        }

        Self {
            dir: Some(dir),
            index_path: Some(index_path),
            index,
            max_bytes,
        }
    }

    pub fn resolve_audio_file(
        &mut self,
        http: &Client,
        song_id: i64,
        br: i64,
        url: &str,
        title: &str,
    ) -> Result<ResolvedAudio, String> {
        let Some(dir) = self.dir.as_ref() else {
            // fallback: no cache dir
            let mut tmp = NamedTempFile::new().map_err(|e| format!("创建临时文件失败: {e}"))?;
            download_to_file(http, &mut tmp, url, title)?;
            return Ok(ResolvedAudio::Temp(tmp));
        };

        let key = format!("{song_id}_{br}");
        let file_name = format!("{key}.bin");
        let path = dir.join(&file_name);

        if path.exists() {
            self.touch(&key, &file_name, &path);
            self.persist_index();
            return Ok(ResolvedAudio::Path(path));
        }

        let mut tmp = NamedTempFile::new_in(dir)
            .map_err(|e| format!("创建缓存临时文件失败({title}): {e}"))?;
        download_to_file(http, &mut tmp, url, title)?;

        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        tmp.persist(&path)
            .map_err(|e| format!("写入缓存文件失败({title}): {e}"))?;

        self.touch(&key, &file_name, &path);
        self.cleanup(Some(&path));
        self.persist_index();

        Ok(ResolvedAudio::Path(path))
    }

    fn touch(&mut self, key: &str, file_name: &str, path: &Path) {
        let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        self.index.entries.insert(
            key.to_owned(),
            CacheEntry {
                file_name: file_name.to_owned(),
                size_bytes,
                last_access_ms: now_ms(),
            },
        );
    }

    fn cleanup(&mut self, keep: Option<&Path>) {
        let Some(dir) = self.dir.as_ref() else {
            return;
        };

        // remove missing
        self.index
            .entries
            .retain(|_, ent| dir.join(&ent.file_name).exists());

        let mut total: u64 = self.index.entries.values().map(|e| e.size_bytes).sum();
        if total <= self.max_bytes {
            return;
        }

        let mut entries = self
            .index
            .entries
            .iter()
            .map(|(k, v)| {
                (
                    k.to_owned(),
                    v.last_access_ms,
                    v.file_name.clone(),
                    v.size_bytes,
                )
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|(_, ts, _, _)| *ts);

        for (k, _ts, file_name, size) in entries {
            if total <= self.max_bytes {
                break;
            }
            let p = dir.join(&file_name);
            if keep.is_some_and(|kp| kp == p.as_path()) {
                continue;
            }
            let _ = fs::remove_file(&p);
            self.index.entries.remove(&k);
            total = total.saturating_sub(size);
        }
    }

    fn persist_index(&self) {
        let (Some(dir), Some(index_path)) = (self.dir.as_ref(), self.index_path.as_ref()) else {
            return;
        };
        let bytes = match serde_json::to_vec_pretty(&self.index) {
            Ok(b) => b,
            Err(_) => return,
        };
        let mut tmp = match NamedTempFile::new_in(dir) {
            Ok(f) => f,
            Err(_) => return,
        };
        if tmp.write_all(&bytes).is_err() {
            return;
        }
        let _ = tmp.persist(index_path);
    }

    pub fn invalidate(&mut self, song_id: i64, br: i64) {
        let Some(dir) = self.dir.as_ref() else {
            return;
        };
        let key = format!("{song_id}_{br}");
        if let Some(ent) = self.index.entries.remove(&key) {
            let _ = fs::remove_file(dir.join(ent.file_name));
        } else {
            let _ = fs::remove_file(dir.join(format!("{key}.bin")));
        }
        self.persist_index();
    }

    pub fn clear_all(&mut self, keep: Option<&Path>) -> (usize, u64) {
        let Some(dir) = self.dir.as_ref() else {
            return (0, 0);
        };

        let (files, bytes) = clear_dir_files(dir, keep);
        self.index.entries.clear();
        self.persist_index();
        (files, bytes)
    }
}
