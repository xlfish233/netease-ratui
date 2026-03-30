# Architecture

系统架构概述：组件、关系、数据流、不变量。

## Data Flow

```
UI (keyboard/mouse) → AppCommand → core::reducer → Features → CoreEffects
  → NeteaseCommand/AudioCommand → NeteaseActor/AudioWorker
  → NeteaseEvent/AudioEvent → core::reducer → AppEvent::State(AppSnapshot) → UI render
```

## Key Components

- **App** (src/app/state.rs): 全量业务状态，~40 字段
- **AppSnapshot** (src/app/state.rs): UI 渲染轻量快照，按视图分片
- **CoreState** (src/core/reducer.rs): 持有 App + settings + 基础设施
- **CoreEffects**: 副作用收集器（状态推送、命令下发）
- **RequestTracker**: req_id 追踪，丢弃过期响应

## UI Layer (src/ui/tui/)

- `event_loop.rs`: 主循环 200ms tick，处理 AppEvent + 用户输入
- `keyboard.rs`: handle_key() 按优先级分发（Toast > Help > 全局 > 视图特定）
- `mouse.rs`: handle_mouse() 当前仅支持标签页点击
- `views.rs`: draw_ui() 主绘制入口
- `player_status.rs`: 底部 footer 渲染
- `widgets.rs`: 进度条等自定义 widget
- `toast.rs`: Toast 通知组件
- `overlays.rs`: 帮助覆盖层

## New Components (to be built)

- `src/ui/tui/menu.rs`: 操作菜单渲染和交互
- `src/keybindings/`: 快捷键配置加载和解析
