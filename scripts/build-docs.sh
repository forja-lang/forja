#!/usr/bin/env bash
# scripts/build-docs.sh — build reproducible del sitio Astro (F10-T3).
#
# En Windows el paquete `esbuild` puede haber sido instalado en otra plataforma;
# `npm rebuild esbuild` descarga el binario correcto.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/docs"

if [ "${FORJA_DOCS_SKIP_INSTALL:-0}" != "1" ]; then
    # `npm ci` garantiza los binarios opcionales correctos para la plataforma
    # (esbuild/rollup/sharp); copiar node_modules entre SO no es portable.
    npm ci --no-audit --no-fund
fi

npm run build
echo "✅ Documentación compilada en docs/dist/"
