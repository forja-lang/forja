// Servidor estático mínimo para la demo (node, sin dependencias).
//
// Sirve este directorio con el MIME correcto — en particular
// `application/wasm`, que hace que el navegador use instantiateStreaming.
//
//   node demo-dom/servidor.mjs [puerto]
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = fileURLToPath(new URL(".", import.meta.url));
const PUERTO = Number(process.argv[2] || 8099);

const TIPOS = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".wasm": "application/wasm",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
};

createServer(async (req, res) => {
  const url = new URL(req.url, "http://localhost");
  let rel = decodeURIComponent(url.pathname);
  if (rel.endsWith("/")) rel += "index.html";
  const ruta = normalize(join(RAIZ, rel));
  if (!ruta.startsWith(RAIZ)) {
    res.writeHead(403).end("prohibido");
    return;
  }
  try {
    const info = await stat(ruta);
    if (!info.isFile()) throw new Error("no es archivo");
    const datos = await readFile(ruta);
    res.writeHead(200, {
      "Content-Type": TIPOS[extname(ruta).toLowerCase()] || "application/octet-stream",
      "Content-Length": datos.length,
      "Cache-Control": "no-store",
    });
    res.end(datos);
  } catch {
    res.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
    res.end(`no encontrado: ${rel}`);
  }
}).listen(PUERTO, "127.0.0.1", () => {
  console.log(`▶ demo de GUI DOM en http://127.0.0.1:${PUERTO}/`);
});
