#!/bin/bash
set -euo pipefail

# Idempotent environment setup for worker sessions
cd /home/xl/play/netease-ratui

# Ensure dependencies are built
cargo check 2>/dev/null || true
