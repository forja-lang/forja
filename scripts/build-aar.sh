#!/usr/bin/env bash
# ============================================================
# build-aar.sh — Build forja-android-rt as an .aar package
#
# Compiles libforja_android_rt.so for all Android targets
# and packages it with the Kotlin API into an .aar file.
#
# Usage:
#   bash scripts/build-aar.sh [version]
#
# Examples:
#   bash scripts/build-aar.sh                  # default version
#   bash scripts/build-aar.sh 0.8.7            # specific version
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATE_DIR="$PROJECT_DIR/crates/forja-android-rt"

VERSION="${1:-0.8.2}"
AAR_NAME="forja-android-rt-${VERSION}.aar"
OUTPUT_DIR="$PROJECT_DIR/dist"

# Android targets
TARGETS=(
    "aarch64-linux-android"     # ARM64 (most modern devices)
    "x86_64-linux-android"      # x86_64 (emulator)
    "armv7-linux-androideabi"   # ARM32 (older devices)
    "i686-linux-android"        # x86 (older emulator)
)

# ABI mapping
declare -A ABI_MAP
ABI_MAP[aarch64-linux-android]="arm64-v8a"
ABI_MAP[x86_64-linux-android]="x86_64"
ABI_MAP[armv7-linux-androideabi]="armeabi-v7a"
ABI_MAP[i686-linux-android]="x86"

# ── 1. Source Android NDK toolchain ───────────────────────────────
echo "=== Setting up Android NDK toolchain ==="
source "$SCRIPT_DIR/toolchain-android.sh"

# ── 2. Install missing Rust targets ───────────────────────────────
echo "=== Checking Rust Android targets ==="
INSTALLED=$(rustup target list --installed 2>/dev/null || true)
for target in "${TARGETS[@]}"; do
    if echo "$INSTALLED" | grep -q "$target"; then
        echo "  [✓] $target"
    else
        echo "  [ ] $target — installing..."
        rustup target add "$target"
    fi
done

# ── 3. Build .so for each target ──────────────────────────────────
BUILD_DIR="$PROJECT_DIR/target"
AAR_STAGING=$(mktemp -d)
JNI_DIR="$AAR_STAGING/jni"

echo ""
echo "=== Building forja-android-rt for all targets ==="

for target in "${TARGETS[@]}"; do
    abi="${ABI_MAP[$target]}"
    echo ""
    echo "  Building for $target → $abi"

    cargo build \
        --package forja-android-rt \
        --target "$target" \
        --release \
        --manifest-path "$CRATE_DIR/Cargo.toml"

    # Copy .so to AAR staging
    ABI_DIR="$JNI_DIR/$abi"
    mkdir -p "$ABI_DIR"
    cp "$BUILD_DIR/$target/release/libforja_android_rt.so" "$ABI_DIR/"
    echo "  → libforja_android_rt.so ($abi)"
done

# ── 4. Package Kotlin sources ─────────────────────────────────────
echo ""
echo "=== Packaging Kotlin API ==="
KOTLIN_DIR="$AAR_STAGING/kotlin/com/forja"
mkdir -p "$KOTLIN_DIR"
cp "$CRATE_DIR/android/src/com/forja/"*.kt "$KOTLIN_DIR/"

# ── 5. Create AndroidManifest.xml ─────────────────────────────────
echo "=== Creating AndroidManifest.xml ==="
cp "$CRATE_DIR/android/AndroidManifest.xml" "$AAR_STAGING/AndroidManifest.xml"

# ── 6. Create R.txt (empty, no resources yet) ────────────────────
touch "$AAR_STAGING/R.txt"

# ── 7. Package .aar ──────────────────────────────────────────────
echo ""
echo "=== Packaging $AAR_NAME ==="
mkdir -p "$OUTPUT_DIR"
cd "$AAR_STAGING"
zip -r "$OUTPUT_DIR/$AAR_NAME" . -x ".*" > /dev/null 2>&1
cd "$PROJECT_DIR"

# ── 8. Cleanup ───────────────────────────────────────────────────
rm -rf "$AAR_STAGING"

echo ""
echo "=========================================="
echo "  ✅ $AAR_NAME"
echo "  📍 $OUTPUT_DIR/$AAR_NAME"
echo "=========================================="
echo ""
echo "To use in Android Studio:"
echo "  1. Copy $AAR_NAME to your app/libs/"
echo "  2. In build.gradle.kts:"
echo '     implementation(files("libs/'"$AAR_NAME"'"))'
echo ""
echo "Or publish to a Maven repository:"
echo "  mvn install:install-file -Dfile=$OUTPUT_DIR/$AAR_NAME \\"
echo "    -DgroupId=com.forja -DartifactId=forja-android-rt \\"
echo "    -Dversion=$VERSION -Dpackaging=aar"
