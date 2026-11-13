#!/usr/bin/env bash
# scripts/construir-demo-dom.sh — compila el wasm de la demo y lo optimiza.
#
#   bash scripts/construir-demo-dom.sh
#
# Pasos: wasm-pack (release, sin optimizar) → wasm-opt -Oz → demo-dom/pkg/.
# `wasm-opt` sale de binaryen: si no está, se busca en un `node_modules` local
# (`npm i binaryen`) o en el PATH; sin él la demo funciona igual, sólo pesa más.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "▶ compilando wasm-gui (wasm-pack)"
(cd library/wasm-gui && RUSTC_WRAPPER= wasm-pack build --target web --release --no-opt)

OPT="$(command -v wasm-opt || true)"
if [ -z "$OPT" ]; then
    for candidato in \
        "$ROOT/node_modules/binaryen/bin/wasm-opt" \
        "/tmp/bin/node_modules/binaryen/bin/wasm-opt"; do
        if [ -x "$candidato" ]; then OPT="$candidato"; break; fi
    done
fi

mkdir -p demo-dom/pkg
cp library/wasm-gui/pkg/forja_wasm_gui.js demo-dom/pkg/

if [ -n "$OPT" ]; then
    echo "▶ optimizando con wasm-opt -Oz"
    #  Flags de las features modernas que emite LLVM (ver scripts/build-wasm.ps1).
    "$OPT" --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        -Oz library/wasm-gui/pkg/forja_wasm_gui_bg.wasm \
        -o demo-dom/pkg/forja_wasm_gui_bg.wasm
else
    echo "· sin wasm-opt (npm i binaryen): se usa el wasm sin optimizar"
    cp library/wasm-gui/pkg/forja_wasm_gui_bg.wasm demo-dom/pkg/
fi

echo "✅ demo lista en demo-dom/pkg ($(wc -c < demo-dom/pkg/forja_wasm_gui_bg.wasm) bytes)"
echo "   servila con: bash scripts/servir-demo-dom.sh 8099"
