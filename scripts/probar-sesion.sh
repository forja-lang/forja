#!/usr/bin/env bash
# scripts/probar-sesion.sh — e2e de std/sesion contra el servidor web real.
#
# Levanta un servidor con login de juguete y verifica con `curl`:
#   - /entrar crea sesión y manda Set-Cookie HttpOnly
#   - /privado exige sesión (403 sin cookie, 200 con cookie)
#   - una cookie manipulada no vale
#   - CSRF: /accion exige el token derivado de la sesión
#   - /salir cierra la sesión y /privado vuelve a dar 403
#
# Uso:
#   bash scripts/probar-sesion.sh
#   FORJA=/ruta/al/binario bash scripts/probar-sesion.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FORJA="${FORJA:-}"
if [ -z "$FORJA" ]; then
    echo "▶ compilando forja-cli…"
    cargo build -q -p forja-cli --bin forja || exit 1
    FORJA="target/debug/forja"
fi

if ! command -v curl >/dev/null 2>&1; then
    echo "· sin curl: e2e de sesiones salteado"
    exit 0
fi

PUERTO=$(( 19300 + ($$ % 500) ))
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"; [ -n "${SERVIDOR_PID:-}" ] && kill "$SERVIDOR_PID" 2>/dev/null' EXIT

cat > "$TMP/servidor.fa" <<EOF
importar "std/servidor_web"
importar "std/sesion"

variable almacen_sesiones = sesion.almacen("clave-de-pruebas-1234567890abcdef", 3600, falso)

funcion entrar(req, res) {
    variable nueva = sesion.crear(almacen_sesiones, {"usuario": "ana"})
    sesion.fijar_cookie(res, almacen_sesiones, nueva)
    retornar servidor_web.texto_plano(res, sesion.token_csrf(almacen_sesiones, nueva))
}

funcion privado(req, res) {
    variable actual = sesion.leer_de_solicitud(almacen_sesiones, req)
    si (actual == nulo) {
        variable denegado = servidor_web.texto_plano(res, "sin sesion")
        denegado.codigo = 403
        retornar denegado
    }
    retornar servidor_web.texto_plano(res, "hola " + a_texto(actual.datos["usuario"]))
}

funcion accion(req, res) {
    variable actual = sesion.leer_de_solicitud(almacen_sesiones, req)
    si (!sesion.csrf_valido(almacen_sesiones, actual, req.cabeceras["x-csrf-token"])) {
        variable bloqueado = servidor_web.texto_plano(res, "csrf invalido")
        bloqueado.codigo = 403
        retornar bloqueado
    }
    retornar servidor_web.texto_plano(res, "accion ok")
}

funcion salir(req, res) {
    variable actual = sesion.leer_de_solicitud(almacen_sesiones, req)
    si (actual != nulo) {
        sesion.invalidar(almacen_sesiones, actual.id)
    }
    sesion.borrar_cookie(res)
    retornar servidor_web.texto_plano(res, "fuera")
}

funcion main() {
    variable s = nuevo ServidorWeb($PUERTO)
    servidor_web.registrar_obtener(s, "/privado", privado)
    servidor_web.enviar(s, "/entrar", entrar)
    servidor_web.enviar(s, "/accion", accion)
    servidor_web.enviar(s, "/salir", salir)
    servidor_web.iniciar(s)
}
main()
EOF

echo "▶ arrancando servidor de sesiones en 127.0.0.1:$PUERTO"
"$FORJA" run "$TMP/servidor.fa" > "$TMP/servidor.log" 2>&1 &
SERVIDOR_PID=$!

for _ in $(seq 1 200); do
    if grep -q "iniciado en http" "$TMP/servidor.log" 2>/dev/null; then
        break
    fi
    if ! kill -0 "$SERVIDOR_PID" 2>/dev/null; then
        echo "❌ el servidor terminó antes de escuchar:"
        sed 's/^/   /' "$TMP/servidor.log"
        exit 1
    fi
    sleep 0.1
done

if ! grep -q "iniciado en http" "$TMP/servidor.log" 2>/dev/null; then
    echo "❌ el servidor no llegó a escuchar en 20 s:"
    sed 's/^/   /' "$TMP/servidor.log"
    exit 1
fi

BASE="http://127.0.0.1:$PUERTO"
JAR="$TMP/cookies.txt"
fallo() { echo "❌ $1"; exit 1; }

# ─── Sin sesión no hay acceso ──────────────────────────────────────
codigo="$(curl -s -o /dev/null -w '%{http_code}' "$BASE/privado")"
[ "$codigo" = "403" ] || fallo "/privado sin cookie devolvió $codigo (se esperaba 403)"

codigo="$(curl -s -o /dev/null -w '%{http_code}' -H 'Cookie: sid=basura' "$BASE/privado")"
[ "$codigo" = "403" ] || fallo "/privado con cookie falsa devolvió $codigo (se esperaba 403)"

# ─── Login ─────────────────────────────────────────────────────────
token="$(curl -s -c "$JAR" -X POST "$BASE/entrar")"
if [ "${#token}" != "64" ]; then
    fallo "el token CSRF del login no parece válido ($token)"
fi

cabecera_cookie="$(curl -s -D - -o /dev/null -X POST "$BASE/entrar" | tr -d '\r' | grep -i '^set-cookie:' | head -n 1)"
echo "$cabecera_cookie" | grep -qi 'HttpOnly' || fallo "la cookie no es HttpOnly ($cabecera_cookie)"
echo "$cabecera_cookie" | grep -qi 'SameSite' || fallo "la cookie no tiene SameSite ($cabecera_cookie)"

cuerpo="$(curl -s -b "$JAR" "$BASE/privado")"
[ "$cuerpo" = "hola ana" ] || fallo "/privado con sesión devolvió '$cuerpo'"

# ─── Cookie manipulada (id válido, firma alterada) ─────────────────
cookie_ok="$(grep -i 'sid' "$JAR" | awk '{print $7}' | tail -n 1)"
if [ -z "$cookie_ok" ]; then
    fallo "no se pudo leer la cookie del cookie jar"
fi
ultimo="${cookie_ok: -1}"
if [ "$ultimo" = "0" ]; then
    manipulada="${cookie_ok%?}1"
else
    manipulada="${cookie_ok%?}0"
fi
codigo="$(curl -s -o /dev/null -w '%{http_code}' -H "Cookie: sid=$manipulada" "$BASE/privado")"
[ "$codigo" = "403" ] || fallo "una cookie con la firma alterada devolvió $codigo (se esperaba 403)"

# ─── CSRF ──────────────────────────────────────────────────────────
codigo="$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" -X POST "$BASE/accion")"
[ "$codigo" = "403" ] || fallo "/accion sin token CSRF devolvió $codigo (se esperaba 403)"

codigo="$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" -X POST -H "X-CSRF-Token: $token" "$BASE/accion")"
[ "$codigo" = "200" ] || fallo "/accion con token CSRF devolvió $codigo (se esperaba 200)"

# ─── Logout ────────────────────────────────────────────────────────
curl -s -b "$JAR" -X POST "$BASE/salir" > /dev/null
codigo="$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" "$BASE/privado")"
[ "$codigo" = "403" ] || fallo "/privado después del logout devolvió $codigo (se esperaba 403)"

echo "   sin sesión 403 · login con cookie HttpOnly · cookie manipulada 403"
echo "   CSRF 403 sin token y 200 con token · logout invalida la sesión"
echo "✅ std/sesion de punta a punta OK"
