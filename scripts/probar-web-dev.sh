#!/usr/bin/env bash
# scripts/probar-web-dev.sh — `forja web dev` reinicia la app al cambiar un .fa.
#
# Levanta el servidor de desarrollo sobre una app de prueba, comprueba que
# responde, edita el .fa y verifica que la siguiente petición ya sirve el código
# nuevo (o sea: la app se reinició sola).
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
    echo "· sin curl: e2e de 'forja web dev' salteado"
    exit 0
fi

# Puerto libre: se prueban unos cuantos por si quedó un servidor de otra corrida.
PUERTO=""
for intento in $(seq 1 40); do
    candidato=$(( 21400 + ($$ + intento * 7) % 300 ))
    if ! (exec 3<>"/dev/tcp/127.0.0.1/$candidato") 2>/dev/null; then
        PUERTO="$candidato"
        break
    fi
done
[ -n "$PUERTO" ] || { echo "❌ no se encontró un puerto libre"; exit 1; }
TMP="$(mktemp -d)"
APP="$TMP/app"
mkdir -p "$APP"
trap 'if [ -n "${DEV_PID:-}" ]; then kill "$DEV_PID" 2>/dev/null; fi; sleep 0.3; rm -rf "$TMP"' EXIT

fallo() {
    echo "❌ $1"
    [ -f "$TMP/dev.log" ] && tail -n 20 "$TMP/dev.log" | sed 's/^/   /'
    exit 1
}

escribir_app() {
    cat > "$APP/main.fa" <<EOF
importar "std/servidor_web"
importar "std/plantillas"

funcion inicio(peticion, respuesta) {
    variable html_pagina = plantillas.interpolar("<h1>{{marca}}</h1>", {"marca": "$1"})
    retornar servidor_web.html(respuesta, html_pagina)
}

variable servidor = nuevo ServidorWeb($PUERTO)
servidor_web.registrar_obtener(servidor, "/", inicio)
servidor_web.iniciar(servidor)
EOF
}

escribir_app "marca-v1"

echo "▶ arrancando forja web dev en 127.0.0.1:$PUERTO"
cd "$APP" || exit 1
FORJA_WEB_PUERTO="$PUERTO" "$FORJA" web dev main.fa > "$TMP/dev.log" 2>&1 &
DEV_PID=$!

# Esperar la primera respuesta (el hijo tarda en levantar).
cuerpo=""
for _ in $(seq 1 200); do
    cuerpo="$(curl -s "http://127.0.0.1:$PUERTO/" 2>/dev/null || true)"
    if echo "$cuerpo" | grep -q "marca-v1"; then
        break
    fi
    sleep 0.1
done
echo "$cuerpo" | grep -q "marca-v1" || fallo "la app no respondió con la marca inicial (obtuve '$cuerpo')"

# Cambiar el .fa: el watcher tiene que reiniciar la app.
escribir_app "marca-v2"
echo "▶ archivo modificado; esperando recarga"

cuerpo=""
for _ in $(seq 1 300); do
    cuerpo="$(curl -s "http://127.0.0.1:$PUERTO/" 2>/dev/null || true)"
    if echo "$cuerpo" | grep -q "marca-v2"; then
        break
    fi
    sleep 0.1
done
echo "$cuerpo" | grep -q "marca-v2" || fallo "no se recargó: la página sigue en '$cuerpo'"

grep -q "cambio detectado" "$TMP/dev.log" || fallo "el log de 'web dev' no reporta la recarga"

echo "   GET / -> marca-v1 · edición del .fa · GET / -> marca-v2"
echo "✅ forja web dev OK"
