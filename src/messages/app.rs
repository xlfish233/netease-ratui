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
    Quit,
}

#[derive(Debug)]
pub enum AppEvent {
    State(App),
    Toast(String),
    Error(String),
}
