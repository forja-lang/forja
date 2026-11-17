# HANDOFF — plan web + GUI (estado al cerrar)

> **Alcance**: pila web (HTTP/datos/sesiones/auth/plantillas/colas), GUI DOM y
> unificación de intérpretes. El **backend nativo/LLVM** tiene su propio
> handoff: `plans/handoff2.md` + `plans/TRASPASO-BACKEND-NATIVO.md`.

> Para quien siga: acá está **qué hay**, **cómo verificarlo**, **qué falta** y las
> trampas del entorno. El detalle de diseño vive en `plans/PLAN_WEB_Y_GUI.md`
> (plan web completo) y `plans/B16-UNIFICAR-EVALUADOR.md` (unificar intérpretes).
> Los dos documentos están al día; este es el resumen operativo.

---

## 1. Estado en una pantalla

Forja ya sirve **una página web real** con backend (HTTP + SQLite + sesiones +
auth + plantillas SSR) y una **GUI web sobre DOM** (no canvas), con un **solo
intérprete** compartido con la GUI nativa.

| Área | Estado |
|---|---|
| Servidor HTTP/1.1 (`std/servidor_web`) | ✅ keep-alive, lectura completa, timeouts, límites (413/431), router con grupos/comodín/405/`url_de`, estáticos con ETag/304/HEAD/403, formularios urlencoded, `/salud`+`/metricas`, cabeceras de seguridad, IP del cliente |
| Datos (`std/datos`) | ✅ migraciones idempotentes, transacciones con scope, helpers de consulta · ❌ pool real y mapeo fila→clase |
| Redis (`std/redis`) | ✅ cliente RESP completo · ❌ pool/reconexión, pub/sub bloqueante |
| Sesiones (`std/sesion`) | ✅ cookie firmada HMAC + tiempo constante, expiración, rotación, CSRF, store en memoria **y en Redis** |
| Auth (`std/auth`, `std/limitador`) | ✅ registro/login (PBKDF2), rate limit por usuario e IP, logout |
| Plantillas (`std/plantillas`) | ✅ escape de HTML, `{{x}}`/`{{{x}}}`, `{% si %}`/`{% sino %}`/`{% para %}` anidables |
| Vistas (`std/vista`) | ✅ HTML por funciones con a11y (`label/for`, `required`) |
| Colas (`std/trabajos`) | ✅ FIFO + reintentos + dead letter sobre Redis |
| GUI DOM (`library/gui-dom`) | ✅ 20+ widgets accesibles, estado/eventos, navegación por URL, tabla ordenable, lista paginada con render perezoso, formulario real |
| GUI web (`library/wasm-gui`) | ✅ DOM real (canvas eliminado), eventos delegados, teclado en pestañas |
| Intérprete (`compiler/forja-interprete`) | ✅ uno solo para GUI nativa y web; arreglos reales; clases (constructor/`este`/campos/métodos); nativos de SQLite/archivo/JSON delegados a `forja-native` |
| CLI | ✅ `forja web dev` (recarga), `forja web build` (paquete con ejecutable autónomo), `FORJA_PLANTILLA=web forja nuevo` |
| Demo pública | **https://forja-lang.github.io/forja-gui-dom/** |

---

## 2. Cómo verificar (comandos exactos)

```bash
# 1. Tests de comportamiento de la stdlib (VM) — 24 módulos
bash scripts/probar-stdlib.sh

# 2. E2E de la pila web (levantan servidores reales en puertos libres)
bash scripts/probar-http.sh        # keep-alive, 405, estáticos, 413, salud, métricas, CSP
bash scripts/probar-sesion.sh      # cookie firmada, cookie manipulada, CSRF, logout
bash scripts/probar-auth.sh        # registro/login/rate limit
bash scripts/probar-demo-web.sh    # demos/tareas-web completa (SQLite + sesión + CSRF)
bash scripts/probar-nuevo-web.sh   # FORJA_PLANTILLA=web
bash scripts/probar-web-dev.sh     # recarga al editar un .fa
bash scripts/probar-web-build.sh   # paquete dist/ + ejecutable autónomo
bash scripts/probar-dom.sh         # HTML de gui-dom validado con jsdom
bash scripts/probar-demo-dom.sh    # demo wasm en Chrome/Edge headless (?auto=1 y ?kbd=1)

# 3. Rust
cargo test -p forja-interprete                 # control de flujo, SQLite/archivo/JSON, arreglos, clases
cargo test -p forja-gui-rt --lib               # 34
cargo test -p forja-gui-rt --test paridad_vm   # 12, VM ↔ GUI, sin divergencias
cargo test -p forja-gui-dom                    # 29 + 3
cargo test -p forja --test bytecode_roundtrip  # 3
cargo test -p forja --test redis_cliente       # 1 (servidor RESP en proceso)
cargo test -p forja --test trabajos_cola       # 1
cargo test -p forja --test sesion_redis        # 1
cargo test -p forja-types --lib                 # 30

# 4. Demo local
bash scripts/servir-demo-dom.sh 8099     # compila el wasm si falta y sirve
# → http://127.0.0.1:8099/
```

`probar-redis.sh` se saltea sin Redis local (en CI corre si hay docker).

---

## 3. Trampas del entorno (me costaron tiempo)

1. **La stdlib va embebida en el binario** (`forja-registry/src/stdlib_embedded.rs`,
   `include_str!`). Cualquier cambio en `library/std/*.fa` **no se ve** hasta
   `cargo build -p forja-cli --bin forja`. Ídem `gui-dom` dentro de `wasm-gui`.
2. **`bg_run` ejecuta con cmd.exe**, no bash: `cd /c/...` falla. Usá
   `cd /d C:\...` o comandos sin pipes POSIX (`tail`, `grep` no existen ahí).
3. **wasm**: `RUSTC_WRAPPER= ` (vacío) para saltear sccache; con sccache, `web-sys`
   puede fallar. `wasm-pack build --target web --release --no-opt` y después
   `wasm-opt -Oz` (binaryen vía `npm i binaryen`; `scripts/construir-demo-dom.sh`
   lo resuelve).
4. **`plans/` y `docs/` están en `.gitignore`** del repo raíz: el plan y el sitio
   de documentación no se versionan ahí. Los cambios del playground
   (`docs/src/components/Playground.astro`) son locales.
5. **`library/*` son submódulos** (`std`, `gui-dom`, `gui-rt`, `wasm-gui`, …):
   commitear y pushear **dentro** de cada uno y después actualizar el puntero en
   la raíz (`git add library/x && git commit`).
6. **Verificación del navegador**: `--virtual-time-budget` congela
   `performance.now()` (mediciones dan 0). Para probar comportamiento sirve
   `--dump-dom`; para medir rendimiento, traza de DevTools.
7. **`wasm-gui` y `gui-rt` no se pueden ejecutar acá** (sin pantalla): lo que se
   verifica es `cargo check`, tests del intérprete y la demo wasm headless.
8. **El WIP del dueño** (`src/backend_llvm`, `installer/`, `packaging/`, …) a
   veces no compila y rompe `cargo test -p forja`. No es de este trabajo.

---

## 4. Convenciones del repo que hay que respetar

- **La stdlib vive en un espacio de nombres global** (N17): un módulo nuevo **no
  puede repetir un nombre de función** que ya exista en otro. Hay un test que lo
  bloquea (`tests/stdlib/frontera_nativos.rs`) con el inventario histórico.
- **Nunca llamar a una función con nombre de variable/parámetro igual a una de la
  stdlib** (N4): la global gana. Y `objeto.campo(args)` con un campo que guarda
  función devuelve nulo (N27): pasá la función por una variable.
- **Módulo nuevo** = `library/std/<nombre>.fa` + registro en
  `compiler/forja-registry/src/stdlib_embedded.rs` (const `include_str!` + entrada
  en `MODULOS`) + test en `library/std/tests/<nombre>.fa`.
- **Funciones públicas de módulos**: nombres únicos (ver `sesion.invalidar`,
  `redis.traer`, `plantillas.encabezado`) y **evitar builtins** (`obtener`,
  `longitud`, …).
- **Nombres reservados** que rompen el parseo: `canal`, `clase` (N24).
- Tests de stdlib corren con `forja run`, no `forja test` (N7: `forja test`
  transpila a Rust y no ve nativos).

---

## 5. Qué sigue, en orden, con el bloqueador concreto

### A. Pool de hilos en el servidor (N11) — *bloquea concurrencia real*
`hilo { }` **clona el montículo** (objetos incluidos) y limita cada hilo a
500k instrucciones. `std/compartido` sólo cubre escalares.
**Primer paso**: hacer que `hilo` comparta el montículo (o exponer un store por
proceso `compartido(mapa)` con `Arc<Mutex<...>>`); después, en `iniciar`, aceptar
y atender en hilos con un tope. Verificación: 50 requests concurrentes con
`hey`/`wrk` y estado compartido (contador en `/metricas`).

### B. TLS de servidor (`https`)
Hoy sólo hay TLS **cliente** (`_tls_conectar`). `rustls` ya está en el árbol.
**Primer paso**: nativo `_tls_aceptar(socket, cert_pem, key_pem)` + `_tls_recibir`/
`_tls_enviar` sobre la conexión aceptada; en `servidor_web`, un `habilitar_tls`.
Ojo: `_manejar_solicitud` usa `_socket_*` directo; hay que parametrizar el par
enviar/recibir. Verificación: `curl -k https://…`, `openssl s_client -alpn h2`.

### C. Estáticos grandes: `Range` y `gzip`
El servidor escribe texto; `Range` necesita cortes **por bytes** (hoy
`subcadena` es por caracteres y `longitud` por bytes, N23).
**Primer paso**: nativo `_socket_enviar_binario(socket, hex)` +
`_gzip_comprimir(texto)`; después `Range` con `_archivo_leer_binario` y
`Content-Range`. Verificación: `curl -H "Range: bytes=0-10"` y `Accept-Encoding`.

### D. Subida de archivos (`multipart/form-data`)
`SolicitudHTTP.archivos` se inicializa vacío y nunca se llena; el cuerpo se lee
como texto (pierde binarios).
**Primer paso**: lectura binaria del cuerpo + parser multipart que llene
`archivos[nombre] = {nombre, tipo, tamano, hex}`. Verificación: `curl -F`.

### E. Datos y Redis “de producción”
- `std/datos`: pool real (WAL + N conexiones) y mapeo fila→clase.
- `std/redis`: pool, reconexión y pub/sub con bucle.
- `std/sesion`: store en Redis ya existe; falta probarlo con Redis real en CI
  (`probar-redis.sh` + docker service).

### F. Fase 4 real del plan web (lo grande que queda)
- Renderer DOM **ya está**; falta **hidratación**: que el wasm adopte el HTML
  servido por SSR en vez de repintar.
- `forja web build` ya empaqueta; falta bundle de assets con hash y `forja web dev`
  con recarga de navegador (hoy reinicia el proceso).
- Unificar **también** con la VM (B16 quedó: GUI nativa + web comparten
  intérprete; la VM sigue con bytecode por diseño — ver D1 del plan B16).

### G. Deuda menor pero molesta
- **N22**: `mapa["k"](args)` se parsea como `anon` → `función no definida`.
- **N23**: `subcadena` por caracteres vs `longitud` por bytes.
- **N26**: ya resuelto (`solicitud.ip` es la IP del cliente); queda logs con
  request-id.
- `empujar` de un **campo** o **elemento de lista** no muta (sólo variables).
- `forja compilar` de un programa con `std/servidor_web`: el `.exe` corre bien
  (N29 arreglado), pero no hay verificación de AOT para la demo web.

---

## 6. Mapa de archivos tocados

**Crates nuevos**
- `compiler/forja-interprete/` — `valor.rs` (modelo de valores), `lib.rs`
  (evaluador + nativos + clases), `nativos_compartidos.rs` (puente a
  `forja-native`), `tests/sqlite.rs` (nativos, arreglos, clases).
- `library/gui-dom/` — submódulo propio (`forja-lang/forja-gui-dom`): `src/lib.rs`
  (nodos, render, estado, eventos), `estilos.css`, `tests/`, `examples/pagina.rs`.

**Stdlib nueva** (`library/std/`): `redis.fa`, `sesion.fa`, `auth.fa`,
`limitador.fa`, `plantillas.fa`, `vista.fa`, `trabajos.fa` + tests de cada uno;
`servidor_web.fa`, `datos.fa` y `crypto.fa` reescritos/ampliados.

**CLI** (`src/tools/forja-cli/src/commands/`): `web.rs` (`web dev`/`web build`),
`paquetes.rs` (plantilla web), `ayuda.rs`.

**Tests/e2e**: `tests/{redis_cliente,trabajos_cola,sesion_redis,bytecode_roundtrip}.rs`,
`tests/harness/src/resp.rs` (servidor RESP de prueba), `library/gui-rt/tests/paridad_vm.rs`,
`tests/stdlib/frontera_nativos.rs` (guard de nombres), `scripts/probar-*.sh`.

**Demo**: `demos/tareas-web/` (app completa), `demo-dom/` (demo wasm + servidor
node), `gh-pages` del repo `forja-gui-dom`.

**Docs**: `plans/PLAN_WEB_Y_GUI.md`, `plans/B16-UNIFICAR-EVALUADOR.md`,
`demos/tareas-web/README.md`, `library/std/README.md`.

---

## 7. Si solo tenés una hora

1. `bash scripts/probar-stdlib.sh` y `bash scripts/probar-http.sh` (5 min).
2. Leé `plans/B16-UNIFICAR-EVALUADOR.md` §2 (ya hecho) y §5 (qué no hacer).
3. Agarrá **A (pool de hilos)**: es lo único que separa esto de “dos usuarios a
   la vez”. Todo lo demás ya funciona de a uno.
