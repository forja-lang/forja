#!/usr/bin/env bash
# scripts/probar-demo-web.sh — e2e de la demo demos/tareas-web.
#
# Es la prueba de que el stack completo sirve una página real:
#   registro → cookie firmada → alta de tarea (con CSRF) → aparece listada →
#   marcar hecha → logout → sin sesión.
#
# Usa una base SQLite temporal, así no toca `demos/tareas-web/tareas.db`.
#
# Uso:
#   bash scripts/probar-demo-web.sh
#   FORJA=/ruta/al/binario bash scripts/probar-demo-web.sh
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

# El script corre desde demos/tareas-web: el binario tiene que ser absoluto.
case "$FORJA" in
    /*|[A-Za-z]:[\/]*) ;;
    *) FORJA="$ROOT/$FORJA" ;;
esac

if ! command -v curl >/dev/null 2>&1; then
    echo "· sin curl: e2e de la demo salteado"
    exit 0
fi

PUERTO=$(( 20600 + ($$ % 300) ))
TMP="$(mktemp -d)"
export FORJA_WEB_PUERTO="$PUERTO"
export FORJA_WEB_BD="$TMP/tareas.db"
export FORJA_WEB_SECRETO="secreto-de-pruebas-de-la-demo-123456"

# Primero se mata el servidor: SQLite mantiene la base abierta y en Windows no
# se puede borrar el archivo mientras el proceso vive.
trap 'if [ -n "${SERVIDOR_PID:-}" ]; then kill "$SERVIDOR_PID" 2>/dev/null; sleep 0.4; fi; rm -rf "$TMP"' EXIT

echo "▶ arrancando la demo en 127.0.0.1:$PUERTO (base $FORJA_WEB_BD)"
"$FORJA" run main.fa > "$TMP/servidor.log" 2>&1 &
SERVIDOR_PID=$!

for _ in $(seq 1 300); do
    if grep -q "iniciado en http" "$TMP/servidor.log" 2>/dev/null; then
        break
    fi
    if ! kill -0 "$SERVIDOR_PID" 2>/dev/null; then
        echo "❌ la demo terminó antes de escuchar:"
        sed 's/^/   /' "$TMP/servidor.log"
        exit 1
    fi
    sleep 0.1
done

if ! grep -q "iniciado en http" "$TMP/servidor.log" 2>/dev/null; then
    echo "❌ la demo no llegó a escuchar en 30 s:"
    sed 's/^/   /' "$TMP/servidor.log"
    exit 1
fi

BASE="http://127.0.0.1:$PUERTO"
JAR="$TMP/cookies.txt"
OK=0
fallo() { echo "❌ $1"; echo "--- log:"; tail -n 15 "$TMP/servidor.log" | sed 's/^/   /'; exit 1; }

# ─── Sin sesión: redirige al login ─────────────────────────────────
codigo="$(curl -s -o /dev/null -w '%{http_code}' "$BASE/")"
[ "$codigo" = "302" ] || fallo "/ sin sesión devolvió $codigo (se esperaba 302)"

# ─── Registro (y sesión automática) ────────────────────────────────
codigo="$(curl -s -c "$JAR" -o /dev/null -w '%{http_code}' \
    -d "email=ana@ejemplo.com&clave=clave1234" "$BASE/registrar")"
[ "$codigo" = "302" ] || fallo "registro devolvió $codigo (se esperaba 302 al tablero)"

pagina="$(curl -s -b "$JAR" "$BASE/")"
echo "$pagina" | grep -q "<h1>Tareas</h1>" || fallo "el tablero no se renderizó"
echo "$pagina" | grep -q "0 pendientes, 0 en total" || fallo "el tablero no muestra el conteo vacío"

# CSRF: sin token, el POST tiene que rebotar.
codigo="$(curl -s -b "$JAR" -o /dev/null -w '%{http_code}' -d "titulo=sin-token" "$BASE/tareas")"
[ "$codigo" = "403" ] || fallo "crear tarea sin CSRF devolvió $codigo (se esperaba 403)"

# ─── Alta de tarea con token ───────────────────────────────────────
token="$(printf '%s' "$pagina" | grep -o 'name="csrf" value="[a-f0-9]*"' | head -n 1 | sed 's/.*value="//; s/"//')"
if [ "${#token}" != "64" ]; then
    fallo "no se encontró el token CSRF en el formulario (token='$token')"
fi

curl -s -b "$JAR" -o /dev/null -d "csrf=$token&titulo=comprar%20pan" "$BASE/tareas"
pagina="$(curl -s -b "$JAR" "$BASE/")"
echo "$pagina" | grep -q "comprar pan" || fallo "la tarea no aparece listada"
echo "$pagina" | grep -q "1 pendientes, 1 en total" || fallo "el conteo no se actualizó"

# XSS: un título con HTML no debe salir crudo.
curl -s -b "$JAR" -o /dev/null -d "csrf=$token&titulo=%3Cscript%3Ealert(1)%3C/script%3E" "$BASE/tareas"
pagina="$(curl -s -b "$JAR" "$BASE/")"
echo "$pagina" | grep -q '<script>alert(1)</script>' && fallo "XSS: el título salió sin escapar"
echo "$pagina" | grep -q '&lt;script&gt;' || fallo "el título escapado no aparece"

# ─── Marcar hecha ──────────────────────────────────────────────────
id="$(printf '%s' "$pagina" | grep -o 'action="/tareas/[0-9]*/hecha"' | head -n 1 | grep -o '[0-9]\+')"
[ -n "$id" ] || fallo "no se encontró el formulario para marcar hecha"
curl -s -b "$JAR" -o /dev/null -d "csrf=$token" "$BASE/tareas/$id/hecha"
pagina="$(curl -s -b "$JAR" "$BASE/")"
echo "$pagina" | grep -q "1 pendientes, 2 en total" || fallo "marcar hecha no actualizó los pendientes"

# ─── Login desde cero (otra sesión) ────────────────────────────────
JAR2="$TMP/cookies2.txt"
codigo="$(curl -s -c "$JAR2" -o /dev/null -w '%{http_code}' -d "email=ana@ejemplo.com&clave=clave1234" "$BASE/entrar")"
[ "$codigo" = "302" ] || fallo "login devolvió $codigo (se esperaba 302)"
curl -s -b "$JAR2" "$BASE/" | grep -q "comprar pan" || fallo "la sesión nueva no ve las tareas"

codigo="$(curl -s -o /dev/null -w '%{http_code}' -d "email=ana@ejemplo.com&clave=mala" "$BASE/entrar")"
[ "$codigo" = "401" ] || fallo "login con clave mala devolvió $codigo (se esperaba 401)"

# ─── Logout ────────────────────────────────────────────────────────
curl -s -b "$JAR" -o /dev/null -d "csrf=$token" "$BASE/salir"
codigo="$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" "$BASE/")"
[ "$codigo" = "302" ] || fallo "/ tras logout devolvió $codigo (se esperaba 302)"

# ─── Estáticos y salud ─────────────────────────────────────────────
mime="$(curl -s -D - -o /dev/null "$BASE/publico/estilo.css" | tr -d '\r' | grep -i '^content-type:' | head -n 1)"
echo "$mime" | grep -qi 'text/css' || fallo "el CSS no se sirve con text/css ($mime)"
curl -s "$BASE/salud" | grep -q '"estado":"ok"' || fallo "/salud no responde ok"

# La migración quedó registrada una sola vez.
if command -v sqlite3 >/dev/null 2>&1; then
    aplicadas="$(sqlite3 "$FORJA_WEB_BD" "SELECT count(*) FROM migraciones_aplicadas;")"
    [ "$aplicadas" = "1" ] || fallo "migraciones_aplicadas tiene $aplicadas filas (se esperaba 1)"
fi

echo "   registro 302 · CSRF 403 sin token · alta y listado OK · XSS escapado"
echo "   marcar hecha OK · login 302/401 · logout 302 · CSS text/css · /salud ok"
echo "✅ demo web de punta a punta OK"
