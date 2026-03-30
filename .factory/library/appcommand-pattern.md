# AppCommand 4-File Pattern

When adding a new AppCommand variant that requires business logic handling, changes are needed across 4 files:

1. **`src/messages/app.rs`** — Add the new variant to the `AppCommand` enum
2. **`src/core/reducer/player.rs`** (or relevant reducer) — Map the command to the feature handler
3. **`src/features/player/control.rs`** (or relevant feature) — Route/dispatch the command
4. **`src/features/player/playback.rs`** (or relevant feature) — Implement the actual logic

Example: `PlayerSeekAbsoluteMs` was added across all 4 files.

## Seek Timing Model

When implementing seek (absolute or relative), the timing fields must be updated correctly:

1. Set `play_started_at = now - target_position_ms`
2. If paused: set `play_paused_at = now`
3. Reset `play_paused_accum_ms = 0`

This pattern is used in both `seek_relative()` and `seek_absolute()` in `src/features/player/playback.rs`.
