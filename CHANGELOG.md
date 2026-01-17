# Changelog

## Unreleased

- **性能优化**：优化歌单播放队列的克隆开销
  - 重构 `PlayQueue::set_songs()` 返回旧数据，允许调用方转移所有权
  - 优化预加载歌单克隆：使用 `mem::take` 转移所有权，避免克隆
  - 优化播放选中歌曲：使用 `mem::take` 转移所有权
  - 优化歌单加载完成：使用 `mem::take` 转移所有权
  - 为 `AppSnapshot::from_app()` 添加详细的架构文档说明为何需要克隆
  - 为预加载缓存克隆添加 TODO 注释
  - **性能提升**：每次操作节省 ~20-40KB 内存分配（200 首歌）

- **测试质量提升**：改进测试代码质量
  - 修复 `tests/player_reload.rs` 中的无意义断言（`assert!(true)`）
  - 重写测试以验证具体的字段值和行为
  - 使用 `matches!` 宏简化测试代码
  - 删除文档注释后的空行
  - 修复所有 Clippy 警告（`empty_line_after_doc_comments`、`assertions_on_constants`）

- **代码质量**：优化搜索功能中的不必要克隆
  - 删除 `features/search/mod.rs` 中的 `title.clone()`
  - `title` 已拥有所有权，无需再次克隆

- **文档完善**：为 `expect()` 调用添加详细文档说明
  - 为 `audio_worker/engine.rs` 的 tokio runtime 创建添加 24 行注释
  - 为 `audio_worker/player.rs` 的线程创建添加 21 行注释
  - 为 `audio_worker/transfer.rs` 的 tokio runtime 创建添加 23 行注释
  - 说明为何在这些位置使用 `expect()` 是安全的
  - 解释架构限制、实际风险和设计权衡
  - 提供未来改进方向
  - 创建完整的错误处理策略文档（`docs/error_handling.md`）

- **文档新增**：
  - 新增 `docs/error_handling.md` - 完整的错误处理策略文档
  - 说明分层错误处理（UI、Core、Worker）
  - 记录错误类型体系和最佳实践
  - 为未来的错误处理改进提供方向

## v0.0.7（2026-01-17）

- **修复**：修复重启应用后按空格键无响应的问题
  - 在 `AudioEvent` 中新增 `NeedsReload` 事件类型
  - `AudioEngine::TogglePause` 检测到 sink 为 None 时自动发送 `NeedsReload` 事件
  - `handle_audio_event` 处理 `NeedsReload` 事件，自动重新请求当前歌曲的播放链接
  - 用户重启应用后按空格键即可自动恢复播放，无需手动重新播放
  - 修改文件：
    - `src/audio_worker/messages.rs`：添加 `NeedsReload` 事件
    - `src/audio_worker/engine.rs`：检测 sink 为 None 并发送事件
    - `src/features/player/audio.rs`：处理 `NeedsReload` 事件
- **测试增强**：为 NeedsReload 功能添加 7 个单元测试和集成测试
  - `src/audio_worker/messages.rs`：3 个单元测试
  - `tests/player_reload.rs`：4 个集成测试
- **文档更新**：更新 TESTING.md，记录新增测试

## v0.0.7（2026-01-17）

- **修复**：解决 CI 中的 clippy 警告（Rust 1.92.0）
  - 替换 `assert_eq!(bool, true)` 为 `assert!(bool)`
  - 使用结构体字面量初始化替代 `Default::default()` 后的字段重新赋值
  - 修复 4 个 clippy 错误（1 个 bool 断言比较 + 3 个字段重新赋值）

## v0.0.6（2026-01-17）

- **播放状态持久化**：新增自动保存和恢复播放状态功能（`src/player_state`）
  - 启动时自动恢复播放队列、播放进度、音量、播放模式等
  - 退出时自动保存状态（按 `q`）
  - 每 30 秒定时自动保存，防止意外关闭丢失数据
  - 使用时间戳转换，精确恢复播放位置（毫秒级）
  - 默认恢复为暂停状态，不打扰用户
  - 原子写入确保数据完整性（临时文件 + rename）
  - 完善错误处理：文件不存在、版本不兼容、格式错误等
- **测试增强**：为 `player_state` 模块添加 8 个单元测试
  - 状态快照序列化/反序列化测试
  - 版本兼容性测试
  - PlayQueue cursor 设置测试
  - 播放进度计算测试
  - 错误处理测试
- **总测试数量**：从 68 个增加到 76 个（+12%）
- **文档更新**：更新 README、ARCHITECTURE 和 CHANGELOG 文档

- **测试基础设施**：添加代码覆盖率工具（tarpaulin）、pre-commit hooks、Makefile，CI 集成覆盖率检查
- **DTO 转换测试**：为 `src/netease/models/convert.rs` 添加 15 个单元测试，覆盖登录状态、歌词解析、歌单转换、歌曲 URL 等核心功能
- **改进 .gitignore**：添加覆盖率报告、测试临时文件、编辑器配置等常见忽略项
- **总测试数量**：从 28 个增加到 43 个（+53%）

- **播放队列重构**：引入 `PlayQueue` 统一顺序/随机播放，右侧队列按实际播放顺序展示
- **预缓存逻辑统一**：`NextSongCacheManager` 基于 `PlayQueue` 计算下一首，随机模式也可预取
- **音频下载日志增强**：补充缓存命中/下载启动/完成/失败日志，便于排查播放卡住

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
