# Minimal Dockerfile to build and run the ocodex Rust binary in this repo.
# It builds the crate in ocodex/ and sets it as entrypoint.

FROM rust:1.89 as builder
# Build dependencies (OpenSSL vendored build, MUSL toolchain for static binary)
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        perl \
        musl-tools \
        ca-certificates \
        git \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add aarch64-unknown-linux-musl
WORKDIR /src
# Build using the contents of this directory (the Docker build context
# is the "ocodex/" folder), then compile the Rust workspace under codex-rs.
COPY . /src/ocodex
WORKDIR /src/ocodex/codex-rs
# Build the workspace binary named "ocodex" for MUSL (static linking)
RUN cargo build --release --locked --bin ocodex --target aarch64-unknown-linux-musl
#RUN cargo build --release --manifest-path ocodex/codex-rs/Cargo.toml --bin ocodex

FROM alpine:3.20
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
    openssh \
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
USER app
WORKDIR /work
# Copy the compiled static binary from the MUSL target directory
COPY --from=builder /src/ocodex/codex-rs/target/aarch64-unknown-linux-musl/release/ocodex /usr/local/bin/ocodex
ENV SHELL=/bin/bash
ENTRYPOINT ["/usr/local/bin/ocodex"]
