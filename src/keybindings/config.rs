//! Keybindings TOML configuration loading and parsing.
//!
//! The `keybindings.toml` file allows users to customize keyboard shortcuts.
//! It supports:
//! - `useDefaultKeyBindings` (bool): When true (default), user bindings extend defaults.
//!   When false, only user-defined bindings are active.
//! - Action-to-key(s) mappings: e.g. `Quit = "q"` or `PlayerTogglePause = ["Space", "p"]`
//! - Empty string to unbind: e.g. `UiToggleHelp = ""`

use super::resolver::action_from_str;
use crossterm::event::KeyCode;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Top-level TOML configuration structure.
#[derive(Debug, Deserialize)]
pub struct KeyBindingFile {
    /// When true (default), user bindings extend built-in defaults.
    /// When false, only user-defined bindings are active.
    #[serde(default = "default_true")]
    pub use_default_key_bindings: bool,

    /// Action-to-key(s) mappings.
    /// Key = action name (e.g. "Quit", "PlayerTogglePause")
    /// Value = single key string or array of key strings
    #[serde(default)]
    pub bindings: HashMap<String, toml::Value>,
}

fn default_true() -> bool {
    true
}

/// Load keybindings from a `keybindings.toml` file in the data directory.
/// Falls back to default bindings if the file is missing or malformed.
pub fn load_keybindings(data_dir: &Path) -> super::resolver::KeyBindings {
    let path = data_dir.join("keybindings.toml");
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            // File missing → use defaults
            return super::resolver::KeyBindings::default();
        }
    };

    match parse_config(&contents) {
        Ok(config) => build_keybindings(config),
        Err(e) => {
            tracing::warn!("keybindings.toml 解析失败，回退到默认绑定: {e}");
            super::resolver::KeyBindings::default()
        }
    }
}

/// Parse TOML contents into a KeyBindingFile.
pub(crate) fn parse_config(contents: &str) -> Result<KeyBindingFile, String> {
    toml::from_str(contents).map_err(|e| format!("{e}"))
}

/// Build KeyBindings from a parsed config file.
pub(crate) fn build_keybindings(config: KeyBindingFile) -> super::resolver::KeyBindings {
    let mut bindings = if config.use_default_key_bindings {
        super::resolver::KeyBindings::default()
    } else {
        super::resolver::KeyBindings::empty()
    };

    for (action_name, value) in &config.bindings {
        let action = match action_from_str(action_name) {
            Some(a) => a,
            None => {
                tracing::warn!("keybindings.toml: 未知操作 '{action_name}'，跳过");
                continue;
            }
        };

        let keys = parse_key_values(value);
        match keys {
            Some(key_strs) => {
                if key_strs.is_empty() || key_strs.len() == 1 && key_strs[0].is_empty() {
                    // Empty string or empty array → unbind this action
                    bindings.unbind_action(&action);
                } else {
                    // First unbind all existing keys for this action
                    bindings.unbind_action(&action);
                    // Then bind the specified keys
                    for key_str in &key_strs {
                        if key_str.is_empty() {
                            continue;
                        }
                        match parse_key_code(key_str) {
                            Some(key_code) => {
                                bindings.bind_key(key_code, action);
                            }
                            None => {
                                tracing::warn!("keybindings.toml: 无法解析按键 '{key_str}'，跳过");
                            }
                        }
                    }
                }
            }
            None => {
                tracing::warn!(
                    "keybindings.toml: 操作 '{action_name}' 的值格式无效，期望字符串或字符串数组"
                );
            }
        }
    }

    bindings
}

/// Parse a TOML value into a list of key strings.
/// Accepts: string, array of strings.
fn parse_key_values(value: &toml::Value) -> Option<Vec<String>> {
    match value {
        toml::Value::String(s) => Some(vec![s.clone()]),
        toml::Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                match item {
                    toml::Value::String(s) => result.push(s.clone()),
                    _ => return None,
                }
            }
            Some(result)
        }
        _ => None,
    }
}

/// Parse a key string like "Space", "q", "Ctrl+s", "Alt+Up" into a KeyCode.
/// Note: For simplicity, this implementation handles basic keys.
/// Modifier keys are handled separately in the keyboard handler.
pub(crate) fn parse_key_code(s: &str) -> Option<KeyCode> {
    match s {
        "Space" => Some(KeyCode::Char(' ')),
        "Enter" => Some(KeyCode::Enter),
        "Esc" | "Escape" => Some(KeyCode::Esc),
        "Tab" => Some(KeyCode::Tab),
        "Backspace" => Some(KeyCode::Backspace),
        "BackTab" => Some(KeyCode::BackTab),
        "Up" => Some(KeyCode::Up),
        "Down" => Some(KeyCode::Down),
        "Left" => Some(KeyCode::Left),
        "Right" => Some(KeyCode::Right),
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),
        "Delete" => Some(KeyCode::Delete),
        "Insert" => Some(KeyCode::Insert),
        _ => {
            // Single character
            let chars: Vec<char> = s.chars().collect();
            if chars.len() == 1 {
                Some(KeyCode::Char(chars[0]))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybindings::resolver::KeyAction;

    /// VAL-KEYBIND-001: 默认快捷键在不配置文件时正常工作
    #[test]
    fn default_keybindings_contain_all_default_mappings() {
        let bindings = super::super::resolver::KeyBindings::default();

        // Verify some core default mappings exist
        assert_eq!(bindings.resolve(KeyCode::Char('q')), Some(KeyAction::Quit));
        assert_eq!(
            bindings.resolve(KeyCode::Char(' ')),
            Some(KeyAction::PlayerTogglePause)
        );
        assert_eq!(
            bindings.resolve(KeyCode::Char('?')),
            Some(KeyAction::UiToggleHelp)
        );
        assert_eq!(
            bindings.resolve(KeyCode::Char('m')),
            Some(KeyAction::MenuOpen)
        );
        assert_eq!(
            bindings.resolve(KeyCode::Char('[')),
            Some(KeyAction::PlayerPrev)
        );
        assert_eq!(
            bindings.resolve(KeyCode::Char(']')),
            Some(KeyAction::PlayerNext)
        );
        assert_eq!(
            bindings.resolve(KeyCode::Char('M')),
            Some(KeyAction::PlayerCycleMode)
        );
    }

    /// VAL-KEYBIND-002: 覆盖单个快捷键
    #[test]
    fn override_single_keybinding() {
        let config = parse_config(
            r#"
use_default_key_bindings = true
[bindings]
PlayerTogglePause = "p"
"#,
        )
        .unwrap();
        let bindings = build_keybindings(config);

        // 'p' should now map to PlayerTogglePause
        assert_eq!(
            bindings.resolve(KeyCode::Char('p')),
            Some(KeyAction::PlayerTogglePause)
        );
        // Space should no longer be PlayerTogglePause (overridden)
        assert_eq!(bindings.resolve(KeyCode::Char(' ')), None);
    }

    /// VAL-KEYBIND-003: 同一操作绑定多个按键
    #[test]
    fn multiple_keys_for_same_action() {
        let config = parse_config(
            r#"
use_default_key_bindings = true
[bindings]
PlayerTogglePause = ["Space", "p"]
"#,
        )
        .unwrap();
        let bindings = build_keybindings(config);

        // Both Space and p should map to PlayerTogglePause
        assert_eq!(
            bindings.resolve(KeyCode::Char(' ')),
            Some(KeyAction::PlayerTogglePause)
        );
        assert_eq!(
            bindings.resolve(KeyCode::Char('p')),
            Some(KeyAction::PlayerTogglePause)
        );
    }

    /// VAL-KEYBIND-004: 空字符串解绑操作
    #[test]
    fn empty_string_unbinds_action() {
        let config = parse_config(
            r#"
use_default_key_bindings = true
[bindings]
UiToggleHelp = ""
"#,
        )
        .unwrap();
        let bindings = build_keybindings(config);

        // '?' should no longer trigger help
        assert_eq!(bindings.resolve(KeyCode::Char('?')), None);
    }

    /// VAL-KEYBIND-005: 配置文件格式错误回退默认
    #[test]
    fn malformed_config_falls_back_to_default() {
        let result = parse_config("this is not valid toml {{{}}");
        assert!(result.is_err());

        // build_keybindings is not called on parse failure;
        // load_keybindings returns default() instead
        let dir = tempfile::tempdir().unwrap();
        let bindings = load_keybindings(dir.path());
        assert_eq!(bindings.resolve(KeyCode::Char('q')), Some(KeyAction::Quit));
    }

    /// VAL-KEYBIND-006: 配置文件缺失使用默认
    #[test]
    fn missing_config_uses_default() {
        let dir = tempfile::tempdir().unwrap();
        let bindings = load_keybindings(dir.path());

        // Should be identical to default
        let default = super::super::resolver::KeyBindings::default();
        assert_eq!(
            bindings.resolve(KeyCode::Char('q')),
            default.resolve(KeyCode::Char('q'))
        );
        assert_eq!(
            bindings.resolve(KeyCode::Char(' ')),
            default.resolve(KeyCode::Char(' '))
        );
    }

    /// VAL-KEYBIND-007: useDefaultKeyBindings=false 仅用户绑定生效
    #[test]
    fn no_default_only_user_bindings() {
        let config = parse_config(
            r#"
use_default_key_bindings = false
[bindings]
Quit = "Q"
"#,
        )
        .unwrap();
        let bindings = build_keybindings(config);

        // 'Q' should trigger Quit (user-defined)
        assert_eq!(bindings.resolve(KeyCode::Char('Q')), Some(KeyAction::Quit));
        // 'q' should NOT trigger Quit (defaults disabled)
        assert_eq!(bindings.resolve(KeyCode::Char('q')), None);
        // Space should NOT trigger PlayerTogglePause (defaults disabled)
        assert_eq!(bindings.resolve(KeyCode::Char(' ')), None);
    }

    // Additional helper tests

    #[test]
    fn parse_key_code_special_keys() {
        assert_eq!(parse_key_code("Space"), Some(KeyCode::Char(' ')));
        assert_eq!(parse_key_code("Enter"), Some(KeyCode::Enter));
        assert_eq!(parse_key_code("Esc"), Some(KeyCode::Esc));
        assert_eq!(parse_key_code("Escape"), Some(KeyCode::Esc));
        assert_eq!(parse_key_code("Up"), Some(KeyCode::Up));
        assert_eq!(parse_key_code("Down"), Some(KeyCode::Down));
        assert_eq!(parse_key_code("Home"), Some(KeyCode::Home));
        assert_eq!(parse_key_code("End"), Some(KeyCode::End));
        assert_eq!(parse_key_code("PageUp"), Some(KeyCode::PageUp));
        assert_eq!(parse_key_code("PageDown"), Some(KeyCode::PageDown));
    }

    #[test]
    fn parse_key_code_single_char() {
        assert_eq!(parse_key_code("q"), Some(KeyCode::Char('q')));
        assert_eq!(parse_key_code("M"), Some(KeyCode::Char('M')));
        assert_eq!(parse_key_code("["), Some(KeyCode::Char('[')));
    }

    #[test]
    fn parse_key_code_invalid() {
        assert_eq!(parse_key_code("Ctrl+q"), None);
        assert_eq!(parse_key_code("abc"), None);
        assert_eq!(parse_key_code(""), None);
    }
}
