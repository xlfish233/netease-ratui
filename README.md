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

整体采用“**全 Actor + 消息驱动 + 单一状态源**”的分层架构：UI 不直接做网络/解析/播放，所有跨层交互通过消息通信完成。

## 架构蓝图（重构目标：全 Actor + 消息驱动）

> 目标：把 UI、业务编排、网易云网关、音频播放彻底解耦；UI 只渲染整体状态并发送高层命令；所有跨层交互统一用消息通信。

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

### 领域模型与 DTO 分离（长期收益）

- **Domain（稳定）**：`src/domain/model.rs`（`Song/Playlist/Account/...`），供 AppActor/UI 使用。
- **DTO（易变）**：`src/netease/models/dto.rs`（所有 `serde` 的响应结构体，字段可选、兼容多形态）。
- **转换层**：`src/netease/models/convert.rs`（集中把 DTO -> Domain；缺字段返回明确错误，避免散落 `pointer("/x/y")`）。

### 关键工程约定

- **所有跨 actor 的请求携带 `req_id`**：事件回包携带同 `req_id`，用于丢弃过期响应、避免并发/乱序覆盖状态。
- **AppActor 是唯一状态写入者**：UI 只读状态；网关/音频只发事件。
- **网关不做业务拼装**：例如“歌单详情 -> trackIds -> song_detail_by_ids”由 AppActor 负责串联请求并组装结果。

### 目录结构

- `src/main.rs`：入口；选择运行模式（TUI / 调试模式）。
- `src/tui.rs`：ratatui + crossterm 事件循环；只发送 `AppCommand`、只渲染 `AppEvent::State`（全量）。
- `src/messages/`：UI<->AppActor 的消息协议（`AppCommand/AppEvent`）。
- `src/usecases/actor.rs`：`AppActor`（业务编排 + 单一状态源）。
- `src/domain/`：领域模型（供业务/状态使用）。
- `src/netease/actor.rs`：`NeteaseActor`（网关层：命令/事件 + 强类型解析）。
- `src/netease/models/`：DTO/Domain 转换与容错（响应结构变动的集中处理点）。
- `src/app.rs`：当前整体状态结构（临时命名为 `App`，长期会演进为 `AppState` 分层模块）。
- `src/api_worker.rs`：旧的 worker（已不在主流程使用，后续会删除）。
- `src/netease/`：协议与客户端实现：
  - `src/netease/crypto.rs`：weapi / eapi / linuxapi 加密与表单生成（AES + RSA + MD5）。
  - `src/netease/client.rs`：`NeteaseClient`（cookie、UA、header cookie、请求拼装与发送）。
  - `src/netease/util.rs`：deviceId / anonymous username 等工具函数。

### 数据流（以二维码登录为例）

1. UI（`src/tui.rs`）按 `l` → 发送 `AppCommand::LoginGenerateQr`
2. AppActor（`src/usecases/actor.rs`）收到命令 → 发 `NeteaseCommand::LoginQrKey { req_id }`
3. NeteaseActor（`src/netease/actor.rs`）调用 `NeteaseClient::login_qr_key()`，解析 DTO→Domain → 回 `NeteaseEvent::LoginQrKey { req_id, unikey }`
4. AppActor 生成 `qrurl` + ASCII，并更新状态 → `AppEvent::State(...)` 推送 UI
5. AppActor 定时轮询（2s）→ `NeteaseCommand::LoginQrCheck { req_id, key }` → `NeteaseEvent::LoginQrStatus { req_id, status }`
6. code=803 时认为登录完成：AppActor 拉取账号与歌单并更新状态（cookie 由 `NeteaseClient` 持久化到本地状态文件）

## 设计思路与关键点

### 1) 单一状态源（Single Source of Truth）

- UI 不直接修改业务状态：只发 `AppCommand`。
- AppActor 是唯一状态写入者：状态变更后通过 `AppEvent::State(...)` 全量推送 UI。

### 2) 消息驱动与解耦

- UI/网关/音频互不依赖实现细节：通过命令/事件交互。
- 业务流程（拼装、多步调用、轮询、队列推进）集中在 AppActor，方便扩展功能页与播放能力。

### 3) `req_id`：并发安全与去抖

- AppActor 发出的每个网关请求都带 `req_id`，回包带同 `req_id`。
- AppActor 可丢弃过期响应（例如连续搜索时只接受最新一次结果），避免乱序覆盖 UI 状态。

### 4) Cookie 与设备标识

- `netease_state.json` 持久化 cookie 与 `deviceId`，用于跨次启动复用会话。
- 匿名态会自动走 `/api/register/anonimous` 补齐 `MUSIC_A` 等必要信息（参考 `api-enhanced` 的实现）。

### 5) 协议与加密

- **weapi**：AES-CBC 双层加密 + RSA(NONE padding) 的 `encSecKey`
- **eapi**：构造 `nobody{uri}use{text}md5forencrypt` 的 MD5，拼接后 AES-ECB（hex 大写）
- 请求侧按不同加密模式拼装不同的 URL、UA、Cookie/header cookie（参考 `api-enhanced/util/request.js` 的策略）

### 6) 兼容与降级

- `api-enhanced` 默认 eapi 走 `https://interface.music.163.com`；某些网络环境可能 DNS 不稳定，客户端实现里对该域名做了降级尝试到 `https://music.163.com`。
- DTO 解析兼容多种返回结构（例如二维码 `unikey` 可能在顶层或 `data` 内），统一在转换层处理。

## Roadmap（建议顺序）

1. 播放器内核：`song_url` → 音频拉流/解码/播放（rodio/cpal + symphonia）+ 播放状态事件
2. 播放队列与控制：上一首/下一首/暂停/Seek/音量/循环随机
3. 歌词与进度同步：`/api/song/lyric` + 滚动高亮
4. 歌单/收藏/每日推荐等业务页
5. MPRIS/通知/媒体键（Linux）

## 开发约定

- 目标：优先保持结构清晰与可维护，避免把“网易云协议细节”散落到 UI 里。
- 分层规则：
  - `tui` 只依赖 `messages` 与状态结构，不直接调用 `netease`/`reqwest`/`serde_json`。
  - `netease`（网关）只做“请求/解析/持久化 cookie”，不做业务拼装。
  - 业务拼装与策略统一放 `usecases`（AppActor）。
- 变更方式：按功能切分、逐步提交（commit），便于回滚与 code review。
