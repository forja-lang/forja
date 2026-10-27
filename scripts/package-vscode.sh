#!/usr/bin/env bash
# scripts/package-vscode.sh — empaqueta la extensión VSCode (F6-T9).
#
# Requiere Node.js 20+ y `npx @vscode/vsce`. El .vsix resultante se publica con:
#   npx @vscode/vsce publish --packagePath dist/forja-syntax-<version>.vsix
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXT="$ROOT/vscode/forja-syntax"
DIST="$ROOT/dist"
mkdir -p "$DIST"

cd "$EXT"
if [ ! -d node_modules ]; then
    npm ci
fi
npx tsc -p .
VERSION=$(node -p "require('./package.json').version")
npx @vscode/vsce package --out "$DIST/forja-syntax-$VERSION.vsix"
echo "✅ Extensión empaquetada: $DIST/forja-syntax-$VERSION.vsix"
