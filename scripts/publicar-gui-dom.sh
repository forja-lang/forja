#!/usr/bin/env bash
# scripts/publicar-gui-dom.sh — publica `forja-gui-dom` en GitHub y lo convierte
# en submódulo del repo raíz (como el resto de `library/*`).
#
# Contexto: el crate se escribió dentro de `library/wasm-gui/gui-dom` (que ya es
# submódulo) para poder trabajar sin remoto. Este script lo promueve a su propio
# repositorio y lo registra como submódulo de Forja en `library/gui-dom`,
# dejando las rutas (`Cargo.toml`, dependencia de wasm-gui) actualizadas.
#
# Requisitos: `gh` autenticado (`gh auth status`) con permiso en la organización.
#
# Uso:
#   bash scripts/publicar-gui-dom.sh                 # forja-lang/forja-gui-dom
#   ORG=mi-org NOMBRE=mi-crate bash scripts/publicar-gui-dom.sh
#
# Es idempotente: si ya existe el submódulo, no hace nada.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ORG="${ORG:-forja-lang}"
NOMBRE="${NOMBRE:-forja-gui-dom}"
URL="https://github.com/${ORG}/${NOMBRE}.git"
ORIGEN="library/wasm-gui/gui-dom"
DESTINO="library/gui-dom"

echo "▶ publicando ${ORG}/${NOMBRE}"

if ! gh auth status >/dev/null 2>&1; then
    cat <<'EOF'
❌ `gh` no está autenticado (o el token venció).

   gh auth login -h github.com
   # o bien: export GH_TOKEN=<token con scope repo>

   Después volvé a correr este script.
EOF
    exit 1
fi

if [ -d ".gitmodules" ] || git config --file .gitmodules --get-regexp "path ${DESTINO}$" >/dev/null 2>&1; then
    echo "· ${DESTINO} ya figura como submódulo; nada que hacer"
    exit 0
fi

if [ ! -d "$ORIGEN/.git" ]; then
    echo "❌ no encuentro el repo local en ${ORIGEN} (¿ya se movió?)"
    exit 1
fi

# 1. Crear el repositorio remoto y subir el commit local.
if gh repo view "${ORG}/${NOMBRE}" >/dev/null 2>&1; then
    echo "· el repositorio ${ORG}/${NOMBRE} ya existe; se sube el commit"
    (cd "$ORIGEN" && git remote add origin "$URL" 2>/dev/null || git remote set-url origin "$URL")
    (cd "$ORIGEN" && git push -u origin main)
else
    (cd "$ORIGEN" && gh repo create "${ORG}/${NOMBRE}" --public --source=. --remote=origin --push \
        --description "Forja GUI DOM — modelo de nodos DOM y renderizado HTML accesible")
fi

# 2. Sacar el crate de dentro del submódulo wasm-gui.
#    Ya está en GitHub (paso 1): se borra la copia local y el submódulo se clona.
#    (NO se hace `git checkout -- .` en wasm-gui: borraría los cambios del modo DOM.)
echo "▶ moviendo el crate a ${DESTINO}"
rm -rf "$ORIGEN"
rm -rf "$DESTINO"

# 3. Registrar como submódulo del repo raíz.
git submodule add "$URL" "$DESTINO"

# 4. Actualizar rutas: miembro del workspace y dependencia de wasm-gui.
python3 - <<'PY'
import io, re
p='Cargo.toml'
s=io.open(p,encoding='utf-8').read()
s=s.replace('    "library/wasm-gui/gui-dom",','    "library/gui-dom",')
io.open(p,'w',encoding='utf-8').write(s)

p2='library/wasm-gui/Cargo.toml'
t=io.open(p2,encoding='utf-8').read()
t=t.replace('forja-gui-dom = { path = "gui-dom" }','forja-gui-dom = { path = "../gui-dom" }')
io.open(p2,'w',encoding='utf-8').write(t)
PY

# 5. Verificar que sigue compilando y con los tests en verde.
cargo test -q -p forja-gui-dom

echo "✅ ${ORG}/${NOMBRE} publicado y registrado como submódulo en ${DESTINO}"
echo "   recordá commitear: Cargo.toml, .gitmodules, ${DESTINO}, library/wasm-gui y library/wasm-gui/Cargo.toml"
