use std::path::PathBuf;
use tokio::sync::mpsc;

use super::engine;
use super::messages::{AudioCommand, AudioEvent};
use super::null_engine;
use super::transfer::TransferConfig;
use super::{AudioBackend, AudioSettings};

pub fn spawn_audio_worker(
    backend: AudioBackend,
    data_dir: PathBuf,
    transfer_config: TransferConfig,
    settings: AudioSettings,
) -> (mpsc::Sender<AudioCommand>, mpsc::Receiver<AudioEvent>) {
    let (tx_cmd, rx_cmd) = mpsc::channel::<AudioCommand>(64);
    let (tx_evt, rx_evt) = mpsc::channel::<AudioEvent>(64);

    match backend {
        AudioBackend::Real => {
            engine::spawn(rx_cmd, tx_evt, data_dir, transfer_config, settings);
        }
        AudioBackend::Null => {
            null_engine::spawn(rx_cmd, tx_evt, data_dir, transfer_config, settings);
        }
    }

    (tx_cmd, rx_evt)
}
