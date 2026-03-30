# KeyBindings Patterns

## SharedKeyBindings (Arc Pattern)

`KeyBindings` is wrapped in `Arc<KeyBindings>` (aliased as `SharedKeyBindings`) for cheap cloning across `App` and `AppSnapshot` structs.

- Defined in: `src/keybindings/resolver.rs`
- Used in: `src/app/state.rs` (App and AppSnapshot fields), `src/ui/tui/keyboard.rs`

This pattern is useful when configuration objects need to be shared between state structs without deep copies.

## Keybindings Configuration

- Config file: `~/.local/share/netease-ratui/keybindings.toml`
- Module: `src/keybindings/` (config.rs for TOML loading/parsing, resolver.rs for KeyAction enum + HashMap lookup)
- Supports: `useDefaultKeyBindings` toggle, multiple keys per action, empty string unbind, fallback to defaults on parse error
- Note: Override semantics are replace (not append). User config replaces defaults entirely for each action.
- Note: Modifier key combinations (Ctrl+q, Alt+s) and function keys (F1-F12) are NOT supported in TOML config. These remain hardcoded in keyboard.rs.
