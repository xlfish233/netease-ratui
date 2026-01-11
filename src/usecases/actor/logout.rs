use crate::app::{App, PlaylistMode, View};

pub(super) fn reset_app_after_logout(app: &mut App) {
    app.logged_in = false;
    app.view = View::Login;

    app.login_qr_url = None;
    app.login_qr_ascii = None;
    app.login_unikey = None;
    app.login_status = "按 l 生成二维码；q 退出；Tab 切换页面".to_owned();

    app.account_uid = None;
    app.account_nickname = None;
    app.playlists.clear();
    app.playlists_selected = 0;
    app.playlist_mode = PlaylistMode::List;
    app.playlist_tracks.clear();
    app.playlist_tracks_selected = 0;
    app.playlists_status = "等待登录后加载歌单".to_owned();

    app.playlist_preloads.clear();
    app.preload_summary.clear();

    app.search_results.clear();
    app.search_selected = 0;
    app.search_status = "输入关键词，回车搜索".to_owned();

    app.queue.clear();
    app.queue_pos = None;
    app.now_playing = None;
    app.play_status = "未播放".to_owned();
    app.paused = false;
    app.play_started_at = None;
    app.play_total_ms = None;
    app.play_paused_at = None;
    app.play_paused_accum_ms = 0;
    app.play_id = None;
    app.play_song_id = None;
    app.play_error_count = 0;

    app.lyrics_song_id = None;
    app.lyrics.clear();
    app.lyrics_status = "暂无歌词".to_owned();
    app.lyrics_follow = true;
    app.lyrics_selected = 0;
}
