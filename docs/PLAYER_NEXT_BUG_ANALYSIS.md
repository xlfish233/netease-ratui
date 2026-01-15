# 快速按下一首键无法播放问题分析

## 问题描述

当快速连续按 `]` 键（下一首）时，会出现以下情况：
- 音频请求发送成功（日志显示 "开始播放请求"）
- 但歌曲无法播放（没有声音输出）
- 日志中出现大量 "cache ready token mismatch" 调试信息

## 日志分析

### 问题日志示例

```
2026-01-15T11:15:10.401524Z  INFO netease_ratui::audio_worker::engine: 开始播放请求 song_id=421148673 br=999000 title=I Will Be OK - FlyBoy/Coby Grant/The Onyx Twins
2026-01-15T11:15:10.401573Z  INFO netease_ratui::audio_worker::engine: request cache song_id=421148673 br=999000 token=44
2026-01-15T11:15:10.401616Z  INFO netease_ratui::audio_worker::transfer: cache hit song_id=421148673 br=999000 token=44
2026-01-15T11:15:10.401640Z  INFO netease_ratui::audio_worker::engine: cache ready song_id=421148673 br=999000 path=/home/xl/.local/share/netease-ratui/audio_cache/421148673_999000.bin
2026-01-15T11:15:11.253011Z  INFO netease_ratui::audio_worker::engine: 开始播放请求 song_id=421148673 br=999000 title=I Will Be OK - FlyBoy/Coby Grant/The Onyx Twins
2026-01-15T11:15:11.253063Z  INFO netease_ratui::audio_worker::engine: request cache song_id=421148673 br=999000 token=45
2026-01-15T11:15:11.253107Z  INFO netease_ratui::audio_worker::transfer: cache hit song_id=421148673 br=999000 token=45
2026-01-15T11:15:11.253130Z  INFO netease_ratui::audio_worker::engine: cache ready song_id=421148673 br=999000 path=/home/xl/.local/share/netease-ratui/audio_cache/421148673_999000.bin
```

**关键问题**：同一个歌曲（song_id=421148673）在短时间内被请求了多次（token 44, 45, 46...），但只有最新的 token 会被接受。

## 根本原因

### Token 机制

音频引擎使用 **token 机制**来防止过期的播放请求覆盖新的请求：

1. **请求阶段** (`handle_audio_command`):
   ```rust
   let token = self.next_token;
   self.next_token = self.next_token.wrapping_add(1).max(1);

   self.pending_play = Some(PendingPlay {
       token,
       key,
       title: title.clone(),
       url: url.clone(),
       retries: 0,
   });
   ```

2. **响应阶段** (`handle_transfer_event`):
   ```rust
   if let Some(pending) = self.pending_play.as_ref()
       && pending.token != token
   {
       tracing::debug!(
           token,
           pending_token = pending.token,
           song_id = key.song_id,
           "cache ready token mismatch"
       );
   }
   let Some(mut p) = self.pending_play.take().filter(|p| p.token == token) else {
       return;  // 直接返回，不播放
   };
   ```

### 问题流程

当用户快速按 `]` 键时：

```
时间线：
T1: 按 ] → 发送 PlayTrack(song_A, token=44)
T2: 按 ] → 发送 PlayTrack(song_B, token=45)  → pending_play.token 更新为 45
T3: cache ready (song_A, token=44)           → token 不匹配（44 != 45），丢弃 ❌
T4: 按 ] → 发送 PlayTrack(song_C, token=46)  → pending_play.token 更新为 46
T5: cache ready (song_B, token=45)           → token 不匹配（45 != 46），丢弃 ❌
T6: cache ready (song_C, token=46)           → token 匹配，播放 ✅
```

### 竞态条件

**问题**：快速按键时，旧的缓存响应（token=44）到达时，`pending_play` 已经被新的请求（token=45）覆盖，导致旧响应被丢弃。

**触发条件**：
1. 用户快速连续按 `]` 键（间隔 < 缓存响应时间）
2. 缓存命中（cache hit）时响应很快（~0.1ms）
3. 多个播放请求在短时间内进入队列

## 影响范围

### 受影响场景

1. **快速切歌**：连续按 `]` 或 `[`
2. **预缓存冲突**：预缓存和播放请求同时到达
3. **网络波动**：旧请求延迟到达被新请求覆盖

### 日志统计

从日志可以看到：
- 同一首歌在 8 秒内被请求了 14 次（token 40-53）
- 大量 "cache ready token mismatch" 调试信息
- 实际只有最后一次请求成功播放

## 解决方案

### 方案 1：取消旧请求（推荐）

**思路**：在发送新的 PlayTrack 时，主动取消旧的 pending 请求。

```rust
// AudioEngine::handle_audio_command
AudioCommand::PlayTrack { id, br, url, title } => {
    // 取消旧请求
    if let Some(old_pending) = &self.pending_play {
        tracing::debug!(
            old_token = old_pending.token,
            new_token = self.next_token,
            "取消旧播放请求"
        );
        // 发送取消命令给 transfer
        let _ = self.tx_transfer
            .send(TransferCommand::Cancel {
                token: old_pending.token,
                key: old_pending.key,
            })
            .await;
    }

    self.pending_play = None;
    // ... 继续新请求
}
```

**优点**：
- 明确取消不需要的请求
- 减少不必要的缓存操作
- 日志更清晰

**缺点**：
- 需要修改 transfer 层支持取消
- 增加复杂度

### 方案 2：Pending Queue（简单）

**思路**：维护一个小的 pending 队列，允许多个请求并存。

```rust
struct AudioEngine {
    // ...
    pending_plays: VecDeque<PendingPlay>,
    max_pending: usize,  // 例如 3
}

// 在 handle_transfer_event 中
TransferEvent::Ready { token, key, path } => {
    // 查找匹配的 pending
    let pos = self.pending_plays.iter().position(|p| p.token == token);
    let Some(p) = pos.and_then(|i| self.pending_plays.remove(i)) else {
        tracing::debug!("token not found in pending queue");
        return;
    };

    // 清理所有旧的 pending（这个 token 之前的都过期了）
    self.pending_plays.retain(|p| p.token > token);

    // 播放...
}
```

**优点**：
- 改动较小
- 允许少量并发

**缺点**：
- 可能导致旧的请求延迟播放
- 需要合理设置 max_pending

### 方案 3：忽略重复请求（最简单）

**思路**：在 `handle_audio_command` 中检查是否已经在请求同一首歌。

```rust
AudioCommand::PlayTrack { id, br, url, title } => {
    // 如果正在请求同一首歌，忽略
    if let Some(pending) = &self.pending_play {
        if pending.key.song_id == id && pending.key.br == br {
            tracing::debug!(
                song_id = id,
                "已在请求中，忽略重复请求"
            );
            return;
        }
    }

    // ... 继续新请求
}
```

**优点**：
- 实现最简单
- 避免重复请求

**缺点**：
- 不能处理不同歌曲的快速切换
- 仍然有 token mismatch 问题

### 方案 4：优化 RequestTracker（治本）

**思路**：在 Core 层的 RequestTracker 过滤掉快速重复的下一首请求。

```rust
// 在 features/player/playback.rs 的 play_next 中
pub async fn play_next(...) {
    // 检查是否有未完成的 SongUrl 请求
    if request_tracker.has_pending(&RequestKey::SongUrl) {
        tracing::debug!(
            "有未完成的 SongUrl 请求，忽略快速重复的下一首按键"
        );
        return;  // 或者返回 false
    }

    // ... 继续正常流程
}
```

**优点**：
- 从源头防止问题
- 不需要修改音频层
- 符合现有的 RequestTracker 设计

**缺点**：
- 可能需要调整超时逻辑

## 推荐实施顺序

1. **已实施**：方案 4（RequestTracker 过滤）
   - 在 `play_next` 中通过 `has_pending` 做源头过滤
   - 符合现有架构

2. **已实施**：方案 1（取消旧请求）
   - 新的播放请求会取消旧的 pending 请求
   - Transfer 层支持按 token 清理等待者

## 测试验证

### 测试场景

1. **快速按键测试**：
   ```bash
   # 快速按 ] 键 10 次（间隔 < 100ms）
   # 预期：只播放最后一首，无 token mismatch 调试信息
   ```

2. **正常切歌测试**：
   ```bash
   # 正常速度按 ] 键（间隔 > 500ms）
   # 预期：每首都能正常播放
   ```

3. **预缓存测试**：
   ```bash
   # 播放一首歌，快速切换到下一首
   # 预期：预缓存不会干扰播放
   ```

### 验证日志

修复后应该看到：
- 减少 "cache ready token mismatch" 调试信息
- 每次切歌都成功播放
- 无重复的 "开始播放请求" 日志

## 相关文件

- `src/audio_worker/engine.rs` - 音频引擎和 token 机制
- `src/features/player/playback.rs` - play_next 函数
- `src/core/infra/request_tracker.rs` - 请求追踪器
- `src/audio_worker/transfer.rs` - 缓存传输层

## 总结

这个问题的核心是 **token 机制的竞态条件**：当用户快速切换歌曲时，旧的缓存响应被新的请求覆盖，导致播放失败。

最佳解决方案是在 **Core 层的 RequestTracker 过滤快速重复请求**，这样既符合现有架构，又能从源头解决问题。
