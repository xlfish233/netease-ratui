//! Key-action resolution: maps KeyCode → KeyAction using a HashMap.

use crossterm::event::KeyCode;
use std::collections::HashMap;
use std::sync::Arc;

/// Actions that can be triggered by configurable keybindings.
/// These are the "global" key actions that apply regardless of view/focus.
/// View-specific actions (like text input, list navigation) remain hardcoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAction {
    Quit,
    UiToggleHelp,
    MenuOpen,
    PlayerTogglePause,
    PlayerPrev,
    PlayerNext,
    PlayerCycleMode,
    PlayerStop,
}

/// Parse an action name string into a KeyAction.
/// Returns None for unknown action names.
pub fn action_from_str(s: &str) -> Option<KeyAction> {
    match s {
        "Quit" => Some(KeyAction::Quit),
        "UiToggleHelp" => Some(KeyAction::UiToggleHelp),
        "MenuOpen" => Some(KeyAction::MenuOpen),
        "PlayerTogglePause" => Some(KeyAction::PlayerTogglePause),
        "PlayerPrev" => Some(KeyAction::PlayerPrev),
        "PlayerNext" => Some(KeyAction::PlayerNext),
        "PlayerCycleMode" => Some(KeyAction::PlayerCycleMode),
        "PlayerStop" => Some(KeyAction::PlayerStop),
        _ => None,
    }
}

/// A collection of key-to-action bindings with efficient HashMap lookup.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    /// Maps KeyCode → KeyAction for O(1) lookup.
    map: HashMap<KeyCode, KeyAction>,
}

impl KeyBindings {
    /// Create an empty binding set (no bindings at all).
    pub fn empty() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Create with the built-in default keybindings.
    fn new_default() -> Self {
        let mut map = HashMap::new();

        // Global actions
        map.insert(KeyCode::Char('q'), KeyAction::Quit);
        map.insert(KeyCode::Char('?'), KeyAction::UiToggleHelp);
        map.insert(KeyCode::Char('m'), KeyAction::MenuOpen);
        map.insert(KeyCode::Char(' '), KeyAction::PlayerTogglePause);
        map.insert(KeyCode::Char('['), KeyAction::PlayerPrev);
        map.insert(KeyCode::Char(']'), KeyAction::PlayerNext);
        map.insert(KeyCode::Char('M'), KeyAction::PlayerCycleMode);

        Self { map }
    }

    /// Look up a key code and return the associated action.
    pub fn resolve(&self, key: KeyCode) -> Option<KeyAction> {
        self.map.get(&key).copied()
    }

    /// Bind a key to an action, overwriting any previous binding for that key.
    pub fn bind_key(&mut self, key: KeyCode, action: KeyAction) {
        self.map.insert(key, action);
    }

    /// Remove all bindings for a specific action.
    pub fn unbind_action(&mut self, action: &KeyAction) {
        self.map.retain(|_, a| a != action);
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::new_default()
    }
}

/// Shared, immutable keybindings used by AppSnapshot.
pub type SharedKeyBindings = Arc<KeyBindings>;
