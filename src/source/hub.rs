#![allow(dead_code)]

use crate::domain::ids::{SourceId, TrackKey};
use crate::messages::source::{Playable, QualityHint, SourceCommand, SourceEvent, TrackSummary};
use crate::netease::NeteaseClientConfig;
use crate::netease::actor::{NeteaseCommand, NeteaseEvent};
use tokio::sync::mpsc;

pub struct SourceHubHandles {
    pub tx_source: mpsc::Sender<SourceCommand>,
    pub rx_source: mpsc::Receiver<SourceEvent>,
    pub tx_netease_hi: mpsc::Sender<NeteaseCommand>,
    pub tx_netease_lo: mpsc::Sender<NeteaseCommand>,
    pub rx_netease: mpsc::Receiver<NeteaseEvent>,
}

/// 统一音源 Hub（当前只集成 Netease，但对 core 暴露双接口）：
/// - `SourceCommand/SourceEvent`：面向“可插拔音源”的统一接口
/// - `NeteaseCommand/NeteaseEvent`：保留旧接口，便于逐步迁移
pub fn spawn_source_hub(cfg: NeteaseClientConfig) -> SourceHubHandles {
    let (tx_source_cmd, mut rx_source_cmd) = mpsc::channel::<SourceCommand>(64);
    let (tx_source_evt, rx_source_evt) = mpsc::channel::<SourceEvent>(64);

    let (tx_netease_hi, tx_netease_lo, mut rx_netease_evt) =
        crate::netease::actor::spawn_netease_actor(cfg);
    let (tx_netease_evt_out, rx_netease_evt_out) = mpsc::channel::<NeteaseEvent>(64);
    let tx_netease_hi_for_task = tx_netease_hi.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(cmd) = rx_source_cmd.recv() => {
                    handle_source_command(&tx_netease_hi_for_task, &tx_source_evt, cmd).await;
                }
                Some(evt) = rx_netease_evt.recv() => {
                    handle_netease_event(&tx_source_evt, &evt).await;
                    let _ = tx_netease_evt_out.send(evt).await;
                }
                else => break,
            }
        }
    });

    SourceHubHandles {
        tx_source: tx_source_cmd,
        rx_source: rx_source_evt,
        tx_netease_hi,
        tx_netease_lo,
        rx_netease: rx_netease_evt_out,
    }
}

async fn handle_source_command(
    tx_netease_hi: &mpsc::Sender<NeteaseCommand>,
    tx_evt: &mpsc::Sender<SourceEvent>,
    cmd: SourceCommand,
) {
    match cmd {
        SourceCommand::Init { req_id, source } => {
            if matches!(source, SourceId::Netease) {
                let _ = tx_netease_hi.send(NeteaseCommand::Init { req_id }).await;
            } else {
                let _ = tx_evt
                    .send(SourceEvent::Error {
                        req_id,
                        track: None,
                        message: format!("Init not supported for source={source}"),
                    })
                    .await;
            }
        }
        SourceCommand::SearchTracks {
            req_id,
            source,
            keywords,
            limit,
            offset,
        } => {
            if matches!(source, SourceId::Netease) {
                let _ = tx_netease_hi
                    .send(NeteaseCommand::CloudSearchSongs {
                        req_id,
                        keywords,
                        limit,
                        offset,
                    })
                    .await;
            } else {
                let _ = tx_evt
                    .send(SourceEvent::Error {
                        req_id,
                        track: None,
                        message: format!("SearchTracks not supported for source={source}"),
                    })
                    .await;
            }
        }
        SourceCommand::ResolvePlayable {
            req_id,
            track,
            quality,
        } => {
            let SourceId::Netease = track.source else {
                let _ = tx_evt
                    .send(SourceEvent::Error {
                        req_id,
                        track: Some(track),
                        message: "ResolvePlayable not supported for this source".to_owned(),
                    })
                    .await;
                return;
            };
            let Some(song_id) = track.id.as_netease_song_id() else {
                let _ = tx_evt
                    .send(SourceEvent::Error {
                        req_id,
                        track: Some(track),
                        message: "invalid netease track id".to_owned(),
                    })
                    .await;
                return;
            };
            let br = match quality {
                Some(QualityHint::Bitrate(v)) => v,
                None => 999_000,
            };
            let _ = tx_netease_hi
                .send(NeteaseCommand::SongUrl {
                    req_id,
                    id: song_id,
                    br,
                })
                .await;
        }
        SourceCommand::Lyric { req_id, track } => {
            let SourceId::Netease = track.source else {
                let _ = tx_evt
                    .send(SourceEvent::Error {
                        req_id,
                        track: Some(track),
                        message: "Lyric not supported for this source".to_owned(),
                    })
                    .await;
                return;
            };
            let Some(song_id) = track.id.as_netease_song_id() else {
                let _ = tx_evt
                    .send(SourceEvent::Error {
                        req_id,
                        track: Some(track),
                        message: "invalid netease track id".to_owned(),
                    })
                    .await;
                return;
            };
            let _ = tx_netease_hi
                .send(NeteaseCommand::Lyric { req_id, song_id })
                .await;
        }
    }
}

async fn handle_netease_event(tx_evt: &mpsc::Sender<SourceEvent>, evt: &NeteaseEvent) {
    match evt {
        NeteaseEvent::ClientReady { req_id, .. } | NeteaseEvent::AnonymousReady { req_id } => {
            let _ = tx_evt.send(SourceEvent::Ready { req_id: *req_id }).await;
        }
        NeteaseEvent::SearchSongs { req_id, songs } => {
            let tracks = songs
                .iter()
                .map(|s| TrackSummary {
                    key: TrackKey::netease(s.id),
                    title: s.name.clone(),
                    artists: s.artists.clone(),
                })
                .collect();
            let _ = tx_evt
                .send(SourceEvent::SearchTracks {
                    req_id: *req_id,
                    tracks,
                })
                .await;
        }
        NeteaseEvent::SongUrl { req_id, song_url } => {
            let track = TrackKey::netease(song_url.id);
            let playable = Playable::RemoteUrl {
                url: song_url.url.clone(),
            };
            let _ = tx_evt
                .send(SourceEvent::PlayableResolved {
                    req_id: *req_id,
                    track,
                    playable,
                })
                .await;
        }
        NeteaseEvent::SongUrlUnavailable { req_id, id } => {
            let _ = tx_evt
                .send(SourceEvent::Error {
                    req_id: *req_id,
                    track: Some(TrackKey::netease(*id)),
                    message: "song url unavailable".to_owned(),
                })
                .await;
        }
        NeteaseEvent::Lyric {
            req_id,
            song_id,
            lyrics,
        } => {
            let _ = tx_evt
                .send(SourceEvent::Lyric {
                    req_id: *req_id,
                    track: TrackKey::netease(*song_id),
                    lrc: lyrics.clone(),
                })
                .await;
        }
        NeteaseEvent::Error { req_id, message } => {
            let _ = tx_evt
                .send(SourceEvent::Error {
                    req_id: *req_id,
                    track: None,
                    message: message.to_string(),
                })
                .await;
        }

        // Not mapped yet (login/playlists, etc.)
        other => {
            tracing::trace!(evt = ?other, "SourceHub ignoring unmapped netease event");
        }
    }
}
