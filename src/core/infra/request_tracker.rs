//! 统一请求去重/过期丢弃管理器
//!
//! 用于处理异步请求的去重逻辑：同一 key 只保留最新的 req_id，
//! 旧请求返回时会被丢弃。

use std::collections::HashMap;
use std::hash::Hash;

/// 通用请求追踪器
///
/// 支持任意 key 类型，用于管理同类请求的去重。
/// 同一 key 只保留最新的 req_id，旧请求的响应会被丢弃。
#[derive(Debug)]
pub struct RequestTracker<K> {
    pending: HashMap<K, u64>,
}

impl<K: Eq + Hash> Default for RequestTracker<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash> RequestTracker<K> {
    /// 创建新的追踪器
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// 发起新请求，返回分配的 req_id
    ///
    /// 如果同一 key 已有 pending 请求，旧请求会被覆盖（后续 accept 会失败）。
    pub fn issue(&mut self, key: K, next_id_fn: impl FnOnce() -> u64) -> u64 {
        let id = next_id_fn();
        self.pending.insert(key, id);
        id
    }

    /// 检查并接受响应
    ///
    /// 只有当 key 对应的 pending req_id 与传入的 req_id 匹配时才返回 true，
    /// 并自动清除该 key 的 pending 状态。
    /// 否则返回 false（表示过期请求，应丢弃）。
    pub fn accept(&mut self, key: &K, req_id: u64) -> bool {
        match self.pending.get(key) {
            Some(&pending_id) if pending_id == req_id => {
                self.pending.remove(key);
                true
            }
            _ => false,
        }
    }

    /// 清除指定 key 的 pending 状态
    #[allow(dead_code)]
    pub fn clear(&mut self, key: &K) {
        self.pending.remove(key);
    }

    /// 重置所有 pending 状态（用于 logout 等场景）
    pub fn reset_all(&mut self) {
        self.pending.clear();
    }

    /// 检查指定 key 是否有 pending 请求
    #[allow(dead_code)]
    pub fn is_pending(&self, key: &K) -> bool {
        self.pending.contains_key(key)
    }

    /// 获取指定 key 的 pending req_id（如果有）
    #[allow(dead_code)]
    pub fn get_pending(&self, key: &K) -> Option<u64> {
        self.pending.get(key).copied()
    }
}

/// 预定义的请求类型 key
///
/// 用于标识不同类型的请求，避免使用字符串 key。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestKey {
    /// 搜索请求
    Search,
    /// 登录二维码 key 请求
    LoginQrKey,
    /// 登录二维码轮询请求
    LoginQrPoll,
    /// Cookie 登录请求
    LoginSetCookie,
    /// 用户账号信息请求
    Account,
    /// 用户歌单列表请求
    Playlists,
    /// 歌单详情（歌曲 ID 列表）请求
    PlaylistDetail,
    /// 歌单歌曲详情分页请求
    PlaylistTracks,
    /// 播放链接请求
    SongUrl,
    /// 歌词请求
    Lyric,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_and_accept() {
        let mut tracker: RequestTracker<&str> = RequestTracker::new();
        let mut id_counter = 1u64;

        let req_id = tracker.issue("search", || {
            let id = id_counter;
            id_counter += 1;
            id
        });

        assert_eq!(req_id, 1);
        assert!(tracker.is_pending(&"search"));

        // 正确的 req_id 应该被 accept
        assert!(tracker.accept(&"search", 1));
        assert!(!tracker.is_pending(&"search"));
    }

    #[test]
    fn test_consecutive_issue_only_accepts_latest() {
        let mut tracker: RequestTracker<&str> = RequestTracker::new();
        let mut id_counter = 1u64;

        // 连续发起两次请求
        let req_id_1 = tracker.issue("search", || {
            let id = id_counter;
            id_counter += 1;
            id
        });
        let req_id_2 = tracker.issue("search", || {
            let id = id_counter;
            id_counter += 1;
            id
        });

        assert_eq!(req_id_1, 1);
        assert_eq!(req_id_2, 2);

        // 第一个请求的响应应该被拒绝（过期）
        assert!(!tracker.accept(&"search", req_id_1));
        // pending 状态仍然存在（因为第二个请求还在）
        assert!(tracker.is_pending(&"search"));

        // 第二个请求的响应应该被接受
        assert!(tracker.accept(&"search", req_id_2));
        assert!(!tracker.is_pending(&"search"));
    }

    #[test]
    fn test_accept_without_issue_returns_false() {
        let mut tracker: RequestTracker<&str> = RequestTracker::new();

        // 没有 issue 的情况下 accept 应该返回 false
        assert!(!tracker.accept(&"search", 999));
    }

    #[test]
    fn test_clear_key() {
        let mut tracker: RequestTracker<&str> = RequestTracker::new();
        let mut id_counter = 1u64;

        tracker.issue("search", || {
            let id = id_counter;
            id_counter += 1;
            id
        });

        assert!(tracker.is_pending(&"search"));

        tracker.clear(&"search");
        assert!(!tracker.is_pending(&"search"));
    }

    #[test]
    fn test_reset_all() {
        let mut tracker: RequestTracker<&str> = RequestTracker::new();
        let mut id_counter = 1u64;

        tracker.issue("search", || {
            let id = id_counter;
            id_counter += 1;
            id
        });
        tracker.issue("playlists", || {
            let id = id_counter;
            id_counter += 1;
            id
        });

        assert!(tracker.is_pending(&"search"));
        assert!(tracker.is_pending(&"playlists"));

        tracker.reset_all();

        assert!(!tracker.is_pending(&"search"));
        assert!(!tracker.is_pending(&"playlists"));
    }

    #[test]
    fn test_different_keys_independent() {
        let mut tracker: RequestTracker<&str> = RequestTracker::new();
        let mut id_counter = 1u64;

        let search_id = tracker.issue("search", || {
            let id = id_counter;
            id_counter += 1;
            id
        });
        let playlists_id = tracker.issue("playlists", || {
            let id = id_counter;
            id_counter += 1;
            id
        });

        // 两个不同 key 的请求应该独立
        assert!(tracker.accept(&"search", search_id));
        assert!(tracker.is_pending(&"playlists"));
        assert!(tracker.accept(&"playlists", playlists_id));
    }

    #[test]
    fn test_request_key_enum() {
        let mut tracker: RequestTracker<RequestKey> = RequestTracker::new();
        let mut id_counter = 1u64;

        let req_id = tracker.issue(RequestKey::Search, || {
            let id = id_counter;
            id_counter += 1;
            id
        });

        assert!(tracker.accept(&RequestKey::Search, req_id));
    }

    #[test]
    fn test_get_pending() {
        let mut tracker: RequestTracker<&str> = RequestTracker::new();
        let mut id_counter = 1u64;

        assert_eq!(tracker.get_pending(&"search"), None);

        let req_id = tracker.issue("search", || {
            let id = id_counter;
            id_counter += 1;
            id
        });

        assert_eq!(tracker.get_pending(&"search"), Some(req_id));
    }
}
