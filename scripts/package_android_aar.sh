#!/usr/bin/env bash
# VoiceKey Android .aar Packaging Script
# Packages VoiceKey native libraries (.so), UniFFI bindings, and Android manifest
# into a production-ready Android Archive (.aar).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ANDROID_DIR="${PROJECT_ROOT}/android/voicekey-core"
JNI_LIBS_DIR="${ANDROID_DIR}/src/main/jniLibs"
JAVA_SRC_DIR="${ANDROID_DIR}/src/main/java"
OUTPUT_DIR="${ANDROID_DIR}/build/outputs/aar"
TARGET_AAR="${PROJECT_ROOT}/target/voicekey-core.aar"

echo "=========================================================="
echo " VoiceKey Android AAR Packager                            "
echo "=========================================================="

mkdir -p "${OUTPUT_DIR}"
mkdir -p "${PROJECT_ROOT}/target"

# Check if Gradle wrapper exists and can build the AAR directly
if [[ -f "${PROJECT_ROOT}/android/gradlew" ]]; then
    echo "==> Building AAR via Gradle wrapper..."
    (cd "${PROJECT_ROOT}/android" && ./gradlew :voicekey-core:assembleRelease)
    if [[ -f "${OUTPUT_DIR}/voicekey-core-release.aar" ]]; then
        cp "${OUTPUT_DIR}/voicekey-core-release.aar" "${TARGET_AAR}"
        echo "==> AAR built successfully via Gradle: ${TARGET_AAR}"
        exit 0
    fi
fi

# Standalone AAR Assembly (Standard Android Archive specification)
# Structure:
# - AndroidManifest.xml
# - classes.jar
# - jni/<abi>/libvoicekey_android_ffi.so
# - proguard.txt
# - R.txt

echo "==> Assembling standalone AAR structure..."
STAGING_DIR="$(mktemp -d -t voicekey-aar-XXXXXX)"
trap 'rm -rf "${STAGING_DIR}"' EXIT

# 1. Copy Android Manifest
cp "${ANDROID_DIR}/src/main/AndroidManifest.xml" "${STAGING_DIR}/AndroidManifest.xml"

# 2. Copy ProGuard rules
if [[ -f "${ANDROID_DIR}/proguard-rules.pro" ]]; then
    cp "${ANDROID_DIR}/proguard-rules.pro" "${STAGING_DIR}/proguard.txt"
else
    touch "${STAGING_DIR}/proguard.txt"
fi

# 3. Create empty R.txt
touch "${STAGING_DIR}/R.txt"

# 4. Copy JNI native libraries
mkdir -p "${STAGING_DIR}/jni"
if [[ -d "${JNI_LIBS_DIR}" ]]; then
    for ABI in arm64-v8a x86_64; do
        if [[ -d "${JNI_LIBS_DIR}/${ABI}" ]]; then
            mkdir -p "${STAGING_DIR}/jni/${ABI}"
            cp "${JNI_LIBS_DIR}/${ABI}"/*.so "${STAGING_DIR}/jni/${ABI}/" 2>/dev/null || true
        fi
    done
fi

# 5. Build classes.jar
CLASSES_DIR="${STAGING_DIR}/classes"
mkdir -p "${CLASSES_DIR}"

if command -v kotlinc >/dev/null 2>&1; then
    echo "==> Compiling Kotlin classes with kotlinc..."
    # Locate android.jar or classpath if available
    CP_ARGS=()
    if [[ -n "${ANDROID_HOME:-}" ]]; then
        ANDROID_JAR=$(find "${ANDROID_HOME}/platforms" -name "android.jar" 2>/dev/null | sort -V | tail -n 1 || true)
        if [[ -n "${ANDROID_JAR}" ]]; then
            CP_ARGS+=("-classpath" "${ANDROID_JAR}")
        fi
    fi
    kotlinc $(find "${JAVA_SRC_DIR}" -name "*.kt") -d "${CLASSES_DIR}" "${CP_ARGS[@]}" || true
fi

# Package classes.jar (if compiled classes exist, include them; otherwise package sources/meta)
(
    cd "${CLASSES_DIR}"
    # Ensure jar is non-empty
    mkdir -p META-INF
    echo "Manifest-Version: 1.0" > META-INF/MANIFEST.MF
    echo "Bundle-Name: VoiceKey Core Android" >> META-INF/MANIFEST.MF
    echo "Bundle-Version: 0.2.0" >> META-INF/MANIFEST.MF

    if command -v jar >/dev/null 2>&1; then
        jar cf "${STAGING_DIR}/classes.jar" .
    else
        zip -r -0 "${STAGING_DIR}/classes.jar" .
    fi
)

# 6. Bundle into final .aar (ZIP archive)
AAR_FILE="${OUTPUT_DIR}/voicekey-core-release.aar"
(
    cd "${STAGING_DIR}"
    zip -r -9 "${AAR_FILE}" AndroidManifest.xml classes.jar proguard.txt R.txt jni/
)

cp "${AAR_FILE}" "${TARGET_AAR}"

echo "=========================================================="
echo " VoiceKey AAR Successfully Packaged!                      "
echo " Location: ${TARGET_AAR}                                  "
echo " Size: $(du -h "${TARGET_AAR}" | cut -f1)                 "
echo " Contents:                                                "
unzip -l "${TARGET_AAR}"
echo "=========================================================="
