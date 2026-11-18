#!/usr/bin/env bash
# scripts/paridad-native-vm.sh
#
# Compara, ejemplo por ejemplo, la salida de la ruta **nativa** (`forja aot`)
# con la de la **VM** (`forja ejecutar`) sobre `ejemplos/`.
#
# Uso:
#   scripts/paridad-native-vm.sh              # informe + falla si hay diferencias nuevas
#   scripts/paridad-native-vm.sh --verbose    # muestra también los casos que coinciden
#   scripts/paridad-native-vm.sh --update     # agrega a `scripts/paridad-exclusiones` lo nuevo
#
# Cómo leerlo:
#   - SALTADO : el ejemplo no baja a nativo (constructo sin soporte). No es un fallo.
#   - TIEMPO  : alguno de los dos caminos no termina en TIMEOUT segundos (juegos que
#               piden entrada, etc.).
#   - DIFIERE : los dos corren y la salida no coincide. Es lo único que falla, y
#               sólo si el ejemplo **no** está en la lista de exclusiones.
#
# `scripts/paridad-exclusiones` documenta las diferencias aceptadas (una por
# línea: `ruta/ejemplo.fa  motivo`). Cualquier DIFIERE que no esté ahí es un
# regresión.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FORJA="${FORJA:-$ROOT/target/release/forja$( [ "$(uname -s)" != "Linux" ] && echo .exe )}"
TIMEOUT="${TIMEOUT:-20}"
OUT="$ROOT/target/paridad"
EXCLUSIONES="$ROOT/scripts/paridad-exclusiones"
VERBOSE=0
ACTUALIZAR=0
for arg in "$@"; do
    case "$arg" in
        --verbose) VERBOSE=1 ;;
        --update) ACTUALIZAR=1 ;;
        -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
        *) echo "argumento desconocido: $arg" >&2; exit 2 ;;
    esac
done

if [ ! -x "$FORJA" ]; then
    echo "❌ Falta $FORJA. Compilá con: cargo build --release -p forja-cli" >&2
    exit 2
fi
if ! command -v timeout >/dev/null 2>&1; then
    echo "❌ Hace falta 'timeout' (coreutils)." >&2
    exit 2
fi

# Cabecera de la corrida: los números sólo son comparables con el mismo árbol,
# el mismo commit y el mismo binario. Media hora perdida una vez por medir con un
# `forja.exe` que no era el que se creía (handoff5 §3.4).
echo "árbol:   $(git rev-parse --short HEAD 2>/dev/null || echo '?') $(git log --oneline -1 2>/dev/null | cut -d' ' -f2- | cut -c1-60)"
sucios="$(git status --short 2>/dev/null | wc -l | tr -d ' ')"
echo "sucio:   $sucios archivo(s) sin commitear"
echo "ejemplos: $(find ejemplos -name '*.fa' | wc -l | tr -d ' ')"

rm -rf "$OUT"
mkdir -p "$OUT"

# Copia del binario: si otra sesión corre `cargo build` mientras se mide, cargo
# reemplaza `target/release/forja.exe` y la mitad de los ejemplos fallan con
# "No such file or directory" (pasó: 122 de 175 "no bajan" eran esto). Se mide
# siempre contra esta copia, que además es la que dice la cabecera.
BIN="$OUT/forja"
cp "$FORJA" "$BIN"
chmod +x "$BIN" 2>/dev/null
echo "binario: $FORJA ($(date -r "$FORJA" '+%Y-%m-%d %H:%M' 2>/dev/null || echo '?')) -> copia en $BIN"
FORJA="$BIN"

norm() { sed -e 's/\r$//' "$1"; }

excluido() {
    [ -f "$EXCLUSIONES" ] || return 1
    # Coincide por ruta exacta al principio de la línea.
    grep -qE "^$1(\s|$)" "$EXCLUSIONES"
}

coinciden=0
saltados=0
tiempos=0
errores=0
crash=0
choques=()
difieren=0
fallos=()

for f in $(find ejemplos -name '*.fa' | sort); do
    n="$(echo "$f" | tr '/' '_' | sed 's/\.fa$//')"
    nat="$OUT/$n.nat"
    vm="$OUT/$n.vm"
    # `forja aot` cae a bytecode en silencio y sale con 0: el único indicio es el
    # mensaje. Si no compiló nativo, el ejemplo no entra en la comparación.
    "$FORJA" aot "$f" "$OUT/$n" >"$OUT/$n.aot" 2>&1
    if ! grep -q "AOT nativo:" "$OUT/$n.aot"; then
        saltados=$((saltados + 1))
        continue
    fi
    if [ ! -x "$OUT/$n" ] && [ ! -x "$OUT/$n.exe" ]; then
        saltados=$((saltados + 1))
        continue
    fi
    exe="$OUT/$n"
    [ -x "$exe.exe" ] && exe="$exe.exe"
    timeout "$TIMEOUT" "$exe" >"$nat" 2>&1
    rc=$?
    if [ "$rc" -ne 0 ]; then
        if [ "$rc" -eq 124 ]; then
            tiempos=$((tiempos + 1))
            [ "$VERBOSE" = 1 ] && echo "no termina (nativo) $f"
        elif [ "$rc" -ge 128 ]; then
            # Crash (segfault & cía): eso nunca es aceptable, ni siquiera con
            # «nunca mentir» (el error debe ser explícito, no una caída).
            crash=$((crash + 1))
            choques+=("$f (señal $((rc - 128)))")
            echo "CRASH     $f (señal $((rc - 128)))"
        else
            errores=$((errores + 1))
            [ "$VERBOSE" = 1 ] && echo "error explícito     $f"
        fi
        continue
    fi
    timeout "$TIMEOUT" "$FORJA" ejecutar "$f" >"$vm" 2>&1
    rc=$?
    if [ "$rc" -ne 0 ]; then
        if [ "$rc" -eq 124 ]; then
            tiempos=$((tiempos + 1))
            [ "$VERBOSE" = 1 ] && echo "no termina (VM)     $f"
        else
            errores=$((errores + 1))
            [ "$VERBOSE" = 1 ] && echo "error (VM)          $f"
        fi
        continue
    fi
    if diff -q <(norm "$nat") <(norm "$vm") >/dev/null; then
        coinciden=$((coinciden + 1))
        [ "$VERBOSE" = 1 ] && echo "igual     $f"
        continue
    fi
    if excluido "$f"; then
        [ "$VERBOSE" = 1 ] && echo "excluido  $f"
        continue
    fi
    difieren=$((difieren + 1))
    fallos+=("$f")
    echo "DIFIERE   $f"
    diff <(norm "$nat") <(norm "$vm") | head -6 | sed 's/^/          /'
done

echo
echo "nativo vs VM: $coinciden coinciden · $difieren difieren (sin excluir) · $saltados no bajan a nativo · $tiempos no terminan en ${TIMEOUT}s · $errores salen con error explícito · $crash revientan"

if [ "$ACTUALIZAR" = 1 ]; then
    # Sólo agrega lo que falta: los motivos ya medidos de las exclusiones
    # existentes se conservan (antes se borraban y quedaban rutas peladas).
    [ -f "$EXCLUSIONES" ] || {
        echo "# Diferencias ACEPTADAS entre la ruta nativa y la VM (una por línea: ruta + motivo)." >"$EXCLUSIONES"
        echo "# Cualquier otra diferencia hace fallar a scripts/paridad-native-vm.sh." >>"$EXCLUSIONES"
    }
    nuevas=0
    for f in "${fallos[@]:-}"; do
        [ -z "$f" ] && continue
        excluido "$f" && continue
        echo "$f  TODO: motivo" >>"$EXCLUSIONES"
        nuevas=$((nuevas + 1))
    done
    echo "✅ $nuevas exclusión(es) nueva(s) en $EXCLUSIONES (las existentes no se tocaron; completá los TODO)"
    exit 0
fi

if [ "$crash" -gt 0 ]; then
    echo
    echo "Falla: $crash ejemplo(s) revientan en la ruta nativa (un error explícito es aceptable, una caída no):" >&2
    for f in "${choques[@]}"; do echo "  - $f" >&2; done
fi

if [ "$difieren" -gt 0 ]; then
    echo
    echo "Falla: ${difieren} ejemplo(s) difieren sin estar en $EXCLUSIONES" >&2
    for f in "${fallos[@]}"; do echo "  - $f" >&2; done
    exit 1
fi
if [ "$crash" -gt 0 ]; then
    exit 1
fi
echo "✅ paridad: ningún ejemplo difiere fuera de las exclusiones ni revienta"
