#!/usr/bin/env bash
# VoiceKey Android Native Cross-Compilation & UniFFI Bindings Generator
# Builds shared native libraries (.so) for arm64-v8a and x86_64 using cargo-ndk,
# and generates Kotlin bindings using uniffi-bindgen.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CRATE_DIR="${PROJECT_ROOT}/crates/voicekey-android-ffi"
ANDROID_DIR="${PROJECT_ROOT}/android/voicekey-core"
JNI_LIBS_DIR="${ANDROID_DIR}/src/main/jniLibs"
JAVA_SRC_DIR="${ANDROID_DIR}/src/main/java"
BINDINGS_DIR="${CRATE_DIR}/bindings/kotlin"

echo "=========================================================="
echo " VoiceKey Android Native Cross-Compilation (cargo-ndk)    "
echo "=========================================================="

# 1. Resolve Android NDK Home
if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
    POSSIBLE_NDK_PATHS=(
        "${HOME}/.android/android-ndk-r26d"
        "${HOME}/Android/Sdk/ndk/26.3.11579264"
        "${ANDROID_HOME:-/nonexistent}/ndk/26.3.11579264"
        "${ANDROID_SDK_ROOT:-/nonexistent}/ndk/26.3.11579264"
    )
    for p in "${POSSIBLE_NDK_PATHS[@]}"; do
        if [[ -d "$p" ]]; then
            export ANDROID_NDK_HOME="$p"
            echo "==> Auto-detected ANDROID_NDK_HOME: ${ANDROID_NDK_HOME}"
            break
        fi
    done
fi

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
    # Check if any ndk folder exists in Android SDK
    if [[ -d "${ANDROID_HOME:-}/ndk" ]]; then
        LATEST_NDK=$(ls -1d "${ANDROID_HOME}/ndk"/* 2>/dev/null | sort -V | tail -n 1 || true)
        if [[ -n "${LATEST_NDK}" && -d "${LATEST_NDK}" ]]; then
            export ANDROID_NDK_HOME="${LATEST_NDK}"
            echo "==> Using latest SDK NDK: ${ANDROID_NDK_HOME}"
        fi
    fi
fi

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
    echo "ERROR: ANDROID_NDK_HOME is not set and no NDK was found in standard locations."
    echo "Please set ANDROID_NDK_HOME=/path/to/android-ndk or install NDK via Android SDK."
    exit 1
fi

# 2. Check cargo-ndk
if ! command -v cargo-ndk >/dev/null 2>&1; then
    echo "==> cargo-ndk not found, installing cargo-ndk..."
    cargo install cargo-ndk
fi

# 3. Ensure rust targets are added
echo "==> Checking required rustup targets..."
rustup target add aarch64-linux-android x86_64-linux-android 2>/dev/null || true

# 4. Create output directories
mkdir -p "${JNI_LIBS_DIR}/arm64-v8a"
mkdir -p "${JNI_LIBS_DIR}/x86_64"
mkdir -p "${JAVA_SRC_DIR}"
mkdir -p "${BINDINGS_DIR}"

# 5. Build for Android targets using cargo-ndk
BUILD_TARGETS=("${@:-arm64-v8a x86_64}")
echo "==> Building native libraries for targets: ${BUILD_TARGETS[*]}..."

export ANDROID_NDK="${ANDROID_NDK_HOME}"
export CMAKE_ANDROID_NDK="${ANDROID_NDK_HOME}"
export CMAKE_MAKE_PROGRAM="$(which make 2>/dev/null || echo /usr/bin/make)"

for TARGET in ${BUILD_TARGETS[*]}; do
    echo "==> Compiling for ${TARGET} (API 29)..."
    if [[ "${TARGET}" == "arm64-v8a" ]]; then
        export CMAKE_TOOLCHAIN_FILE="${SCRIPT_DIR}/cmake/android-arm64-toolchain.cmake"
        export CMAKE_ANDROID_ARCH_ABI="arm64-v8a"
    elif [[ "${TARGET}" == "x86_64" ]]; then
        export CMAKE_TOOLCHAIN_FILE="${SCRIPT_DIR}/cmake/android-x86_64-toolchain.cmake"
        export CMAKE_ANDROID_ARCH_ABI="x86_64"
    fi

    cargo ndk \
        -t "${TARGET}" \
        -P 29 \
        -o "${JNI_LIBS_DIR}" \
        build --release \
        -p voicekey-android-ffi
done

echo "==> Native libraries successfully compiled into ${JNI_LIBS_DIR}:"
find "${JNI_LIBS_DIR}" -name "*.so"

# 6. Generate Kotlin bindings using uniffi-bindgen
echo "==> Building host uniffi-bindgen CLI..."
cargo build -p voicekey-android-ffi --features cli --bin uniffi-bindgen

# Build host cdylib for uniffi-bindgen introspection
cargo build -p voicekey-android-ffi

HOST_LIB="${PROJECT_ROOT}/target/debug/libvoicekey_android_ffi.so"
if [[ ! -f "${HOST_LIB}" ]]; then
    HOST_LIB="${PROJECT_ROOT}/target/release/libvoicekey_android_ffi.so"
fi

echo "==> Generating Kotlin UniFFI bindings from ${HOST_LIB}..."
cargo run -p voicekey-android-ffi --features cli --bin uniffi-bindgen -- \
    generate \
    --library "${HOST_LIB}" \
    --language kotlin \
    --out-dir "${JAVA_SRC_DIR}"

# Also mirror into crate bindings directory for tracking
mkdir -p "${BINDINGS_DIR}"
cp -r "${JAVA_SRC_DIR}"/* "${BINDINGS_DIR}/"

echo "=========================================================="
echo " Android native build & UniFFI generation complete!       "
echo " Output jniLibs: ${JNI_LIBS_DIR}                          "
echo " Output Kotlin:  ${JAVA_SRC_DIR}                          "
echo "=========================================================="
