use netease_ratui::app::App;
use netease_ratui::core::CoreEffects;
use netease_ratui::core::infra::{RequestKey, RequestTracker};
use netease_ratui::domain::model::Song;
use netease_ratui::features::search::handle_search_command;
use netease_ratui::messages::app::AppCommand;

#[tokio::test]
async fn search_play_selected_uses_song_url_request() {
    let mut app = App::default();
    app.search_results.push(Song {
        id: 1,
        name: "Song".to_owned(),
        artists: "Artist".to_owned(),
        ..Default::default()
    });
    let mut req_id = 1u64;
    let mut tracker = RequestTracker::new();
    let mut titles = std::collections::HashMap::new();
    let mut effects = CoreEffects::default();

    handle_search_command(
        AppCommand::SearchPlaySelected,
        &mut app,
        &mut req_id,
        &mut tracker,
        &mut titles,
        &mut effects,
    )
    .await;

    assert!(tracker.accept(&RequestKey::SongUrl, 1));
    assert!(titles.contains_key(&1));
}
