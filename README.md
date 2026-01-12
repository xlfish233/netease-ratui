# netease-ratui

⚠️ **法律声明**：本项目仅用于学习与技术研究，非网易云音乐官方产品，与网易公司无任何关联。请遵守网易云音乐服务条款与当地法律法规。  
⚠️ **风险提示**：本项目在 Rust 侧实现了 weapi/eapi 等请求与加密流程，可能随官方更新失效。

一个基于 **Rust + ratatui** 的网易云音乐 TUI 客户端（进行中）。

## 当前功能

- 匿名态初始化（`/api/register/anonimous`）与本地 cookie 持久化
- 二维码登录：生成 `qrurl` + TUI 内显示二维码（ASCII）
- **Cookie 登录**：手动输入 `MUSIC_U` Cookie 值快速登录（适用于已登录浏览器用户）
- 登录态轮询（扫码状态、成功 code=803）
- 获取账号信息与用户歌单列表；进入歌单后可加载歌曲列表
- 歌单歌曲全量加载：先取 `trackIds`，再按 200 首分批拉取歌曲详情并展示进度
- 歌单后台预加载（默认“我喜欢 + 前 5 个歌单”）：低优先级请求，不阻塞搜索/点歌；打开歌单时若已预加载完成可秒开
- 搜索（`/api/cloudsearch/pc`）并在 TUI 列表展示；可直接播放选中歌曲
- 歌词页：自动滚动 + 当前行高亮 + 居中显示（如有翻译会一并展示）
- 设置页：音量/音质/播放模式/歌词 offset 可调整并持久化；支持退出登录（清理本地 cookie）
- 播放器能力（P0 已落地）：
  - 上一首/下一首、暂停/继续、停止
  - Seek（快进/快退）、音量调节
  - 播放模式：顺序/列表循环/单曲循环/随机
  - 播放错误恢复：URL 失效自动重取并重试（有限次）
- 音频本地缓存（仅音乐）：按 `(song_id, br)` 落盘缓存 + LRU 自动清理（默认上限 2GB）+ 自动预缓存下一首歌（支持顺序/列表循环模式）；**仅保留当前设置的音质 br（切换音质会清理其它 br）**

## 快速开始

```bash
# 运行 TUI
cargo run

# 无交互快速自测（匿名搜索）
cargo run -- skip-login

# 打印二维码登录相关信息（便于排查接口返回）
cargo run -- qr-key
```

## 日志（tracing）

- 默认写入文件日志（避免污染 TUI）：`{data_dir}/logs/netease-ratui.log.YYYY-MM-DD`
- 通过 `RUST_LOG` 控制级别（例如 `RUST_LOG=debug`）
- 可选环境变量：
  - `NETEASE_LOG_DIR=/path/to/logs`：覆盖日志目录
  - `NETEASE_DATA_DIR=/path/to/data`：覆盖数据目录（settings/cookie/cache/logs 都在此目录下）

## 安装（预编译包）

在 GitHub Release 下载与你系统匹配的压缩包，解压后运行可执行文件即可。

Linux 可能需要系统音频依赖（以 Debian/Ubuntu 为例）：

```bash
sudo apt-get update
sudo apt-get install -y libasound2-dev
```

## 开发与测试

- 运行测试：`cargo test`（integration tests 按模块放在 `tests/` 目录下）

## 快捷键

- 全局：`Tab` 切换页，`q` 退出，**鼠标左键点击标签页切换**
- 底部状态栏提供"帮助"提示（多行显示，便于阅读）
- **登录页**：
  - `l` 生成二维码（扫码登录）
  - `c` 切换到 Cookie 登录模式
  - Cookie 模式：`Enter` 提交，`Esc` 取消，返回扫码模式
- 播放器：`Space` 暂停/继续，`Ctrl+S` 停止，`[`/`]` 上一首/下一首，`Ctrl+←/→` Seek（±5s），`Alt+↑/↓` 音量，`M` 切换播放模式
- 歌单页：`↑↓` 选择，`Enter` 打开歌单，`p` 播放选中，`b` 返回歌单列表
- 搜索页：输入关键词，`Enter` 搜索，`↑↓` 选择，`p` 播放选中
- 歌词页：`o` 跟随/锁定滚动，锁定后 `↑↓` 滚动，`g` 回到当前行，`Alt+←/→` offset（±200ms），`Shift+Alt+←/→` offset（±50ms）
- 设置页：`↑↓` 选择，`←→` 调整，`Enter` 操作（含退出登录）

## 架构

整体仍采用“**Actor + 消息驱动 + 单一状态源**”的约定：UI 只发 `AppCommand`，由 `core::spawn_app_actor` 维护唯一的 `App` 状态，并通过 `AppEvent` 通知 UI 。`core::reducer` 在 tokio 任务中循环处理来自 UI / Netease / Audio / 定时器的 `CoreMsg`，通过 `features` 模块完成业务逻辑，再将 `CoreEffects` 中收集的状态推送、toast、错误、`NeteaseCommand`、`AudioCommand` 一并下发。`core::infra` 提供 `RequestTracker`、`PreloadManager`、`NextSongCacheManager` 等基础设施，`core::prelude` 把常见类型导出给每个 feature。`settings` 在 actor 启动时从 `src/settings/store.rs` 读取并应用，音频工作线程通过 std mpsc 与 tokio 任务互通。

### 分层与职责

- **UI 层（`src/ui`）**：`cli.rs` 负责命令行参数、`run_tui` 运行 ratatui 界面。`src/ui/tui/` 包含事件循环、生命周期 guard、键盘/鼠标处理、视图管理、播放器状态面板、歌词/歌单/搜索/设置界面、组件、工具函数等；只渲染 `AppEvent::State`，所有用户输入都翻译为 `AppCommand`。
- **Core 层（`src/core`）**：`spawn_app_actor` 启动 `NeteaseActor`（高/低优先级通道）、音频工作线程并运行 `core::reducer`，`CoreState` 持有 `App`、`settings`、`RequestTracker`、`PreloadManager`、`NextSongCacheManager`、待处理请求等信息；`CoreEffects` 统一管理 `EmitState`/`Toast`/`Error` 和命令；`core::infra` 提供预加载、预取、请求跟踪能力。
- **Features（`src/features`）**：按业务拆分为 login/logout/lyrics/player/playlists/search/settings，提供命令与事件处理函数。每个 handler 接收 `AppCommand`/`NeteaseEvent`/`AudioEvent`、操作 `App`、调度 `CoreEffects`，特定模块之间只通过 `App` 状态、`PreloadManager`、`NextSongCacheManager` 等共享上下文。
- **状态与消息**：`src/app/state.rs` 定义 `App`、`View`、`TabConfig`、播放/歌单/歌词状态，`src/app/parsers.rs` 保留搜索/歌单解析工具；`src/messages/app.rs` 定义 `AppCommand`/`AppEvent`，`core::prelude` 把 `App`、`NeteaseCommand/Event`、`AudioCommand/Event` 等再导出一次。
- **Domain / 网关 / 音频**：`src/domain` 提供 `Song`/`Playlist`/`LyricLine` 等稳定模型；`src/netease` 包含 `NeteaseClient`（config/cookie/error/types/crypto 等）、`NeteaseActor`；`src/audio_worker` 负责播放线程（`messages`/`worker`/`player`/`cache`/`download`/`transfer`），通过 std mpsc 与 `core` 的 tokio 通道互通；`src/settings/store.rs` 实现设置与 cookie 持久化；`src/logging.rs`、`src/error.rs` 分别负责日志与错误边界。

### 消息通道与协议

- UI 通过 `AppCommand` 交给 `core::reducer`；命令可能直接在 `features` 中处理（如登录、搜索、歌词）、也可能触发 `NeteaseCommand` 或 `AudioCommand`。
- `core::reducer` 依赖 `RequestTracker` 防止重复/过期响应（每条跨层请求携带 `req_id`），使用 `PreloadManager` 维护歌单预加载状态、`NextSongCacheManager` 预取下一首下载链接，调用 `features` 掌握业务细节，最后把 `App` 状态交给 `CoreEffects`。
- `CoreEffects` 将 `AppEvent::State`/`Toast`/`Error` 发送给 UI，并将 `NeteaseCommand` 交由 `NeteaseActor` 的高/低优先级通道，`AudioCommand` 交给 `audio_worker`；事件（Netease/Audio）被打包成 `CoreMsg` 送回 reducer，形成完整的单向数据流。
- UI 只读 `App`，所有跨 layer 写操作都由 `core` 自动调度，`features` 共享 `App` 全量快照，保持 View/Model 同步。

### 关键约定（长期收益）

- **req_id + RequestTracker**：所有跨 actor 的请求携带 `req_id`，`RequestTracker` 只接收最新的响应，避免界面被旧数据覆盖。
- **Core 是唯一状态写入者**：UI 只渲染，`core::reducer` + `features` 通过 `App` 结构更新状态；`features` 之间不直接传数据，所有共享数据都在 `App` 或 `core::infra`。
- **网关/播放层不做业务拼装**：`NeteaseActor`、`audio_worker` 只做请求/解析/播放，`features` 负责业务流程。
- **Domain/DTO 分离**：`src/domain/model.rs` 提供稳定领域模型，`src/netease/models/dto.rs` 负责不稳定的 API 结构；`src/netease/models/convert.rs` 统一转换。

### 目录结构

- `src/main.rs`：入口；解析 CLI、初始化日志、构造 `NeteaseClientConfig`，通过 `core::spawn_app_actor` 启动 actor 用于 `run_tui`。
- `src/ui/cli.rs`：命令行参数与子命令（TUI / SkipLogin / QrKey）；`src/ui/tui.rs` 搭建 ratatui 渲染与事件处理。
- `src/ui/tui/`：纯 UI 组织（event_loop、guard、keyboard、mouse、views、widgets、player_status、login/lyrics/playlists/search/settings 视图、utils）。
- `src/core`：`spawn_app_actor`、`reducer`、`effects`、`infra`（`RequestTracker`/`PreloadManager`/`NextSongCacheManager`）、`prelude`、`utils`。
- `src/features`：按功能拆分的 handler 模块（login/logout/lyrics/player/playlists/search/settings），共享 `core::prelude`。
- `src/app`：`App` 状态、`TabConfig`、搜索/歌单解析函数。
- `src/messages`：`AppCommand`、`AppEvent`。
- `src/domain`：`Song`、`Playlist`、`LyricLine` 等稳定模型。
- `src/netease`：`NeteaseClient` + `NeteaseActor`、models、crypto、util、client 子模块。
- `src/audio_worker`：音频 worker 线程的 messages/worker/player/cache/download/transfer。
- `src/settings`：设置、cookie、缓存、日志等存储接口（`store.rs`）。
- `src/logging.rs` / `src/error.rs`：日志与错误入口。

## Roadmap（建议顺序）

> 方向参考：go-musicfox 的功能与分层设计，但保持本项目“Actor + 消息驱动 + 强类型模型”的实现风格。

### P0：播放器完成度（像一个真正的播放器）

（已完成）

- 播放控制：上一首/下一首、音量调节、Seek（快进/快退）
- 播放模式：顺序/列表循环/单曲循环/随机
- 播放错误恢复：URL 失效自动重取、连续失败重试上限、错误提示不阻塞 UI
- 音频本地缓存：按 `(song_id, br)` 缓存音频文件 + LRU 清理（默认上限 2GB，可通过 `NETEASE_AUDIO_CACHE_MAX_MB` 调整）；仅保留当前设置的音质 br

### 音频缓存相关环境变量

- `NETEASE_AUDIO_CACHE_MAX_MB`：音频缓存上限（默认 2048）
- `NETEASE_AUDIO_DOWNLOAD_CONCURRENCY`：并发下载数（默认 CPU 核心数）
- `NETEASE_AUDIO_HTTP_TIMEOUT_SECS`：HTTP 总超时（默认 30）
- `NETEASE_AUDIO_HTTP_CONNECT_TIMEOUT_SECS`：HTTP 连接超时（默认 10）
- `NETEASE_AUDIO_DOWNLOAD_RETRIES`：下载重试次数（默认 2）
- `NETEASE_AUDIO_DOWNLOAD_RETRY_BACKOFF_MS`：重试退避基准毫秒（默认 250）
- `NETEASE_AUDIO_DOWNLOAD_RETRY_BACKOFF_MAX_MS`：重试退避上限毫秒（默认 2000）

### P1：歌词与信息展示（体验分水岭）

- 歌词页：滚动歌词 + 当前行高亮（已完成）
- 歌词翻译与偏移（offset）调整；逐字歌词支持（可选）
- 更完整的 Now Playing：进度条、封面/专辑/歌手信息

### P2：可配置与可定制

- 配置系统：缓存上限、音质、主题等
- 可自定义快捷键（keybindings）
- 主题/布局：配色、双栏布局、动态列表高度等

### P3：外部集成

- Linux MPRIS / 系统媒体键
- 桌面通知（可选带封面）
- Last.fm scrobble（可选）

### P4：高级特性（维护成本更高）

- 多播放器后端（如 mpv/mpd）可插拔
- UNM（解锁灰歌/无版权替代音源，需谨慎）

## 开发约定

- 目标：优先保持结构清晰与可维护，避免把“网易云协议细节”散落到 UI 里。
- 分层规则：
  - `src/ui` 只依赖 `messages` 与 `App`，渲染 `AppEvent`，不直接访问 `netease`/`audio_worker`/`core`。
  - `src/core` + `src/features` 是唯一的状态写入者，`core::reducer` 调度命令、事件与 `CoreEffects`，`features` 负责具体业务流程，所有跨层交互都通过消息。
  - `src/netease` 只做请求/解析/持久化 Cookie，不承担业务拼装；`src/audio_worker` 只处理播放命令和状态返回。
  - `src/settings/store.rs` 管理设置/缓存/日志数据，`core` 在启动时加载并在需要时更新。
- 变更方式：按功能切分、逐步提交（commit），便于回滚与 code review。

## 致谢

- https://github.com/feng-yifan/Netease-Cloud-Music-Web-Player
- https://github.com/NeteaseCloudMusicApiEnhanced
- https://github.com/go-musicfox/go-musicfox
