# Changelog

## Unreleased

- **测试基础设施**：添加代码覆盖率工具（tarpaulin）、pre-commit hooks、Makefile，CI 集成覆盖率检查
- **DTO 转换测试**：为 `src/netease/models/convert.rs` 添加 15 个单元测试，覆盖登录状态、歌词解析、歌单转换、歌曲 URL 等核心功能
- **改进 .gitignore**：添加覆盖率报告、测试临时文件、编辑器配置等常见忽略项
- **总测试数量**：从 28 个增加到 43 个（+53%）

- **自动跳过版权限制歌曲**：添加 `NeteaseEvent::SongUrlUnavailable` 事件类型，当歌曲无可用播放链接时（版权限制、VIP 专享等）自动跳转到下一首，改善用户体验。
- **音频缓存优化**：`AudioCache` 引入 `dirty` 标志实现延迟持久化，显著减少频繁切歌场景下的磁盘 I/O（从每次 lookup 都持久化优化为仅在程序退出时持久化）。
- **音频线程验证**：添加 `attach_sink` 线程创建/退出的调试日志，验证 `stop()` 能立即终止 `sleep_until_end()`，确认当前架构无线程泄漏问题。
- **音频缓存测试**：为 `AudioCache` 添加 9 个单元测试，覆盖 dirty 标志、延迟持久化、Drop 机制等核心功能。
- **简化 purge 逻辑**：`transfer.rs` 缓存清理逻辑从 3 个分支简化为 2 个，删除冗余的 `else` 分支，明确只有开启"仅保留音质"功能时才执行清理。

## v0.0.5（2026-01-12）

- 音频模块重构：引入独立线程 + LocalSet 的 AudioEngine，隔离 !Send 音频资源并保持 async 控制流。
- 全面支持 300ms crossfade（切歌/下一首/上一首/自动切歌），并可在设置中调整或关闭。
- 新增无声模式：支持 `--no-audio` 与 `NETEASE_NO_AUDIO=1`，CI/无声环境可用。

## v0.0.4（2026-01-12）

- 音频 worker 改为在独立线程的单线程 tokio runtime 中运行，避免非 Send 资源跨线程导致构建失败，同时保持异步 IO。

## v0.0.3（2026-01-12）

### 核心 + 特性模块化

- `src/core` 重新承担 App Actor 角色：`spawn_app_actor` 启动 `NeteaseActor`（高/低优先级）、音频工作线程与 tokio `CoreMsg` 循环，`core::reducer` 维护 `CoreState`（`App`、`settings`、`RequestTracker`、`PreloadManager`、`NextSongCacheManager` 等）并把状态更新、`NeteaseCommand`、`AudioCommand` 统一交给 `CoreEffects`；`core::prelude` 负责导出常用类型与工具。
- `core::reducer` 进一步拆分为 `src/core/reducer/` 子模块（login/search/playlists/player/lyrics/settings），`reducer.rs` 仅保留消息路由与调度，业务处理与测试覆盖各子模块关键路径。
- `src/app/state.rs` 把 `App`、`View`、`TabConfig`、播放/歌词/歌单状态与默认值集中起来，`src/app/parsers.rs` 保留搜索/歌单解析辅助。
- `src/features` 按业务拆分为 login/logout/lyrics/player/playlists/search/settings，专注命令/事件处理、`App` 维护与 `CoreEffects` 调度，保持 `core::reducer` 业务面向最小。
- `src/messages/app.rs` 继续定义 `AppCommand`/`AppEvent`；所有功能模块通过 `AppCommand` 触发，`AppEvent::State`/`Toast`/`Error` 推给 UI。

### 基础设施与状态管理

- `core::infra::RequestTracker` 让每条跨层请求携带 `req_id`，只接受最新响应，并在登录、搜索、歌单等链路里避免旧数据覆盖。
- `core::infra::PreloadManager` 维护歌单预加载状态，`core::infra::NextSongCacheManager` 负责下一首 URL 缓存、播放队列变化时自动失效、停止时重置，preload 与自动调度共享 `CoreState`。
- `src/settings/store.rs` 提供设置/缓存/日志目录等持久化，`core` 在启动时加载并根据用户操作更新，`audio_worker` 启动阶段同步当前 `play_br`。

### UI / 网关 / 音频整理

- `src/ui` 现在托管 CLI (`cli.rs`) 以及完整的 TUI（`tui.rs` + `tui/` 目录）；ratatui 的事件循环、guard、keyboard/mouse、views、widgets、player_status、login/lyrics/playlists/search/settings 视图都在这里，UI 只接收 `AppEvent` 并发出 `AppCommand`。
- `src/audio_worker` 维持 `messages`/`worker`/`player`/`cache`/`download`/`transfer` 结构，通过 std mpsc 与 tokio `CoreMsg` 互通，下载/落盘/缓存逻辑全部由 worker 线程执行。
- `src/netease` 将 `NeteaseClient` 拆分为 config/cookie/error/types，`actor.rs` 集中命令/事件，models/convert 处理 DTO→Domain；`core` 和 `NeteaseActor` 之间依旧保持高/低优先级通道。

### 其他功能

- **新增“缓存下一首歌”能力**：`NextSongCacheManager` 在播放列表顺序/列表循环模式下提前获取下一首并把 URL 交给 `AudioWorker`，队列变化、模式切换、停止、登出时自动失效，失败静默处理。
- **音频缓存下载链路升级**：`audio_worker::transfer` 置于 tokio 异步下载器，支持超时/重试/退避、默认并发等配置，缓存策略保持“只保留当前音质 br”。
- **Cookie 登录支持**：加入 `MUSIC_U` 手动输入、输入校验与错误提示；`c` 键在登录页内切换 Cookie 模式，`Enter` 提交，`Esc` 取消。
- 登录页 UI 改进：更加明显地展示二维码与 Cookie 登录模式的说明和快捷键提示。

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
