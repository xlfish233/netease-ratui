# netease-ratui

![Rust](https://img.shields.io/badge/Rust-2024%20edition-DEA584?logo=rust&logoColor=white)
![TUI](https://img.shields.io/badge/TUI-ratatui-2b2b2b)
![Status](https://img.shields.io/badge/status-WIP-yellow)

> 网易云音乐 TUI 客户端（Rust + ratatui），学习向，持续迭代中。

⚠️ **法律声明**：本项目仅用于学习与技术研究，非网易云音乐官方产品，与网易公司无任何关联。请遵守网易云音乐服务条款与当地法律法规。

⚠️ **风险提示**：本项目在 Rust 侧实现了 weapi/eapi 等请求与加密流程，可能随官方更新失效。

## 目录

- [特性](#特性)
- [预览](#预览)
- [安装](#安装)
- [运行](#运行)
- [配置与数据目录](#配置与数据目录)
- [快捷键](#快捷键)
- [架构](#架构)
- [开发](#开发)
- [路线图](#路线图)
- [致谢](#致谢)

## 特性

- 登录与鉴权：匿名态初始化、二维码登录、Cookie 登录
- 歌单与搜索：加载用户歌单与歌曲、搜索并播放选中歌曲
- 歌词体验：自动滚动、当前行高亮、偏移调整
- 播放能力：暂停/继续、上一首/下一首、Seek、音量与播放模式切换
- 预加载与缓存：歌单预加载、音频缓存与下一首预取
- 设置持久化：音质/音量/播放模式/歌词 offset 等写入 `settings.json`
- **播放状态持久化：自动保存播放队列、播放进度、音量设置，重启后精确恢复**
- 日志体系：tracing 日志落盘，便于排查问题
- 直观交互：UI 面板显示快捷键提示（F1-F4 切换视图，1-4 切换焦点，Alt+1-4 搜索中切换）

## 预览

暂无截图，欢迎补充。

## 安装

### 预编译包

从 GitHub Releases 下载与系统匹配的压缩包，解压后运行可执行文件即可。

### 从源码构建

```bash
# 需要支持 Rust 2024 edition 的工具链
cargo run
```

Linux 可能需要系统音频依赖（Debian/Ubuntu 示例）：

```bash
sudo apt-get update
sudo apt-get install -y libasound2-dev
```

## 运行

```bash
# 运行 TUI（默认）
cargo run

# 无声模式（禁用音频输出）
cargo run -- --no-audio

# 无交互快速自测（匿名搜索）
cargo run -- skip-login "周杰伦" --limit 5

# 打印二维码登录相关信息（便于排查接口返回）
cargo run -- qr-key
```

运行时终端最小画布为 `122x29`，尺寸更大时会居中显示，尺寸更小时会提示放大。
右侧队列按实际播放顺序展示，随机模式为洗牌后的顺序。

也可以通过环境变量走兼容入口：

- `NETEASE_SKIP_LOGIN=1`：等价于 `skip-login`
- `NETEASE_QR_KEY=1`：等价于 `qr-key`

## 配置与数据目录

### 数据目录

默认由 `directories` 计算（Linux 通常为 `~/.local/share/netease-ratui`）。可通过以下方式覆盖：

- `--data-dir` 或 `NETEASE_DATA_DIR`
- 日志目录：`--log-dir` 或 `NETEASE_LOG_DIR`

目录内主要文件：

- `settings.json`：UI 设置与下载/缓存参数
- `player_state.json`：播放状态持久化（播放队列、播放进度、音量等）
- `netease_state.json`：Cookie 与设备信息
- `audio_cache/`：音频缓存
- `logs/netease-ratui.log.YYYY-MM-DD`：运行日志

### 播放状态持久化

应用会自动保存和恢复播放状态：

- **保存时机**：
  - 应用退出时（按 `q`）
  - 每 30 秒自动保存（防止意外关闭丢失数据）
- **恢复时机**：
  - 应用启动时自动恢复
  - 默认恢复为暂停状态，不会自动播放
  - **重启后按空格键自动恢复播放**（无需手动重新选择歌曲）
- **保存内容**：
  - 播放队列（歌曲列表和顺序）
  - 播放进度（精确到毫秒）
  - 播放模式（顺序/列表循环/单曲循环/随机）
  - 音量设置
  - 音质设置
  - Crossfade 时长
  - 歌单列表

### player_state.json 格式

```json
{
  "version": 1,
  "player": {
    "version": 1,
    "play_song_id": 12345,
    "progress": {
      "started_at_epoch_ms": 1704067200000,
      "total_ms": 180000,
      "paused": true,
      "paused_at_epoch_ms": 1704067380000,
      "paused_accum_ms": 5000
    },
    "play_queue": {
      "songs": [
        {
          "id": 12345,
          "name": "歌曲名",
          "artists": "歌手名"
        }
      ],
      "order": [0, 1, 2],
      "cursor": 0,
      "mode": "ListLoop"
    },
    "volume": 0.8,
    "play_br": 320000,
    "crossfade_ms": 300
  },
  "playlists": [
    {
      "id": 1,
      "name": "我的歌单",
      "track_count": 100
    }
  ],
  "playlists_selected": 0,
  "saved_at_epoch_ms": 1704067380000
}
```

### settings.json

启动时读取，设置页修改会自动写回；手动编辑后建议重启生效。

```json
{
  "volume": 1.0,
  "br": 999000,
  "play_mode": "ListLoop",
  "lyrics_offset_ms": 0,
  "crossfade_ms": 300,
  "preload_count": 5,
  "audio_cache_max_mb": 2048,
  "download_concurrency": null,
  "http_timeout_secs": 30,
  "http_connect_timeout_secs": 10,
  "download_retries": 2,
  "download_retry_backoff_ms": 250,
  "download_retry_backoff_max_ms": 2000
}
```

`play_mode` 可选值：`Sequential`、`ListLoop`、`SingleLoop`、`Shuffle`。
`download_concurrency` 为 `null` 时自动检测 CPU 并发。

### 环境变量

- `RUST_LOG`：日志级别（如 `debug`）
- `NETEASE_DOMAIN`：覆盖网易域名（默认 `https://music.163.com`）
- `NETEASE_API_DOMAIN`：覆盖 API 域名（默认 `https://interface.music.163.com`）
- `NETEASE_NO_AUDIO=1`：禁用音频输出（无声模式）

## 快捷键

全局：

- `F1-F4` 切换页签；`1-4` 切换焦点；`Alt+1-4` 搜索中切换焦点；`Tab` 循环焦点；`q` 退出；`?` 帮助
- `Space` 播放/暂停；`[`/`]` 上一首/下一首
- `Ctrl+S` 停止；`Ctrl+←/→` Seek（±5s）
- `Alt+↑/↓` 音量；`M` 切换播放模式
- `Alt+←/→` 歌词 offset（±200ms），`Shift+Alt+←/→`（±50ms，仅歌词页）
- 鼠标左键点击页签切换页面

登录页：

- `l` 生成二维码；`c` 切换 Cookie 登录
- Cookie 模式：`Enter` 提交，`Esc` 取消，`Backspace` 删除

歌单页：

- `↑/↓` 选择；`Enter` 打开歌单；`p` 播放选中；`b` 返回列表

搜索页：

- 输入关键词；`Enter` 搜索；`p` 播放选中；`↑/↓` 选择

歌词页：

- `o` 跟随/锁定滚动；`g` 回到当前行；`↑/↓` 手动滚动

设置页：

- `↑/↓` 选择；`←/→` 调整；`Enter` 执行操作

## 架构

项目遵循“Actor + 消息驱动 + 单一状态源”设计，UI 只渲染 `AppSnapshot`。
详细说明见 `ARCHITECTURE.md`。

## 开发

```bash
# 运行测试
cargo test

# 运行所有检查（格式 + clippy + 测试）
make check

# 代码覆盖率检查
make coverage

# 安装 pre-commit hooks
make install-hooks
```

## 路线图

- 完善 Now Playing 信息展示（封面/艺人/专辑）
- 可配置快捷键与主题
- MPRIS 与系统媒体键集成
- 桌面通知与可选封面

## 致谢

- https://github.com/feng-yifan/Netease-Cloud-Music-Web-Player
- https://github.com/NeteaseCloudMusicApiEnhanced
- https://github.com/go-musicfox/go-musicfox
