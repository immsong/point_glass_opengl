#!/usr/bin/env bash

set -euo pipefail

LIB_NAME="point_glass_opengl_core"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="${SCRIPT_DIR}/target/release"
OUTPUT_DIR="${SCRIPT_DIR}/../linux/shared"

SHARED_LIB="lib${LIB_NAME}.so"

cd "${SCRIPT_DIR}"
cargo build --release

mkdir -p "${OUTPUT_DIR}"

copy_artifact() {
    local filename="$1"
    local source="${TARGET_DIR}/${filename}"

    if [[ ! -f "${source}" ]]; then
        echo "[ERROR] Build artifact not found: ${source}"
        exit 1
    fi

    cp -f "${source}" "${OUTPUT_DIR}/${filename}"
    echo "[INFO] Copied ${filename}"
}

copy_artifact "${SHARED_LIB}"

echo "[INFO] Build completed:"
echo "       ${OUTPUT_DIR}"