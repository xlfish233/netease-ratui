# 错误处理策略

## 概述

netease-ratui 使用分层错误处理策略，在不同层次采用不同的错误处理方式。

## 错误类型体系

项目使用 `thiserror` 定义结构化错误类型（位于 `src/error/`）：

- `AppError` - 应用通用错误
  - IO 错误
  - 序列化错误
  - 设置错误
  - 网易云音乐 API 错误
  - 音频错误

- `AudioError` - 音频播放错误
  - 打开音频文件失败
  - 解码音频失败
  - 下载错误
  - 缓存操作失败
  - 播放器初始化失败
  - Seek 失败
  - 音频输出流创建失败

- `NeteaseError` - 网易云音乐 API 错误
  - 网络请求错误
  - IO 错误
  - 序列化错误
  - 加密错误
  - Cookie 验证失败
  - API 返回业务错误
  - HTTP 头构造失败
  - 输入参数无效

- `CacheError` - 缓存操作错误
  - 缓存目录不可用
  - 提交临时文件失败
  - 索引加载/保存失败
  - 缓存大小超限
  - 文件操作失败
  - 序列化失败

- `DownloadError` - 下载错误
  - HTTP 请求错误
  - HTTP 状态码错误
  - 创建/写入文件失败
  - 超过最大重试次数
  - 下载 URL 无效

所有错误类型都通过 `thiserror` 自动实现了 `Display` 和 `Error` trait，支持错误链（`#[source]`）。

## 分层错误处理

### UI 层（ratatui）
- **职责**: 接收并显示错误消息
- **实现**: 接收 `AppEvent::Error(String)` 和 `AudioEvent::Error(String)`
- **处理方式**: 在状态栏显示错误消息给用户
- **不处理错误恢复**: 由用户决定后续操作（重试、切换歌曲等）

### Core 层（tokio）
- **职责**: 业务逻辑和错误传播
- **实现**: 使用 `Result<T, E>` 传播错误
- **处理方式**: 使用 `?` 操作符自动转换错误类型
- **转换**: 将错误转换为 `AppEvent::Error` 发送到 UI

```rust
// 示例：在 Core 层处理错误
pub async fn handle_command(cmd: AppCommand) -> Result<(), AppError> {
    match cmd {
        AppCommand::SomeAction => {
            // 使用 ? 传播错误
            some_failable_operation()?;
            Ok(())
        }
    }
}
```

### Worker 层（std::thread）
- **职责**: 在独立线程中执行阻塞操作
- **实现**: 使用 `std::thread::spawn` 创建独立线程
- **限制**: 闭包无法返回 `Result`，无法直接传播错误
- **处理方式**:
  - 使用 `expect()` 处理系统级错误（如 runtime 创建）
  - 通过 channel 发送错误事件到主线程
  - 已处理真正的业务错误（如下载失败、IO 错误）

```rust
// 示例：Worker 层错误处理
std::thread::spawn(move || {
    // 使用 expect() 处理系统级错误
    let rt = tokio::runtime::Runtime::new()
        .expect("tokio runtime: 系统资源不足或配置错误");

    rt.block_on(async move {
        // 业务错误通过 Result 返回
        match some_operation().await {
            Ok(result) => { /* 处理成功 */ }
            Err(e) => {
                // 发送错误事件到主线程
                let _ = tx_evt.send(AudioEvent::Error(e.to_string())).await;
            }
        }
    });
});
```

## expect() 使用策略

### 何时使用 expect()

在以下情况下使用 `expect()` 是合理的：

1. **在 `std::thread::spawn` 闭包中**
   - 无法返回 `Result`
   - 失败表示系统级问题
   - 已通过其他方式处理业务错误

2. **理论上不可能失败的操作**
   - 经过验证的前置条件
   - 编译时保证的正确性

3. **测试代码中**
   - 测试失败应该 panic
   - 使用 `unwrap()` 和 `expect()` 是标准做法

### 何时不使用 expect()

避免在以下情况使用 `expect()`：

1. **可恢复的业务错误**
   - 使用 `Result<T, E>` 返回错误
   - 让调用方决定如何处理

2. **外部输入处理**
   - 网络请求、文件 I/O、用户输入
   - 使用 `?` 传播错误

3. **可能导致 panic 的用户操作**
   - 提供友好的错误消息
   - 优雅降级

## 当前项目中的 expect()

### 1. audio_worker/engine.rs:394

```rust
let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .expect("tokio runtime: 系统资源不足或配置错误");
```

- **用途**: 创建 tokio runtime
- **位置**: `std::thread::spawn` 闭包中
- **安全性**: 系统资源耗尽时才失败
- **业务错误处理**: `OutputStreamBuilder::open_default_stream()` 失败会发送 `AudioEvent::Error`
- **替代方案**: `tokio::task::spawn_blocking`（需架构改动）

### 2. audio_worker/transfer.rs:488

```rust
let rt = tokio::runtime::Runtime::new()
    .expect("tokio runtime: 系统资源不足或配置错误");
```

- **用途**: 创建 tokio runtime
- **位置**: `std::thread::spawn` 闭包中
- **安全性**: 与 engine.rs 相同
- **业务错误处理**: 下载、缓存错误通过 `JobResult` 返回
- **替代方案**: 同 engine.rs

> 说明：此前项目曾使用“每首歌启动一个播放结束监控线程”的方式检测播放结束；该实现已移除，改为在音频引擎中定时轮询 `sink` 状态触发 `Ended` 事件，从而避免线程风暴与相关 `expect()`。

## 错误恢复策略

### 可重试的错误

以下错误类型定义了 `is_retryable()` 方法：

- `DownloadError::Http` - 网络请求错误，自动重试
- `DownloadError::StatusCode` - HTTP 状态码错误，自动重试
- `DownloadError::Write` - 写入文件失败，自动重试
- `NeteaseError::Reqwest` - 网络请求错误，可重试
- `AudioError::Download` - 下载错误，可重试
- `AudioError::Seek` - Seek 失败，可重试

### 不可重试的错误

- `NeteaseError::CookieValidationFailed` - Cookie 无效，需重新登录
- `DownloadError::MaxRetriesExceeded` - 超过最大重试次数
- `AudioError::Init` - 播放器初始化失败
- `CacheError::DirUnavailable` - 缓存目录不可用

### 用户可见的错误

所有错误最终都转换为 `AppEvent::Error(String)` 或 `AudioEvent::Error(String)`，在 UI 的状态栏显示：

```
错误: 无法连接到网易云音乐 API
错误: 音频文件不存在
错误: 初始化音频输出失败: No such device
```

## 未来改进方向

### 1. 使用 `tokio::task::spawn_blocking`

**优点**:
- 可以在异步上下文中运行阻塞代码
- 可以使用 `?` 传播错误
- 完全消除 `expect()`

**缺点**:
- 需要较大的架构改动
- 可能影响性能
- 需要仔细测试

**示例**:
```rust
// 当前
std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().expect("...");
    rt.block_on(async { /* ... */ });
});

// 改进后
tokio::task::spawn_blocking(move || {
    // 可以直接返回 Result
    block_on_operation()
}).await??;
```

### 2. 更细粒度的错误分类

- 区分可恢复和不可恢复错误
- 提供自动重试机制
- 添加错误等级（Info、Warning、Error、Critical）

### 3. 错误监控和日志

- 集成 `tracing` 进行结构化日志
- 收集错误统计
- 改进错误消息的可读性
- 添加错误上下文（用户操作、系统状态）

### 4. 用户友好的错误消息

- 技术错误 → 用户可理解的描述
- 提供解决建议
- 国际化支持

## 最佳实践

### 1. 使用结构化错误类型

```rust
// ✅ 推荐
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("文件读取失败: {path}")]
    ReadFile { path: PathBuf, #[source] std::io::Error },
}

// ❌ 避免
pub type MyError = String;
```

### 2. 提供错误上下文

```rust
// ✅ 推荐
let file = File::open(path)
    .map_err(|e| MyError::ReadFile {
        path: path.clone(),
        source: e,
    })?;

// ❌ 避免
let file = File::open(path).expect("无法打开文件");
```

### 3. 使用 `?` 传播错误

```rust
// ✅ 推荐
async fn process() -> Result<(), MyError> {
    fetch_data().await?;
    Ok(())
}

// ❌ 避免
async fn process() {
    fetch_data().await.unwrap();
}
```

### 4. 为 `expect()` 提供清晰的错误消息

```rust
// ✅ 推荐
.expect("tokio runtime: 系统资源不足或配置错误")

// ❌ 避免
.expect("failed")
```

## 相关文档

- [错误类型定义](../src/error/)
- [thiserror 文档](https://docs.rs/thiserror/)
- [Rust 错误处理最佳实践](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
