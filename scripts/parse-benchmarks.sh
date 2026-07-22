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

echo "📊 Benchmark Dashboard - Snapshot $SNAPSHOT_DATE"
echo "   Versión: $FORJA_VERSION, Commit: $COMMIT_HASH"
echo ""

# Array para almacenar todos los resultados
RESULTS='[]'

# Función para ejecutar un benchmark y parsear su output
run_benchmark() {
    local BENCH_NAME="$1"
    local BENCH_BIN="$2"
    
    echo "▶️  Ejecutando: $BENCH_NAME ($BENCH_BIN)..."
    
    # Ejecutar benchmark y capturar output
    local OUTPUT
    OUTPUT=$(cargo run --release --bin "$BENCH_BIN" 2>&1 || true)
    
    # Parsear líneas con formato: nombre  valor  unidad  ratio
    # Ejemplo: "  ForjaFast (hot)    1234.56 μs   4.80x ⚡"
    local BENCH_RESULTS='[]'
    
    while IFS= read -r line; do
        # Ignorar líneas vacías o de encabezado
        [[ -z "$line" ]] && continue
        [[ "$line" =~ ^#{1,} ]] && continue
        [[ "$line" =~ ^[[:space:]]*$ ]] && continue
        
        # Patrón: 2+ espacios separan nombre, valor, unidad, ratio
        if [[ "$line" =~ ^[[:space:]]{2}(.+)[[:space:]]{2,}([0-9]+\.[0-9]+)[[:space:]]*(μs|us)[[:space:]]+([0-9]+\.[0-9]*)x ]]; then
            local NAME="${BASH_REMATCH[1]}"
            local VALUE="${BASH_REMATCH[2]}"
            local UNIT="${BASH_REMATCH[3]}"
            local RATIO="${BASH_REMATCH[4]}"
            
            # Limpiar nombre (quitar espacios extras)
            NAME=$(echo "$NAME" | xargs)
            
            # Agregar a resultados
            BENCH_RESULTS=$(echo "$BENCH_RESULTS" | jq --arg name "$NAME" \
                --arg value "$VALUE" \
                --arg unit "$UNIT" \
                --arg ratio "$RATIO" \
                '. + [{"name": $name, "value": ($value | tonumber), "unit": $unit, "ratio": ($ratio | tonumber)}]')
        fi
    done <<< "$OUTPUT"
    
    echo "$BENCH_RESULTS"
}

# Ejecutar benchmarks (usar bench_forjafast como principal)
echo "  → Ejecutando bench_forjafast (principal)..."
FORJAFAST_RESULTS=$(run_benchmark "ForjaFast" "bench_forjafast")

echo "  → Ejecutando bench_jit..."
JIT_RESULTS=$(run_benchmark "JIT" "bench_jit")

echo "  → Ejecutando bench_vms..."
VMS_RESULTS=$(run_benchmark "VMs" "bench_vms")

echo ""
echo "✅ Benchmarks completados"

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

# Guardar snapshot individual
SNAPSHOT_FILE="$OUTPUT_DIR/snapshot-$(date -u +%Y%m%d-%H%M%S).json"
echo "$SNAPSHOT_JSON" > "$SNAPSHOT_FILE"
echo "📁 Snapshot guardado: $SNAPSHOT_FILE"

# Acumular historial: concatenar con historial existente (si existe)
HISTORY_FILE="$OUTPUT_DIR/history.json"
if [ -f "$HISTORY_FILE" ]; then
    # Agregar nuevo snapshot al array history
    jq --argjson snapshot "$SNAPSHOT_JSON" '. += [$snapshot]' "$HISTORY_FILE" > "${HISTORY_FILE}.tmp"
    mv "${HISTORY_FILE}.tmp" "$HISTORY_FILE"
    echo "📁 Historial actualizado: $HISTORY_FILE ($(jq length "$HISTORY_FILE") snapshots)"
else
    # Crear nuevo historial
    echo "[$SNAPSHOT_JSON]" > "$HISTORY_FILE"
    echo "📁 Historial creado: $HISTORY_FILE (1 snapshot)"
fi

# Copiar history.json a la carpeta de salida para deploy
cp "$HISTORY_FILE" "$OUTPUT_DIR/../history.json"
echo "✅ Dashboard data actualizado"
