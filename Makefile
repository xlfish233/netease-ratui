.PHONY: all test fmt clippy run clean coverage help

# 默认目标
all: test

# 运行所有测试
test:
	@echo "Running tests..."
	cargo test

# 格式化代码
fmt:
	@echo "Formatting code..."
	cargo fmt

# 检查代码格式
fmt-check:
	@echo "Checking code format..."
	cargo fmt --check

# 运行 clippy
clippy:
	@echo "Running clippy..."
	cargo clippy --all-targets -- -D warnings

# 运行应用
run:
	@echo "Running application..."
	cargo run

# 清理构建产物
clean:
	@echo "Cleaning..."
	cargo clean

# 运行覆盖率检查
coverage:
	@echo "Running coverage..."
	cargo tarpaulin --out Html --timeout 300 -- --test-threads=1

# 运行所有检查（CI 相同的流程）
check: fmt-check clippy test
	@echo "All checks passed!"

# 安装 pre-commit hooks
install-hooks:
	@echo "Installing pre-commit hooks..."
	pre-commit install

# 运行 pre-commit hooks
pre-commit:
	@echo "Running pre-commit hooks..."
	pre-commit run --all-files

# 显示帮助
help:
	@echo "Available targets:"
	@echo "  make test         - Run tests"
	@echo "  make fmt          - Format code"
	@echo "  make fmt-check    - Check code format"
	@echo "  make clippy       - Run clippy"
	@echo "  make run          - Run application"
	@echo "  make clean        - Clean build artifacts"
	@echo "  make coverage     - Run coverage check"
	@echo "  make check        - Run all checks (fmt + clippy + test)"
	@echo "  make install-hooks - Install pre-commit hooks"
	@echo "  make pre-commit   - Run pre-commit hooks"
	@echo "  make help         - Show this help message"
