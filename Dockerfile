## Minimal Dockerfile to build and run the ocodex Rust binary in this repo.
## Stage 1: Build static MUSL binary matching TARGETPLATFORM, and stage toolpack.
FROM --platform=$BUILDPLATFORM rust:1.89-slim-bookworm AS builder
ARG TARGETPLATFORM
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        perl \
        musl-tools \
        ca-certificates \
        git \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    case "${TARGETPLATFORM}" in \
      "linux/amd64")  RT=x86_64-unknown-linux-musl  ;; \
      "linux/arm64")  RT=aarch64-unknown-linux-musl ;; \
      *)               RT=x86_64-unknown-linux-musl  ;; \
    esac; \
    echo "$RT" > /tmp/RUST_TARGET; \
    rustup target add "$RT"
WORKDIR /src
# Build using the contents of this directory (build context is the "ocodex/" folder)
COPY . /src/ocodex
WORKDIR /src/ocodex/codex-rs
RUN set -eux; \
    RT="$(cat /tmp/RUST_TARGET)"; \
    cargo build --release --locked --bin ocodex --target "$RT"; \
    install -D -m 0755 \
      "/src/ocodex/codex-rs/target/${RT}/release/ocodex" \
      "/out/ocodex"

## Stage 2: Runtime image
FROM alpine:3.20
ARG TARGETPLATFORM
# Install a modern baseline of tools commonly needed by the agent
RUN apk add --no-cache \
    bash \
    ca-certificates \
    coreutils \
    curl \
    wget \
    jq \
    ripgrep \
    fd \
    git \
    #openssh \
    tzdata \
    python3 \
    py3-pip \
    nodejs \
    npm \
    make \
    cmake \
    build-base \
    pkgconf \
    libc6-compat \
 && update-ca-certificates \
 && adduser -D app

# Prepare a built-in toolpack so ocodex inside Docker has MCP servers by default.
# This avoids requiring a project-local ocodex/.codex mount; orchestrator may override CODEX_HOME.
RUN mkdir -p /usr/local/share/ocodex/.codex
# Copy the entire toolpack directory from the repo so new tools are available automatically.
COPY --from=0 /src/ocodex/.codex/ /usr/local/share/ocodex/.codex/

# Copy the compiled static binary from the builder's stable output path
COPY --from=0 /out/ocodex /usr/local/bin/ocodex

# Default environment and working directory
ENV SHELL=/bin/bash \
    CODEX_HOME=/usr/local/share/ocodex/.codex
WORKDIR /work

# Ensure the non-root user owns binaries and config
RUN chown -R app:app /usr/local/bin/ocodex /usr/local/share/ocodex
USER app

# Run the installed binary (copied to /usr/local/bin/ocodex in this image)
ENTRYPOINT ["/usr/local/bin/ocodex"]
