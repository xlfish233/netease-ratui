use crate::app::App;

#[derive(Debug)]
pub enum AppCommand {
    Bootstrap,
    TabNext,
    LoginGenerateQr,
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
    Toast(String),
    Error(String),
}
