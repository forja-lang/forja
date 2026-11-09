#!/usr/bin/env bash
# scripts/probar-stdlib.sh — tests de comportamiento de la biblioteca estándar.
#
# Uso:
#   bash scripts/probar-stdlib.sh              # todos los tests de library/std/tests
#   bash scripts/probar-stdlib.sh colecciones  # sólo los que coincidan
#
# Por qué existe: `tests/stdlib/stdlib_smoke.rs` sólo verifica que cada módulo
# IMPORTE. Eso dejó pasar durante meses que `std/servidor_web` llamara a
# `claves_mapa`, una función inexistente: el servidor HTTP no arrancaba y ningún
# test lo detectaba.
#
# Estos tests corren con la VM (`forja run`), no con `forja test`, porque
# `forja test` transpila a Rust y los módulos de la stdlib usan nativos.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# FORJA=/ruta/al/binario permite reusar uno ya compilado (lo que hace el CI).
# Sin esa variable se recompila: usar un binario viejo da falsos negativos.
FORJA="${FORJA:-}"
if [ -z "$FORJA" ]; then
    echo "▶ compilando forja-cli…"
    cargo build -q -p forja-cli --bin forja || exit 1
    FORJA="target/debug/forja"
fi

FILTRO="${1:-}"
total=0
fallidos=0

for archivo in library/std/tests/*.fa; do
    nombre="$(basename "$archivo" .fa)"
    if [ "$nombre" = "arnes" ]; then
        continue
    fi
    if [ -n "$FILTRO" ] && [[ "$nombre" != *"$FILTRO"* ]]; then
        continue
    fi
    total=$((total + 1))
    salida="$("$FORJA" run "$archivo" 2>&1)"
    estado=$?
    if [ $estado -ne 0 ]; then
        fallidos=$((fallidos + 1))
        echo "❌ $nombre (exit $estado)"
        echo "$salida" | sed 's/^/   /'
    else
        resumen="$(echo "$salida" | tail -n 1 | sed 's/^ *//')"
        echo "✅ $nombre — $resumen"
    fi
done

if [ $total -eq 0 ]; then
    echo "No se encontraron tests en library/std/tests/"
    exit 2
fi

echo
if [ $fallidos -gt 0 ]; then
    echo "Fallaron $fallidos de $total módulos"
    exit 1
fi
echo "Todos los módulos de la stdlib pasaron ($total archivos)"
