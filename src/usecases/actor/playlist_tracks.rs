use crate::app::Song;

pub(super) const PLAYLIST_TRACKS_PAGE_SIZE: usize = 200;

pub(super) struct PlaylistTracksLoad {
    pub(super) playlist_id: i64,
    pub(super) total: usize,
    pub(super) ids: Vec<i64>,
    pub(super) cursor: usize,
    pub(super) songs: Vec<Song>,
    pub(super) inflight_req_id: Option<u64>,
}

impl PlaylistTracksLoad {
    pub(super) fn new(playlist_id: i64, ids: Vec<i64>) -> Self {
        let total = ids.len();
        Self {
            playlist_id,
            total,
            ids,
            cursor: 0,
            songs: Vec::new(),
            inflight_req_id: None,
        }
    }

    pub(super) fn is_done(&self) -> bool {
        self.cursor >= self.ids.len()
    }

    pub(super) fn next_chunk(&mut self) -> Vec<i64> {
        let start = self.cursor;
        let end = (start + PLAYLIST_TRACKS_PAGE_SIZE).min(self.ids.len());
        self.cursor = end;
        self.ids[start..end].to_vec()
    }
}
