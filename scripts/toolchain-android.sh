#!/usr/bin/env bash
# ============================================================
# toolchain-android.sh — Cross-platform Android NDK detection
#
# Detects the Android NDK on Linux, macOS, and Windows (Git Bash)
# and exports CARGO_TARGET_*_LINKER variables for cross-compilation.
#
# Usage:
#   source scripts/toolchain-android.sh        # Export vars, then build
#   bash scripts/toolchain-android.sh check    # Validate NDK only
#   bash scripts/toolchain-android.sh          # Same as 'check'
# ============================================================
set -euo pipefail

# ── Detect if being sourced (export mode) or executed (check mode) ──────────
_IS_SOURCED=false
if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
    _IS_SOURCED=true
fi

# ── Helper: exit / return with error ────────────────────────────────────────
_die() {
    echo "ERROR: $*" >&2
    if $_IS_SOURCED; then
        # With set -e this will exit the sourcing shell — intentional fail-fast
        return 1
    fi
    exit 1
}

# ── Helper: version-aware sort (Linux / macOS compatible) ──────────────────
# Reads paths from stdin, sorts them by the last path component (the version
# number, e.g. 27.0.12077973), and prints them in ascending order.
_semver_sort() {
    if sort -V </dev/null 2>/dev/null; then
        # GNU sort with --version-sort available (Linux, macOS coreutils)
        sort -V
    else
        # Fallback for native macOS sort (no -V flag).
        # Prepend the basename so we sort by the version string, then strip it.
        awk -F/ '{ print $NF, $0 }' | sort -t. -k1,1n -k2,2n -k3,3n | cut -d' ' -f2-
    fi
}

# ── Helper: detect the NDK host tag ─────────────────────────────────────────
_detect_host_tag() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os" in
        Linux)
            echo "linux-${arch}"
            ;;
        Darwin)
            echo "darwin-${arch}"
            ;;
        CYGWIN*|MINGW*|MSYS*)
            echo "windows-x86_64"
            ;;
        *)
            _die "Unsupported operating system: $os"
            ;;
    esac
}

# ── NDK detection (order of precedence) ────────────────────────────────────
_detect_ndk() {
    local ndk_home="${ANDROID_NDK_HOME:-}"

    # 1. ANDROID_NDK_HOME
    if [[ -n "$ndk_home" ]]; then
        if [[ -d "$ndk_home" ]]; then
            echo "$ndk_home"
            return 0
        else
            echo "WARNING: ANDROID_NDK_HOME is set but does not exist: $ndk_home" >&2
        fi
    fi

    local search_paths=()
    local home="${HOME:-}"

    # 2. ANDROID_HOME/ndk/
    if [[ -n "${ANDROID_HOME:-}" ]]; then
        search_paths+=("${ANDROID_HOME}/ndk")
    fi

    # 3. Linux default: ~/Android/Sdk/ndk/
    if [[ -n "$home" ]]; then
        search_paths+=("${home}/Android/Sdk/ndk")
    fi

    # 4. macOS default: ~/Library/Android/Sdk/ndk/
    if [[ -n "$home" ]]; then
        search_paths+=("${home}/Library/Android/Sdk/ndk")
    fi

    # 5. Windows (Git Bash / MSYS2): %LOCALAPPDATA%/Android/Sdk/ndk/
    if [[ "$(uname -s)" =~ ^(CYGWIN|MINGW|MSYS) ]]; then
        local local_appdata="${LOCALAPPDATA:-}"
        if [[ -n "$local_appdata" ]]; then
            # Convert Windows backslashes to forward slashes
            local_appdata="${local_appdata//\\//}"
            search_paths+=("${local_appdata}/Android/Sdk/ndk")
        fi
    fi

    # Search each path for NDK versions
    for path in "${search_paths[@]}"; do
        if [[ -d "$path" ]]; then
            local versions
            versions="$(find "$path" -maxdepth 1 -type d -name '[0-9]*' 2>/dev/null || true)"
            if [[ -n "$versions" ]]; then
                local highest
                highest="$(echo "$versions" | _semver_sort | tail -1)"
                if [[ -n "$highest" ]]; then
                    echo "$highest"
                    return 0
                fi
            fi
        fi
    done

    return 1
}

# ── Linker definitions ─────────────────────────────────────────────────────
# Maps Rust target → NDK linker name
declare -A _LINKER_MAP
_LINKER_MAP[aarch64-linux-android]="aarch64-linux-android23-clang"
_LINKER_MAP[x86_64-linux-android]="x86_64-linux-android23-clang"
_LINKER_MAP[armv7-linux-androideabi]="armv7a-linux-androideabi23-clang"
_LINKER_MAP[i686-linux-android]="i686-linux-android23-clang"

# ── Main ────────────────────────────────────────────────────────────────────
_main() {
    # Locate NDK
    local NDK_ROOT
    NDK_ROOT="$(_detect_ndk)" || _die "\
No Android NDK found.

Install the NDK via one of these methods:
  1. Android Studio → SDK Manager → SDK Tools → NDK (side-by-side)
  2. sdkmanager \"ndk;27.0.12077973\"
  3. Set ANDROID_NDK_HOME to your NDK installation path

Searched locations (in order):
  - \$ANDROID_NDK_HOME                    (if set)
  - \$ANDROID_HOME/ndk/                   (if set)
  - \$HOME/Android/Sdk/ndk/               (Linux)
  - \$HOME/Library/Android/Sdk/ndk/       (macOS)
  - \$LOCALAPPDATA/Android/Sdk/ndk/       (Windows / Git Bash)"

    # Detect host platform tag
    local HOST_TAG
    HOST_TAG="$(_detect_host_tag)"
    local LLVM_BIN="${NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}/bin"

    if [[ ! -d "$LLVM_BIN" ]]; then
        _die "LLVM toolchain not found at: ${LLVM_BIN}"$'\n'"\
Your NDK may be incomplete or corrupted. Reinstall the NDK."
    fi

    # Determine linker extension for Windows (Git Bash / MSYS2 / Cygwin)
    local _LINKER_EXT=""
    if [[ "$(uname -s)" =~ ^(CYGWIN|MINGW|MSYS) ]]; then
        if [[ -f "${LLVM_BIN}/aarch64-linux-android23-clang.cmd" ]]; then
            _LINKER_EXT=".cmd"
        fi
    fi

    # ── Validate all linkers exist ─────────────────────────────────────────────
    echo "=========================================="
    echo "  Android NDK Toolchain"
    echo "  NDK Root:  ${NDK_ROOT}"
    echo "  LLVM Bin:  ${LLVM_BIN}"
    echo "  Host:      ${HOST_TAG}"
    echo "=========================================="

    local _ALL_OK=true
    for target in "${!_LINKER_MAP[@]}"; do
        local linker_name="${_LINKER_MAP[$target]}${_LINKER_EXT}"
        local linker_path="${LLVM_BIN}/${linker_name}"

        if [[ -x "$linker_path" ]] || [[ -f "$linker_path" ]]; then
            echo "  [✓] ${target} → ${linker_path}"
        else
            echo "  [✗] ${target} → MISSING: ${linker_path}" >&2
            _ALL_OK=false
        fi
    done
    echo ""

    if ! $_ALL_OK; then
        _die "One or more NDK linkers are missing."$'\n'"\
Your NDK may be incomplete or corrupted. Reinstall the NDK (r25+ recommended)."
    fi

    # ── Export environment variables (only when sourced) ────────────────────────
    if $_IS_SOURCED; then
        echo "Exporting CARGO_TARGET_*_LINKER variables..."

        export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${LLVM_BIN}/${_LINKER_MAP[aarch64-linux-android]}${_LINKER_EXT}"
        export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,--no-undefined"

        export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="${LLVM_BIN}/${_LINKER_MAP[x86_64-linux-android]}${_LINKER_EXT}"
        export CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,--no-undefined"

        export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="${LLVM_BIN}/${_LINKER_MAP[armv7-linux-androideabi]}${_LINKER_EXT}"
        export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_RUSTFLAGS="-C link-arg=-Wl,--no-undefined"

        export CARGO_TARGET_I686_LINUX_ANDROID_LINKER="${LLVM_BIN}/${_LINKER_MAP[i686-linux-android]}${_LINKER_EXT}"
        export CARGO_TARGET_I686_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,--no-undefined"

        echo "Done. Android toolchain is ready."
        echo "Run: cargo build --target <target> --features gui --release"
    else
        echo "NDK validation passed."
        echo "Run: source scripts/toolchain-android.sh  (to set up the environment)"
    fi
}

# Execute main
_main
