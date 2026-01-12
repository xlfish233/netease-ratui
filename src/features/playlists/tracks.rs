use crate::app::Song;

pub(super) const PLAYLIST_TRACKS_PAGE_SIZE: usize = 200;

pub struct PlaylistTracksLoad {
    pub playlist_id: i64,
    pub total: usize,
    pub ids: Vec<i64>,
    pub cursor: usize,
    pub songs: Vec<Song>,
    pub inflight_req_id: Option<u64>,
}

impl PlaylistTracksLoad {
    pub fn new(playlist_id: i64, ids: Vec<i64>) -> Self {
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

    pub fn is_done(&self) -> bool {
        self.cursor >= self.ids.len()
    }

    pub fn next_chunk(&mut self) -> Vec<i64> {
        let start = self.cursor;
        let end = (start + PLAYLIST_TRACKS_PAGE_SIZE).min(self.ids.len());
        self.cursor = end;
        self.ids[start..end].to_vec()
    }
}
