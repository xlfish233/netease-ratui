use crate::app::{AppSnapshot, UiFocus};
use crate::error::MessageError;

#[derive(Debug)]
pub enum AppCommand {
    Bootstrap,
    TabNext,
    TabTo {
        index: usize,
    },
    UiFocusNext,
    UiFocusPrev,
    UiFocusSet {
        focus: UiFocus,
    },
    UiToggleHelp,
    LoginGenerateQr,
    LoginToggleCookieInput,
    LoginCookieInputChar {
        c: char,
    },
    LoginCookieInputBackspace,
    LoginCookieSubmit,
    SearchInputChar {
        c: char,
    },
    SearchInputBackspace,
    SearchSubmit,
    SearchMoveUp,
    SearchMoveDown,
    SearchMoveTo {
        index: usize,
    },
    SearchPageDown,
    SearchPageUp,
    SearchJumpTop,
    SearchJumpBottom,
    SearchPlaySelected,
    PlaylistsMoveUp,
    PlaylistsMoveDown,
    PlaylistsMoveTo {
        index: usize,
    },
    PlaylistsPageDown,
    PlaylistsPageUp,
    PlaylistsJumpTop,
    PlaylistsJumpBottom,
    PlaylistsOpenSelected,
    PlaylistTracksMoveUp,
    PlaylistTracksMoveDown,
    PlaylistTracksMoveTo {
        index: usize,
    },
    PlaylistTracksPageDown,
    PlaylistTracksPageUp,
    PlaylistTracksJumpTop,
    PlaylistTracksJumpBottom,
    PlaylistTracksPlaySelected,
    Back,
    PlayerTogglePause,
    PlayerStop,
    PlayerPrev,
    PlayerNext,
    PlayerSeekBackwardMs {
        ms: u64,
    },
    PlayerSeekForwardMs {
        ms: u64,
    },
    PlayerSeekAbsoluteMs {
        ms: u64,
    },
    PlayerVolumeDown,
    PlayerVolumeUp,
    PlayerCycleMode,
    LyricsToggleFollow,
    LyricsMoveUp,
    LyricsMoveDown,
    LyricsGotoCurrent,
    LyricsOffsetAddMs {
        ms: i64,
    },
    SettingsDecrease,
    SettingsIncrease,
    SettingsActivate,
    SettingsGroupPrev,
    SettingsGroupNext,
    SettingsItemPrev,
    SettingsItemNext,
    Quit,
    #[allow(dead_code)]
    ToastDismiss,
    MenuOpen,
    MenuCancel,
    MenuSelect,
    MenuMoveUp,
    MenuMoveDown,
}

#[derive(Debug)]
pub enum AppEvent {
    State(Box<AppSnapshot>),
    #[allow(dead_code)]
    Toast(String),
    #[allow(dead_code)]
    Error(MessageError),
}
