FROM mcr.microsoft.com/devcontainers/base:ubuntu

ENV DEBIAN_FRONTEND=noninteractive

SHELL ["/bin/bash", "-lc"]

# Base system dependencies + C toolchain (for arc_solver.c)
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    git-lfs \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    liblapack-dev \
    libblas-dev \
    python3 \
    python3-venv \
    python3-pip \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Common CLI tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    ripgrep \
    jq \
    gh \
    tree \
    tmux \
    trash-cli \
    && rm -rf /var/lib/apt/lists/*

# VS Code tunnel CLI
RUN set -eux; \
    tmpdir="$(mktemp -d)"; \
    curl -fsSL "https://update.code.visualstudio.com/latest/cli-linux-x64/stable" -o "$tmpdir/vscode-cli.tar.gz"; \
    tar -xzf "$tmpdir/vscode-cli.tar.gz" -C "$tmpdir"; \
    install -m 0755 "$tmpdir/code" /usr/local/bin/code-tunnel; \
    rm -rf "$tmpdir"

# Node.js 22.x
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/* \
    && node -v && npm -v

# uv (Python packaging)
ENV UV_INSTALL_DIR=/usr/local
ENV UV_VERSION=0.5.14
RUN curl -LsSf https://astral.sh/uv/${UV_VERSION}/install.sh | sh

# Switch to dev user for language toolchains
USER vscode
ENV HOME=/home/vscode
ENV PATH="$HOME/.cargo/bin:$HOME/.local/bin:/usr/local:$PATH"

# Rust toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable \
    && "$HOME/.cargo/bin/rustup" component add rustfmt clippy rust-analyzer || true

# Cargo tools
RUN curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C "$HOME/.cargo/bin" \
    && "$HOME/.cargo/bin/cargo" install cargo-watch

WORKDIR /workspaces/transversal-arc-solver
