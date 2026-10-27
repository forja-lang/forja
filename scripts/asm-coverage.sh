#!/usr/bin/env bash
# scripts/asm-coverage.sh
#
# F4-T6 — Mide la cobertura de `src/compiler_asm.rs` (backend ASM multi-target)
# ejecutando la batería por target (`tests/asm_targets.rs`) y los tests unitarios
# del módulo (`--lib compiler_asm`), con `-C instrument-coverage`.
#
# Requiere el componente `llvm-tools-preview`:
#   rustup component add llvm-tools-preview
#
# Uso:
#   scripts/asm-coverage.sh                 # verifica >= 80% (defecto)
#   ASM_COVERAGE_THRESHOLD=85 scripts/asm-coverage.sh
#
# Exit 1 si la cobertura de líneas es inferior al umbral.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

THRESHOLD="${ASM_COVERAGE_THRESHOLD:-80}"
CARGO="${CARGO:-cargo}"
TARGET_DIR="${ASM_COV_TARGET_DIR:-target/cov-asm}"

# En Windows/WSL los ejecutables llevan .exe y hay que propagar variables vía WSLENV.
EXE_SUFFIX=""
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) EXE_SUFFIX=".exe" ;;
esac
if [ -n "${WSL_DISTRO_NAME:-}" ]; then
    # En WSL normalmente se usa el toolchain de Windows (cargo.exe → binarios .exe).
    EXE_SUFFIX=".exe"
    export WSLENV="${WSLENV:-}:CARGO_TARGET_DIR:RUSTFLAGS:LLVM_PROFILE_FILE"
fi

find_tool() {
    local name="$1"
    if command -v "$name" >/dev/null 2>&1; then
        command -v "$name"
        return 0
    fi
    # Buscar en RUSTUP_HOME, ~/.rustup y (WSL) los perfiles de Windows.
    local candidatos=(
        "${RUSTUP_HOME:-$HOME/.rustup}"
        "$HOME/.rustup"
    )
    for glob in /mnt/c/Users/*/.rustup; do
        [ -d "$glob" ] && candidatos+=("$glob")
    done
    for base in "${candidatos[@]}"; do
        [ -d "$base/toolchains" ] || continue
        local encontrado
        encontrado="$(find "$base/toolchains" -name "${name}${EXE_SUFFIX}" 2>/dev/null | head -1 || true)"
        if [ -n "$encontrado" ]; then
            echo "$encontrado"
            return 0
        fi
    done
    return 0
}

PROFDATA="${LLVM_PROFDATA:-$(find_tool llvm-profdata)}"
COV="${LLVM_COV:-$(find_tool llvm-cov)}"
if [ -z "$PROFDATA" ] || [ -z "$COV" ]; then
    echo "❌ Faltan llvm-profdata/llvm-cov. Instalá: rustup component add llvm-tools-preview" >&2
    exit 2
fi

COVDIR="$TARGET_DIR/cov"
rm -rf "$COVDIR"
mkdir -p "$COVDIR"

echo "== F4-T6 · Cobertura del backend ASM ==" >&2
build() {
    CARGO_TARGET_DIR="$TARGET_DIR" RUSTFLAGS="-C instrument-coverage" "$CARGO" "$@"
}
build test --locked --test asm_targets --no-run >/dev/null
build test --locked --lib compiler_asm --no-run >/dev/null

LIBEXE="$(ls "$TARGET_DIR"/debug/deps/forja-*"$EXE_SUFFIX" 2>/dev/null | head -1 || true)"
ASMEXE="$(ls "$TARGET_DIR"/debug/deps/asm_targets-*"$EXE_SUFFIX" 2>/dev/null | head -1 || true)"
if [ -z "$LIBEXE" ] || [ -z "$ASMEXE" ]; then
    echo "❌ No se encontraron los binarios de test instrumentados" >&2
    exit 2
fi

LLVM_PROFILE_FILE="$COVDIR/lib-%p-%m.profraw" "$LIBEXE" compiler_asm >/dev/null 2>&1 || true
LLVM_PROFILE_FILE="$COVDIR/asm-%p-%m.profraw" "$ASMEXE" >/dev/null 2>&1 || true

"$PROFDATA" merge -sparse "$COVDIR"/*.profraw -o "$COVDIR/asm.profdata"
"$COV" report "$LIBEXE" -object "$ASMEXE" -instr-profile="$COVDIR/asm.profdata" > "$COVDIR/report.txt"

LINEA="$(grep -E 'compiler_asm\.rs' "$COVDIR/report.txt" | head -1)"
if [ -z "$LINEA" ]; then
    echo "❌ No se encontró compiler_asm.rs en el reporte" >&2
    cat "$COVDIR/report.txt" >&2
    exit 2
fi

# Columnas: ... Lines  Missed Lines  Cover ... → el cover de líneas es el campo 10.
PCT="$(echo "$LINEA" | awk '{print $10}' | tr -d '%')"
echo "$LINEA"
echo ""
echo "Cobertura de líneas de src/compiler_asm.rs: ${PCT}% (umbral ${THRESHOLD}%)" >&2
echo "Reporte completo: $COVDIR/report.txt" >&2

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    {
        echo "## 🧵 Cobertura backend ASM (F4-T6)"
        echo ""
        echo "\`\`\`"
        echo "$LINEA"
        echo "\`\`\`"
        echo ""
        echo "- Líneas: **${PCT}%** (umbral ${THRESHOLD}%)"
    } >> "$GITHUB_STEP_SUMMARY"
fi

awk -v pct="$PCT" -v thr="$THRESHOLD" 'BEGIN { exit (pct + 0 >= thr + 0) ? 0 : 1 }' || {
    echo "❌ Cobertura ${PCT}% < ${THRESHOLD}%" >&2
    exit 1
}
echo "✅ Cobertura ${PCT}% ≥ ${THRESHOLD}%" >&2
