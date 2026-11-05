#!/usr/bin/env bash
# Stages the Docker build context in target/docker (see docker/Dockerfile):
# a static musl build of photosee-server plus the built web UI.
#   bash scripts/docker-stage.sh
#   then: docker build -t photosee:<ver> target/docker
# Cross-compiles from Windows with no C toolchain: every runtime dependency
# is pure Rust, and rust-lld links the self-contained musl CRT. Needs
# `rustup target add x86_64-unknown-linux-musl` once.
set -euo pipefail
cd "$(dirname "$0")/.."
target=x86_64-unknown-linux-musl
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
    cargo build --release -p photosee-server --target "$target"
(cd web && npx vite build --logLevel warn)
out=target/docker
rm -rf "$out"
mkdir -p "$out"
cp "target/$target/release/photosee-server" "$out/"
cp -r web/dist "$out/web"
cp docker/Dockerfile "$out/"
ls -la "$out"
