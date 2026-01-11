# netease-ratui

⚠️ **法律声明**：本项目仅用于学习与技术研究，非网易云音乐官方产品，与网易公司无任何关联。请遵守网易云音乐服务条款与当地法律法规。  
⚠️ **风险提示**：本项目在 Rust 侧实现了 weapi/eapi 等请求与加密流程，可能随官方更新失效。

一个基于 **Rust + ratatui** 的网易云音乐 TUI 客户端（进行中）。

## 当前功能（MVP）

- 匿名态初始化（`/api/register/anonimous`）与本地 cookie 持久化
- 二维码登录：生成 `qrurl` + TUI 内显示二维码（ASCII）
- 登录态轮询（扫码状态、成功 code=803）
- 搜索（`/api/cloudsearch/pc`）并在 TUI 列表展示

## 快速开始

```bash
# 运行 TUI
cargo run

# 无交互快速自测（匿名搜索）
NETEASE_SKIP_LOGIN=1 cargo run

# 打印 login_qr_key 原始响应（排查 unikey 路径/接口变化）
NETEASE_QR_KEY=1 cargo run
```

## 基础架构

整体按“**TUI 事件循环** + **异步服务层** + **协议/加密实现**”拆分，避免 UI 被网络/加密细节耦合。

## 架构蓝图（重构目标：全 Actor + 消息驱动）

> 目标：把 UI、业务编排、网易云网关、音频播放彻底解耦；UI 只渲染整体状态并发送高层命令；所有跨层交互统一用消息通信。

### 分层与职责

- **TuiActor（UI）**：处理键盘输入、渲染 `AppState`；不直接做网络/解析/播放。
- **AppActor（应用层 / 业务编排）**：接收 `AppCommand`，维护唯一 `AppState`，负责登录轮询、歌单加载链路、播放队列推进等业务流程。
- **NeteaseActor（网关 / 基础设施）**：持有 `NeteaseClient`（cookie/加密/请求发送）；接收 `NeteaseCommand` 并返回强类型 `NeteaseEvent`；不包含 UI/业务策略。
- **AudioActor（播放器 / 基础设施）**：接收 `AudioCommand` 并返回 `AudioEvent`（播放状态机）。

### 消息通道拓扑（简化）

```
TuiActor  -- AppCommand -->  AppActor  -- NeteaseCommand -->  NeteaseActor
TuiActor  <-- AppEvent  --  AppActor  <-- NeteaseEvent  --  NeteaseActor

TuiActor  <-- AppEvent  --  AppActor  -- AudioCommand  -->  AudioActor
                     AppActor  <-- AudioEvent  --  AudioActor
```

### UI 协议：整体状态推送（先全量，后续可增量）

- UI -> App：高层 `AppCommand`（例如 `SearchSubmit`、`PlaySelected`、`LoginGenerateQr`）。
- App -> UI：`AppEvent::State(AppState)`（每次状态变更推送整状态；后续可替换为 patch）。

### 领域模型与 DTO 分离（长期收益）

- **Domain（稳定）**：`Song/Playlist/Account/...`，供 UI/业务使用。
- **DTO（易变）**：所有 `serde` 的响应结构体（字段可选、兼容多形态）。
- **转换层**：集中把 DTO -> Domain，缺字段则返回明确错误，避免 UI/worker 内散落 `pointer("/x/y")`。

### 关键工程约定

- **所有跨 actor 的请求携带 `req_id`**：事件回包携带同 `req_id`，避免并发/乱序覆盖状态。
- **AppActor 是唯一 `AppState` 写入者**：UI 只读状态；网关/音频只发事件。

### 目录结构

- `src/main.rs`：入口；选择运行模式（TUI / 调试模式）。
- `src/tui.rs`：ratatui + crossterm 事件循环；只发送 `AppCommand`、只渲染 `AppState`（全量）。
- `src/messages/`：UI<->AppActor 的消息协议（`AppCommand/AppEvent`）。
- `src/usecases/actor.rs`：`AppActor`（业务编排 + 唯一状态源）。
- `src/netease/actor.rs`：`NeteaseActor`（网关层，命令/事件 + 强类型解析）。
- `src/netease/models/`：DTO/Domain 转换与容错（响应结构变动的集中处理点）。
- `src/app.rs`：`App`（当前 UI 状态结构，后续会进一步演进为 `AppState` 分层模块）。
- `src/api_worker.rs`：旧的 worker（计划淘汰，改由 `AppActor/NeteaseActor` 替代）。
- `src/netease/`：协议与客户端实现：
  - `src/netease/crypto.rs`：weapi / eapi / linuxapi 加密与表单生成（AES + RSA + MD5）。
  - `src/netease/client.rs`：`NeteaseClient`（cookie、UA、header cookie、请求拼装与发送）。
  - `src/netease/util.rs`：deviceId / anonymous username 等工具函数。

### 数据流（以二维码登录为例）

1. UI（`tui.rs`）按 `l` → 发送 `ApiRequest::LoginQrKey`
2. worker（`api_worker.rs`）调用 `NeteaseClient::login_qr_key()` 获取 `unikey`
3. worker 生成 `qrurl` 并转为 ASCII（`qrcode`）→ 发 `ApiEvent::LoginQrReady`
4. UI 显示二维码，并定时轮询 → worker 调 `login_qr_check` → `ApiEvent::LoginQrStatus`
5. code=803 时认为登录完成，并将 `set-cookie` 写入本地状态文件

## 设计思路与关键点

### 1) UI 与网络隔离

- UI 线程只处理输入、绘制与状态更新，不直接进行网络请求。
- 网络请求放到 worker 任务中，通过 mpsc channel 通信，减少 UI 卡顿与复杂度。

### 2) Cookie 与设备标识

- `netease_state.json` 持久化 cookie 与 `deviceId`，用于跨次启动复用会话。
- 匿名态会自动走 `/api/register/anonimous` 补齐 `MUSIC_A` 等必要信息（参考 `api-enhanced` 的实现）。

### 3) 协议与加密

- **weapi**：AES-CBC 双层加密 + RSA(NONE padding) 的 `encSecKey`
- **eapi**：构造 `nobody{uri}use{text}md5forencrypt` 的 MD5，拼接后 AES-ECB（hex 大写）
- 请求侧按不同加密模式拼装不同的 URL、UA、Cookie/header cookie（参考 `api-enhanced/util/request.js` 的策略）

### 4) 兼容与降级

- `api-enhanced` 默认 eapi 走 `https://interface.music.163.com`；某些网络环境可能 DNS 不稳定，客户端实现里对该域名做了降级尝试到 `https://music.163.com`。
- 二维码 `unikey` 的提取兼容多种返回结构（`/unikey`、`/data/unikey` 等），便于接口变化时快速定位。

## Roadmap（建议顺序）

1. 播放器内核：`song_url` → 音频拉流/解码/播放（rodio/cpal + symphonia）+ 播放状态事件
2. 播放队列与控制：上一首/下一首/暂停/Seek/音量/循环随机
3. 歌词与进度同步：`/api/song/lyric` + 滚动高亮
4. 歌单/收藏/每日推荐等业务页
5. MPRIS/通知/媒体键（Linux）

## 开发约定

- 目标：优先保持结构清晰与可维护，避免把“网易云协议细节”散落到 UI 里。
- 变更方式：按功能切分、逐步提交（commit），便于回滚与 code review。
