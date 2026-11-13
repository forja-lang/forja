#!/usr/bin/env bash
# scripts/probar-demo-dom.sh — prueba la demo de GUI DOM en un navegador real.
#
# Levanta `demo-dom/` (node) y carga la página con Chrome/Edge headless, que
# ejecuta el wasm y el JS de verdad. Verifica que el DOM final tiene los nodos
# esperados y que no quedó ningún error en pantalla.
#
# Se saltea (exit 0) si no hay node o ningún navegador Chromium.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PAQUETE="demo-dom/pkg/forja_wasm_gui.js"
if [ ! -f "$PAQUETE" ]; then
    echo "▶ compilando wasm de la GUI DOM"
    (cd library/wasm-gui && RUSTC_WRAPPER= wasm-pack build --target web --release --no-opt) >/dev/null 2>&1 || {
        echo "· sin wasm (¿falta wasm-pack?): demo salteada"
        exit 0
    }
    mkdir -p demo-dom/pkg
    cp library/wasm-gui/pkg/forja_wasm_gui.js \
       library/wasm-gui/pkg/forja_wasm_gui_bg.wasm demo-dom/pkg/
fi

command -v node >/dev/null 2>&1 || { echo "· sin node: demo salteada"; exit 0; }

NAVEGADOR=""
for candidato in \
    "/c/Program Files/Google/Chrome/Application/chrome.exe" \
    "/c/Program Files (x86)/Google/Chrome/Application/chrome.exe" \
    "/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" \
    "/c/Program Files/Microsoft/Edge/Application/msedge.exe" \
    "$(command -v google-chrome 2>/dev/null)" \
    "$(command -v chromium 2>/dev/null)"; do
    if [ -n "$candidato" ] && [ -x "$candidato" ]; then
        NAVEGADOR="$candidato"
        break
    fi
done
[ -n "$NAVEGADOR" ] || { echo "· sin Chrome/Edge: demo salteada"; exit 0; }

PUERTO=$(( 8300 + ($$ % 300) ))
node demo-dom/servidor.mjs "$PUERTO" > /tmp/demo-dom.log 2>&1 &
SERVIDOR=$!
trap 'kill "$SERVIDOR" 2>/dev/null; rm -f /tmp/demo-dom.log' EXIT

for _ in $(seq 1 50); do
    if curl -s -o /dev/null "http://127.0.0.1:$PUERTO/" 2>/dev/null; then break; fi
    sleep 0.1
done

echo "▶ cargando la demo en el navegador headless (puerto $PUERTO)"
DOM="$("$NAVEGADOR" --headless=new --disable-gpu --no-sandbox --virtual-time-budget=20000 \
    --dump-dom "http://127.0.0.1:$PUERTO/" 2>/dev/null)"

fallo() { echo "❌ $1"; echo "$DOM" | tail -c 600 | sed 's/^/   /'; exit 1; }

echo "$DOM" | grep -q 'Renderizado como DOM real' || fallo "la página no reportó renderizado OK"
echo "$DOM" | grep -q 'class="fg-columna"' || fallo "no hay contenedor fg-columna"
echo "$DOM" | grep -q '<h1>Tareas</h1>' || fallo "no se renderizó el título"
echo "$DOM" | grep -q 'data-fg-variable="titulo"' || fallo "la entrada no está ligada a la variable"
echo "$DOM" | grep -q 'data-fg-accion="crear_tarea"' || fallo "el botón no trae su acción"
echo "$DOM" | grep -q 'label for="fg-titulo"' || fallo "falta la etiqueta accesible del campo"
# El diálogo existe pero arranca cerrado (se abre con la acción abrir:<id>).
echo "$DOM" | grep -q '<dialog id="fg-dialogo-confirmar"' || fallo "falta el diálogo"
echo "$DOM" | grep -q '<dialog id="fg-dialogo-confirmar"[^>]*open' && fallo "el diálogo arrancó abierto"

# Interacción real: clics que ejecutan el callback Forja (dos sumas) y un
# cambio de campo que escribe en la variable del programa.
echo "▶ probando interacción (callbacks + binding de ida)"
DOM_AUTO="$("$NAVEGADOR" --headless=new --disable-gpu --no-sandbox --virtual-time-budget=30000     --dump-dom "http://127.0.0.1:$PUERTO/?auto=1" 2>/dev/null)"

echo "$DOM_AUTO" | grep -q 'data-auto="listo"' || fallo "la simulación no terminó"
echo "$DOM_AUTO" | grep -q 'data-fg-variable="tareas">2<'     || fallo "los clics no ejecutaron el callback Forja (se esperaba tareas = 2)"
echo "$DOM_AUTO" | grep -q 'data-fg-variable="titulo">pan integral<'     || fallo "el cambio del campo no llegó a la variable del programa"

echo "$DOM_AUTO" | grep -q 'aria-selected="true" data-fg-accion="tab:pestana:detalle"'     || fallo "la pestaña no cambió al hacer clic"
echo "$DOM_AUTO" | grep -q '<dialog id="fg-dialogo-confirmar"[^>]*open'     || fallo "el diálogo no se abrió con la acción abrir:confirmar"
echo "$DOM_AUTO" | grep -q '<details class="fg-plegable" open'     || fallo "el bloque plegable no se renderizó abierto"
echo "$DOM_AUTO" | grep -q 'aria-selected="true" data-fg-accion="sel:vista:semana"'     || fallo "la selección de la lista no cambió (convención sel:)"
echo "$DOM_AUTO" | grep -q 'data-fg-variable="vista">semana<'     || fallo "la variable de la lista no tomó el valor elegido"

# Navegación por teclado de pestañas (flecha derecha). El patrón ARIA la exige.
echo "▶ probando teclado en pestañas"
DOM_KBD="$("$NAVEGADOR" --headless=new --disable-gpu --no-sandbox --virtual-time-budget=25000     --dump-dom "http://127.0.0.1:$PUERTO/?kbd=1" 2>/dev/null)"
echo "$DOM_KBD" | grep -q 'activa:tab:pestana:detalle'     || fallo "la flecha derecha no cambió de pestaña"

echo "   wasm cargado · DOM real · label/binding/acción presentes"
echo "   pestañas cambian (ARIA) · diálogo nativo se abre · plegable abierto"
echo "   lista con selección (sel:) actualiza la variable"
echo "   clics ejecutan callbacks Forja (0 → 2) · el campo escribe la variable"
echo "✅ demo de GUI DOM funciona en navegador"
