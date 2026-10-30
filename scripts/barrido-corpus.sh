#!/usr/bin/env bash
# Barrido del corpus (F12-T3): clasifica cada ejemplo como OK / NO_COMPILA / CUELGA.
# Usa el binario release ya compilado (no toca el árbol de trabajo).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
FORJA=./target/release/forja.exe
salida=/tmp/corpus.tsv
: > "$salida"
compilan=0; diag=0; cuelgan=0; total=0
for f in ejemplos/*.fa; do
    total=$((total + 1))
    out=$(timeout 6 "$FORJA" ejecutar "$f" 2>&1)
    estado=$?
    if [ $estado -eq 124 ]; then
        clase=CUELGA; cuelgan=$((cuelgan + 1))
    elif printf '%s' "$out" | grep -qE "● .*(Ortografía|Gramática|Tipos|Semántica|Módulos|FFI|Contrato|Préstamo)"; then
        clase=NO_COMPILA; diag=$((diag + 1))
    else
        clase=OK; compilan=$((compilan + 1))
    fi
    printf '%s\t%s\n' "$clase" "$(basename "$f")" >> "$salida"
done
echo "total=$total ok=$compilan no_compila=$diag cuelgan=$cuelgan"
echo "--- detalle (no compilan / cuelgan):"
grep -v "^OK" "$salida" | sort
