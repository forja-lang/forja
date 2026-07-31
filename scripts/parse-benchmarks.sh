#!/bin/bash
# scripts/parse-benchmarks.sh
# Ejecuta los benchmarks principales y genera datos históricos en JSON.
set -euo pipefail

OUTPUT_DIR="benchmarks/dashboard/data"
mkdir -p "$OUTPUT_DIR"

# Fecha ISO para este snapshot
SNAPSHOT_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
# Obtener commit hash y versión
COMMIT_HASH=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
FORJA_VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)

echo " Métricas de benchmarks - Snapshot $SNAPSHOT_DATE"
echo "   Versión: $FORJA_VERSION, Commit: $COMMIT_HASH"
echo ""

# Array para almacenar todos los resultados
RESULTS='[]'

# Función para ejecutar un benchmark y parsear su output.
# Garantiza devolver SIEMPRE un array JSON válido (nunca texto de error),
# para que el snapshot final no sea JSON inválido.
run_benchmark() {
    local BENCH_BIN="$1"

    # Los benchmarks están definidos como [[bench]] (harness = false), no como
    # [[bin]], así que se compilan con --bench y se ejecuta el binario resultante.
    local BENCH_EXE
    if [ -x "./target/release/$BENCH_BIN" ]; then
        BENCH_EXE="./target/release/$BENCH_BIN"
    elif [ -x "./target/release/${BENCH_BIN}.exe" ]; then
        BENCH_EXE="./target/release/${BENCH_BIN}.exe"
    else
        cargo build --release --bench "$BENCH_BIN" >/dev/null 2>&1 || true
        if [ -x "./target/release/$BENCH_BIN" ]; then
            BENCH_EXE="./target/release/$BENCH_BIN"
        elif [ -x "./target/release/${BENCH_BIN}.exe" ]; then
            BENCH_EXE="./target/release/${BENCH_BIN}.exe"
        else
            echo "[]"
            return 0
        fi
    fi

    local OUTPUT
    OUTPUT=$("$BENCH_EXE" 2>&1 || true)

    local BENCH_RESULTS='[]'

    while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        [[ "$line" =~ ^#{1,} ]] && continue
        [[ "$line" =~ ^[[:space:]]*$ ]] && continue

        local NAME="" VALUE="" UNIT="" RATIO=""

        # Pattern 1: bench-forjafast (cold, hot, ratio_vm, ratio_rust)
        # name is left-aligned in 22-char field: "  ForjaFast                1234.56   ..."
        if [[ "$line" =~ ^[[:space:]]{2}(.{22})[[:space:]]{2,}([0-9]+\.[0-9]+)[[:space:]]{2,}([0-9]+\.[0-9]+)[[:space:]]{2,}([0-9]+\.[0-9]+)x ]]; then
            NAME="${BASH_REMATCH[1]}"
            VALUE="${BASH_REMATCH[3]}"
            UNIT="μs"
            RATIO="${BASH_REMATCH[4]}"

        # Pattern 2: bench-jit (name, value, ratio)
        # "  ForjaFast (vm_fast)        123.45 us        4x ⚡⚡"
        elif [[ "$line" =~ ^[[:space:]]{2}(.+)[[:space:]]{2,}([0-9]+\.[0-9]+)[[:space:]]+(us|μs)[[:space:]]+([0-9]+(\.[0-9]+)?)x ]]; then
            NAME="${BASH_REMATCH[1]}"
            VALUE="${BASH_REMATCH[2]}"
            UNIT="${BASH_REMATCH[3]}"
            RATIO="${BASH_REMATCH[4]}"

        # Pattern 3: bench-vms (name, value, ratio in parens)
        # "  VM Original                      123.45 μs/iter  (4.80x) ⭐"
        elif [[ "$line" =~ ^[[:space:]]{2}(.+)[[:space:]]{2,}([0-9]+\.[0-9]+)[[:space:]]+(us|μs)/iter[[:space:]]+\(([0-9]+(\.[0-9]+)?)x\) ]]; then
            NAME="${BASH_REMATCH[1]}"
            VALUE="${BASH_REMATCH[2]}"
            UNIT="${BASH_REMATCH[3]}"
            RATIO="${BASH_REMATCH[4]}"
        fi

        if [[ -n "$NAME" ]]; then
            NAME=$(echo "$NAME" | xargs)
            # Validar que value/ratio sean numéricos antes de insertar.
            # Si jq falla (número no parseable), descartar esta línea en vez
            # de romper el JSON del snapshot.
            local NUEVO_RESULTADO
            NUEVO_RESULTADO=$(echo "$BENCH_RESULTS" | jq --arg name "$NAME" \
                --arg value "$VALUE" \
                --arg unit "$UNIT" \
                --arg ratio "$RATIO" \
                '. + [{"name": $name, "value": ($value | tonumber), "unit": $unit, "ratio": ($ratio | tonumber)}]' 2>/dev/null) || true
            if [[ -n "$NUEVO_RESULTADO" ]]; then
                BENCH_RESULTS="$NUEVO_RESULTADO"
            fi
        fi
    done <<< "$OUTPUT"

    # Fallback de seguridad: si por cualquier motivo no quedó JSON válido, usar []
    if ! echo "$BENCH_RESULTS" | jq empty >/dev/null 2>&1; then
        echo "[]"
        return 0
    fi
    echo "$BENCH_RESULTS"
}

# Ejecutar benchmarks
echo "  → Ejecutando bench-forjafast (principal)..."
FORJAFAST_RESULTS=$(run_benchmark "bench-forjafast")

echo "  → Ejecutando bench-jit..."
JIT_RESULTS=$(run_benchmark "bench-jit")

echo "  → Ejecutando bench-vms..."
VMS_RESULTS=$(run_benchmark "bench-vms")

echo ""
echo "✅ Benchmarks completados"

# Sanear cada resultado: si no es JSON válido, reemplazarlo con []
# (evita que el heredoc genere un snapshot con JSON inválido).
es_json_array() {
    echo "$1" | jq -e type == "array" >/dev/null 2>&1
}
if ! es_json_array "$FORJAFAST_RESULTS"; then FORJAFAST_RESULTS="[]"; fi
if ! es_json_array "$JIT_RESULTS"; then JIT_RESULTS="[]"; fi
if ! es_json_array "$VMS_RESULTS"; then VMS_RESULTS="[]"; fi

# Construir JSON completo con historial
SNAPSHOT_JSON=$(cat << SNAPSHOT_EOF
{
  "date": "$SNAPSHOT_DATE",
  "version": "$FORJA_VERSION",
  "commit": "$COMMIT_HASH",
  "benchmarks": {
    "forjafast": $FORJAFAST_RESULTS,
    "jit": $JIT_RESULTS,
    "vms": $VMS_RESULTS
  }
}
SNAPSHOT_EOF
)

# Validar el snapshot completo: si no es JSON válido, construir uno vacío
if ! echo "$SNAPSHOT_JSON" | jq empty >/dev/null 2>&1; then
    echo "⚠️  Snapshot JSON inválido; usando estructura vacía"
    SNAPSHOT_JSON=$(cat << SNAPSHOT_EOF
{
  "date": "$SNAPSHOT_DATE",
  "version": "$FORJA_VERSION",
  "commit": "$COMMIT_HASH",
  "benchmarks": {
    "forjafast": [],
    "jit": [],
    "vms": []
  }
}
SNAPSHOT_EOF
)
fi

# Guardar snapshot individual
SNAPSHOT_FILE="$OUTPUT_DIR/snapshot-$(date -u +%Y%m%d-%H%M%S).json"
echo "$SNAPSHOT_JSON" > "$SNAPSHOT_FILE"
echo "📁 Snapshot guardado: $SNAPSHOT_FILE"

# Acumular historial: concatenar con historial existente (si existe)
HISTORY_FILE="$OUTPUT_DIR/history.json"
HISTORY_OK=false
if [ -f "$HISTORY_FILE" ]; then
    # Verificar que el historial existente sea JSON válido (el caché de CI
    # puede contener un history.json corrupto de una corrida anterior).
    if jq empty "$HISTORY_FILE" >/dev/null 2>&1; then
        HISTORY_OK=true
    else
        echo "⚠️  history.json existente corrupto; recreándolo"
    fi
fi

if $HISTORY_OK; then
    # Agregar nuevo snapshot al array history
    if jq --argjson snapshot "$SNAPSHOT_JSON" '. += [$snapshot]' "$HISTORY_FILE" > "${HISTORY_FILE}.tmp" 2>/dev/null; then
        mv "${HISTORY_FILE}.tmp" "$HISTORY_FILE"
        echo "📁 Historial actualizado: $HISTORY_FILE ($(jq length "$HISTORY_FILE") snapshots)"
    else
        # Si la operación falló, recrear historial con este snapshot
        echo "[$SNAPSHOT_JSON]" > "$HISTORY_FILE"
        echo "📁 Historial recreado: $HISTORY_FILE (1 snapshot)"
    fi
else
    # Crear nuevo historial
    echo "[$SNAPSHOT_JSON]" > "$HISTORY_FILE"
    echo "📁 Historial creado: $HISTORY_FILE (1 snapshot)"
fi

# Copiar history.json a la carpeta de salida para deploy
cp "$HISTORY_FILE" "$OUTPUT_DIR/../history.json"
echo "✅ Dashboard data actualizado"
