use rand::seq::SliceRandom;

use crate::domain::model::Song;

use super::PlayMode;

#[derive(Debug, Clone)]
pub struct PlayQueue {
    songs: Vec<Song>,
    order: Vec<usize>,
    cursor: Option<usize>,
    mode: PlayMode,
}

impl PlayQueue {
    pub fn new(mode: PlayMode) -> Self {
        Self {
            songs: Vec::new(),
            order: Vec::new(),
            cursor: None,
            mode,
        }
    }

    pub fn set_mode(&mut self, mode: PlayMode) {
        if self.mode == mode {
            return;
        }
        let current = self.current_index();
        self.mode = mode;
        self.rebuild_order(current);
    }

    /// 设置播放队列的歌曲列表
    ///
    /// 返回旧的 songs 向量，允许调用方重用或丢弃
    pub fn set_songs(&mut self, songs: Vec<Song>, start_index: Option<usize>) -> Vec<Song> {
        let old = std::mem::replace(&mut self.songs, songs);
        self.rebuild_order(start_index);
        old
    }

    pub fn clear(&mut self) {
        self.songs.clear();
        self.order.clear();
        self.cursor = None;
    }

    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn songs(&self) -> &[Song] {
        &self.songs
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn ordered_songs(&self) -> Vec<Song> {
        self.order
            .iter()
            .filter_map(|&idx| self.songs.get(idx).cloned())
            .collect()
    }

    pub fn current_index(&self) -> Option<usize> {
        self.cursor.and_then(|pos| self.order.get(pos).copied())
    }

    pub fn cursor_pos(&self) -> Option<usize> {
        self.cursor
    }

    pub fn current(&self) -> Option<&Song> {
        self.current_index().and_then(|idx| self.songs.get(idx))
    }

    pub fn set_current_index(&mut self, index: usize) -> bool {
        if index >= self.songs.len() {
            return false;
        }
        if let Some(pos) = self.order.iter().position(|&i| i == index) {
            self.cursor = Some(pos);
            true
        } else {
            false
        }
    }

    pub fn clear_cursor(&mut self) {
        self.cursor = None;
    }

    pub fn restore(&mut self, songs: Vec<Song>, order: Vec<usize>, cursor: Option<usize>) -> bool {
        self.songs = songs;
        let len = self.songs.len();
        if len == 0 {
            self.order.clear();
            self.cursor = None;
            return order.is_empty() && cursor.is_none();
        }

        let valid_order = Self::is_valid_order(&order, len);
        self.order = if valid_order {
            order
        } else {
            (0..len).collect()
        };
        self.cursor = cursor.filter(|&pos| pos < self.order.len());
        valid_order
    }

    pub fn peek_next_index(&self) -> Option<usize> {
        let pos = self.cursor?;
        let len = self.order.len();
        if len == 0 {
            return None;
        }
        match self.mode {
            PlayMode::SingleLoop => self.order.get(pos).copied(),
            PlayMode::Sequential => {
                if pos + 1 < len {
                    self.order.get(pos + 1).copied()
                } else {
                    None
                }
            }
            PlayMode::ListLoop => {
                let next = (pos + 1) % len;
                self.order.get(next).copied()
            }
            PlayMode::Shuffle => {
                let next = if pos + 1 < len { pos + 1 } else { 0 };
                self.order.get(next).copied()
            }
        }
    }

    pub fn next_index(&mut self) -> Option<usize> {
        let pos = self.cursor?;
        let len = self.order.len();
        if len == 0 {
            return None;
        }
        match self.mode {
            PlayMode::SingleLoop => self.order.get(pos).copied(),
            PlayMode::Sequential => {
                if pos + 1 < len {
                    let next = pos + 1;
                    self.cursor = Some(next);
                    self.order.get(next).copied()
                } else {
                    self.cursor = None;
                    None
                }
            }
            PlayMode::ListLoop => {
                let next = (pos + 1) % len;
                self.cursor = Some(next);
                self.order.get(next).copied()
            }
            PlayMode::Shuffle => {
                let next = if pos + 1 < len { pos + 1 } else { 0 };
                self.cursor = Some(next);
                self.order.get(next).copied()
            }
        }
    }

    pub fn prev_index(&mut self) -> Option<usize> {
        let pos = self.cursor?;
        let len = self.order.len();
        if len == 0 {
            return None;
        }
        match self.mode {
            PlayMode::SingleLoop => self.order.get(pos).copied(),
            PlayMode::Sequential => {
                let prev = pos.saturating_sub(1);
                self.cursor = Some(prev);
                self.order.get(prev).copied()
            }
            PlayMode::ListLoop | PlayMode::Shuffle => {
                let prev = if pos == 0 { len - 1 } else { pos - 1 };
                self.cursor = Some(prev);
                self.order.get(prev).copied()
            }
        }
    }

    fn rebuild_order(&mut self, start_index: Option<usize>) {
        let len = self.songs.len();
        self.order.clear();
        if len == 0 {
            self.cursor = None;
            return;
        }
        self.order.extend(0..len);
        if matches!(self.mode, PlayMode::Shuffle) {
            self.order.shuffle(&mut rand::thread_rng());
        }
        let start = start_index.unwrap_or(0).min(len.saturating_sub(1));
        let pos = self.order.iter().position(|&i| i == start).unwrap_or(0);
        self.cursor = Some(pos);
    }

    fn is_valid_order(order: &[usize], len: usize) -> bool {
        if order.len() != len {
            return false;
        }

        let mut seen = vec![false; len];
        for &idx in order {
            if idx >= len || seen[idx] {
                return false;
            }
            seen[idx] = true;
        }
        true
    }
}
