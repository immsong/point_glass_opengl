#!/bin/bash
set -euo pipefail

# 패키지 루트의 rust 폴더에서 실행해야 합니다.
echo "Rust Desktop (Linux & Windows) Start..."

# ---------------------------------------------------------
# 1. Linux x86_64 빌드 (.so)
# ---------------------------------------------------------
echo "Building Linux x86_64 shared library (.so)..."

cargo build --release --target x86_64-unknown-linux-gnu

LINUX_X86_64_SHARED_DIR="../linux/shared/x86_64"

mkdir -p "$LINUX_X86_64_SHARED_DIR"

cp target/x86_64-unknown-linux-gnu/release/libpoint_glass_opengl_core.so \
    "$LINUX_X86_64_SHARED_DIR/"

echo "Linux x86_64 Build Done: $LINUX_X86_64_SHARED_DIR/libpoint_glass_opengl_core.so"

# ---------------------------------------------------------
# 2. Linux aarch64 빌드 (.so) - ARM64 크로스 컴파일
# ---------------------------------------------------------
echo "Building Linux aarch64 shared library (.so)..."

CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    cargo build --release --target aarch64-unknown-linux-gnu

LINUX_AARCH64_SHARED_DIR="../linux/shared/aarch64"

mkdir -p "$LINUX_AARCH64_SHARED_DIR"

cp target/aarch64-unknown-linux-gnu/release/libpoint_glass_opengl_core.so \
    "$LINUX_AARCH64_SHARED_DIR/"

echo "Linux aarch64 Build Done: $LINUX_AARCH64_SHARED_DIR/libpoint_glass_opengl_core.so"

# ---------------------------------------------------------
# 3. Windows x86_64 빌드 (.dll) - MinGW 크로스 컴파일
# ---------------------------------------------------------
echo "Building Windows shared library (.dll) (MinGW)..."

cargo build --release --target x86_64-pc-windows-gnu

WINDOWS_SHARED_DIR="../windows/shared"

mkdir -p "$WINDOWS_SHARED_DIR"

cp target/x86_64-pc-windows-gnu/release/point_glass_opengl_core.dll \
    "$WINDOWS_SHARED_DIR/"

echo "Windows Build Done: $WINDOWS_SHARED_DIR/point_glass_opengl_core.dll"

echo "Desktop (Linux, Windows) Build Done!"
