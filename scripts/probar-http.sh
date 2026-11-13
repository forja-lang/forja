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

mkdir -p "$TMP/estaticos"
cat > "$TMP/estaticos/index.html" <<'EOF'
<h1>hola estático</h1>
EOF
cat > "$TMP/estaticos/app.css" <<'EOF'
body { color: red }
EOF

cat > "$TMP/servidor.fa" <<EOF
importar "std/servidor_web"
importar "std/plantillas"

funcion main() {
    variable s = nuevo ServidorWeb($PUERTO)
    servidor_web.servir_estaticos(s, "/publico", "ESTATICOS_PLACEHOLDER")
    servidor_web.habilitar_salud(s)
    servidor_web.habilitar_seguridad(s, falso)
    servidor_web.fijar_max_cuerpo(s, 2048)
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
    servidor_web.registrar_obtener(s, "/pagina", func(peticion, respuesta) {
        variable pagina = plantillas.interpolar(
            "<h1>{{t}}</h1><ul>{% para x en xs %}<li>{{x}}</li>{% fin %}</ul>",
            {"t": peticion.parametros["t"], "xs": ["uno", "dos"]}
        )
        retornar servidor_web.html(respuesta, pagina)
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

# El path del directorio va DENTRO del .fa, así que en Windows/mingw hay que
# pasarlo en formato nativo (la conversión automática de argumentos no aplica).
EST_DIR="$TMP/estaticos"
if command -v cygpath >/dev/null 2>&1; then
    EST_DIR="$(cygpath -m "$EST_DIR")"
fi
sed -i "s#ESTATICOS_PLACEHOLDER#$EST_DIR#" "$TMP/servidor.fa"
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

# ─── Keep-alive ────────────────────────────────────────────────────
# Dos requests en la misma invocación de curl: la segunda debe reutilizar la
# conexión (`num_connects` = 0). Se saltea si no hay curl.
if comando_curl="$(command -v curl)" && [ -n "$comando_curl" ]; then
    echo "▶ keep-alive con curl"
    conexiones="$(curl -s -o /dev/null -o /dev/null -w '%{num_connects}\n' \
        "http://127.0.0.1:$PUERTO/texto" "http://127.0.0.1:$PUERTO/texto" 2>/dev/null | tr -d '\r')"
    segunda="$(echo "$conexiones" | sed -n 2p)"
    if [ "$segunda" != "0" ]; then
        echo "❌ keep-alive: la segunda request abrió otra conexión (num_connects=$segunda)"
        echo "   salida de curl: $(echo "$conexiones" | tr '\n' ' ')"
        exit 1
    fi
    echo "   segunda request reutilizó la conexión"

    cabecera_conexion="$(curl -s -D - -o /dev/null "http://127.0.0.1:$PUERTO/texto" | tr -d '\r' | grep -i '^connection:' | head -n 1 || true)"
    if ! echo "$cabecera_conexion" | grep -qi 'keep-alive'; then
        echo "❌ keep-alive: la respuesta HTTP/1.1 no anuncia keep-alive ($cabecera_conexion)"
        exit 1
    fi
    echo "   respuesta anuncia: $cabecera_conexion"

    # 405: la ruta existe pero con otro método, y la respuesta dice cuáles.
    codigo_405="$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "http://127.0.0.1:$PUERTO/texto")"
    if [ "$codigo_405" != "405" ]; then
        echo "❌ 405: DELETE /texto devolvió $codigo_405 (se esperaba 405)"
        exit 1
    fi
    allow="$(curl -s -D - -o /dev/null -X DELETE "http://127.0.0.1:$PUERTO/texto" | tr -d '\r' | grep -i '^allow:' | head -n 1)"
    if ! echo "$allow" | grep -qi 'GET'; then
        echo "❌ 405: la respuesta no anuncia Allow con GET ($allow)"
        exit 1
    fi
    echo "   405 con: $allow"

    # ─── Estáticos ────────────────────────────────────────────────
    cuerpo_est="$(curl -s "http://127.0.0.1:$PUERTO/publico/")"
    if ! echo "$cuerpo_est" | grep -q 'hola estático'; then
        echo "❌ estáticos: /publico/ no devolvió index.html ($cuerpo_est)"
        exit 1
    fi
    etag="$(curl -s -D - -o /dev/null "http://127.0.0.1:$PUERTO/publico/app.css" | tr -d '\r' | grep -i '^etag:' | cut -d' ' -f2-)"
    if [ -z "$etag" ]; then
        echo "❌ estáticos: la respuesta no trae ETag"
        exit 1
    fi
    codigo_304="$(curl -s -o /dev/null -w '%{http_code}' -H "If-None-Match: $etag" "http://127.0.0.1:$PUERTO/publico/app.css")"
    if [ "$codigo_304" != "304" ]; then
        echo "❌ estáticos: If-None-Match devolvió $codigo_304 (se esperaba 304)"
        exit 1
    fi
    mime_css="$(curl -s -D - -o /dev/null "http://127.0.0.1:$PUERTO/publico/app.css" | tr -d '\r' | grep -i '^content-type:' | head -n 1)"
    if ! echo "$mime_css" | grep -qi 'text/css'; then
        echo "❌ estáticos: Content-Type incorrecto ($mime_css)"
        exit 1
    fi
    codigo_traversal="$(curl --path-as-is -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PUERTO/publico/../servidor.fa")"
    if [ "$codigo_traversal" != "403" ]; then
        echo "❌ estáticos: traversal devolvió $codigo_traversal (se esperaba 403)"
        exit 1
    fi
    largo_head="$(curl -sI "http://127.0.0.1:$PUERTO/publico/app.css" | tr -d '\r' | grep -i '^content-length:' | cut -d' ' -f2-)"
    largo_real="$(wc -c < "$TMP/estaticos/app.css" | tr -d ' ')"
    if [ "$largo_head" != "$largo_real" ]; then
        echo "❌ estáticos: HEAD devolvió Content-Length=$largo_head (se esperaba $largo_real)"
        exit 1
    fi
    echo "   estáticos: 200/ETag/304/403/HEAD OK"

    # ─── Salud y métricas ────────────────────────────────────────
    salud="$(curl -s "http://127.0.0.1:$PUERTO/salud")"
    if ! echo "$salud" | grep -q '"estado":"ok"'; then
        echo "❌ /salud no devolvió estado ok ($salud)"
        exit 1
    fi
    metricas="$(curl -s "http://127.0.0.1:$PUERTO/metricas")"
    if ! echo "$metricas" | grep -q '"200"'; then
        echo "❌ /metricas no cuenta los 200 ($metricas)"
        exit 1
    fi
    if ! grep -q ' ms, #' "$TMP/servidor.log"; then
        echo "❌ el log por request no incluye latencia en ms"
        tail -n 5 "$TMP/servidor.log" | sed 's/^/   /'
        exit 1
    fi
    echo "   salud: $salud"
    echo "   métricas: $metricas"

    # ─── Cabeceras de seguridad ─────────────────────────────────
    seguridad="$(curl -s -D - -o /dev/null "http://127.0.0.1:$PUERTO/texto" | tr -d '\r')"
    if ! echo "$seguridad" | grep -qi '^x-content-type-options: nosniff'; then
        echo "❌ faltan cabeceras de seguridad en la respuesta"
        echo "$seguridad" | sed 's/^/   /'
        exit 1
    fi
    if ! echo "$seguridad" | grep -qi '^content-security-policy:'; then
        echo "❌ falta Content-Security-Policy"
        exit 1
    fi
    echo "   seguridad: nosniff + CSP"

    # ─── Límite de cuerpo: 413 en vez de cerrar en silencio ────
    grande="$(head -c 5000 /dev/zero | tr '\0' 'a')"
    codigo_413="$(curl -s -o /dev/null -w '%{http_code}' -X POST --data-binary "$grande" "http://127.0.0.1:$PUERTO/eco")"
    if [ "$codigo_413" != "413" ]; then
        echo "❌ cuerpo de 5000 bytes con límite 2048 devolvió $codigo_413 (se esperaba 413)"
        exit 1
    fi
    echo "   413 cuando el cuerpo supera el límite"

    # ─── Plantillas / SSR ───────────────────────────────────────
    pagina="$(curl -s "http://127.0.0.1:$PUERTO/pagina?t=%3Cscript%3Ealert(1)%3C/script%3E")"
    if ! echo "$pagina" | grep -q '&lt;script&gt;'; then
        echo "❌ la plantilla no escapó el HTML del parámetro ($pagina)"
        exit 1
    fi
    if echo "$pagina" | grep -q '<script>'; then
        echo "❌ XSS: el parámetro salió sin escapar"
        exit 1
    fi
    if ! echo "$pagina" | grep -q '<li>uno</li><li>dos</li>'; then
        echo "❌ la plantilla no recorrió la lista ($pagina)"
        exit 1
    fi
    echo "   plantillas: HTML escapado (sin XSS) y bucle {% para %} OK"
else
    echo "· sin curl: se saltea la prueba de keep-alive"
fi

echo "✅ stack HTTP de punta a punta OK"
