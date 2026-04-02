use std::time::Duration;

use netease_ratui::audio_worker::AudioBackend;
use netease_ratui::core::spawn_app_actor;
use netease_ratui::messages::app::AppCommand;
use netease_ratui::netease::NeteaseClientConfig;
use netease_ratui::player_state::load_player_state_async;

#[tokio::test]
async fn quit_waits_for_final_state_save() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = NeteaseClientConfig {
        data_dir: dir.path().to_path_buf(),
        ..Default::default()
    };

    let (tx, _rx, app_actor) = spawn_app_actor(cfg, AudioBackend::Null);
    tx.send(AppCommand::Quit).await.expect("send quit");
    drop(tx);

    tokio::time::timeout(Duration::from_secs(5), app_actor)
        .await
        .expect("app actor should finish promptly")
        .expect("app actor join should succeed");

    let snapshot = load_player_state_async(dir.path())
        .await
        .expect("player_state.json should exist after quit");
    assert_eq!(snapshot.version, 3);
}
