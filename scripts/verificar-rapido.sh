#!/usr/bin/env bash
# scripts/verificar-rapido.sh — verificación acotada a las crates que tocaste.
#
# Uso:
#   bash scripts/verificar-rapido.sh forja-repl forja-cli
#   bash scripts/verificar-rapido.sh --todo   # workspace completo (lento: decenas de minutos)
#
# Por qué existe: `cargo test -p forja` y `cargo check --workspace --all-targets` compilan
# los ~60 binarios de test del compilador, el corpus, los goldens y las crates de GUI.
# Para un renombre de locales eso es tiempo perdido: alcanza con la lib de cada crate.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CARGO_BIN="${CARGO:-cargo}"
command -v "$CARGO_BIN" >/dev/null 2>&1 || CARGO_BIN="cargo.exe"

if [ "${1:-}" = "--todo" ]; then
    echo "▶ workspace completo (--all-targets): puede tardar decenas de minutos"
    "$CARGO_BIN" fmt --all -- --check
    "$CARGO_BIN" check --workspace --all-targets
    exit 0
fi

CRATES=("$@")
if [ ${#CRATES[@]} -eq 0 ]; then
    echo "Uso: bash scripts/verificar-rapido.sh <crate> [crate...] | --todo" >&2
    exit 2
fi

for crate in "${CRATES[@]}"; do
    echo "▶ $crate"
    "$CARGO_BIN" fmt -p "$crate" -- --check
    # Los binarios (cli/lsp/dap) no tienen lib: se chequean como binario.
    if ! "$CARGO_BIN" check -p "$crate" --lib --quiet 2>/dev/null; then
        "$CARGO_BIN" check -p "$crate" --bins --quiet
        continue
    fi
    "$CARGO_BIN" test -p "$crate" --lib --quiet
done

echo "✅ listo"
