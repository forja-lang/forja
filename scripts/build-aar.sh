#!/usr/bin/env bash
# ============================================================
# build-aar.sh — Compila forja-android-rt como un paquete .aar
#
# Compila libforja_android_rt.so para los targets Android y lo
# empaqueta con la API Kotlin en un .aar.
#
# Uso:
#   bash scripts/build-aar.sh                          # todos los ABIs + AAR
#   bash scripts/build-aar.sh 0.9.5                    # versión específica
#   bash scripts/build-aar.sh --target aarch64-linux-android --build-only
#   bash scripts/build-aar.sh --package-only           # empaqueta .so ya compilados
#
# Opciones:
#   --target <triple>   Compila/empaqueta solo ese target Android.
#   --build-only        Compila los .so pero no genera el .aar.
#   --package-only      No compila: empaqueta los .so de dist/jni/<abi>/.
#   -h, --help          Muestra esta ayuda.
#
# Variables de entorno:
#   FORJA_AAR_PROFILE   Perfil de Cargo (default: dist-android).
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATE_DIR="$PROJECT_DIR/crates/forja-android-rt"

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

PROFILE="${FORJA_AAR_PROFILE:-dist-android}"
BUILD_DIR="$PROJECT_DIR/target"
OUTPUT_DIR="$PROJECT_DIR/dist"
JNI_DIR="$OUTPUT_DIR/jni"

VERSION=""
ONLY_TARGET=""
BUILD_ONLY=0
PACKAGE_ONLY=0

usage() {
    cat <<'EOF'
Uso:
  bash scripts/build-aar.sh                          # todos los ABIs + AAR
  bash scripts/build-aar.sh 0.9.5                    # versión específica
  bash scripts/build-aar.sh --target aarch64-linux-android --build-only
  bash scripts/build-aar.sh --package-only           # empaqueta .so ya compilados

Opciones:
  --target <triple>   Compila/empaqueta solo ese target Android.
  --build-only        Compila los .so pero no genera el .aar.
  --package-only      No compila: empaqueta los .so de dist/jni/<abi>/.
  -h, --help          Muestra esta ayuda.

Variables de entorno:
  FORJA_AAR_PROFILE   Perfil de Cargo (default: dist-android).
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --target)
            if [ $# -lt 2 ]; then
                echo "❌ Falta el valor de --target" >&2
                exit 1
            fi
            ONLY_TARGET="$2"
            shift 2
            ;;
        --build-only)
            BUILD_ONLY=1
            shift
            ;;
        --package-only)
            PACKAGE_ONLY=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -*)
            echo "❌ Opción desconocida: $1" >&2
            usage >&2
            exit 1
            ;;
        *)
            VERSION="$1"
            shift
            ;;
    esac
done

if [ "$BUILD_ONLY" -eq 1 ] && [ "$PACKAGE_ONLY" -eq 1 ]; then
    echo "❌ --build-only y --package-only son mutuamente excluyentes" >&2
    exit 1
fi

if [ -n "$ONLY_TARGET" ]; then
    TARGET_VALIDO=0
    for target in "${TARGETS[@]}"; do
        if [ "$target" = "$ONLY_TARGET" ]; then
            TARGET_VALIDO=1
        fi
    done
    if [ "$TARGET_VALIDO" -eq 0 ]; then
        echo "❌ Target no soportado: $ONLY_TARGET" >&2
        echo "   Válidos: ${TARGETS[*]}" >&2
        exit 1
    fi
    TARGETS=("$ONLY_TARGET")
fi

# Si no se pasa versión como argumento, la lee automáticamente de Cargo.toml
if [ -z "$VERSION" ]; then
    VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | cut -d'"' -f2)
    if [ -z "$VERSION" ]; then
        echo "❌ Error: No se pudo detectar versión desde Cargo.toml"
        echo "   Especificala manualmente: $0 <version>"
        exit 1
    fi
fi
AAR_NAME="forja-android-rt-${VERSION}.aar"

# ── Empaquetado ───────────────────────────────────────────────────
# Requiere que los .so estén en dist/jni/<abi>/libforja_android_rt.so.
package_aar() {
    echo ""
    echo "=== Packaging Kotlin API ==="
    AAR_STAGING=$(mktemp -d)
    trap 'rm -rf "$AAR_STAGING"' EXIT
    mkdir -p "$AAR_STAGING/jni"

    for target in "${TARGETS[@]}"; do
        abi="${ABI_MAP[$target]}"
        so="$JNI_DIR/$abi/libforja_android_rt.so"
        if [ ! -f "$so" ]; then
            echo "❌ Falta $so" >&2
            echo "   Compilalo con: bash scripts/build-aar.sh --target $target --build-only" >&2
            exit 1
        fi
        mkdir -p "$AAR_STAGING/jni/$abi"
        cp "$so" "$AAR_STAGING/jni/$abi/"
    done

    KOTLIN_DIR="$AAR_STAGING/kotlin/com/forja"
    mkdir -p "$KOTLIN_DIR"
    cp "$CRATE_DIR/android/src/com/forja/"*.kt "$KOTLIN_DIR/"

    echo "=== Creating AndroidManifest.xml ==="
    cp "$CRATE_DIR/android/AndroidManifest.xml" "$AAR_STAGING/AndroidManifest.xml"

    # R.txt vacío: todavía no hay recursos Android.
    touch "$AAR_STAGING/R.txt"

    echo ""
    echo "=== Packaging $AAR_NAME ==="
    mkdir -p "$OUTPUT_DIR"
    (cd "$AAR_STAGING" && zip -r "$OUTPUT_DIR/$AAR_NAME" . -x ".*" > /dev/null)

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
}

if [ "$PACKAGE_ONLY" -eq 1 ]; then
    package_aar
    exit 0
fi

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
echo ""
echo "=== Building forja-android-rt (profile: $PROFILE) ==="
mkdir -p "$JNI_DIR"

for target in "${TARGETS[@]}"; do
    abi="${ABI_MAP[$target]}"
    echo ""
    echo "  Building for $target → $abi"

    # F0-T4: el AAR es un artefacto de distribución → perfil de tamaño
    # (opt-level = "z", panic = "abort", strip = "symbols").
    # F7-T6: `dist-android` usa ThinLTO para no pagar LTO fat en cada ABI.
    cargo build \
        --package forja-android-rt \
        --target "$target" \
        --profile "$PROFILE" \
        --locked \
        --manifest-path "$CRATE_DIR/Cargo.toml"

    ABI_DIR="$JNI_DIR/$abi"
    rm -rf "$ABI_DIR"
    mkdir -p "$ABI_DIR"
    cp "$BUILD_DIR/$target/$PROFILE/libforja_android_rt.so" "$ABI_DIR/"
    echo "  → libforja_android_rt.so ($abi)"
done

if [ "$BUILD_ONLY" -eq 1 ]; then
    echo ""
    echo "✅ .so compilados en $JNI_DIR"
    exit 0
fi

# ── 4-7. Package Kotlin sources, manifest, R.txt and .aar ─────────
package_aar
