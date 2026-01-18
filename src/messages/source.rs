#![allow(dead_code)]

use crate::domain::ids::SourceId;
use crate::domain::ids::TrackKey;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QualityHint {
    Bitrate(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Playable {
    RemoteUrl { url: String },
    LocalPath { path: PathBuf },
}

#[derive(Debug, Clone)]
pub enum SourceCommand {
    Init {
        req_id: u64,
        source: SourceId,
    },

    SearchTracks {
        req_id: u64,
        source: SourceId,
        keywords: String,
        limit: i64,
        offset: i64,
    },

    ResolvePlayable {
        req_id: u64,
        track: TrackKey,
        quality: Option<QualityHint>,
    },

    Lyric {
        req_id: u64,
        track: TrackKey,
    },
}

#[derive(Debug, Clone)]
pub enum SourceEvent {
    Ready {
        req_id: u64,
    },

    SearchTracks {
        req_id: u64,
        tracks: Vec<TrackSummary>,
    },

    PlayableResolved {
        req_id: u64,
        track: TrackKey,
        playable: Playable,
    },

    Lyric {
        req_id: u64,
        track: TrackKey,
        lrc: Vec<crate::domain::model::LyricLine>,
    },

    Error {
        req_id: u64,
        track: Option<TrackKey>,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackSummary {
    pub key: TrackKey,
    pub title: String,
    pub artists: String,
}
