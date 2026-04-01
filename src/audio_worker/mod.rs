mod cache;
mod download;
mod engine;
mod fade;
mod messages;
mod null_engine;
mod player;
mod streaming;
mod transfer;
mod worker;

pub use messages::{
    AudioBufferState, AudioCommand, AudioEvent, AudioLoadStage, AudioPlaybackMode, AudioStreamHint,
};
#[allow(unused_imports)]
pub use streaming::{ProgressiveReader, SessionSnapshot, StreamingSession};
pub use transfer::TransferConfig;

#[derive(Debug, Clone, Copy)]
pub enum AudioBackend {
    Real,
    Null,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioSettings {
    pub crossfade_ms: u64,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { crossfade_ms: 300 }
    }
}

pub use worker::spawn_audio_worker;
