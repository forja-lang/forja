# ============================================================
# Forja Makefile
# ============================================================
.POSIX:

# ── Android targets ─────────────────────────────────────────────────────────
ANDROID_TARGETS = aarch64-linux-android x86_64-linux-android armv7-linux-androideabi i686-linux-android

# Ensure toolchain script is executable
SCRIPTS_DIR := scripts
TOOLCHAIN_SCRIPT := $(SCRIPTS_DIR)/toolchain-android.sh

.PHONY: all android-all android-arm64 android-x86_64 android-armv7 android-x86 setup-android

# ── Build all Android targets ───────────────────────────────────────────────
android-all: $(ANDROID_TARGETS)

# ── Individual targets ─────────────────────────────────────────────────────
# Each target calls `source toolchain-android.sh && cargo build` in a single
# shell invocation so the exported environment variables persist for cargo.
$(ANDROID_TARGETS):
	@echo "=== Building for $@ ==="
	. $(TOOLCHAIN_SCRIPT) && cargo build --target $@ --features gui --release

# ── Convenience aliases ────────────────────────────────────────────────────
android-arm64: aarch64-linux-android

android-x86_64: x86_64-linux-android

android-armv7: armv7-linux-androideabi

android-x86: i686-linux-android

# ── Validate NDK setup without building ────────────────────────────────────
setup-android:
	@echo "=== Checking Android NDK setup ==="
	@bash $(TOOLCHAIN_SCRIPT) check
