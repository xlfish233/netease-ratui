use std::path::PathBuf;
use tokio::select;
use tokio::sync::mpsc;

use super::AudioSettings;
use super::messages::{AudioCommand, AudioEvent};
use super::transfer::{
    CacheKey, Priority, TransferCommand, TransferConfig, TransferEvent, TransferReceiver,
    TransferSender, spawn_transfer_actor_with_config,
};

struct NullEngine {
    tx_evt: mpsc::Sender<AudioEvent>,
    rx_cmd: mpsc::Receiver<AudioCommand>,
    tx_transfer: TransferSender,
    rx_transfer: TransferReceiver,
    play_id: u64,
    paused: bool,
    _settings: AudioSettings,
}

impl NullEngine {
    fn new(
        tx_evt: mpsc::Sender<AudioEvent>,
        rx_cmd: mpsc::Receiver<AudioCommand>,
        tx_transfer: TransferSender,
        rx_transfer: TransferReceiver,
        settings: AudioSettings,
    ) -> Self {
        Self {
            tx_evt,
            rx_cmd,
            tx_transfer,
            rx_transfer,
            play_id: 0,
            paused: false,
            _settings: settings,
        }
    }

    async fn run(mut self) {
        loop {
            select! {
                maybe_evt = self.rx_transfer.recv() => {
                    let Some(evt) = maybe_evt else {
                        break;
                    };
                    if let TransferEvent::CacheCleared { files, bytes } = evt {
                        let _ = self.tx_evt.send(AudioEvent::CacheCleared { files, bytes }).await;
                    }
                }
                maybe_cmd = self.rx_cmd.recv() => {
                    let Some(cmd) = maybe_cmd else {
                        break;
                    };
                    self.handle_audio_command(cmd).await;
                }
            }
        }
    }

    async fn handle_audio_command(&mut self, cmd: AudioCommand) {
        match cmd {
            AudioCommand::PlayTrack { id, title, .. } => {
                self.play_id = self.play_id.wrapping_add(1).max(1);
                self.paused = false;
                let _ = self
                    .tx_evt
                    .send(AudioEvent::NowPlaying {
                        song_id: id,
                        play_id: self.play_id,
                        title,
                        duration_ms: None,
                    })
                    .await;
            }
            AudioCommand::TogglePause => {
                self.paused = !self.paused;
                let _ = self.tx_evt.send(AudioEvent::Paused(self.paused)).await;
            }
            AudioCommand::Stop => {
                self.paused = false;
                let _ = self.tx_evt.send(AudioEvent::Stopped).await;
            }
            AudioCommand::SeekToMs(_) => {}
            AudioCommand::SetVolume(_) => {}
            AudioCommand::SetCrossfadeMs(_) => {}
            AudioCommand::ClearCache => {
                let _ = self
                    .tx_transfer
                    .send(TransferCommand::ClearAll { keep: None })
                    .await;
            }
            AudioCommand::SetCacheBr(br) => {
                let _ = self
                    .tx_transfer
                    .send(TransferCommand::PurgeNotBr { br, keep: None })
                    .await;
            }
            AudioCommand::PrefetchAudio { id, br, url, title } => {
                let key = CacheKey { song_id: id, br };
                let _ = self
                    .tx_transfer
                    .send(TransferCommand::EnsureCached {
                        token: 0,
                        key,
                        url,
                        title,
                        priority: Priority::Low,
                    })
                    .await;
            }
        }
    }
}

pub(super) fn spawn(
    rx_cmd: mpsc::Receiver<AudioCommand>,
    tx_evt: mpsc::Sender<AudioEvent>,
    data_dir: PathBuf,
    transfer_config: TransferConfig,
    settings: AudioSettings,
) {
    let (tx_transfer, rx_transfer) = spawn_transfer_actor_with_config(data_dir, transfer_config);
    tokio::spawn(async move {
        let engine = NullEngine::new(tx_evt, rx_cmd, tx_transfer, rx_transfer, settings);
        engine.run().await;
    });
}
