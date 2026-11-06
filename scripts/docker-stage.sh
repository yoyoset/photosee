#!/usr/bin/env bash
# Stages the Docker build context in target/docker (see docker/Dockerfile):
# a static musl build of photosee-server plus the built web UI.
#   bash scripts/docker-stage.sh
#   then, in target/docker:
#     docker build --build-arg VERSION=$(cat VERSION) -t photosee:local -t photosee:$(cat VERSION) .
# VERSION is the product version from Cargo.toml, the same number the
# installer and the portable zip carry (scripts/gates.mjs checks).
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
sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml > "$out/VERSION"
echo "version $(cat "$out/VERSION")"
ls -la "$out"
