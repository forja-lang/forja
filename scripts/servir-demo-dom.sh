#!/usr/bin/env bash
# scripts/servir-demo-dom.sh — compila el wasm (si hace falta) y sirve la demo.
#
#   bash scripts/servir-demo-dom.sh [puerto]     # por defecto 8099
#
# Abrí http://127.0.0.1:8099/ en el navegador.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PUERTO="${1:-8099}"
cd "$ROOT"

PAQUETE="demo-dom/pkg/forja_wasm_gui.js"
if [ ! -f "$PAQUETE" ]; then
    echo "▶ compilando gui-dom/wasm-gui a wasm (una sola vez, tarda unos minutos)"
    (cd library/wasm-gui && RUSTC_WRAPPER= wasm-pack build --target web --release --no-opt) || {
        echo "❌ falló wasm-pack (¿está instalado? cargo install wasm-pack)"
        exit 1
    }
    mkdir -p demo-dom/pkg
    cp library/wasm-gui/pkg/forja_wasm_gui.js \
       library/wasm-gui/pkg/forja_wasm_gui_bg.wasm \
       library/wasm-gui/pkg/forja_wasm_gui_bg.wasm.d.ts \
       demo-dom/pkg/ 2>/dev/null
fi

if ! command -v node >/dev/null 2>&1; then
    echo "❌ hace falta node para servir la demo"
    exit 1
fi

exec node demo-dom/servidor.mjs "$PUERTO"
