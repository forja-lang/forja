#!/usr/bin/env bash
# scripts/probar-redis.sh — e2e de std/redis contra un Redis real.
#
# Se saltea (exit 0) si no hay Redis escuchando: el CI sin docker no puede
# correrlo, pero en desarrollo/local sí.
#
# Uso:
#   bash scripts/probar-redis.sh
#   REDIS_HOST=127.0.0.1 REDIS_PORT=6379 FORJA=target/debug/forja bash scripts/probar-redis.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

REDIS_HOST="${REDIS_HOST:-127.0.0.1}"
REDIS_PORT="${REDIS_PORT:-6379}"

# ¿Hay algo escuchando? (bash /dev/tcp, sin dependencias)
if ! (exec 3<>"/dev/tcp/$REDIS_HOST/$REDIS_PORT") 2>/dev/null; then
    echo "· sin Redis en $REDIS_HOST:$REDIS_PORT — e2e salteado"
    exit 0
fi

FORJA="${FORJA:-}"
if [ -z "$FORJA" ]; then
    echo "▶ compilando forja-cli…"
    cargo build -q -p forja-cli --bin forja || exit 1
    FORJA="target/debug/forja"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/cliente.fa" <<'EOF'
importar "std/redis"
importar "std/tests/arnes"

variable r = redis.conectar(HOST_PLACEHOLDER, PUERTO_PLACEHOLDER)
comprobar_texto("PING", redis.ping(r), "PONG")

variable clave = "forja:test:contador"
redis.eliminar(r, clave)
comprobar_igual("INCR desde cero", redis.incrementar(r, clave), 1)
comprobar_igual("INCR dos veces", redis.incrementar(r, clave), 2)
comprobar("GET del contador", a_texto(redis.traer(r, clave)) == "2")

variable clave_texto = "forja:test:texto"
comprobar("SET texto", redis.fijar(r, clave_texto, "hola á"))
comprobar_texto("GET texto (UTF-8)", redis.traer(r, clave_texto), "hola á")
comprobar("EXISTS", redis.existe(r, clave_texto))

comprobar("EXPIRE", redis.expirar(r, clave_texto, 100))
variable ttl = redis.segundos_de_vida(r, clave_texto)
comprobar("TTL entre 1 y 100", ttl > 0 && ttl <= 100)

variable clave_hash = "forja:test:hash"
redis.eliminar(r, clave_hash)
comprobar("HSET", redis.hash_fijar(r, clave_hash, "nombre", "ana"))
variable hash = redis.hash_obtener_todo(r, clave_hash)
comprobar_texto("HGETALL nombre", hash["nombre"], "ana")
comprobar_texto("HGET campo", redis.hash_obtener(r, clave_hash, "nombre"), "ana")

variable clave_lista = "forja:test:lista"
redis.eliminar(r, clave_lista)
comprobar_igual("LPUSH", redis.lista_empujar(r, clave_lista, "trabajo"), 1)
comprobar_igual("LLEN", redis.lista_largo(r, clave_lista), 1)
comprobar_texto("RPOP", redis.lista_sacar(r, clave_lista), "trabajo")

comprobar_igual("DEL", redis.eliminar(r, clave_hash), 1)
comprobar("GET de clave borrada", redis.traer(r, clave_hash) == nulo)

# Comando crudo: la puerta de escape para lo que no está envuelto.
variable crudo = redis.comando_crudo(r, ["ECHO", "eco"])
comprobar_texto("comando crudo ECHO", crudo["valor"], "eco")

redis.eliminar(r, clave)
redis.desconectar(r)
resumen()
EOF

sed -i "s/HOST_PLACEHOLDER/\"$REDIS_HOST\"/; s/PUERTO_PLACEHOLDER/$REDIS_PORT/" "$TMP/cliente.fa"

echo "▶ probando std/redis en $REDIS_HOST:$REDIS_PORT"
salida="$("$FORJA" run "$TMP/cliente.fa" 2>&1)"
estado=$?

if [ $estado -ne 0 ]; then
    echo "❌ std/redis falló (exit $estado)"
    echo "$salida" | sed 's/^/   /'
    exit 1
fi

echo "$salida" | sed 's/^/   /'
echo "✅ std/redis OK"
