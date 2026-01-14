use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

use super::download::{clear_dir_files, now_ms};

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

pub struct AudioCache {
    dir: Option<PathBuf>,
    index_path: Option<PathBuf>,
    index: CacheIndex,
    max_bytes: u64,
    dirty: bool,
}

impl AudioCache {
    pub fn new_with_config(data_dir: &Path, max_mb: usize) -> Self {
        const INDEX_VERSION: u32 = 2;

        let max_bytes = (max_mb as u64).saturating_mul(1024).saturating_mul(1024);

        let dir = data_dir.join("audio_cache");
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!(dir = %dir.display(), err = %e, "创建音频缓存目录失败，将禁用缓存");
            return Self {
                dir: None,
                index_path: None,
                index: CacheIndex::default(),
                max_bytes,
                dirty: false,
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
            dirty: false,
        }
    }

    pub fn cache_dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    pub fn lookup_path(&mut self, song_id: i64, br: i64) -> Option<PathBuf> {
        let dir = self.dir.as_ref()?;

        let key = cache_key(song_id, br);
        let file_name = format!("{key}.bin");
        let path = dir.join(&file_name);

        if !path.exists() {
            self.index.entries.remove(&key);
            self.dirty = true;
            return None;
        }

        self.touch(&key, &file_name, &path);
        self.dirty = true;
        Some(path)
    }

    pub fn commit_tmp_file(
        &mut self,
        song_id: i64,
        br: i64,
        tmp_path: &Path,
    ) -> Result<PathBuf, String> {
        let dir = self
            .dir
            .as_ref()
            .ok_or_else(|| "缓存目录不可用".to_owned())?;

        let key = cache_key(song_id, br);
        let file_name = format!("{key}.bin");
        let final_path = dir.join(&file_name);

        if final_path.exists() {
            let _ = fs::remove_file(&final_path);
        }

        fs::rename(tmp_path, &final_path).map_err(|e| format!("写入缓存文件失败: {e}"))?;

        self.touch(&key, &file_name, &final_path);
        self.cleanup(Some(&final_path));
        self.dirty = true;
        self.persist_index_if_dirty();

        Ok(final_path)
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

    fn persist_index_if_dirty(&mut self) {
        if self.dirty {
            self.persist_index();
            self.dirty = false;
        }
    }

    pub fn invalidate(&mut self, song_id: i64, br: i64) {
        let Some(dir) = self.dir.as_ref() else {
            return;
        };
        let key = cache_key(song_id, br);
        if let Some(ent) = self.index.entries.remove(&key) {
            let _ = fs::remove_file(dir.join(ent.file_name));
        } else {
            let _ = fs::remove_file(dir.join(format!("{key}.bin")));
        }
        self.dirty = true;
        self.persist_index_if_dirty();
    }

    pub fn clear_all(&mut self, keep: Option<&Path>) -> (usize, u64) {
        let Some(dir) = self.dir.as_ref() else {
            return (0, 0);
        };

        let (files, bytes) = clear_dir_files(dir, keep);
        self.index.entries.clear();
        self.dirty = true;
        self.persist_index_if_dirty();
        (files, bytes)
    }

    pub fn purge_not_br(&mut self, keep_br: i64, keep: Option<&Path>) {
        let Some(dir) = self.dir.as_ref() else {
            return;
        };

        let keep = keep.and_then(|p| p.file_name().map(|n| dir.join(n)));

        let keys = self.index.entries.keys().cloned().collect::<Vec<_>>();

        for key in keys {
            let Some((_song_id, br)) = parse_cache_key(&key) else {
                continue;
            };
            if br == keep_br {
                continue;
            }

            if let Some(ent) = self.index.entries.remove(&key) {
                let p = dir.join(&ent.file_name);
                if keep.as_ref().is_some_and(|kp| kp == &p) {
                    self.index.entries.insert(key, ent);
                    continue;
                }
                let _ = fs::remove_file(p);
            }
        }

        self.cleanup(keep.as_deref());
        self.dirty = true;
        self.persist_index_if_dirty();
    }

    pub fn purge_song_other_brs(&mut self, song_id: i64, keep_br: i64, keep: Option<&Path>) {
        let Some(dir) = self.dir.as_ref() else {
            return;
        };

        let keep = keep.and_then(|p| p.file_name().map(|n| dir.join(n)));

        let keys = self.index.entries.keys().cloned().collect::<Vec<_>>();

        for key in keys {
            let Some((sid, br)) = parse_cache_key(&key) else {
                continue;
            };
            if sid != song_id || br == keep_br {
                continue;
            }

            if let Some(ent) = self.index.entries.remove(&key) {
                let p = dir.join(&ent.file_name);
                if keep.as_ref().is_some_and(|kp| kp == &p) {
                    self.index.entries.insert(key, ent);
                    continue;
                }
                let _ = fs::remove_file(p);
            }
        }

        self.cleanup(keep.as_deref());
        self.dirty = true;
        self.persist_index_if_dirty();
    }
}

impl Drop for AudioCache {
    fn drop(&mut self) {
        if self.dirty {
            tracing::debug!("AudioCache dropped with dirty index, persisting...");
            self.persist_index();
            tracing::debug!("AudioCache index persisted on drop");
        }
    }
}

fn cache_key(song_id: i64, br: i64) -> String {
    format!("{song_id}_{br}")
}

fn parse_cache_key(key: &str) -> Option<(i64, i64)> {
    let (a, b) = key.split_once('_')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cache_new_with_dirty_flag() {
        let temp_dir = TempDir::new().unwrap();
        let cache = AudioCache::new_with_config(temp_dir.path(), 100);

        assert!(!cache.dirty, "new cache should not be dirty");
    }

    #[test]
    fn test_lookup_path_sets_dirty_on_hit() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = AudioCache::new_with_config(temp_dir.path(), 100);

        // Create a test cache file
        let cache_dir = cache.cache_dir().unwrap();
        let test_file = cache_dir.join("123_456.bin");
        fs::write(&test_file, b"test data").unwrap();

        // Reset dirty flag (from file creation)
        cache.dirty = false;

        // Lookup should set dirty flag
        let result = cache.lookup_path(123, 456);
        assert!(result.is_some(), "lookup should find the file");
        assert!(cache.dirty, "lookup_path should set dirty flag on hit");
    }

    #[test]
    fn test_lookup_path_sets_dirty_on_miss() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = AudioCache::new_with_config(temp_dir.path(), 100);

        // Reset dirty flag
        cache.dirty = false;

        // Lookup miss should also set dirty flag (entry removed from index)
        let result = cache.lookup_path(999, 999);
        assert!(result.is_none(), "lookup should not find the file");
        assert!(cache.dirty, "lookup_path should set dirty flag on miss");
    }

    #[test]
    fn test_persist_index_if_dirty() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = AudioCache::new_with_config(temp_dir.path(), 100);

        // Set dirty flag
        cache.dirty = true;

        // persist_index_if_dirty should persist and clear dirty flag
        cache.persist_index_if_dirty();
        assert!(
            !cache.dirty,
            "persist_index_if_dirty should clear dirty flag"
        );
    }

    #[test]
    fn test_persist_index_if_dirty_when_not_dirty() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = AudioCache::new_with_config(temp_dir.path(), 100);

        // Don't set dirty flag
        assert!(!cache.dirty);

        // persist_index_if_dirty should do nothing (not crash)
        cache.persist_index_if_dirty();
        assert!(!cache.dirty, "dirty flag should remain false");
    }

    #[test]
    fn test_commit_tmp_file_persists_immediately() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = AudioCache::new_with_config(temp_dir.path(), 100);

        // Create a temp file
        let tmp_file = temp_dir.path().join("tmp.bin");
        fs::write(&tmp_file, b"test data").unwrap();

        // Reset dirty flag
        cache.dirty = false;

        // commit_tmp_file should set dirty and persist
        let result = cache.commit_tmp_file(123, 456, &tmp_file);
        assert!(result.is_ok(), "commit_tmp_file should succeed");

        // Dirty flag should be cleared after persist
        assert!(
            !cache.dirty,
            "commit_tmp_file should persist and clear dirty flag"
        );

        // File should exist
        let cache_dir = cache.cache_dir().unwrap();
        let final_file = cache_dir.join("123_456.bin");
        assert!(final_file.exists(), "cached file should exist");
    }

    #[test]
    fn test_invalidate_persists_immediately() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = AudioCache::new_with_config(temp_dir.path(), 100);

        // Create a test cache file
        let cache_dir = cache.cache_dir().unwrap();
        let test_file = cache_dir.join("123_456.bin");
        fs::write(&test_file, b"test data").unwrap();

        // Reset dirty flag
        cache.dirty = false;

        // invalidate should set dirty and persist
        cache.invalidate(123, 456);

        // Dirty flag should be cleared after persist
        assert!(
            !cache.dirty,
            "invalidate should persist and clear dirty flag"
        );

        // File should be removed
        assert!(!test_file.exists(), "cached file should be removed");
    }

    #[test]
    fn test_clear_all_persists_immediately() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = AudioCache::new_with_config(temp_dir.path(), 100);

        // Create a test cache file
        let cache_dir = cache.cache_dir().unwrap();
        let test_file = cache_dir.join("123_456.bin");
        fs::write(&test_file, b"test data").unwrap();

        // Reset dirty flag
        cache.dirty = false;

        // clear_all should set dirty and persist
        cache.clear_all(None);

        // Dirty flag should be cleared after persist
        assert!(
            !cache.dirty,
            "clear_all should persist and clear dirty flag"
        );

        // File should be removed
        assert!(!test_file.exists(), "cached file should be removed");
    }

    #[test]
    fn test_multiple_lookups_before_persist() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = AudioCache::new_with_config(temp_dir.path(), 100);

        // Create multiple test cache files
        let cache_dir = cache.cache_dir().unwrap();
        for i in 1..=3 {
            let test_file = cache_dir.join(format!("{}_{i}.bin", 100 + i));
            fs::write(&test_file, b"test data").unwrap();
        }

        // Reset dirty flag
        cache.dirty = false;

        // Multiple lookups should all set dirty flag
        for i in 1..=3 {
            let result = cache.lookup_path(100 + i, i);
            assert!(result.is_some(), "lookup should find the file");
            assert!(cache.dirty, "lookup should set dirty flag");
        }

        // dirty should still be true (not cleared yet)
        assert!(
            cache.dirty,
            "dirty flag should still be true after multiple lookups"
        );

        // Now persist
        cache.persist_index_if_dirty();
        assert!(!cache.dirty, "dirty flag should be cleared after persist");
    }
}
