#!/usr/bin/env bash
# scripts/probar-nuevo-web.sh — `FORJA_PLANTILLA=web forja nuevo <app>` sirve una página.
#
# Verifica que la plantilla web que genera la CLI arranca, responde HTML
# escapado, sirve el CSS con el tipo correcto y expone /salud.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FORJA="${FORJA:-}"
if [ -z "$FORJA" ]; then
    echo "▶ compilando forja-cli…"
    cargo build -q -p forja-cli --bin forja || exit 1
    FORJA="target/debug/forja"
fi
case "$FORJA" in
    /*|[A-Za-z]:[\\/]*) ;;
    *) FORJA="$ROOT/$FORJA" ;;
esac

if ! command -v curl >/dev/null 2>&1; then
    echo "· sin curl: e2e de la plantilla web salteado"
    exit 0
fi

PUERTO=$(( 21200 + ($$ % 200) ))
TMP="$(mktemp -d)"
APPDIR="$TMP/app"
trap 'if [ -n "${SERVIDOR_PID:-}" ]; then kill "$SERVIDOR_PID" 2>/dev/null; sleep 0.3; fi; rm -rf "$TMP"' EXIT

fallo() { echo "❌ $1"; if [ -f "$TMP/servidor.log" ]; then tail -n 12 "$TMP/servidor.log" | sed 's/^/   /'; fi; exit 1; }

echo "▶ generando proyecto con FORJA_PLANTILLA=web"
FORJA_PLANTILLA=web "$FORJA" nuevo "$APPDIR" > "$TMP/nuevo.log" 2>&1 || {
    echo "❌ 'forja nuevo' falló:"
    sed 's/^/   /' "$TMP/nuevo.log"
    exit 1
}

for archivo in main.fa forja.json README.md publico/estilo.css; do
    [ -f "$APPDIR/$archivo" ] || fallo "la plantilla no generó $archivo"
done

echo "▶ arrancando la app generada en 127.0.0.1:$PUERTO"
cd "$APPDIR" || exit 1
FORJA_WEB_PUERTO="$PUERTO" "$FORJA" run main.fa > "$TMP/servidor.log" 2>&1 &
SERVIDOR_PID=$!

for _ in $(seq 1 200); do
    if grep -q "iniciado en http" "$TMP/servidor.log" 2>/dev/null; then
        break
    fi
    if ! kill -0 "$SERVIDOR_PID" 2>/dev/null; then
        fallo "la app terminó antes de escuchar"
    fi
    sleep 0.1
done
grep -q "iniciado en http" "$TMP/servidor.log" 2>/dev/null || fallo "la app no llegó a escuchar"

BASE="http://127.0.0.1:$PUERTO"

pagina="$(curl -s "$BASE/")"
echo "$pagina" | grep -q "<h1>Hola desde" || fallo "la página no se renderizó ($pagina)"
echo "$pagina" | grep -q "<li>editar main.fa</li>" || fallo "el bucle {% para %} no se renderizó"
echo "$pagina" | grep -q "<title>" || fallo "el layout no se aplicó"

mime="$(curl -s -D - -o /dev/null "$BASE/publico/estilo.css" | tr -d '\r' | grep -i '^content-type:' | head -n 1)"
echo "$mime" | grep -qi 'text/css' || fallo "el CSS no se sirve como text/css ($mime)"

curl -s "$BASE/salud" | grep -q '"estado":"ok"' || fallo "/salud no responde ok"

echo "   archivos: main.fa + forja.json + README.md + publico/estilo.css"
echo "   GET / con SSR y bucle · CSS text/css · /salud ok"
echo "✅ plantilla web de 'forja nuevo' OK"
