#!/usr/bin/env bash
set -euo pipefail

# Launch a VS Code tunnel from the host into the devcontainer.

if [[ ${1:-} == "--help" || ${1:-} == "-h" ]]; then
  cat <<'EOF'
Usage: .devcontainer/host-vscode-tunnel.sh

Launch a VS Code tunnel from the host into the devcontainer.
Ensures the devcontainer is running, then starts the tunnel.

Requires:
  - devcontainer CLI (npm i -g @devcontainers/cli)
  - code-tunnel binary in the container (baked into image)

Environment:
  CODE_TUNNEL_BIN  Path to code-tunnel binary (default: /usr/local/bin/code-tunnel)
EOF
  exit 0
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v devcontainer >/dev/null 2>&1; then
  echo "devcontainer CLI not found. Install with 'npm i -g @devcontainers/cli' or via VS Code Dev Containers extension." >&2
  exit 1
fi

TUNNEL_NAME="transversal-arc-solver"
CODE_TUNNEL_BIN="${CODE_TUNNEL_BIN:-/usr/local/bin/code-tunnel}"
CONFIG_FILE="${REPO_ROOT}/.devcontainer/devcontainer.json"

# Ensure the devcontainer is running
devcontainer up --workspace-folder "${REPO_ROOT}" --config "${CONFIG_FILE}" >/dev/null

devcontainer exec --workspace-folder "${REPO_ROOT}" --config "${CONFIG_FILE}" -- "${CODE_TUNNEL_BIN}" tunnel --accept-server-license-terms --name "${TUNNEL_NAME}"
