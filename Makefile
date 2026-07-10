# ============================================================
# Forja Makefile
# ============================================================
.POSIX:

# ── Android targets ─────────────────────────────────────────────────────────
ANDROID_TARGETS = aarch64-linux-android x86_64-linux-android armv7-linux-androideabi i686-linux-android

# Ensure toolchain script is executable
SCRIPTS_DIR := scripts
TOOLCHAIN_SCRIPT := $(SCRIPTS_DIR)/toolchain-android.sh
BUILD_SCRIPT := $(SCRIPTS_DIR)/build-android.sh

.PHONY: all android-all android-arm64 android-x86_64 android-armv7 android-x86 setup-android bump-patch bump-minor bump-major

# ── Build all Android targets ───────────────────────────────────────────────
android-all:
	@bash $(BUILD_SCRIPT)

# ── Individual targets ─────────────────────────────────────────────────────
android-arm64:
	@bash $(BUILD_SCRIPT) aarch64-linux-android

android-x86_64:
	@bash $(BUILD_SCRIPT) x86_64-linux-android

android-armv7:
	@bash $(BUILD_SCRIPT) armv7-linux-androideabi

android-x86:
	@bash $(BUILD_SCRIPT) i686-linux-android

# ── Validate NDK setup without building ────────────────────────────────────
setup-android:
	@echo "=== Checking Android NDK setup ==="
	@bash $(TOOLCHAIN_SCRIPT) check

# ── Version Bumping ────────────────────────────────────────────────────────
bump-patch:
	@python -m bumpversion patch
	@git push origin main --tags

bump-minor:
	@python -m bumpversion minor
	@git push origin main --tags

bump-major:
	@python -m bumpversion major
	@git push origin main --tags
