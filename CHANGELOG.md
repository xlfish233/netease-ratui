# Changelog

## Unreleased（自 v0.0.2 起，更新于 2026-01-12）

### 大规模模块化重构

将 4 个核心大文件拆分为 37 个职责单一的子模块，提高代码可维护性：

- **AppActor 重构** (`src/usecases/actor.rs`，1072 行 → 510 行，-52%)：
  - 拆分为 12 个子模块：login.rs、search.rs、playlists.rs、lyrics.rs、player_control.rs、settings_handler.rs、audio_handler.rs、playback.rs、preload.rs、playlist_tracks.rs、logout.rs、utils.rs

- **TUI 重构** (`src/tui.rs`，891 行)：
  - 拆分为 14 个子模块：event_loop.rs、guard.rs、keyboard.rs、mouse.rs、views.rs、login_view.rs、lyrics_view.rs、playlists_view.rs、search_view.rs、settings_view.rs、player_status.rs、widgets.rs、utils.rs

- **AudioWorker 重构** (`src/audio_worker.rs`，613 行)：
  - 拆分为 6 个子模块：messages.rs、worker.rs、player.rs、cache.rs、download.rs

- **NeteaseClient 重构** (`src/netease/client.rs`，735 行 → ~100 行)：
  - 拆分为 5 个子模块：config.rs、cookie.rs、error.rs、types.rs

- **Bug 修复**：
  - fix(tui): 支持 Ctrl+C 信号处理（raw mode 下启用 tokio signal）

- **代码质量优化**：
  - 移除冗余 clone 和死代码
  - 简化 Option 处理（使用链式方法）
  - 优化模式匹配语法
  - 改进函数签名（引用传递替代值传递）

### 其他功能

- **新增"缓存下一首歌"功能**：播放时自动预缓存下一首歌曲，实现无缝切换
  - 支持播放模式：顺序播放、列表循环
  - 不支持模式：随机播放（不可预测）、单曲循环（下一首是当前）
  - 智能失效：队列改变/播放模式切换/停止播放/用户登出时自动失效
  - 错误处理：预缓存失败静默处理，不影响当前播放
- **音频缓存下载链路升级**：TransferActor 改为 tokio 异步下载与落盘缓存（消息协议保持稳定）
  - 支持下载超时与自动重试（可配置）
  - 并发下载默认等于 CPU 核心数（可配置）
  - 缓存策略：仅保留当前设置的音质 br（切换音质会清理其它 br）
- **新增 Cookie 登录功能**：支持手动输入 `MUSIC_U` Cookie 值快速登录
  - 登录页按 `c` 键切换到 Cookie 登录模式
  - 支持输入验证和错误提示
  - 适用于已通过浏览器登录 music.163.com 的用户
- 登录页 UI 改进：显示两种登录方式的说明和快捷键提示

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
