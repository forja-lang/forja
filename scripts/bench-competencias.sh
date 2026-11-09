#!/usr/bin/env bash
# scripts/bench-competencias.sh
#
# Mide los 10 programas de `benches/competencias/` en Forja nativo (`forja aot`)
# y en Rust (`rustc -O`), e imprime la tabla de ratios.
#
# Uso:
#   scripts/bench-competencias.sh                 # mide y compara con la baseline
#   scripts/bench-competencias.sh --update        # regenera la baseline
#   REPETICIONES=5 scripts/bench-competencias.sh  # corridas por caso (default 3)
#
# Sale con error si un caso no compila en Forja o si un ratio empeora más del
# 20 % respecto de `benches/competencias/baseline.json`.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DIR="benches/competencias"
FORJA="${FORJA:-$ROOT/target/release/forja$( [ "$(uname -s)" != "Linux" ] && echo .exe )}"
REPS="${REPETICIONES:-3}"
BASE="$DIR/baseline.json"
OUT="$DIR/../..//target/bench-competencias"
mkdir -p "$OUT"

ACTUALIZAR=0
[ "${1:-}" = "--update" ] && ACTUALIZAR=1

PYTHON="${PYTHON:-python3}"
command -v "$PYTHON" >/dev/null 2>&1 || PYTHON=python

if [ ! -x "$FORJA" ]; then
    echo "❌ Falta $FORJA. Compilá con: cargo build --release -p forja-cli" >&2
    exit 2
fi

# ms de pared, mínimo de N corridas
medir() {
    local mejor=999999
    for _ in $(seq "$REPS"); do
        local t0 t1
        t0=$(date +%s%N)
        "$@" > /dev/null 2>&1
        t1=$(date +%s%N)
        local ms=$(( (t1 - t0) / 1000000 ))
        [ "$ms" -lt "$mejor" ] && mejor=$ms
    done
    echo "$mejor"
}

# caso | fuente forja | fuente rust
CASOS=(
  "ffi|$DIR/ffi.fa|$DIR/ffi.rs"
  "dispatch|$DIR/dispatch.fa|$DIR/rust_escala/dispatch.rs"
  "parseo|$DIR/parseo.fa|$DIR/parseo.rs"
  "memcache|$DIR/memcache_repetir.fa|$DIR/rust_escala/memcache.rs"
  "vectores|$DIR/vectores.fa|$DIR/vectores.rs"
  "overflowpanic|$DIR/overflowpanic.fa|$DIR/rust_escala/overflowpanic.rs"
  "hash|$DIR/hash.fa|$DIR/hash.rs"
  "erroresanidados|$DIR/erroresanidados.fa|$DIR/erroresanidados.rs"
  "concurrencia|$DIR/concurrencia.fa|$DIR/rust_escala/concurrencia.rs"
  "stress|$DIR/stress5m.fa|$DIR/stress.rs"
)

echo "Forja: $FORJA"
echo "Repeticiones: $REPS (se toma el mínimo)"
echo ""
printf '%-18s %12s %12s %10s\n' "caso" "forja (ms)" "rust (ms)" "ratio"

RESULTADO="$OUT/resultados.tsv"
: > "$RESULTADO"
fallo=0

for fila in "${CASOS[@]}"; do
    IFS='|' read -r caso fsrc rsrc <<< "$fila"
    if ! "$FORJA" aot "$fsrc" "$OUT/$caso" > "$OUT/$caso.build.log" 2>&1; then
        echo "❌ $caso: Forja no compiló (ver $OUT/$caso.build.log)" >&2
        tail -3 "$OUT/$caso.build.log" >&2
        fallo=1
        continue
    fi
    if ! rustc -O -o "$OUT/$caso.rust" "$rsrc" > "$OUT/$caso.rust.log" 2>&1; then
        echo "❌ $caso: rustc falló" >&2
        fallo=1
        continue
    fi
    tf=$(medir "$OUT/$caso")
    tr=$(medir "$OUT/$caso.rust")
    ratio=$("$PYTHON" -c "print(f'{$tf / max($tr, 1):.2f}')")
    printf '%-18s %12s %12s %9sx\n' "$caso" "$tf" "$tr" "$ratio"
    printf '%s\t%s\t%s\t%s\n' "$caso" "$tf" "$tr" "$ratio" >> "$RESULTADO"
done

if [ "$fallo" -ne 0 ]; then
    echo ""
    echo "❌ Algunos casos no compilaron." >&2
    exit 1
fi

if [ "$ACTUALIZAR" -eq 1 ]; then
    "$PYTHON" - "$RESULTADO" "$BASE" <<'PY'
import json, sys
filas = {}
for linea in open(sys.argv[1], encoding="utf-8"):
    caso, tf, tr, ratio = linea.split()
    filas[caso] = {"forja_ms": int(tf), "rust_ms": int(tr), "ratio": float(ratio)}
with open(sys.argv[2], "w", encoding="utf-8") as f:
    json.dump(
        {
            "nota": "Ratio = forja/rust (pared, mínimo de N). Regenerar con "
                    "scripts/bench-competencias.sh --update",
            "casos": filas,
        },
        f,
        indent=2,
        sort_keys=True,
    )
    f.write("\n")
print(f"✅ baseline actualizada: {sys.argv[2]}")
PY
    exit 0
fi

if [ -f "$BASE" ]; then
    "$PYTHON" - "$RESULTADO" "$BASE" <<'PY'
import json, sys
umbral = 1.20  # 20 % de margen por ruido de máquina
base = json.load(open(sys.argv[2], encoding="utf-8"))["casos"]
peor = 0
for linea in open(sys.argv[1], encoding="utf-8"):
    caso, _tf, _tr, ratio = linea.split()
    esperado = base.get(caso, {}).get("ratio")
    if esperado is None:
        continue
    ratio = float(ratio)
    if ratio > esperado * umbral:
        print(f"⚠️  {caso}: ratio {ratio:.2f}x vs baseline {esperado:.2f}x")
        peor = 1
print("✅ ratios dentro de la baseline" if peor == 0 else "❌ regresión de rendimiento")
sys.exit(peor)
PY
    exit $?
fi

echo ""
echo "(sin baseline todavía: generá una con --update)"
