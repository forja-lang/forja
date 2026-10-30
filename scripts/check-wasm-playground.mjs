#!/usr/bin/env node
// scripts/check-wasm-playground.mjs — valida el par glue+wasm que publica el
// playground, sin navegador.
//
// Detecta el fallo que rompió el playground (v0.9.5): la glue generada por un
// wasm-bindgen distinto al que compiló el .wasm explota en la init con
// "WebAssembly.Table.grow(): failed to grow table by 4". Acá se instancia
// cada par de verdad, así que un desajuste se ve antes de publicar.
//
// Uso: node scripts/check-wasm-playground.mjs <dir con los pares>
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';

const dir = path.resolve(process.argv[2] ?? 'docs/src/wasm');

// La glue usa `fetch` sobre una URL relativa al módulo: en Node la servimos
// desde disco.
globalThis.fetch = async (url) => {
  const p = url instanceof URL ? fileURLToPath(url) : String(url);
  return new Response(readFileSync(p), { headers: { 'Content-Type': 'application/wasm' } });
};

const pares = [
  { nombre: 'forja_wasm', prueba: (m) => m.forja_ejecutar('escribir("hola")') === 'hola' },
  // El runtime GUI necesita DOM para renderizar; la init ya delata el desajuste.
  { nombre: 'forja_wasm_gui', prueba: () => true },
];

let fallos = 0;
for (const { nombre, prueba } of pares) {
  try {
    const modulo = await import(pathToFileURL(path.join(dir, `${nombre}.js`)).href);
    await modulo.default();
    if (!prueba(modulo)) throw new Error('la ejecución de prueba no dio el resultado esperado');
    console.log(`✅ ${nombre}`);
  } catch (e) {
    fallos++;
    console.error(`❌ ${nombre}: ${e.message}`);
  }
}
process.exit(fallos === 0 ? 0 : 1);
