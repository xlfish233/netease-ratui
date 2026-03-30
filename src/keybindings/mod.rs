pub mod config;
pub mod resolver;

pub use config::load_keybindings;
pub use resolver::{KeyAction, KeyBindings, SharedKeyBindings};
