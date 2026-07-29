#!/bin/bash

# 패키지 루트의 rust 폴더에서 실행해야 합니다.
echo "Rust Desktop (Linux & Windows) Start..."

# ---------------------------------------------------------
# 1. Linux 빌드 (.so)
# ---------------------------------------------------------
echo "Building Linux shared library (.so)..."
cargo build --release

# 플러그인의 linux 폴더 내에 shared 디렉토리 생성 및 복사
LINUX_SHARED_DIR="../linux/shared"
mkdir -p $LINUX_SHARED_DIR
cp target/release/libpoint_glass_opengl_core.so $LINUX_SHARED_DIR/
echo "Linux Build Done: $LINUX_SHARED_DIR/libpoint_glass_opengl_core.so"


# ---------------------------------------------------------
# 2. Windows 빌드 (.dll) - MinGW 크로스 컴파일
# ---------------------------------------------------------
echo "Building Windows shared library (.dll) (MinGW)..."
cargo build --target x86_64-pc-windows-gnu --release

# 플러그인의 windows 폴더 내에 shared 디렉토리 생성 및 복사
WINDOWS_SHARED_DIR="../windows/shared"
mkdir -p $WINDOWS_SHARED_DIR
cp target/x86_64-pc-windows-gnu/release/point_glass_opengl_core.dll $WINDOWS_SHARED_DIR/
echo "Windows Build Done: $WINDOWS_SHARED_DIR/point_glass_opengl_core.dll"


echo "Desktop (Linux, Windows) Build Done!"