# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo run                    # Run TUI (default)
cargo test                   # Run all tests
cargo fmt --check            # Format check
cargo clippy -- -D warnings  # Lint check

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
  → NeteaseActor/AudioWorker
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

## Environment Variables

- `NETEASE_DATA_DIR` / `NETEASE_LOG_DIR` - Override data/log directories
- `RUST_LOG` - Log level (default: `info,reqwest=warn,hyper=warn`)
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
