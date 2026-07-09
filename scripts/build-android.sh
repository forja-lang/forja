#!/usr/bin/env bash
# ============================================================
# build-android.sh — Build Forja for Android targets
#
# A convenience script that detects the NDK and builds one
# or all Android targets in a single invocation.
#
# Usage:
#   bash scripts/build-android.sh                          # all targets, release
#   bash scripts/build-android.sh aarch64-linux-android    # single target
#   bash scripts/build-android.sh all debug gui            # all, debug profile
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

# Parse arguments
TARGET="${1:-all}"
PROFILE="${2:-release}"
FEATURES="${3:-gui}"

# Source the toolchain (exports CARGO_TARGET_*_LINKER to the current shell)
source "$SCRIPT_DIR/toolchain-android.sh"

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
    for target in aarch64-linux-android x86_64-linux-android armv7-linux-androideabi i686-linux-android; do
        build_target "$target"
    done
else
    build_target "$TARGET"
fi

echo ""
echo "=== Android build(s) complete ==="
