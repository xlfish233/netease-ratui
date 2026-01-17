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
- `PlayQueue`：统一播放顺序/随机顺序与游标位置，UI 队列按播放顺序展示
- `AppSnapshot`：UI 渲染用轻量快照（减少 UI 线程负担）
- `CoreState`：持有 `App` + settings + 请求/预加载相关上下文

## 请求追踪与乱序丢弃

- 每次跨层请求携带 `req_id`
- `RequestTracker` 只接受最新请求对应的响应，避免旧响应覆盖新状态

## 预加载与缓存

- `PreloadManager`：歌单预加载状态管理
- `NextSongCacheManager`：基于 `PlayQueue` 计算下一首音频预取（含随机模式）
- `TransferActor`：下载并发控制、重试与缓存管理

## 配置与持久化

数据目录包含：

- `settings.json`：设置与下载/缓存参数
- `player_state.json`：播放状态持久化（新增）
- `netease_state.json`：Cookie 与设备信息
- `audio_cache/`：音频缓存
- `logs/`：tracing 日志

### 播放状态持久化

`src/player_state` 模块负责播放状态的自动保存和恢复：

**保存时机：**
- 应用退出时（`q` 命令）
- 每 30 秒定时自动保存

**恢复时机：**
- 应用启动时自动加载
- 恢复后默认为暂停状态

**数据结构：**
- `AppStateSnapshot`：完整的可序列化状态快照
- `PlayerState`：播放器状态（播放进度、队列、设置）
- `PlayQueueState`：播放队列状态（歌曲、顺序、游标）
- `PlaybackProgress`：播放进度（使用时间戳替代 `Instant`）

**关键函数：**
- `save_player_state()`：序列化 App 状态到 JSON（原子写入）
- `load_player_state()`：从 JSON 反序列化
- `apply_snapshot_to_app()`：将快照恢复到 App 状态

**时间戳转换：**
- 保存时：`Instant` → epoch milliseconds（自 1970-01-01 的毫秒数）
- 恢复时：epoch milliseconds → `Instant`（精确计算播放位置）

## 目录结构（要点）

- `src/ui`：TUI 渲染与输入
- `src/core`：reducer、effects、infra
- `src/features`：业务模块
- `src/player_state`：播放状态持久化
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

### 播放状态保存与恢复

**保存流程（退出时）：**
1. 用户按 `q` → `AppCommand::Quit`
2. `core::reducer` 设置 `should_quit = true`
3. 退出前调用 `player_state::save_player_state()`
4. 序列化 `App` 状态为 `AppStateSnapshot`
5. 转换 `Instant` 为 epoch milliseconds
6. 原子写入 `player_state.json.tmp` → 重命名为 `player_state.json`

**恢复流程（启动时）：**
1. `core::reducer` 创建 `CoreState`
2. 调用 `player_state::load_player_state()`
3. 从 `player_state.json` 反序列化
4. 调用 `player_state::apply_snapshot_to_app()`
5. 转换 epoch milliseconds 为 `Instant`
6. 设置 `paused = true`（默认不自动播放）
7. 应用其他设置到音频 worker

**重启后自动恢复播放（NeedsReload 机制）：**
当用户重启应用后按空格键时，音频引擎检测到 sink 为 None，会触发自动恢复流程：

1. 用户按空格键 → `AppCommand::PlayerTogglePause`
2. `AudioEngine::TogglePause` 检测到 `sink = None`
3. 发送 `AudioEvent::NeedsReload` 事件
4. `features::player::audio` 处理 `NeedsReload` 事件：
   - 从 `play_song_id` 或播放队列获取当前歌曲
   - 清理旧的请求记录
   - 重新发送 `NeteaseCommand::SongUrl` 请求播放链接
5. `NeteaseActor` 返回播放链接
6. `AudioWorker` 开始播放

这样用户无需手动重新选择歌曲，只需按空格键即可自动恢复播放。

**定时保存（每 30 秒）：**
1. `tokio::time::interval` 触发
2. 调用 `player_state::save_player_state()`
3. 成功：debug 日志（可选）
4. 失败：warn 日志，不阻塞主循环
