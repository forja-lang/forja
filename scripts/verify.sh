#!/usr/bin/env bash
# scripts/verify.sh — verificación reproducible (F7-T1)
#
# Reproduce localmente lo que corre el CI: formato, clippy, compilación y tests.
# Uso:
#   bash scripts/verify.sh            # suite completa
#   FORJA_VERIFY_SKIP_CLIPPY=1 bash scripts/verify.sh
#   FORJA_VERIFY_SKIP_FMT=1 bash scripts/verify.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# En WSL/Git Bash el toolchain puede ser el de Windows (`cargo.exe`).
CARGO_BIN="${CARGO:-cargo}"
if ! command -v "$CARGO_BIN" >/dev/null 2>&1; then
    if command -v cargo.exe >/dev/null 2>&1; then
        CARGO_BIN="cargo.exe"
    else
        for candidato in /mnt/c/Users/*/.cargo/bin/cargo.exe "$HOME/.cargo/bin/cargo"; do
            if [ -x "$candidato" ]; then
                CARGO_BIN="$candidato"
                break
            fi
        done
    fi
fi
if ! command -v "$CARGO_BIN" >/dev/null 2>&1 && [ ! -x "$CARGO_BIN" ]; then
    echo "❌ No se encontró cargo (ni cargo.exe)" >&2
    exit 127
fi

echo "== Forja · verificación reproducible =="
echo "   directorio: $ROOT"
echo "   cargo:      $CARGO_BIN"

if [ "${FORJA_VERIFY_SKIP_FMT:-0}" != "1" ]; then
    echo "==> cargo fmt --check"
    "$CARGO_BIN" fmt --all -- --check
fi

if [ "${FORJA_VERIFY_SKIP_CLIPPY:-0}" != "1" ]; then
    echo "==> cargo clippy (workspace)"
    "$CARGO_BIN" clippy --workspace --all-targets --locked
fi

echo "==> cargo check --workspace --locked"
"$CARGO_BIN" check --workspace --locked

echo "==> cargo test --workspace --locked"
"$CARGO_BIN" test --workspace --locked

echo "==> métricas de documentación"
if command -v python >/dev/null 2>&1; then
    PYTHON=python
elif command -v python3 >/dev/null 2>&1; then
    PYTHON=python3
else
    echo "⚠️  Python no encontrado; se omite scripts/metricas.py --check" >&2
    PYTHON=""
fi
if [ -n "$PYTHON" ]; then
    "$PYTHON" scripts/metricas.py --check
fi

echo "✅ Verificación completa (exit 0)"
