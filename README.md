# netease-ratui

⚠️ **法律声明**：本项目仅用于学习与技术研究，非网易云音乐官方产品，与网易公司无任何关联。请遵守网易云音乐服务条款与当地法律法规。  
⚠️ **风险提示**：本项目在 Rust 侧实现了 weapi/eapi 等请求与加密流程，可能随官方更新失效。

一个基于 **Rust + ratatui** 的网易云音乐 TUI 客户端（进行中）。

## 当前功能

- 匿名态初始化（`/api/register/anonimous`）与本地 cookie 持久化
- 二维码登录：生成 `qrurl` + TUI 内显示二维码（ASCII）
- 登录态轮询（扫码状态、成功 code=803）
- 获取账号信息与用户歌单列表；进入歌单后可加载歌曲列表
- 歌单歌曲全量加载：先取 `trackIds`，再按 200 首分批拉取歌曲详情并展示进度
- 搜索（`/api/cloudsearch/pc`）并在 TUI 列表展示；可直接播放选中歌曲
- 歌词页：自动滚动 + 当前行高亮（如有翻译会一并展示）
- 设置页：音量/音质/播放模式/歌词 offset 可调整并持久化；支持退出登录（清理本地 cookie）
- 播放器能力（P0 已落地）：
  - 上一首/下一首、暂停/继续、停止
  - Seek（快进/快退）、音量调节
  - 播放模式：顺序/列表循环/单曲循环/随机
  - 播放错误恢复：URL 失效自动重取并重试（有限次）
- 音频本地缓存（仅音乐）：按 `(song_id, br)` 落盘缓存 + LRU 自动清理（默认上限 2GB）

## 快速开始

```bash
# 运行 TUI
cargo run

# 无交互快速自测（匿名搜索）
NETEASE_SKIP_LOGIN=1 cargo run
```

## 快捷键

- 全局：`Tab` 切换页，`q` 退出
- 播放器：`Space` 暂停/继续，`Ctrl+S` 停止，`[`/`]` 上一首/下一首，`Ctrl+←/→` Seek（±5s），`Alt+↑/↓` 音量，`M` 切换播放模式
- 歌单页：`↑↓` 选择，`Enter` 打开歌单，`p` 播放选中，`b` 返回歌单列表
- 搜索页：输入关键词，`Enter` 搜索，`↑↓` 选择，`p` 播放选中
- 歌词页：`o` 跟随/锁定滚动，锁定后 `↑↓` 滚动，`g` 回到当前行，`Alt+←/→` offset（±200ms），`Shift+Alt+←/→` offset（±50ms）
- 设置页：`↑↓` 选择，`←→` 调整，`Enter` 操作（含退出登录）

## 架构

整体采用“**全 Actor + 消息驱动 + 单一状态源**”的分层架构：UI 不直接做网络/解析/播放，所有跨层交互通过消息通信完成。

### 分层与职责

- **TuiActor（UI）**：处理键盘输入、渲染整体状态；只发送 `AppCommand`，只接收 `AppEvent`。
- **AppActor（应用层 / 业务编排）**：接收 `AppCommand`，维护唯一状态（当前实现为 `src/app.rs` 的 `App`），编排登录轮询、搜索、歌单链路拼装、播放队列推进等业务流程。
- **NeteaseActor（网关 / 基础设施）**：持有 `NeteaseClient`（cookie/加密/请求发送）；接收 `NeteaseCommand` 并返回强类型 `NeteaseEvent`；不包含 UI 选择/队列策略。
- **AudioActor（播放器 / 基础设施）**：接收 `AudioCommand` 并返回 `AudioEvent`（播放状态机、停止/切歌取消 Ended 上报）。

### 消息通道拓扑（简化）

```
TuiActor  -- AppCommand -->  AppActor  -- NeteaseCommand -->  NeteaseActor
TuiActor  <-- AppEvent  --  AppActor  <-- NeteaseEvent  --  NeteaseActor

TuiActor  <-- AppEvent  --  AppActor  -- AudioCommand  -->  AudioActor
                     AppActor  <-- AudioEvent  --  AudioActor
```

### UI 协议：整体状态推送（先全量，后续可增量）

- UI -> App：高层 `AppCommand`（例如 `SearchSubmit`、`SearchPlaySelected`、`LoginGenerateQr`）。
- App -> UI：`AppEvent::State(AppState)`（当前实现为 `AppEvent::State(App)`，每次变更推送整状态；后续可替换为 patch）。

### 关键约定（长期收益）

- **所有跨 actor 的请求携带 `req_id`**：事件回包携带同 `req_id`，用于丢弃过期响应、避免并发/乱序覆盖状态。
- **AppActor 是唯一状态写入者**：UI 只读状态；网关/音频只发事件。
- **网关不做业务拼装**：例如“歌单详情 -> trackIds -> song_detail_by_ids”由 AppActor 负责串联请求并组装结果。
- **Domain/DTO 分离**：
  - Domain（稳定）：`src/domain/model.rs`（供 AppActor/UI 使用）
  - DTO（易变）：`src/netease/models/dto.rs`
  - 转换层：`src/netease/models/convert.rs`

### 目录结构

- `src/main.rs`：入口；选择运行模式（TUI / 调试模式）。
- `src/tui.rs`：ratatui + crossterm 事件循环；只发送 `AppCommand`、只渲染 `AppEvent::State`（全量）。
- `src/messages/`：UI<->AppActor 的消息协议（`AppCommand/AppEvent`）。
- `src/usecases/actor.rs`：`AppActor`（业务编排 + 单一状态源）。
- `src/domain/`：领域模型（供业务/状态使用）。
- `src/netease/actor.rs`：`NeteaseActor`（网关层：命令/事件 + 强类型解析）。
- `src/netease/models/`：DTO/Domain 转换与容错（响应结构变动的集中处理点）。
- `src/app.rs`：当前整体状态结构（临时命名为 `App`，长期会演进为 `AppState` 分层模块）。
- `src/netease/`：协议与客户端实现：
  - `src/netease/crypto.rs`：weapi / eapi / linuxapi 加密与表单生成（AES + RSA + MD5）。
  - `src/netease/client.rs`：`NeteaseClient`（cookie、UA、header cookie、请求拼装与发送）。
  - `src/netease/util.rs`：deviceId / anonymous username 等工具函数。

## Roadmap（建议顺序）

> 方向参考：go-musicfox 的功能与分层设计，但保持本项目“Actor + 消息驱动 + 强类型模型”的实现风格。

### P0：播放器完成度（像一个真正的播放器）

（已完成）

- 播放控制：上一首/下一首、音量调节、Seek（快进/快退）
- 播放模式：顺序/列表循环/单曲循环/随机
- 播放错误恢复：URL 失效自动重取、连续失败重试上限、错误提示不阻塞 UI
- 音频本地缓存：按 `(song_id, br)` 缓存音频文件 + LRU 清理（默认上限 2GB，可通过 `NETEASE_AUDIO_CACHE_MAX_MB` 调整）

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
  - `tui` 只依赖 `messages` 与状态结构，不直接调用 `netease`/`reqwest`/`serde_json`。
  - `netease`（网关）只做“请求/解析/持久化 cookie”，不做业务拼装。
  - 业务拼装与策略统一放 `usecases`（AppActor）。
- 变更方式：按功能切分、逐步提交（commit），便于回滚与 code review。

## 致谢

- https://github.com/feng-yifan/Netease-Cloud-Music-Web-Player
- https://github.com/NeteaseCloudMusicApiEnhanced
- https://github.com/go-musicfox/go-musicfox
