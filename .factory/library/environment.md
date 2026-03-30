# Environment

环境变量、外部依赖和设置说明。

**What belongs here:** Required env vars, external dependencies, platform notes.
**What does NOT belong here:** Service ports/commands (use `.factory/services.yaml`).

---

## Build Dependencies

- Rust toolchain: rustc 1.92.0, edition 2024
- Linux ALSA: `libasound2-dev` for audio support
- 新增 cargo 依赖: `toml` crate (用于 keybindings.toml 解析)

## Data Directory

Default `~/.local/share/netease-ratui`:
- `settings.json` - UI 设置
- `keybindings.toml` - 快捷键配置（新增，可选）
- `player_state.json` - 播放状态
- `netease_state.json` - Cookie
- `audio_cache/` - 音频缓存
