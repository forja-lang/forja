#!/usr/bin/env bash
# scripts/probar-http.sh — prueba de punta a punta del stack HTTP de la stdlib.
#
# Levanta `std/servidor_web` en un puerto libre, espera a que anuncie que está
# escuchando y lo consulta con `std/cliente_http` (GET, parámetros de ruta,
# 404 y POST). Falla si alguna respuesta no coincide.
#
# Uso:
#   bash scripts/probar-http.sh
#   FORJA=/ruta/al/binario bash scripts/probar-http.sh   # reusar un binario
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FORJA="${FORJA:-}"
if [ -z "$FORJA" ]; then
    echo "▶ compilando forja-cli…"
    cargo build -q -p forja-cli --bin forja || exit 1
    FORJA="target/debug/forja"
fi

PUERTO=$(( 18800 + ($$ % 500) ))
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"; [ -n "${SERVIDOR_PID:-}" ] && kill "$SERVIDOR_PID" 2>/dev/null' EXIT

cat > "$TMP/servidor.fa" <<EOF
importar "std/servidor_web"

funcion main() {
    variable s = nuevo ServidorWeb($PUERTO)
    servidor_web.registrar_obtener(s, "/texto", func(peticion, respuesta) {
        retornar servidor_web.texto_plano(respuesta, "hola")
    })
    servidor_web.registrar_obtener(s, "/json", func(peticion, respuesta) {
        variable r = servidor_web.texto_plano(respuesta, "{\"ok\":true}")
        r.cabeceras["Content-Type"] = "application/json"
        retornar r
    })
    servidor_web.registrar_obtener(s, "/param/:id", func(peticion, respuesta) {
        retornar servidor_web.texto_plano(respuesta, peticion.params_ruta["id"])
    })
    servidor_web.enviar(s, "/eco", func(peticion, respuesta) {
        retornar servidor_web.texto_plano(respuesta, "eco:" + peticion.cuerpo)
    })
    servidor_web.iniciar(s)
}
main()
EOF

cat > "$TMP/cliente.fa" <<'EOF'
importar "std/cliente_http"
importar "std/tests/arnes"

variable base = "http://127.0.0.1:" + a_texto(PUERTO_PLACEHOLDER)

variable r1 = cliente_http.obtener_recurso(base + "/texto")
comprobar_igual("GET /texto: código", r1.codigo, 200)
comprobar_texto("GET /texto: cuerpo", r1.cuerpo, "hola")

variable r2 = cliente_http.obtener_recurso(base + "/param/42")
comprobar_texto("GET /param/:id: captura el parámetro", r2.cuerpo, "42")

variable r3 = cliente_http.obtener_recurso(base + "/json")
comprobar("GET /json: cuerpo", contiene(r3.cuerpo, "\"ok\""))
comprobar("GET /json: Content-Type", contiene(a_texto(r3.cabeceras), "json") || contiene(a_texto(r3.cabeceras), "JSON"))

variable r4 = cliente_http.obtener_recurso(base + "/no-existe")
comprobar_igual("GET inexistente: 404", r4.codigo, 404)

variable r5 = cliente_http.enviar(base + "/eco", "abc=1")
comprobar_igual("POST /eco: código", r5.codigo, 200)
comprobar("POST /eco: el cuerpo viaja", contiene(r5.cuerpo, "abc=1"))

resumen()
EOF

sed -i "s/PUERTO_PLACEHOLDER/$PUERTO/" "$TMP/cliente.fa"

echo "▶ arrancando servidor en 127.0.0.1:$PUERTO"
"$FORJA" run "$TMP/servidor.fa" > "$TMP/servidor.log" 2>&1 &
SERVIDOR_PID=$!

# Esperar a que anuncie que escucha (máximo 20 s).
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

echo "▶ consultando con std/cliente_http"
salida="$("$FORJA" run "$TMP/cliente.fa" 2>&1)"
estado=$?

if [ $estado -ne 0 ]; then
    echo "❌ el cliente falló (exit $estado)"
    echo "$salida" | sed 's/^/   /'
    echo "--- log del servidor:"
    tail -n 20 "$TMP/servidor.log" | sed 's/^/   /'
    exit 1
fi

echo "$salida" | sed 's/^/   /'
echo "✅ stack HTTP de punta a punta OK"
