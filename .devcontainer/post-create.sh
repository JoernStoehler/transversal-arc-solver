#!/usr/bin/env bash
# Local devcontainer post-create setup.

set -euo pipefail

echo "[post-create] Local devcontainer post-create..."

# Ensure user directories exist
sudo mkdir -p \
  "${HOME}/.config" \
  "${HOME}/.local" \
  "${HOME}/.cache"
sudo chown -R "${USER}:${USER}" \
  "${HOME}/.config" \
  "${HOME}/.local" \
  "${HOME}/.cache"

# Configure npm paths and install global packages
if command -v npm >/dev/null 2>&1; then
  mkdir -p "${HOME}/.local/bin" "${HOME}/.cache/npm"
  npm config set prefix "${HOME}/.local"
  npm config set cache "${HOME}/.cache/npm"
  npm install -g pyright
fi

# Configure git credentials via GitHub CLI
if command -v gh >/dev/null 2>&1; then
  gh auth setup-git || true
fi

# Install Claude Code CLI
curl -fsSL https://claude.ai/install.sh | bash

# Clone ARC-AGI dataset if not present
if [ ! -d "data/ARC-AGI" ]; then
  echo "[post-create] Cloning ARC-AGI dataset..."
  git clone https://github.com/fchollet/ARC-AGI data/ARC-AGI
fi

# Compile upstream arc_solver if not present
if [ ! -f "arc_solver" ] && [ -f "arc_solver.c" ]; then
  echo "[post-create] Compiling arc_solver.c..."
  cc -O3 -march=native -D'__CLPK_integer=int' -o arc_solver arc_solver.c -lm -llapack -lblas
fi

# tmux config for Claude Code TUI compatibility
cat > ~/.tmux.conf << 'TMUXCONF'
set -g mouse on
set -g status off
set -g set-titles on
set -g set-titles-string "[#S] #{pane_title}"
set -g @scroll-down-exit-copy-mode off

set -g allow-passthrough on
set -sg escape-time 0
set -g extended-keys always
set -as terminal-features 'xterm*:extkeys'
set -as terminal-features 'xterm-kitty:extkeys'
set -g set-clipboard on
set -g history-limit 250000
set -g focus-events on
set -g default-terminal "tmux-256color"
set -ag terminal-overrides ",xterm-256color:RGB"

set -g bell-action any
set -g visual-bell on
set -g monitor-bell on

set -g mode-style "bg=#a8d1ff,fg=#000000"
TMUXCONF

# Safe delete wrapper
if ! grep -q 'trash-put' ~/.bashrc 2>/dev/null; then
  cat >> ~/.bashrc << 'BASHRC'

# Safe delete: redirect rm to trash-put (use /bin/rm for real deletes)
rm() { trash-put "$@"; }
export -f rm
BASHRC
fi

echo "[post-create] Verifying tools..."
echo "[post-create] code-tunnel: $(code-tunnel --version 2>/dev/null || echo 'not found')"
echo "[post-create] rustc: $(rustc --version 2>/dev/null || echo 'not found')"
echo "[post-create] cargo: $(cargo --version 2>/dev/null || echo 'not found')"
echo "[post-create] cc: $(cc --version 2>/dev/null | head -1 || echo 'not found')"
[ -f arc_solver ] && echo "[post-create] arc_solver: compiled" || echo "[post-create] WARNING: arc_solver not compiled"

echo "[post-create] Local post-create complete."
