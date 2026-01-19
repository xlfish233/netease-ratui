# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo run                    # Run TUI (default)
cargo test                   # Run all tests
cargo test test_name         # Run single test
cargo test -- --list         # List all tests
cargo fmt --check            # Format check
cargo clippy -- -D warnings  # Lint check
make check                   # Run all checks (fmt + clippy + test)

# Debug modes
cargo run -- --no-audio                    # No audio output
cargo run -- skip-login "keywords" --limit 5  # Anonymous search test
cargo run -- qr-key                        # QR login debug
```

Linux requires `libasound2-dev` for ALSA audio support.

## Architecture

Actor + message-driven + single source of truth design. UI only renders `AppSnapshot`.

```
UI (keyboard/mouse)
  → AppCommand
  → core::reducer
  → Features (login/search/playlists/player/lyrics/settings)
  → CoreEffects (NeteaseCommand/AudioCommand)
  → NeteaseActor
  → NeteaseEvent/AudioEvent
  → core::reducer
  → AppEvent::State(AppSnapshot)
  → UI render
```

### Key Modules

- `src/core/reducer.rs` - Main state reducer, only writer of `App` state
- `src/core/reducer/` - Feature-specific reducers (login, search, playlists, player, lyrics, settings)
- `src/core/infra/` - RequestTracker (prevents stale responses), PreloadManager, NextSongCacheManager
- `src/features/` - Business logic by domain
- `src/netease/` - API gateway with weapi/eapi/linuxapi encryption
- `src/audio_worker/` - Audio playback on dedicated thread with LocalSet for !Send rodio resources
- `src/ui/tui/` - TUI components and event handling

### State Flow

- `App` - Full business state
- `AppSnapshot` - Lightweight UI render snapshot
- `CoreState` - Holds App + settings + request/preload context

### Request Tracking

Every cross-layer request carries `req_id`. `RequestTracker` only accepts responses matching the latest request ID, discarding stale responses.

## Error Handling

The project uses a unified error handling pattern with structured error types:

- `src/error/mod.rs` - Central error module with all error types
- `src/error/message.rs` - `MessageError` for cross-Actor message passing (cloneable, lightweight)
- `src/error/app.rs` - `AppError` - Application-level errors
- `src/error/netease.rs` - `NeteaseError` - NetEase API errors
- `src/error/audio.rs` - `AudioError` - Audio playback errors
- `src/error/player_state.rs` - `PlayerStateError` - State persistence errors

**Key principle**: Use `MessageError` in event enums (AppEvent, NeteaseEvent, AudioEvent) instead of `String` to preserve error context and enable structured error handling.

## Environment Variables

- `NETEASE_DATA_DIR` / `NETEASE_LOG_DIR` - Override data/log directories
- `RUST_LOG` - Log level (default: `info,reqwest=warn,hyper=warn`); for deep debugging use `netease_ratui=trace,reqwest=warn,hyper=warn` (or CLI `--log-filter`)
- `NETEASE_NO_AUDIO=1` - Disable audio
- `NETEASE_SKIP_LOGIN=1` - Anonymous test mode
- `NETEASE_QR_KEY=1` - QR debug mode

## Data Directory

Default `~/.local/share/netease-ratui`:
- `settings.json` - UI settings
- `netease_state.json` - Cookie & device info
- `audio_cache/` - Audio cache
- `logs/` - Tracing logs

## Notes

- Rust 2024 edition - requires latest toolchain
- AudioWorker runs on dedicated thread with single-threaded tokio runtime
- Crossfade default 300ms, configurable in settings
- Ended detection is polled in the audio engine (no per-track thread)
