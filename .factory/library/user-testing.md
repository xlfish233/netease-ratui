# User Testing

测试面、工具、资源成本分类。

## Validation Surface

- **Surface**: 终端 TUI 应用
- **Primary Tool**: `cargo test` (单元测试)
- **Secondary Tool**: `tuistory` (E2E 黑盒测试，可选)
- **Build**: `cargo build`
- **Lint**: `cargo clippy -- -D warnings && cargo fmt --check`

## Validation Concurrency

- **cargo test**: 资源消耗极低（纯 Rust 单元测试），max concurrent = 5
- **tuistory**: 每实例约 50-100MB (PTY + Node)，系统 59GB/16核，max concurrent = 5

## Test Patterns

- 键盘测试：构造 AppSnapshot + KeyEvent → 调用 handle_key() → 断言 channel 命令
- 鼠标测试：构造 AppSnapshot + MouseEvent → 调用 handle_mouse() → 断言 channel 命令
- Reducer 测试：构造 CoreState → 调用 handle_ui() → 断言 state 变更和 effects
- 渲染测试：构造 TestBackend → terminal.draw() → 断言输出内容
