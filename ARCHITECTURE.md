# ARCHITECTURE

本文描述当前架构与运行时模型，侧重“数据流、模块边界与持久化”。

## 设计目标

- 单一状态源：所有可见状态都来自 `App`，UI 只读
- 消息驱动：UI 与业务/网关/音频通过命令与事件通信
- 清晰分层：UI、Core、Features、Gateways 各自负责单一职责
- 异步优先：tokio 运行时 + `tokio::sync::mpsc` 通道

## 总览

```mermaid
flowchart LR
  UI[UI (ratatui)] -->|AppCommand| Core[core::reducer]
  Core -->|AppEvent::State| UI
  Core -->|NeteaseCommand| NeteaseActor
  NeteaseActor -->|NeteaseEvent| Core
  Core -->|AudioCommand| AudioWorker
  AudioWorker -->|AudioEvent| Core
```

## 核心组件

### UI 层（`src/ui`）

- 负责渲染与输入处理，不直接写业务状态
- `AppSnapshot` 作为渲染输入，避免大对象深拷贝
- 事件 -> `AppCommand`，统一送入 `core::reducer`

### Core 层（`src/core`）

- `core::reducer` 是唯一状态写入者
- `CoreEffects` 收集“状态推送、提示、命令下发”等副作用
- `core::infra` 提供 `RequestTracker`、`PreloadManager`、`NextSongCacheManager`

### Features（`src/features`）

- 按领域划分：login/logout/lyrics/player/playlists/search/settings
- 处理 `AppCommand` / `NeteaseEvent` / `AudioEvent`
- 通过 `CoreEffects` 下发后续动作

### 网关与音频

- `src/netease`：请求、加密、Cookie 持久化、事件上报
- `src/audio_worker`：音频播放、缓存、预取、下载
- 音频传输使用 `tokio::sync::mpsc` 与 `select!` 协调播放与缓存任务

## 状态与快照

- `App`：全量业务状态（登录、播放、歌词、歌单、设置等）
- `AppSnapshot`：UI 渲染用轻量快照（减少 UI 线程负担）
- `CoreState`：持有 `App` + settings + 请求/预加载相关上下文

## 请求追踪与乱序丢弃

- 每次跨层请求携带 `req_id`
- `RequestTracker` 只接受最新请求对应的响应，避免旧响应覆盖新状态

## 预加载与缓存

- `PreloadManager`：歌单预加载状态管理
- `NextSongCacheManager`：下一首音频预取
- `TransferActor`：下载并发控制、重试与缓存管理

## 配置与持久化

数据目录包含：

- `settings.json`：设置与下载/缓存参数
- `netease_state.json`：Cookie 与设备信息
- `audio_cache/`：音频缓存
- `logs/`：tracing 日志

## 目录结构（要点）

- `src/ui`：TUI 渲染与输入
- `src/core`：reducer、effects、infra
- `src/features`：业务模块
- `src/netease`：API 与网关
- `src/audio_worker`：播放与缓存
- `src/app` / `src/domain`：状态与模型

## 关键流程示例

### 播放一首歌

1. UI 发送 `AppCommand::PlaylistTracksPlaySelected`
2. `features::playlists` 更新 `App` 并下发 `NeteaseCommand`
3. `NeteaseActor` 拉取播放链接，回传 `NeteaseEvent`
4. `features::player` 组装 `AudioCommand::PlayTrack`
5. `AudioWorker` 缓存/播放并回传 `AudioEvent` 更新 UI
