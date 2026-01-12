use crate::app::{App, PlaylistPreload, PreloadStatus};
use std::collections::{HashMap, HashSet};

use crate::core::prelude::{effects::CoreEffects, netease::NeteaseCommand, utils::next_id};
use crate::features::playlists::PlaylistTracksLoad;

#[derive(Debug, Clone, Copy)]
enum PreloadPendingKind {
    PlaylistDetail { playlist_id: i64 },
    SongsChunk { playlist_id: i64 },
}

#[derive(Default)]
pub struct PreloadManager {
    generation: u64,
    pending: HashMap<u64, (u64, PreloadPendingKind)>,
    loaders: HashMap<i64, PlaylistTracksLoad>,
    active_playlists: HashSet<i64>,
}

impl PreloadManager {
    pub fn owns_req(&self, req_id_evt: u64) -> bool {
        self.pending
            .get(&req_id_evt)
            .is_some_and(|(generation, _)| *generation == self.generation)
    }

    pub fn reset(&mut self, app: &mut App) {
        self.generation = self.generation.wrapping_add(1);
        self.pending.clear();
        self.loaders.clear();
        self.active_playlists.clear();
        app.playlist_preloads.clear();
        app.preload_summary.clear();
    }

    pub async fn start_for_playlists(
        &mut self,
        app: &mut App,
        effects: &mut CoreEffects,
        req_id: &mut u64,
        preload_count: usize,
    ) {
        self.generation = self.generation.wrapping_add(1);
        self.pending.clear();
        self.loaders.clear();
        self.active_playlists.clear();
        app.playlist_preloads.clear();
        app.preload_summary.clear();

        let selected = select_preload_targets(&app.playlists, preload_count);
        if selected.is_empty() {
            return;
        }

        for playlist_id in &selected {
            app.playlist_preloads.insert(
                *playlist_id,
                PlaylistPreload {
                    status: PreloadStatus::Loading {
                        loaded: 0,
                        total: 0,
                    },
                    songs: Vec::new(),
                },
            );
        }
        update_preload_summary(app);

        for playlist_id in selected {
            self.active_playlists.insert(playlist_id);
            let rid = next_id(req_id);
            self.pending.insert(
                rid,
                (
                    self.generation,
                    PreloadPendingKind::PlaylistDetail { playlist_id },
                ),
            );
            effects.send_netease_lo(NeteaseCommand::PlaylistDetail {
                req_id: rid,
                playlist_id,
            });
        }
    }

    pub fn cancel_playlist(&mut self, app: &mut App, playlist_id: i64) {
        self.active_playlists.remove(&playlist_id);
        self.loaders.remove(&playlist_id);
        let to_remove: Vec<u64> = self
            .pending
            .iter()
            .filter_map(|(rid, (_, kind))| match kind {
                PreloadPendingKind::PlaylistDetail { playlist_id: p }
                | PreloadPendingKind::SongsChunk { playlist_id: p } => {
                    if *p == playlist_id {
                        Some(*rid)
                    } else {
                        None
                    }
                }
            })
            .collect();
        for rid in to_remove {
            self.pending.remove(&rid);
        }

        if let Some(p) = app.playlist_preloads.get_mut(&playlist_id) {
            p.status = PreloadStatus::Cancelled;
            p.songs.clear();
        }
        update_preload_summary(app);
    }

    pub async fn on_playlist_track_ids(
        &mut self,
        app: &mut App,
        effects: &mut CoreEffects,
        req_id: &mut u64,
        req_id_evt: u64,
        playlist_id_evt: i64,
        ids: &[i64],
    ) -> bool {
        let Some((generation, kind)) = self.pending.remove(&req_id_evt) else {
            return false;
        };
        if generation != self.generation {
            return true;
        }
        let PreloadPendingKind::PlaylistDetail { playlist_id } = kind else {
            return true;
        };
        if playlist_id != playlist_id_evt {
            return true;
        }

        if !self.active_playlists.contains(&playlist_id) {
            return true;
        }

        if ids.is_empty() {
            if let Some(p) = app.playlist_preloads.get_mut(&playlist_id) {
                p.status = PreloadStatus::Failed("歌单为空或无法解析".to_owned());
            }
            update_preload_summary(app);
            return true;
        }

        let total = ids.len();
        let mut loader = PlaylistTracksLoad::new(playlist_id, ids.to_vec());

        let rid = next_id(req_id);
        let chunk = loader.next_chunk();
        loader.inflight_req_id = Some(rid);
        self.loaders.insert(playlist_id, loader);
        self.pending.insert(
            rid,
            (
                self.generation,
                PreloadPendingKind::SongsChunk { playlist_id },
            ),
        );

        if let Some(p) = app.playlist_preloads.get_mut(&playlist_id) {
            p.status = PreloadStatus::Loading { loaded: 0, total };
        }
        update_preload_summary(app);

        effects.send_netease_lo(NeteaseCommand::SongDetailByIds {
            req_id: rid,
            ids: chunk,
        });
        true
    }

    pub async fn on_songs(
        &mut self,
        app: &mut App,
        effects: &mut CoreEffects,
        req_id: &mut u64,
        req_id_evt: u64,
        songs: &[crate::app::Song],
    ) -> bool {
        let Some((generation, kind)) = self.pending.remove(&req_id_evt) else {
            return false;
        };
        if generation != self.generation {
            return true;
        }
        let PreloadPendingKind::SongsChunk { playlist_id } = kind else {
            return true;
        };

        if !self.active_playlists.contains(&playlist_id) {
            return true;
        }

        let Some(loader) = self.loaders.get_mut(&playlist_id) else {
            return true;
        };
        if loader.inflight_req_id != Some(req_id_evt) {
            return true;
        }
        loader.inflight_req_id = None;
        loader.songs.extend(songs.iter().cloned());

        if let Some(p) = app.playlist_preloads.get_mut(&playlist_id) {
            p.status = PreloadStatus::Loading {
                loaded: loader.songs.len(),
                total: loader.total,
            };
        }
        update_preload_summary(app);

        if loader.is_done() {
            let Some(loader) = self.loaders.remove(&playlist_id) else {
                tracing::warn!(playlist_id, "预加载 loader 丢失（已完成但无法取出）");
                return true;
            };
            if let Some(p) = app.playlist_preloads.get_mut(&playlist_id) {
                p.status = PreloadStatus::Completed;
                p.songs = loader.songs;
            }
            update_preload_summary(app);
            return true;
        }

        let rid = next_id(req_id);
        let chunk = loader.next_chunk();
        loader.inflight_req_id = Some(rid);
        self.pending.insert(
            rid,
            (
                self.generation,
                PreloadPendingKind::SongsChunk { playlist_id },
            ),
        );
        effects.send_netease_lo(NeteaseCommand::SongDetailByIds {
            req_id: rid,
            ids: chunk,
        });
        true
    }

    pub fn on_error(&mut self, app: &mut App, req_id_evt: u64, message: &str) -> bool {
        let Some((generation, kind)) = self.pending.remove(&req_id_evt) else {
            return false;
        };
        if generation != self.generation {
            return true;
        }

        let playlist_id = match kind {
            PreloadPendingKind::PlaylistDetail { playlist_id } => playlist_id,
            PreloadPendingKind::SongsChunk { playlist_id } => playlist_id,
        };

        if let Some(p) = app.playlist_preloads.get_mut(&playlist_id) {
            p.status = PreloadStatus::Failed(message.to_owned());
            p.songs.clear();
        }
        self.loaders.remove(&playlist_id);
        self.active_playlists.remove(&playlist_id);
        update_preload_summary(app);
        true
    }
}

pub fn update_preload_summary(app: &mut App) {
    if app.playlist_preloads.is_empty() {
        app.preload_summary.clear();
        return;
    }

    let total = app.playlist_preloads.len();
    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut cancelled = 0usize;
    let mut loading = 0usize;
    let mut loaded_sum = 0usize;
    let mut total_sum = 0usize;

    for p in app.playlist_preloads.values() {
        match &p.status {
            PreloadStatus::Completed => completed += 1,
            PreloadStatus::Failed(message) => {
                failed += 1;
                let _ = message.len();
            }
            PreloadStatus::Cancelled => cancelled += 1,
            PreloadStatus::Loading { loaded, total } => {
                loading += 1;
                loaded_sum = loaded_sum.saturating_add(*loaded);
                total_sum = total_sum.saturating_add(*total);
            }
            PreloadStatus::NotStarted => {}
        }
    }

    app.preload_summary = if failed > 0 {
        format!("预加载: {}/{} 完成 | {} 失败", completed, total, failed)
    } else if loading > 0 {
        format!(
            "预加载: {}/{} 完成 | {} 加载中({}/{})",
            completed, total, loading, loaded_sum, total_sum
        )
    } else if cancelled > 0 {
        format!(
            "预加载: {}/{} 完成 | {} 已取消",
            completed, total, cancelled
        )
    } else {
        format!("预加载: {}/{} 完成", completed, total)
    };
}

fn select_preload_targets(
    playlists: &[crate::domain::model::Playlist],
    max_count: usize,
) -> Vec<i64> {
    if max_count == 0 || playlists.is_empty() {
        return vec![];
    }

    let mut out = Vec::with_capacity(max_count);

    if let Some(p) = playlists
        .iter()
        .find(|p| p.special_type == 5 || p.name.contains("我喜欢"))
    {
        out.push(p.id);
    }

    for p in playlists {
        if out.len() >= max_count {
            break;
        }
        if out.contains(&p.id) {
            continue;
        }
        out.push(p.id);
    }

    out
}
