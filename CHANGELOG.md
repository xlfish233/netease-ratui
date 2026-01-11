# Changelog

## Unreleased（自 v0.0.2 起，更新于 2026-01-11）

- **AppActor 大规模模块化重构**：`src/usecases/actor.rs`（1072 行）拆分为 12 个子模块
  - `login.rs`：登录流程处理（QR 码、Cookie 输入、轮询）
  - `search.rs`：搜索功能处理
  - `playlists.rs`：歌单列表和歌曲管理
  - `lyrics.rs`：歌词显示和偏移调整
  - `player_control.rs`：播放器控制命令处理
  - `settings_handler.rs`：设置管理和持久化
  - `audio_handler.rs`：音频 worker 事件处理
  - `playback.rs`：播放控制逻辑（next/prev/seek）
  - `preload.rs`：歌单预加载管理器
  - `playlist_tracks.rs`：歌单歌曲加载器
  - `logout.rs`：退出登录后状态重置
  - `utils.rs`：共享工具函数（next_id、push_state）
  - 主文件从 1072 行减少到 510 行（-52%），每个子模块职责单一、易于维护
- **代码质量优化**：
  - 移除冗余 clone 和死代码
  - 简化 Option 处理（使用链式方法）
  - 优化模式匹配（使用 `cmd @ (A | B | C)` 或模式语法）
  - 改进函数签名（引用传递替代值传递）

- **新增 Cookie 登录功能**：支持手动输入 `MUSIC_U` Cookie 值快速登录
  - 登录页按 `c` 键切换到 Cookie 登录模式
  - 支持输入验证和错误提示
  - 适用于已通过浏览器登录 music.163.com 的用户
- 登录页 UI 改进：显示两种登录方式的说明和快捷键提示

## v0.0.2（2026-01-11）

- 支持鼠标左键点击标签页切换
- TUI 底部“帮助”提示改为多行显示
- 重构标签页配置管理（TabConfig/TabConfigs/TabIndex）消除重复代码
- 歌词页面支持水平和垂直居中显示
- 歌单后台预加载（低优先级，不阻塞搜索/点歌/打开歌单）
- NeteaseActor 支持高/低优先级命令通道（优先处理用户交互请求）
- AppActor 代码拆分：`src/usecases/actor.rs` 拆出 `src/usecases/actor/` 子模块
- 升级依赖并修复升级后的播放/TUI 回归问题

## v0.0.1（2026-01-11）

- P0 播放器：上一首/下一首、暂停/继续、停止、Seek、音量、播放模式、错误重试
- 音频缓存：按 `(song_id, br)` 缓存 + LRU 清理；缓存索引版本化；设置页支持“一键清除缓存”
- 歌单歌曲全量加载：按 200 首分批拉取直至完整
- 歌词页：自动滚动/当前行高亮；支持 offset 调整；支持跟随/锁定滚动
- 设置页：音量/音质/播放模式/歌词 offset 持久化；支持退出登录（清理本地 cookie）
