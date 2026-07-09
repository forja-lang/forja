#!/usr/bin/env bash
# ============================================================
# build-android.sh — Build Forja for Android (one-command)
#
# Detects NDK, installs missing rustup targets, and builds.
# Usage:
#   bash scripts/build-android.sh [target] [profile] [features]
#
# Examples:
#   bash scripts/build-android.sh                    # all targets, release
#   bash scripts/build-android.sh aarch64-linux-android
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ── 1. Ensure required rustup targets are installed ─────────────────────
ANDROID_TARGETS=(aarch64-linux-android x86_64-linux-android armv7-linux-androideabi i686-linux-android)

echo "=== Checking Rust Android targets ==="
INSTALLED_TARGETS="$(rustup target list --installed 2>/dev/null || true)"
for target in "${ANDROID_TARGETS[@]}"; do
    if echo "$INSTALLED_TARGETS" | grep -q "$target"; then
        echo "  [✓] $target"
    else
        echo "  [ ] $target — installing..."
        rustup target add "$target"
    fi
done
echo ""

# ── 2. Source toolchain (detects NDK, exports CARGO_TARGET_*_LINKER) ────
source "$SCRIPT_DIR/toolchain-android.sh"

# ── 3. Parse arguments and build ─────────────────────────────────────────
TARGET="${1:-all}"
PROFILE="${2:-release}"
FEATURES="${3:-gui}"

build_target() {
    local target="$1"
    echo ""
    echo "=========================================="
    echo "  Building for ${target}"
    echo "  Profile:   ${PROFILE}"
    echo "  Features:  ${FEATURES}"
    echo "=========================================="
    cargo build --target "$target" --features "$FEATURES" --profile "$PROFILE"
}

if [ "$TARGET" = "all" ]; then
    for target in "${ANDROID_TARGETS[@]}"; do
        build_target "$target"
    done
else
    build_target "$TARGET"
fi

echo ""
echo "=== Android build(s) complete ==="
