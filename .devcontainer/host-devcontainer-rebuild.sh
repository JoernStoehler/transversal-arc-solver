#!/usr/bin/env bash
set -euo pipefail

# Rebuild the devcontainer image and recreate the container from the host.

if [[ ${1:-} == "--help" || ${1:-} == "-h" ]]; then
  cat <<'EOF'
Usage: .devcontainer/host-devcontainer-rebuild.sh [devcontainer build args...]

Rebuild the devcontainer image and recreate the container.
Run from the host machine (not inside the container).

Requires:
  - devcontainer CLI (npm i -g @devcontainers/cli)
EOF
  exit 0
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG_FILE="${REPO_ROOT}/.devcontainer/devcontainer.json"

if ! command -v devcontainer >/dev/null 2>&1; then
  echo "[host-devcontainer-rebuild] devcontainer CLI not found. Install with 'npm i -g @devcontainers/cli' or via VS Code Dev Containers extension." >&2
  exit 1
fi

echo "[host-devcontainer-rebuild] Building devcontainer image for ${REPO_ROOT}..."
devcontainer build --workspace-folder "${REPO_ROOT}" --config "${CONFIG_FILE}" "$@"

echo "[host-devcontainer-rebuild] Starting devcontainer (replacing existing container if present)..."
devcontainer up --workspace-folder "${REPO_ROOT}" --config "${CONFIG_FILE}" --remove-existing-container

echo "[host-devcontainer-rebuild] Devcontainer rebuild complete."
