use crate::app::App;

#[derive(Debug)]
pub enum AppCommand {
    Bootstrap,
    TabNext,
    TabTo { index: usize },
    LoginGenerateQr,
    LoginToggleCookieInput,
    LoginCookieInputChar { c: char },
    LoginCookieInputBackspace,
    LoginCookieSubmit,
    SearchInputChar { c: char },
    SearchInputBackspace,
    SearchSubmit,
    SearchMoveUp,
    SearchMoveDown,
    SearchPlaySelected,
    PlaylistsMoveUp,
    PlaylistsMoveDown,
    PlaylistsOpenSelected,
    PlaylistTracksMoveUp,
    PlaylistTracksMoveDown,
    PlaylistTracksPlaySelected,
    Back,
    PlayerTogglePause,
    PlayerStop,
    PlayerPrev,
    PlayerNext,
    PlayerSeekBackwardMs { ms: u64 },
    PlayerSeekForwardMs { ms: u64 },
    PlayerVolumeDown,
    PlayerVolumeUp,
    PlayerCycleMode,
    LyricsToggleFollow,
    LyricsMoveUp,
    LyricsMoveDown,
    LyricsGotoCurrent,
    LyricsOffsetAddMs { ms: i64 },
    SettingsMoveUp,
    SettingsMoveDown,
    SettingsDecrease,
    SettingsIncrease,
    SettingsActivate,
    Quit,
}

#[derive(Debug)]
pub enum AppEvent {
    State(Box<App>),
    #[allow(dead_code)]
    Toast(String),
    #[allow(dead_code)]
    Error(String),
}
