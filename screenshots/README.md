# 截图指南

本目录存放 netease-ratui 的项目截图。

## 截图清单

### 建议的截图场景

> 提示：TUI 运行时终端最小画布为 `122x29`，建议使用更大尺寸截图以获得更好的观感。

#### 1. 登录页面
- **文件名**: `login.png`
- **内容**: 
  - 二维码显示
  - Cookie 登录输入框
  - 状态提示信息
- **尺寸**: 建议至少 122x29 字符

#### 2. 主界面 - 歌单视图
- **文件名**: `playlist_view.png`
- **内容**:
  - 左侧：歌单列表
  - 中间：歌曲列表
  - 右侧：播放队列
  - 底部：播放状态和快捷键提示
- **状态**: 已登录状态

#### 3. 主界面 - 搜索视图
- **文件名**: `search_view.png`
- **内容**:
  - 搜索输入框
  - 搜索结果列表
  - 播放状态
- **搜索词**: 建议使用热门歌曲（如"周杰伦"）

#### 4. 主界面 - 歌词视图
- **文件名**: `lyrics_view.png`
- **内容**:
  - 当前播放歌词
  - 自动滚动高亮
  - 歌词偏移信息

#### 5. 主界面 - 设置视图
- **文件名**: `settings_view.png`
- **内容**:
  - 音质设置
  - 音量设置
  - 播放模式
  - 缓存管理选项

#### 6. 播放状态
- **文件名**: `playing.png`
- **内容**:
  - 歌曲正在播放
  - 显示播放进度
  - 播放控制按钮

## 如何截图

### 方法 1: 使用终端截图工具

#### Linux ( GNOME Terminal )
```bash
# 使用 gnome-screenshot
gnome-screenshot -a

# 或使用 scrot
scrot -s screenshot.png
```

#### Linux ( KDE Konsole )
```bash
# 使用 spectacle
spectacle -r -b -n screenshot.png
```

#### macOS
```bash
# 使用内置截图
Cmd + Shift + 4  # 选择区域
Cmd + Shift + 4 + Space  # 截图窗口
```

#### Windows ( Windows Terminal / PowerShell )
```bash
# 使用 Windows 截图工具
Win + Shift + S  # 选择区域
```

### 方法 2: 使用 Rust 集成测试截图

在 TUI 测试中添加截图功能：

```rust
#[tokio::test]
async fn test_take_screenshot() {
    // 运行 TUI
    // 使用 rustshot 或其他库
    // 保存到 screenshots/ 目录
}
```

### 方法 3: 使用虚拟终端

```bash
# 使用 vhs (Video Helper for Script)
# https://github.com/charmbracelet/vhs

vhs demo.tape
```

示例 `demo.tape`:
```yaml
Output screenshots/demo.gif
Font family "DejaVu Sans Mono"
Font size 14
Set "font-size: 14px" # 设置终端字体

Type "cargo run"
Enter
Sleep 5s
```

## 截图要求

### 格式
- **格式**: PNG（推荐）或 GIF（动图）
- **分辨率**: 最小 122x29 字符
- **文件大小**: 建议 < 500KB

### 内容
- 展示核心功能
- 界面清晰可读
- 避免敏感信息（如个人 Cookie、二维码内容）

### 风格
- 使用亮色主题（更容易识别）
- 窗口大小适中
- 避免过多空白

## 自动化截图（推荐）

### 使用 vhs 创建 GIF 动图

1. 安装 vhs:
```bash
cargo install vhs
```

2. 创建 tape 文件 `screenshots/demo.tape`:

```yaml
Output screenshots/demo.gif
Width 800
Height 480
Font family "DejaVu Sans Mono"
Font size 16

# 设置主题
Set "font-size: 16px"
Set "background-color: #1e1e1e"
Set "font-color: #d4d4d4"

# 启动应用
Type "cargo run"
Enter
Sleep 2s

# 展示登录页面
Sleep 3s

# 登录（使用匿名模式）
Type "c"
Enter
Sleep 1s

# 切换到搜索
Type "\t"
Enter
Sleep 1s

# 搜索歌曲
Type "周杰伦"
Enter
Sleep 2s

# 播放歌曲
Type "Enter"
Sleep 3s

# 暂停/继续
Type " "
Sleep 2s

# 展示设置
Type "F4"
Sleep 2s
```

3. 生成 GIF:
```bash
vhs screenshots/demo.tape
```

## 更新 README

添加截图后，更新 README.md 的"预览"部分：

```markdown
## 预览

### 登录页面
![登录页面](screenshots/login.png)

### 主界面
![主界面](screenshots/main.png)

### 播放中
![播放中](screenshots/playing.png)
```

## 检查清单

- [ ] 所有截图清晰可读
- [ ] 文件名符合命名规范
- [ ] 文件大小合理（< 500KB）
- [ ] 已更新 README.md
- [ ] 截图添加到 Git 仓库

## 工具推荐

- **vhs** - 终端 GIF 录制工具
- **asciinema** - 终端会话录制
- **terminalizer** - 终端截图生成器
- **rustshot** - Rust 截图库
