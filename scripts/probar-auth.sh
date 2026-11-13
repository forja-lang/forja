#!/usr/bin/env bash
# scripts/probar-auth.sh — e2e de std/auth contra el servidor web real.
#
# Verifica con `curl`:
#   - registro (201), duplicado (409) y contraseña débil (400)
#   - login: credenciales malas (401), buenas (200 + Set-Cookie)
#   - ruta privada con y sin sesión
#   - rate limit: tras N intentos fallidos responde 429
#   - logout invalida la sesión
#
# El store de usuarios es un mapa en memoria del script (lo mismo que haría un
# `buscar_usuario` sobre SQLite): así el e2e no depende de la base.
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
    echo "· sin curl: e2e de auth salteado"
    exit 0
fi

PUERTO=$(( 19900 + ($$ % 400) ))
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"; [ -n "${SERVIDOR_PID:-}" ] && kill "$SERVIDOR_PID" 2>/dev/null' EXIT

cat > "$TMP/servidor.fa" <<EOF
importar "std/servidor_web"
importar "std/sesion"
importar "std/auth"

variable usuarios = {}
variable almacen_sesiones = sesion.almacen("clave-de-pruebas-1234567890abcdef", 3600, falso)

funcion buscar_usuario(identificador) {
    retornar usuarios[identificador]
}

funcion crear_usuario(identificador, hash) {
    si (usuarios[identificador] != nulo) {
        retornar nulo
    }
    variable id = longitud(claves(usuarios)) + 1
    usuarios[identificador] = {"id": id, "hash": hash}
    retornar id
}

variable guardia = auth.autenticador(almacen_sesiones, buscar_usuario, 3, 300)

funcion estado_para(error_auth) {
    si (error_auth.codigo == "demasiados_intentos") {
        retornar 429
    }
    si (error_auth.codigo == "duplicado") {
        retornar 409
    }
    si (error_auth.codigo == "credenciales") {
        retornar 401
    }
    retornar 400
}

funcion registrar_usuario(req, res) {
    variable r = auth.registrar(guardia, req.parametros["email"], req.parametros["clave"], crear_usuario)
    si (auth.es_error(r)) {
        variable error_r = auth.error_de(r)
        variable mala = servidor_web.texto_plano(res, error_r.mensaje)
        mala.codigo = estado_para(error_r)
        retornar mala
    }
    variable bien = servidor_web.texto_plano(res, "id:" + a_texto(auth.valor_de(r)))
    bien.codigo = 201
    retornar bien
}

funcion entrar(req, res) {
    variable r = auth.iniciar_sesion(guardia, req, res, req.parametros["email"], req.parametros["clave"])
    si (auth.es_error(r)) {
        variable error_r = auth.error_de(r)
        variable mala = servidor_web.texto_plano(res, error_r.codigo)
        mala.codigo = estado_para(error_r)
        retornar mala
    }
    retornar servidor_web.texto_plano(res, "dentro")
}

funcion yo(req, res) {
    variable actual = auth.sesion_actual(guardia, req)
    si (actual == nulo) {
        variable denegado = servidor_web.texto_plano(res, "sin sesion")
        denegado.codigo = 403
        retornar denegado
    }
    retornar servidor_web.texto_plano(res, "usuario " + a_texto(actual.datos["usuario_id"]))
}

funcion salir(req, res) {
    auth.cerrar_sesion(guardia, req, res)
    retornar servidor_web.texto_plano(res, "fuera")
}

funcion main() {
    variable s = nuevo ServidorWeb($PUERTO)
    servidor_web.enviar(s, "/registrar", registrar_usuario)
    servidor_web.enviar(s, "/entrar", entrar)
    servidor_web.registrar_obtener(s, "/yo", yo)
    servidor_web.enviar(s, "/salir", salir)
    servidor_web.iniciar(s)
}
main()
EOF

echo "▶ arrancando servidor de auth en 127.0.0.1:$PUERTO"
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

codigo_post() { curl -s -o /dev/null -w '%{http_code}' -X POST "$1"; }
codigo_con_jar() { curl -s -o /dev/null -w '%{http_code}' -b "$JAR" -X POST "$1"; }

# ─── Registro ──────────────────────────────────────────────────────
codigo="$(codigo_post "$BASE/registrar?email=ana@ejemplo.com&clave=clave1234")"
[ "$codigo" = "201" ] || fallo "registro devolvió $codigo (se esperaba 201)"

codigo="$(codigo_post "$BASE/registrar?email=ana@ejemplo.com&clave=clave1234")"
[ "$codigo" = "409" ] || fallo "registro duplicado devolvió $codigo (se esperaba 409)"

codigo="$(codigo_post "$BASE/registrar?email=bob@ejemplo.com&clave=corta")"
[ "$codigo" = "400" ] || fallo "contraseña débil devolvió $codigo (se esperaba 400)"

# ─── Login ─────────────────────────────────────────────────────────
codigo="$(codigo_con_jar "$BASE/entrar?email=ana@ejemplo.com&clave=incorrecta")"
[ "$codigo" = "401" ] || fallo "login con contraseña mala devolvió $codigo (se esperaba 401)"

codigo="$(curl -s -c "$JAR" -o /dev/null -w '%{http_code}' -X POST "$BASE/entrar?email=ana@ejemplo.com&clave=clave1234")"
[ "$codigo" = "200" ] || fallo "login correcto devolvió $codigo (se esperaba 200)"

cuerpo="$(curl -s -b "$JAR" "$BASE/yo")"
[ "$cuerpo" = "usuario 1" ] || fallo "/yo devolvió '$cuerpo'"

codigo="$(curl -s -o /dev/null -w '%{http_code}' "$BASE/yo")"
[ "$codigo" = "403" ] || fallo "/yo sin cookie devolvió $codigo (se esperaba 403)"

# ─── Rate limit (3 intentos por ventana) ───────────────────────────
# Ya gastamos 2 intentos (uno malo, uno bueno que lo reinició). Tres malos más
# dejan la clave bloqueada.
for _ in 1 2 3; do
    curl -s -o /dev/null -X POST "$BASE/entrar?email=ana@ejemplo.com&clave=mala"
done
codigo="$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/entrar?email=ana@ejemplo.com&clave=clave1234")"
[ "$codigo" = "429" ] || fallo "tras los intentos fallidos devolvió $codigo (se esperaba 429)"

# ─── Logout ────────────────────────────────────────────────────────
curl -s -b "$JAR" -X POST "$BASE/salir" > /dev/null
codigo="$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" "$BASE/yo")"
[ "$codigo" = "403" ] || fallo "/yo tras logout devolvió $codigo (se esperaba 403)"

echo "   registro 201 · duplicado 409 · débil 400 · login malo 401 y bueno 200"
echo "   /yo con cookie 200 y sin cookie 403 · rate limit 429 · logout invalida"
echo "✅ std/auth de punta a punta OK"
