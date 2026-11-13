#!/usr/bin/env bash
# scripts/probar-dom.sh — valida el HTML de `gui-dom` con un DOM real (jsdom).
#
# Los tests de Rust verifican el string; esto verifica que el navegador lo
# entienda: etiquetas vinculadas a sus campos, roles de accesibilidad, <dialog>,
# <progress>, <select> con su opción marcada, y que no quede marcado sin escapar.
#
# Se saltea (exit 0) si no hay node/npm: el resto de la suite no depende de esto.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
    echo "· sin node/npm: prueba de DOM salteada"
    exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "▶ generando la página de ejemplo"
cargo run -q -p forja-gui-dom --example pagina > "$TMP/pagina.html" 2>/dev/null || {
    echo "❌ el ejemplo no generó HTML"
    exit 1
}
[ -s "$TMP/pagina.html" ] || { echo "❌ página vacía"; exit 1; }

echo "▶ instalando jsdom (solo para esta prueba)"
cd "$TMP"
if ! npm i jsdom --silent --no-fund --no-audit > "$TMP/npm.log" 2>&1; then
    echo "· no se pudo instalar jsdom (¿sin red?): prueba de DOM salteada"
    tail -3 "$TMP/npm.log" | sed 's/^/   /'
    exit 0
fi

cat > "$TMP/verificar.mjs" <<'EOF'
import { readFileSync } from "node:fs";
import { JSDOM } from "jsdom";

const html = readFileSync(process.argv[2], "utf8");
const dom = new JSDOM(html);
const doc = dom.window.document;

const fallos = [];
const comprobar = (nombre, condicion) => {
  if (!condicion) fallos.push(nombre);
};

// Documento
comprobar("declara idioma", doc.documentElement.lang === "es");
comprobar("tiene título", (doc.title || "").length > 0);
comprobar("tiene viewport", doc.querySelector('meta[name="viewport"]') !== null);

// Etiquetas vinculadas a sus campos (accesibilidad).
const etiquetas = [...doc.querySelectorAll("label[for]")];
comprobar("hay etiquetas", etiquetas.length >= 4);
for (const label of etiquetas) {
  const id = label.getAttribute("for");
  comprobar(`la etiqueta '${id}' apunta a un control`, doc.getElementById(id) !== null);
}

// Controles con nombre y binding.
const controles = [...doc.querySelectorAll("input[name], select[name], textarea[name]")];
comprobar("hay controles con nombre", controles.length >= 4);
for (const control of controles) {
  comprobar(
    `el control '${control.getAttribute("name")}' tiene data-fg-variable`,
    control.getAttribute("data-fg-variable") === control.getAttribute("name")
  );
}
comprobar("el campo requerido lo declara", doc.querySelector("input[required]") !== null);

// Select con la opción marcada.
const select = doc.querySelector("select");
comprobar("el select tiene opciones", select && select.options.length === 2);
comprobar("el select marca la opción elegida", select && select.value === "baja");

// Semántica y roles.
comprobar("hay navegación", doc.querySelector("nav a") !== null);
comprobar("hay tabla con cabeceras", doc.querySelectorAll("table th").length === 2);
comprobar("el aviso informa su rol", doc.querySelector('[role="note"]') !== null);
comprobar("hay barra de progreso accesible", doc.querySelector("progress[aria-label]") !== null);
comprobar("hay diálogo nativo", doc.querySelector("dialog") !== null);
comprobar("el diálogo está cerrado", doc.querySelector("dialog").open === false);
comprobar("la imagen tiene alt", (doc.querySelector("img")?.getAttribute("alt") || "").length > 0);
comprobar("el botón trae su acción", doc.querySelector("button[data-fg-accion]") !== null);
comprobar("hay indicador de carga", doc.querySelector('[aria-busy="true"]') !== null);

// Nada de marcado del usuario sin escapar ni plantillas sin resolver.
comprobar("sin marcas de plantilla", !html.includes("{{"));
comprobar("sin scripts", doc.querySelector("script") === null);

if (fallos.length > 0) {
  console.error("❌ fallos de DOM:");
  for (const fallo of fallos) console.error("   -", fallo);
  process.exit(1);
}
console.log("   DOM válido: labels vinculadas, roles, dialog, progress, select, alt");
EOF

node "$TMP/verificar.mjs" "$TMP/pagina.html" || exit 1
echo "✅ HTML de gui-dom validado con un DOM real"
