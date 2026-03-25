#!/usr/bin/env bash
set -euo pipefail

# Ensure cargo/rustc are on PATH for this session
if [ -n "${CLAUDE_ENV_FILE:-}" ] && [ -d "$HOME/.cargo/bin" ]; then
  echo "export PATH=\"\$HOME/.cargo/bin:\$PATH\"" >> "$CLAUDE_ENV_FILE"
fi

# Install Rust if not present
if ! command -v rustc &>/dev/null && ! [ -x "$HOME/.cargo/bin/rustc" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
    echo "export PATH=\"\$HOME/.cargo/bin:\$PATH\"" >> "$CLAUDE_ENV_FILE"
  fi
fi

exit 0
