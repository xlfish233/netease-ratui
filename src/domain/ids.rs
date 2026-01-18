#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;

/// Music source identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    Netease,
    Local,
    /// Forward-compatible/custom sources.
    Other(String),
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceId::Netease => write!(f, "netease"),
            SourceId::Local => write!(f, "local"),
            SourceId::Other(v) => write!(f, "{v}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackKey {
    pub source: SourceId,
    pub id: TrackId,
}

impl TrackKey {
    pub fn netease(song_id: i64) -> Self {
        Self {
            source: SourceId::Netease,
            id: TrackId::Netease { song_id },
        }
    }

    pub fn local(library_id: impl Into<String>, rel_path: impl Into<String>) -> Self {
        Self {
            source: SourceId::Local,
            id: TrackId::Local {
                library_id: library_id.into(),
                rel_path: rel_path.into(),
                fingerprint: None,
            },
        }
    }
}

impl fmt::Display for TrackKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.id {
            TrackId::Netease { song_id } => write!(f, "{}:{song_id}", self.source),
            TrackId::Local {
                library_id,
                rel_path,
                ..
            } => write!(f, "{}:{library_id}:{rel_path}", self.source),
            TrackId::Opaque { namespace, id } => write!(f, "{}:{namespace}:{id}", self.source),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub size_bytes: u64,
    pub mtime_epoch_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrackId {
    Netease {
        song_id: i64,
    },
    /// Local track: identified by a library and a stable relative path.
    ///
    /// `fingerprint` is optional and can be used to detect "same rel_path but different file"
    /// (e.g., replace-in-place).
    Local {
        library_id: String,
        rel_path: String,
        #[serde(default)]
        fingerprint: Option<FileFingerprint>,
    },
    /// Forward-compatible/custom track id.
    Opaque {
        namespace: String,
        id: String,
    },
}

impl TrackId {
    pub fn as_netease_song_id(&self) -> Option<i64> {
        match self {
            TrackId::Netease { song_id } => Some(*song_id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_key_serde_roundtrip() {
        let k = TrackKey {
            source: SourceId::Local,
            id: TrackId::Local {
                library_id: "music".to_owned(),
                rel_path: "foo/bar.mp3".to_owned(),
                fingerprint: Some(FileFingerprint {
                    size_bytes: 123,
                    mtime_epoch_ms: 1_700_000_000_000,
                }),
            },
        };

        let s = serde_json::to_string(&k).unwrap();
        let back: TrackKey = serde_json::from_str(&s).unwrap();
        assert_eq!(back, k);
    }
}
