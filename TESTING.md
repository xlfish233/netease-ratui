# 测试指南

本文档描述项目的测试策略和如何运行测试。

## 测试类型

### 单元测试

单元测试位于源文件中的 `#[cfg(test)]` 模块内，测试单个函数和模块。

```bash
# 运行所有单元测试
cargo test --lib

# 运行特定模块的测试
cargo test --lib netease::models::convert

# 运行特定测试
cargo test test_extract_unikey
```

### 集成测试

集成测试位于 `tests/` 目录，测试多个模块协同工作。

```bash
# 运行所有集成测试
cargo test --tests
```

## 代码覆盖率

使用 `cargo-tarpaulin` 生成代码覆盖率报告：

```bash
# 安装 tarpaulin
cargo install cargo-tarpaulin

# 生成 HTML 报告
cargo tarpaulin --out Html --timeout 300 -- --test-threads=1

# 生成 XML 报告（用于 CI）
cargo tarpaulin --out Xml --output-dir coverage --timeout 300 -- --test-threads=1
```

覆盖率报告会在 CI 中自动生成并上传到 Codecov。

## 开发工具

### Makefile

项目提供了 Makefile 简化常见任务：

```bash
make help           # 显示所有可用命令
make test           # 运行测试
make fmt            # 格式化代码
make fmt-check      # 检查代码格式
make clippy         # 运行 clippy
make check          # 运行所有检查（fmt + clippy + test）
make coverage       # 生成覆盖率报告
make clean          # 清理构建产物
```

### Pre-commit Hooks

使用 pre-commit 自动在提交前运行检查：

```bash
# 安装 pre-commit（需要 Python）
pip install pre-commit

# 安装 hooks
make install-hooks
# 或
pre-commit install

# 手动运行所有 hooks
make pre-commit
# 或
pre-commit run --all-files
```

## CI/CD

GitHub Actions CI 会自动运行：

1. **格式检查** (`cargo fmt --check`)
2. **Clippy** (`cargo clippy --all-targets -- -D warnings`)
3. **测试** (`cargo test`)
4. **覆盖率** (`cargo tarpaulin`) - 仅 Linux

## 测试最佳实践

1. **单元测试**：测试纯函数和独立逻辑
2. **集成测试**：测试模块间的交互
3. **使用 `assert!` 宏**：提供清晰的错误消息
4. **测试边界条件**：空值、错误输入、极限值
5. **保持测试独立**：每个测试应该独立运行
6. **使用 `tempfile`**：创建临时文件进行文件系统测试

## 当前测试覆盖

- **总测试数**: 43 个
- **核心模块**:
  - `RequestTracker`: 9 个测试
  - `AudioCache`: 9 个测试
  - `DTO 转换`: 15 个测试
  - Reducer 各模块: 10 个测试

## 贡献测试

添加新功能时，请确保：

1. 为公共 API 添加单元测试
2. 为复杂的业务逻辑添加测试
3. 更新此文档说明新增的测试
4. 确保所有测试通过后再提交
