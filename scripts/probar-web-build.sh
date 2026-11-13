#!/usr/bin/env bash
# scripts/probar-web-build.sh — `forja web build` arma un paquete que corre.
#
# Genera el paquete sobre la demo `demos/tareas-web` en un directorio temporal,
# comprueba los archivos esperados y lo arranca desde ese directorio (como haría
# `ejecutar.sh`) para verificar que sirve la página, el CSS y /salud.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO="$ROOT/demos/tareas-web"
cd "$DEMO" || exit 1

FORJA="${FORJA:-}"
if [ -z "$FORJA" ]; then
    echo "▶ compilando forja-cli…"
    (cd "$ROOT" && cargo build -q -p forja-cli --bin forja) || exit 1
    FORJA="$ROOT/target/debug/forja"
fi
case "$FORJA" in
    /*|[A-Za-z]:[\\/]*) ;;
    *) FORJA="$ROOT/$FORJA" ;;
esac

if ! command -v curl >/dev/null 2>&1; then
    echo "· sin curl: e2e de 'forja web build' salteado"
    exit 0
fi

PUERTO=$(( 21600 + ($$ % 200) ))
TMP="$(mktemp -d)"
SALIDA="$TMP/dist"
trap 'if [ -n "${APP_PID:-}" ]; then kill "$APP_PID" 2>/dev/null; sleep 0.3; fi; rm -rf "$TMP"' EXIT

fallo() {
    echo "❌ $1"
    [ -f "$TMP/app.log" ] && tail -n 15 "$TMP/app.log" | sed 's/^/   /'
    exit 1
}

echo "▶ generando paquete en $SALIDA"
"$FORJA" web build "$SALIDA" > "$TMP/build.log" 2>&1 || {
    echo "❌ 'forja web build' falló:"
    sed 's/^/   /' "$TMP/build.log"
    exit 1
}

for archivo in main.fa Dockerfile ejecutar.sh .env.example README.md publico/estilo.css migraciones/001_inicial.sql; do
    [ -e "$SALIDA/$archivo" ] || fallo "el paquete no incluye $archivo"
done

# Ejecutable autónomo: tiene que existir y correr sin el intérprete.
BINARIO="$SALIDA/app"
[ -x "$BINARIO" ] || BINARIO="$SALIDA/app.exe"
[ -f "$BINARIO" ] || fallo "el paquete no incluye el ejecutable autónomo (app)"

echo "▶ arrancando el paquete en 127.0.0.1:$PUERTO"
cd "$SALIDA" || exit 1
# Se corre el ejecutable del paquete (no el intérprete): es lo que se despliega.
FORJA_WEB_PUERTO="$PUERTO" FORJA_WEB_BD="$TMP/dist.db" "$BINARIO" > "$TMP/app.log" 2>&1 &
APP_PID=$!

for _ in $(seq 1 200); do
    if grep -q "iniciado en http" "$TMP/app.log" 2>/dev/null; then
        break
    fi
    if ! kill -0 "$APP_PID" 2>/dev/null; then
        fallo "el paquete no arrancó"
    fi
    sleep 0.1
done
grep -q "iniciado en http" "$TMP/app.log" 2>/dev/null || fallo "el paquete no llegó a escuchar"

BASE="http://127.0.0.1:$PUERTO"
codigo="$(curl -s -o /dev/null -w '%{http_code}' "$BASE/")"
[ "$codigo" = "302" ] || fallo "/ devolvió $codigo (se esperaba 302 al login)"

pagina="$(curl -s "$BASE/entrar")"
echo "$pagina" | grep -q "<h1>Entrar</h1>" || fallo "la página de login no se renderizó"

mime="$(curl -s -D - -o /dev/null "$BASE/publico/estilo.css" | tr -d '\r' | grep -i '^content-type:' | head -n 1)"
echo "$mime" | grep -qi 'text/css' || fallo "el CSS no se sirve como text/css ($mime)"

curl -s "$BASE/salud" | grep -q '"estado":"ok"' || fallo "/salud no responde ok"

echo "   paquete con ejecutable autónomo + fuentes + estáticos + migraciones + Dockerfile"
echo "   GET / 302 · /entrar 200 · CSS text/css · /salud ok"
echo "✅ forja web build OK"
