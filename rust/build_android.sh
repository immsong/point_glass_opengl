#!/bin/bash

# 패키지 루트의 rust 폴더에서 실행해야 합니다.
echo "Rust Android Build Start..."

# 빌드 결과물이 들어갈 jniLibs 폴더 생성
JNI_DIR="../android/src/main/jniLibs"
mkdir -p $JNI_DIR

# cargo-ndk를 사용하여 각 아키텍처별로 빌드하고 jniLibs 폴더로 자동 복사
cargo ndk -t arm64-v8a -o $JNI_DIR build --release
cargo ndk -t armeabi-v7a -o $JNI_DIR build --release
cargo ndk -t x86_64 -o $JNI_DIR build --release

echo "Complete android build and copy .so files to jniLibs folder."