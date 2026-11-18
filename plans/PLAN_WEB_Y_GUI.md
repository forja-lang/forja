# Plan: Forja con Web y GUI — de lo que hay a una página real

> Objetivo del documento: responder con evidencia **qué falta en Forja y en la GUI** para poder
> construir una página web real y funcional (backend + BD + Redis + frontend) y qué hay que
> mejorar para que eso no sea un prototipo.
>
> **Para retomar rápido**: `plans/HANDOFF-WEB-Y-GUI.md` (estado, comandos de
> verificación, trampas del entorno y qué sigue con sus bloqueadores).
>
> Fecha de medición: sobre `main` con la españolización cerrada (inventario de nombres = 0).
> Todo lo que dice "verificado" se ejecutó en esta máquina; el anexo §10 tiene los comandos.

---

## 0. Veredicto en 30 segundos

1. **Forja como lenguaje y backend está mucho más cerca de lo que parece**: hay sockets, HTTP/1.1,
   HTTP/2 (h2c), WebSocket, TLS cliente, SQLite con parámetros, JSON, TOML, CSV, hashing
   (`bcrypt`/`pbkdf2`/`scrypt`), hilos reales con canales, registry de paquetes y `.exe` nativo.
2. **Pero la capa HTTP de la stdlib no arranca**: `std/servidor_web` (y `cliente_http`,
   `cliente_h2`, `servidor_h2`) llaman a `claves_mapa()`, que **no existe**. Verificado: el
   servidor muere en la primera petición con `función no definida: 'claves_mapa'`.
3. **El servidor, cuando funcione, no sirve para producción**: un hilo, una conexión a la vez,
   una sola lectura de 8 KiB por request (cuerpo truncado), `Connection: close` siempre, sin
   timeouts, sin TLS de servidor, sin pool.
4. **No hay capa de datos**: SQLite existe pero sin pool, sin migraciones, sin ORM/query builder
   tipado. **Redis no existe** (ni cliente, ni caché, ni colas).
5. **La GUI web no es una web**: `wasm-gui` dibuja en un `<canvas>` y soporta **10 de las 115**
   variantes de layout de la GUI nativa; clic sí, teclado **no** (clickear un `entrada_texto`
   escribe en la consola del navegador). Sin DOM, sin CSS, sin HTML semántico, sin accesibilidad,
   sin SEO, sin scroll, sin resize.
6. **Hay tres intérpretes**: la VM (`forja-vm`, 206 nativos), la GUI nativa
   (`gui-rt/src/evaluador.rs`, ~60 nativos) y la GUI web (≈10 construcciones + `escribir`).
   Cada una implementa un subconjunto distinto del lenguaje.
7. **La stdlib no tiene tests de comportamiento**: 47 módulos / 7.363 líneas; el único test
   ("smoke") verifica que *importen*, no que funcionen. Por eso el bug de `claves_mapa` pasó.

**Conclusión**: para una web real falta, en orden: (a) arreglar y endurecer el servidor HTTP,
(b) capa de datos + Redis, (c) sesiones/auth/seguridad, (d) una UI web basada en DOM (o SSR)
que reemplace el canvas, (e) dev server + deploy, (f) red de tests de stdlib para que nada de
esto se vuelva a romper en silencio y (g) un solo intérprete compartido.

---

## 0-bis. Estado de avance (se actualiza a medida que se ejecuta)

### Hecho ✅

| # | Qué | Evidencia |
|---|---|---|
| **B1** | El stack HTTP arranca | `claves_mapa` → `claves`; `curl` devuelve 200/404/params/query/POST/CORS |
| **N1** | `_mapa_claves/_mapa_valores/_mapa_pares` | nuevos nativos en `native_registry/mapas.rs` |
| **N2** | `claves/valores/pares/fusionar/filtrar_claves` reales | `library/std/colecciones.fa` (antes devolvían `[]`/`{}`) |
| **N3** | `iguales(a, b)` estructural | `==` sigue comparando identidad (decisión D8) |
| **B21** | `retornar {mapa}` ya no devuelve nulo | parser: `parece_mapa_literal()` |
| **B23** | `resultado.es_ok(...)` compila | parser: `resultado` + `.` es el módulo, no la palabra de contrato |
| **B24** | `_salir` no pierde la salida | `Anfitrion::volcar_salida()` |
| **Fase 0** | Tests de comportamiento de stdlib | `library/std/tests/*.fa` (**165 aserciones, 15 módulos**) + `scripts/probar-stdlib.sh` + CI |
| **B2 (a)** | El servidor lee el request completo | `_leer_solicitud` + `_longitud_de_cuerpo`; POST de 200 KB llega entero (antes 8 KiB) |
| **Fase 4 (d)** | `forja-gui-dom`: nodos DOM + HTML con escape, accesibilidad (`<label for>`), **estado y eventos** (`Estado`, `Evento`, `aplicar`, bindings `data-fg-variable`) | `library/wasm-gui/gui-dom` (submódulo de wasm-gui) con 15 tests; `wasm-gui` traduce su `Layout` a nodos, expone `renderizar_dom(raiz_id, codigo)` y tiene **eventos delegados**: clic → `ejecutar_funcion(accion)` + repintar, `change` → `Estado` y `VariableAlmacen`. Compila para `wasm32-unknown-unknown`. Sin verificar en navegador real |
| **Fase 4 (e)** | 11 widgets en el modelo DOM (select, textarea, fieldset, progress, insignia, tarjeta, aviso con `role`, navegación, `<dialog>`, carga) + hoja `estilos.css` con contrato testeado | `library/gui-dom` en GitHub (`forja-lang/forja-gui-dom`), submódulo de Forja. 20 tests unitarios + 3 de integración + `scripts/probar-dom.sh` (jsdom: labels vinculadas, roles, dialog, progress, select, alt) |
| **Fase 4 (g)** | Pestañas ARIA (`role=tab/tabpanel`, `aria-selected`), plegable nativo (`<details>`) y **acciones de interfaz sin Forja**: `tab:<var>:<valor>` cambia de pestaña, `abrir:<id>`/`cerrar:<id>` abren/cierran `<dialog>` | `forja-gui-dom` (23 tests) + runtime wasm; demo pública con las tres cosas, verificada en Chrome headless (incluye el `?auto=1` que clickea pestaña y abre el diálogo) |
| **Fase 4 (h)** | Gap de los botones del diálogo (se desarmaban los grupos `fila`/`columna` al anidarlos) + **navegación por teclado en pestañas** (flechas/Home/End, patrón ARIA) + lista con selección `sel:` | demo pública; `probar-demo-dom.sh` verifica clic de pestaña, `?kbd=1` (flecha derecha), diálogo abierto, `aria-selected` de la lista, callbacks y binding |
| **Fase 4 (i)** | **Upgrades de widgets**: botón deshabilitado, botón-icono con `aria-label`, error de campo (`aria-invalid` + `aria-describedby` + `role=alert`) y tabla ordenable (`th[aria-sort]`, acción `orden:<columna>`) | `forja-gui-dom` (27 tests + 3 de integración), estilos y contrato de CSS verificados |
| **Fase 4 (j)** | **Rendimiento**: la interacción ya no recrea el árbol (`actualizar_dom` toca sólo nodos ligados, `hidden` de paneles y `aria-selected`); el AST se prepara con `forja::analizar` (sin optimizar ni generar Rust) | Traza del usuario: `set_inner_html` completo costaba ~50 ms por input y el arranque ~1.5 s; el GC real era sólo ~35 ms (el «stutter de GC» eran tareas largas + basura del repintado). Medición in-page imposible con `--virtual-time-budget` (reloj congelado); medido nativo: `analizar` 0,3 ms vs `transpilar` 0,4 ms |
| **Fase 4 (k)** | **wasm sin el compilador**: `wasm-gui` depende sólo de `forja-syntax` (lexer+parser) en vez del crate `forja` completo (VM, bytecode, IR, codegen) | **1,17 MB → 0,70 MB** (−40 %) y build 43 s → 15 s; la demo sigue verde. El «editor en la página» (compilar/ejecutar Forja en el navegador) sí necesita el compilador completo: si se quiere, va en un wasm aparte que se carga bajo demanda |
| **Fase 4 (l)** | **Diagnóstico con trazas reales**: el arranque de ~1,2 s medido no era CPU nuestra sino **dos descargas en serie** (glue JS y después el `.wasm`) desde GitHub Pages; el GC total era 36 ms y cada interacción ~0,8 ms de media | Se agregó `<link rel=preload as=fetch>` del wasm + `?v=` al glue (paraleliza la descarga y evita servir un wasm viejo). Evidencia: `RunMicrotasks` 1139 ms coincide con la espera del JS; `v8.wasm.compileConsume` 6 ms; `GpuVSyncThread` explica los «RunTask» de 4 ms que parecían trabajo |
| **Fase 4 (m)** | `wasm-opt -Oz` sobre el wasm de la demo: **703 KB → 556 KB** (−21 %), reproducible con `scripts/construir-demo-dom.sh` (usa binaryen del PATH o `npm i binaryen`) | Verificado en navegador tras optimizar; la demo pública sirve el wasm optimizado. Sin wasm-opt el script sigue funcionando (copia sin optimizar) |
| **Fase 4 (n)** | Widgets desde Forja: `tabla(...)`/`renglon(...)`, `insignia(texto, tono)` y `aviso(texto, gravedad)` (antes sólo existían en el modelo `gui-dom`); celdas de texto sin `<p>` | Demo pública con tabla, insignias y aviso; `forja-gui-dom` 27+3 tests; verificado en Chrome headless |
| **Fase 4 (o)** | **Tabla ordenable interactiva** sin lógica de la app: `tabla_ordenable(...)` y la acción `orden:<columna>` reordena el `tbody` comparando texto, alterna sentido y deja `aria-sort` en las cabeceras | Demo pública (3 filas: clic en *Tarea* → orden descendente) y aserción en `probar-demo-dom.sh`; `wasm-gui` 9794271 |
| **Fase 4 (p)** | **Formulario real** (`<form>`): `formulario(accion, metodo, hijos...)`; con `accion` vacía el runtime intercepta el `submit` (Enter) y dispara la acción del primer botón; con `accion` puesta funciona **sin JS** (POST normal) | `forja-gui-dom` (28 tests + 3), `wasm-gui` 02f4c0d; demo pública verifica `tareas = 3` (2 clics + 1 Enter) |
| **Fase 4 (q)** | Más widgets desde Forja: `progreso(valor, maximo, etiqueta)` (con `aria-valuenow`), `cargando(texto)` (`aria-busy`) y `boton_icono(icono, etiqueta, accion)` (`aria-label`) | `wasm-gui` a8544c5; demo pública y e2e con las tres aserciones nuevas |
| **Fase 4 (r)** | **Se eliminó el renderizador de canvas** de `wasm-gui`: el DOM pasa a ser la única ruta. API nueva `ejecutar_dom(raiz, codigo)` / `rerenderizar_dom()`; el playground de la documentación migra a `<div id="forja-gui">` + `gui-dom.css` (scoped) | `wasm-gui` b691e3f: 2783 → 2145 líneas, sin `AppState`/hit-testing/colores/Canvas 2D; `web-sys` sin features de canvas. Verificado: demo e2e, `docs` build (134 páginas) y el wasm del playground ejecutando el ejemplo de contador (`Valor: 2`) |
| **Fase 4 (s)** | **Navegación por URL** en la GUI DOM: acción `ir:#/ruta` (`history.pushState`), `popstate` reaplica atrás/adelante y la ruta actual se expone como variable `ruta` al arrancar | `wasm-gui` 7f97fe5; demo con botones «Ir a lista/detalle/opciones» y aserción `ruta = /detalle` en `probar-demo-dom.sh` |
| **Fase 4 (t)** | **Lista paginada con render perezoso**: `lista_paginada(variable, por_pagina, items...)` genera **sólo** los ítems de la página actual (7 ítems y 3 por página → 3 `<li>` en el DOM), con `pag:<variable>:-1|1`, `data-fg-paginas` y recorte de página fuera de rango | `forja-gui-dom` (29 tests + 3), `wasm-gui` 990bec9; demo pública con 5 ítems de a 2 y aserción «Página 2 de 3» en `probar-demo-dom.sh` |
| **B16 (a)** | **Paridad VM ↔ GUI y dos bugs del evaluador de la GUI arreglados**: `romper`/`continuar` se perdían (un `mientras(verdadero)` con `romper` colgaba la ventana) y un `retornar` dentro de `si`/`mientras` se descartaba | `library/gui-rt/tests/paridad_vm.rs` (10 programas, divergencias conocidas documentadas) + 3 tests de control en el crate; **37 tests verdes**. Plan completo en `plans/B16-UNIFICAR-EVALUADOR.md` |
| **B16 (b)** | **Evaluador unificado**: `compiler/forja-interprete` compartido por la GUI nativa y la web (`Valor`, `Ambito`, `ejecutar_funcion`, 22 nativos); cada host aporta su almacén por el trait `AlmacenValores` | `gui-rt/src/evaluador.rs` 2.108 → **25 líneas**; `wasm-gui` 2.145 → **1.647** (−498); tests: crate 3, `gui-rt --lib` 34, paridad 12, `gui-dom` 29+3, demo headless completa |
| **B16 (c)** | **Arreglos reales** (`Valor::Arreglo`) y **clases completas** en el intérprete (instancia, constructor `nuevo`, `este`, campos y métodos mutadores) | `forja-interprete` 3+4 tests; `paridad_vm` **12/12 sin divergencias conocidas**; demo headless OK con el wasm reconstruido |
| **B16 (d)** | **Nativos compartidos con la VM**: SQLite, archivo y JSON del intérprete delegan en `forja-native` (`nativos_compartidos::invocar`, `ForjaFast` como `Anfitrion`); el intérprete deja de depender de `rusqlite` | Una sola implementación de esos 12 nativos; feature `nativos` (off en wasm32); tests de SQLite/archivo/JSON del intérprete en verde |
| **Fase 4 (f)** | **Demo wasm hosteada y verificada en navegador**: `demo-dom/` (wasm 1.1 MB) pinta DOM real; clics ejecutan el callback Forja y re-renderizan (0→2), y escribir en un campo actualiza la variable del programa | `bash scripts/probar-demo-dom.sh` usa **Chrome/Edge headless** (`--dump-dom` con `?auto=1`) y verifica ambos flujos; `bash scripts/servir-demo-dom.sh` sirve la demo. Bug arreglado en el camino: los listeners se duplicaban en cada repintado y el estado no se resincronizaba tras un callback |
| **N29** | El ejecutable autónomo arrancaba en el **REPL**: el serializador no escribía el payload de `NuevoArreglo`/`NuevoMapa` (el lector esperaba 4 bytes) → stream desalineado → `deserializar_bytecode` devolvía `None` y el runtime seguía como si no hubiera programa | **hecho**: writer escribe ambos payloads; `tests/bytecode_roundtrip.rs` (3 casos, compara opcode por opcode) y diagnóstico permanente en el deserializador |
| **B18 (c)** | `forja web build [salida]`: paquete de despliegue (`dist/`) con fuentes, estáticos, migraciones, `Dockerfile`, `ejecutar.sh`, `.env.example` y README | `src/tools/forja-cli/src/commands/web.rs`; `scripts/probar-web-build.sh` genera el paquete desde la demo y lo arranca desde `dist/` |
| **B18 (b)** | `forja web dev [archivo.fa]`: corre la app y la reinicia al cambiar `.fa/.sql/.css/.html/.json` (polling 500 ms; reinicia también si murió por error) | `src/tools/forja-cli/src/commands/web.rs`; `scripts/probar-web-dev.sh` sirve `marca-v1`, edita el `.fa` y comprueba `marca-v2` |
| **B18 (a)** | Plantilla de proyecto web en la CLI: `FORJA_PLANTILLA=web forja nuevo <app>` genera `main.fa` SSR + `publico/estilo.css` + `README.md` (la CLI no usa flags: se elige por entorno) | `scripts/probar-nuevo-web.sh` genera el proyecto, lo arranca y verifica página, `{% para %}`, CSS y `/salud` |
| **Fase 5 (a)** | Demo de página real: `demos/tareas-web` (SQLite + migraciones + sesión + CSRF + plantillas SSR + CSS + `/salud`) | `scripts/probar-demo-web.sh` e2e: registro, CSRF 403, alta/listado, XSS escapado, marcar hecha, login 302/401, logout, CSS, migración única |
| **Fase 4 (b)** | Formularios HTML: el cuerpo `application/x-www-form-urlencoded` se parsea a `peticion.parametros` | sin esto cualquier `<form method="post">` llegaba vacío |
| **Fase 4 (c)** | `std/vista`: HTML por funciones con escape automático y accesibilidad (label/for, required) | `library/std/vista.fa` + 46 aserciones; la demo `demos/tareas-web` ya lo usa (menos HTML a mano). Variantes explícitas `*_textos` (escapa) y `*_html` (confía) |
| **Fase 4 (a)** | `std/plantillas`: escape de HTML, `{{clave}}` / `{{{crudo}}}`, `{% si %}`/`{% sino %}`/`{% para %}` anidables, `interpolar_filas` | `library/std/plantillas.fa` + 40 aserciones; `probar-http.sh` verifica que un parámetro con `<script>` sale escapado y que un `{% para %}` recorre una lista |
| **Fase 3 (c)** | `std/auth`: registro, login, logout, contraseñas (PBKDF2 del runtime), rate limit por usuario y por IP | `library/std/auth.fa` + `std/limitador`; `scripts/probar-auth.sh` e2e (201/409/400/401/200/403/429) |
| **N28** | Con varios módulos importados, dos funciones con el mismo nombre y argumentos de tipo dinámico se confunden: `modulo.funcion(args)` da «Llamada ambigua» **aunque el prefijo esté puesto**, y un fallback por aridad hace *despacho cruzado* (`sqlite.ejecutar` terminaba llamando a `redis.ejecutar`) | cualquier nombre repetido en la stdlib (`cerrar`, `obtener`, `crear`, `ejecutar`) es una trampa | **aviso al compilar**: si una función del usuario se llama como una del módulo importado, ahora sale un `aviso:` por stderr (`forja_hir::imports::funciones_que_pisan_imports`). La stdlib nueva evita nombres repetidos (`sesion.invalidar`, `redis.desconectar`, `redis.comando_crudo`, `redis._respuesta_valor` porque `_valor` ya existía en `datos`). Inventario: **41 nombres duplicados** en `library/std` (`cerrar` en 8 módulos, `enviar` en 8, `parsear` en 6…). El arreglo real es que el checker/codegen resuelva por módulo en las llamadas calificadas |
| **N27** | `objeto.campo(args)` cuando el campo guarda una función devuelve **nulo** en silencio (se interpreta como método inexistente) | callbacks en objetos (`auth`) no funcionan sin extraer a variable; hay que arreglar el evaluador | **abierto** (workaround: `variable f = obj.campo; f(...)`) |
| **N26** | `solicitud.ip` era la dirección **local** del servidor (se usaba `_socket_direccion_local`) | ahora es la del cliente (`_socket_direccion_remota`) + `solicitud.ip_local`; rate limit por IP y logs correctos |
| **Fase 3 (a)** | `std/sesion`: cookie firmada HMAC + tiempo constante, almacén con expiración, rotación de id, CSRF derivado, **almacén en Redis** (`sesion.almacen_redis`) | `library/std/sesion.fa` + 38 aserciones; `scripts/probar-sesion.sh` e2e; `tests/sesion_redis.rs` prueba sesiones compartidas entre dos clientes y `auth` sobre Redis |
| **Fase 3 (b)** | Cabeceras de seguridad por defecto | `habilitar_seguridad(servidor, hsts)` → `nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy`, CSP y HSTS opcional |
| **N25** | `_random_bytes` devolvía bytes crudos pasados por `from_utf8_lossy` (basura con caracteres de reemplazo) | ahora hex en minúscula: sirve como id de sesión / sal |
| **B10** | `std/trabajos`: cola en Redis con FIFO, reintentos con contador y dead letter | `library/std/trabajos.fa`; `tests/trabajos_cola.rs` la ejercita contra el servidor RESP en proceso. Falta backoff y `BRPOP` bloqueante |
| **Fase 2 (b)** | `std/redis`: cliente RESP (código + parseo, UTF-8, pipelining) | `library/std/redis.fa` + 29 aserciones de framing; `tests/redis_cliente.rs` corre el cliente contra un servidor RESP en proceso (sin docker); `scripts/probar-redis.sh` (e2e, se saltea sin Redis real). Falta pool/reconexión/pub-sub bloqueante |
| **B6** | Log por request con latencia + `/salud` + `/metricas` | `_responder()` centraliza envío/métrica/log (`GET / -> 200 (1 ms, #3)`); `habilitar_salud()` registra ambos endpoints |
| **B4** | Estáticos de verdad: MIME, `ETag`, `304`, `HEAD`, `403` ante traversal, y **413/431** en vez de cerrar la conexión en silencio | `_respuesta_estatica()`; antes `servir_estaticos` devolvía 404 siempre (y el path era atravesable con `..`). Falta `Range`, gzip y binarios |
| **B5** | Router determinista: grupos, comodín `*`, 405 + `Allow`, URL por nombre | `grupo()` (devuelve un objeto grupo, sin callback), `nombrar_ruta()`, `url_de()`, `_coincidir_patron()` (gana el patrón con más literales); `curl -X DELETE` → 405 |
| **B2 (d)** | Keep-alive HTTP/1.1 (y `1.0` sólo si el cliente lo pide) | `_leer_solicitud` devuelve `{datos, resto}` para no perder bytes del siguiente request; `_mantener_conexion`/`_fijar_conexion`; tope de 100 requests por conexión y 10 s de inactividad |
| **B2 (b)** | Timeout de 10 s por conexión | `_socket_fijar_timeout` al aceptar |
| **B2 (d)** | Tope de 64 KiB de cabeceras (slowloris) | `MAX_CABECERAS` en `_leer_solicitud` |
| **N5** | `url.parsear`/`query_parsear` devuelven mapas | nativos reescritos; `query_parsear` con parámetros decodificados |
| **N13** | `csv` y `toml` no envolvían en `Ok` (tipo mentido) | `csv.fa`, `toml.fa` |
| **N4 (a)** | `variable X = X(...)` ya no devuelve nulo | codegen quita el nombre de `var_indices` mientras compila el inicializador |
| **N15** | El **cliente HTTP** no funcionaba: `_cabeceras_a_texto` usaba `unir()` (inexistente) → toda petición salía vacía | `cliente_http`, `cliente_h2` |
| **B6 (a)** | Logs del servidor por request en stderr (visibles en vivo) | `iniciacion` + `informacion(...)` en `servidor_web` |
| **Fase 0+** | Prueba de punta a punta servidor+cliente | `scripts/probar-http.sh` + CI: GET, params, 404, POST, JSON |
| **N18** | Un nativo que falla ya **no** devuelve nulo: aborta con el mensaje | `vm_fast/ejecucion.rs`; `std/archivo` usa el centinela `-1` que esperaba |
| **N19** | `mapa_async`/`paralelo_for_async` volvían nulo (closures en hilos) | ahora secuenciales y documentados (dependen de N11) |
| **B2(c)** | Estado compartido entre hilos | `std/compartido` + nativos `_compartido_*` (celdas escalares); 4 hilos × 50 sumas = 200 verificado |
| **B6 (b)** | Contador de requests en el log | `servidor_web` usa una celda compartida: `GET / -> 200 (#3)` |
| **N9** | `coincidir` sin rama que matchee **falla con mensaje** | `ErrFast::SinCoincidencia` + `_coincidencia_sin_rama`; los brazos ahora saltan siempre al final |
| **N21** | `importar` dentro de una función es **error de compilación** | `TipoChecker` con `profundidad_funcion` |
| **N10 (b)** | Al fallar en runtime se vuelca lo impreso | `src/lib.rs` (7 puntos) |
| **Fase 2 (a)** | `std/datos`: pool básico, migraciones, transacciones | `library/std/datos.fa` + `sqlite.ejecutar_lote`; 17 aserciones |
| **N14** | `modulo.funcion(...)` ya resuelve **dentro** de los módulos | reescritura movida a `forja-hir::imports`, aplicada por los tres resolvers de `forja-vm` |
| **N4 (a)** | `variable X = X(...)` (re)aplicado | el pipeline IR todavía devuelve nulo en `400_exacto_debug`; anotado en `DIVERGENCIAS_CONOCIDAS` |
| — | Ejemplos `170_cifrado_cesar` y `async_await/paralelo` arreglados | barrido de ejemplos: 9 errores → 7, todos esperados (FFI/red/contratos) |
| — | Date correcto en respuestas | `_http_fecha_texto()` sin argumento |
| — | El handler recibe `(solicitud, respuesta)` | mutar o devolver ambos funcionan |

Verificación final (todo ejecutado, todo en verde):
- `bash scripts/probar-stdlib.sh` → **24 módulos de stdlib** (VM)
- `bash scripts/probar-http.sh` · `probar-sesion.sh` · `probar-auth.sh` ·
  `probar-demo-web.sh` · `probar-nuevo-web.sh` · `probar-web-dev.sh` ·
  `probar-web-build.sh` · `probar-dom.sh` (jsdom) · `probar-demo-dom.sh`
  (Chrome/Edge headless) — **10 e2e**
- Rust: `forja-gui-dom` 29 · `forja-gui-rt --lib` 37 · `forja-gui-rt --test
  paridad_vm` 12 · `forja --test stdlib` 7 · `bytecode_roundtrip` 3 ·
  `redis_cliente` 1 · `trabajos_cola` 1 · `sesion_redis` 1 · `forja-types` 30
- `probar-redis.sh` se saltea sin Redis local (queda activo en CI con docker)
- Demo pública: **https://forja-lang.github.io/forja-gui-dom/** (wasm 566 KB,
  `wasm-opt -Oz`, precarga del wasm)

Repos: raíz `forja-lang/forja` (main) + submódulos `library/std`,
`library/gui-dom`, `library/wasm-gui`, `library/gui-rt`, todos pusheados.
Pendiente por diseño (documentado): B16 (`plans/B16-UNIFICAR-EVALUADOR.md`),
pool de hilos (N11), TLS de servidor, `Range`/gzip en estáticos, subida de
archivos, hidratación wasm y virtualización real por ventana de scroll.

### Nuevos hallazgos (encontrados al arreglar lo anterior)

| # | Hallazgo | Impacto | Estado |
|---|---|---|---|
| **N4** | Una variable homónima tapa a la función/método en **todo el ámbito** (no sólo en su inicializador), y **un parámetro pierde contra una función global**: `funcion grupo(s, p, registrar) { registrar(g) }` llamaba a `std/auth.registrar` al importar `std/auth` | `variable extension = …` + `ruta.extension(...)` → error confuso; parámetros hay que nombrarlos distinto | **abierto** (renombrar el parámetro es el workaround) |
| **N5** | `std/url.parsear` y `query_parsear` declaran `Resultado<...>` pero devuelven un objeto `URL` sin campos nombrados y un texto `"a\|1\|b\|2"` | no se podía usar `url.parsear` | **hecho** (devuelven mapas) |
| **N6** | `std/sqlite` devuelve `Resultado` en cada llamada y los ejemplos de la doc no desenvuelven | frustrante pero usable con `?`/`coincidir` | documentar |
| **N7** | `forja test` transpila a Rust, así que no puede testear módulos que usan nativos | por eso los tests corren con `forja run` | workaround |
| **N8** | `longitud(nulo)` devuelve `0` en silencio | enmascara errores | **abierto** |
| **N9** | `coincidir` sin rama que matchee no dice nada (programa sigue en silencio) | depuración difícil | **hecho** (`ErrFast::SinCoincidencia`) |
| **N10** | `_salir` no vaciaba la salida | tests no reportaban fallas | **hecho** |
| **N11** | `hilo { }` **clona la VM y todo el montículo** (objetos incluidos) y limita cada hilo a 500 000 instrucciones | el hilo no ve ni modifica estado del principal; no sirve para un pool de conexiones con estado compartido | **abierto** (bloquea B2-bis) |
| **N12** | Límite de cabeceras | implementado (`MAX_CABECERAS`, 64 KiB) | **hecho** |
| **N14** | ~~`modulo.funcion(...)` no resuelve dentro de módulos~~ | — | **hecho** |
| **N16** | Llamar a una función inexistente **sí** da error (verificado) | — | **descartado** |
| **N17** | **La stdlib entera está en el espacio de nombres global**: `parsear`, `unir`, `generar`… existen sin importar y colisionan entre módulos (`unir` resuelve a `ruta.unir`) | llamadas silenciosamente equivocadas | **abierto** |
| **N18** | Errores de nativos se convertían en `nulo` | enmascaraba fallos de IO/tipos | **hecho** |
| **N22** | `expresion[clave](args)` (o `f()(args)`) se parsea como `LlamadaFuncion{nombre:"anon"}` → `función no definida: 'anon'` | hay que pasar el valor por una variable antes de llamarlo | **abierto** (`primarias.rs:513`) |
| **N23** | `subcadena` indexa por caracteres pero `longitud` cuenta bytes | mezclar ambos con texto no ASCII desalinea índices (el parser de RESP lo compensa con `_caracteres_de_bytes`) | **abierto** (verificar antes de tocar) |
| **N24** | `canal` es palabra reservada | usarla como parámetro de función da «Se esperaba '(' después de 'canal'» sin explicar la causa | workaround: otro nombre |

---

## 1. Método

- Lectura del código de `library/std/*.fa`, `library/gui-rt`, `library/wasm-gui`,
  `compiler/forja-native` y `src/tools/forja-cli`.
- **Pruebas de ejecución** con `forja run` / `forja test` (ver §10): servidor web, SQLite, hilos,
  canales, aritmética, builtins.
- Conteo automático de variantes de layout y de nativos por intérprete.
- Comparación entre lo que dicen los docs (`docs/src/pages/arquitectura/std/http.astro`) y lo que
  hace el código.

---

## 2. Inventario verificado

| Capacidad | Estado | Evidencia / límite |
|---|---|---|
| Sockets TCP/UDP, cliente y servidor | ✅ funciona | `native_registry/sockets.rs` (13 nativos) |
| Hilos reales + canales mpsc | ✅ funciona | `Opcode::CrearHilo` → `std::thread::spawn`; `ejemplos/concurrencia/126_hilo_simple.fa` ejecutado OK |
| HTTP/1.1 cliente | ⚠️ roto | `cliente_http.fa:65` usa `claves_mapa` inexistente |
| HTTP/1.1 servidor | ❌ roto | `servidor_web.fa:92` idem; verificado en ejecución |
| HTTP/2 h2c servidor/cliente | ❌ roto | `servidor_h2.fa`, `cliente_h2.fa` mismo `claves_mapa` |
| WebSocket | ⚠️ sin verificar | `websocket.fa` (328 líneas) usa `_ws_frame_*` que sí están registrados; sin tests |
| TLS **cliente** | ⚠️ parcial | `native_h2_tls.rs:29` registra `_tls_conectar`; `tls.fa:20` llama `_tls_conectar_con_pin` (**no registrado**) |
| TLS **servidor** (https) | ❌ no existe | no hay `_tls_aceptar` ni uso de `rustls` server en ningún lado |
| SQLite | ✅ funciona | 9 nativos en `native_sqlite.rs`; `INSERT`+`SELECT` verificado |
| Migraciones / ORM / pool | ❌ no existe | `sqlite.fa` es wrapper 1:1 sobre los nativos |
| Redis / colas / caché | ❌ no existe | `grep -ri redis` no encuentra nada en `library/` ni `compiler/` |
| JSON / TOML / CSV | ✅ funciona | `std/json`, `std/toml`, `std/csv` |
| Hashes y password | ✅ funciona | `_hash_password`, `_verify_password`, `_hmac_sha256`, `_pbkdf2`, `_scrypt` |
| Sesiones / cookies firmadas / CSRF | ❌ no existe | `cookies` se parsea (`servidor_web.fa:112`) pero no hay sesión ni firma |
| Plantillas / SSR | ❌ no existe | solo concatenación de strings y `html(respuesta, texto)` |
| Assets / bundler / CSS | ❌ no existe | hay `servir_estaticos` (sin rangos, sin caché, sin compresión) |
| Subida de archivos | ❌ no existe | `SolicitudHTTP.archivos` se inicializa vacío y nunca se llena |
| GUI nativa (escritorio) | ✅ funciona | 115 variantes de layout en `gui_nativa.rs:439` |
| GUI web (navegador) | ⚠️ prototipo | 10 de 115 variantes; canvas 2D; solo clic |
| Compilar `.fa` → wasm | ❌ no existe | el roadmap lo marca "Planeado" (`docs/src/pages/roadmap.astro:136`); lo que hay es la **VM en wasm** |
| Dev server / `forja web` | ❌ no existe | el CLI no tiene comando de servidor de desarrollo |
| Hot reload | ⚠️ solo GUI nativa | recompila el `.exe` con cargo (`ejecucion.rs:505`), tarda segundos |
| Registry de paquetes | ✅ funciona (dev) | `forja add/publish/registry-serve`; sin lockfile |
| Tests de stdlib | ❌ inexistentes | `tests/stdlib/stdlib_smoke.rs` solo importa cada módulo |

---

## 3. Los huecos, uno por uno

### 3.1 Backend HTTP

#### B1. La stdlib HTTP no arranca (bug bloqueante)
- **Síntoma**: `curl` no obtiene nada; el proceso muere con
  `función no definida: 'claves_mapa'`.
- **Causa**: `library/std/servidor_web.fa:92` (y `:239`, `:342`), `cliente_http.fa:65,156`,
  `cliente_h2.fa:93,263`, `servidor_h2.fa` — 10 llamadas a una función que no existe. El builtin
  real es `claves()` de `std/colecciones`.
- **Impacto**: **todo** el stack HTTP está muerto. Ningún ejemplo de la carpeta `redes/` sirve
  una página (el ejemplo oficial `217_servidor_web.fa` ni siquiera llama a `iniciar`, con un
  comentario que dice "bloquea y aquí no hay nadie escuchando").
- **Arreglo**: reemplazar por `claves(...)`; agregar `importar std/colecciones` donde falte.
  Después, test de comportamiento (ver Fase 0).
- **Verificación**: `curl` contra un server de prueba en CI.

#### B2. Servidor mono-hilo y de una sola lectura
- **Causa**: bucle `aceptar → recibir(8192) → responder → cerrar`
  (`servidor_web.fa:433-455`). Sin bucle de lectura: si el request llega partido en dos paquetes
  TCP o el cuerpo pesa > 8 KiB, se parsea mal o se pierde. `Connection: close` fijo (`:404`).
- **Impacto**: no sirve para formularios, JSON grande, subidas, keep-alive ni para dos usuarios
  simultáneos. Cada request bloquea al siguiente.
- **Arreglo**:
  1. Leer hasta `\r\n\r\n`, parsear `Content-Length`, leer el cuerpo completo (con tope y timeout).
  2. Aceptar en un hilo por conexión (`hilo { atender(cliente) }`) o, mejor, con un pool acotado
     + colas; keep-alive con `Connection: keep-alive` y timeout de inactividad.
  3. Timeouts de lectura/escritura con `_socket_fijar_timeout`.
- **Verificación**: test que hace 50 requests concurrentes y compara; test con cuerpo de 2 MB;
  test con request partido byte a byte.

#### B3. Sin TLS de servidor
- **Causa**: `native_h2_tls.rs` solo implementa el lado cliente (`_tls_conectar`).
- **Impacto**: no se puede servir `https://`, ni HTTP/2 real (los navegadores exigen TLS para h2),
  ni cookies `Secure`.
- **Arreglo**: `_tls_aceptar(cert, key)` con `rustls` (ya es dependencia con feature `h2-tls`,
  `forja-native/Cargo.toml:26`) + ALPN (`h2`, `http/1.1`), carga de certificados PEM, y
  `ServerWeb::tls(cert, key)`. Para dev: certificado autogenerado.
- **Verificación**: `curl -k https://...` y `openssl s_client -alpn h2`.

#### B4. Sin resiliencia
- Sin límite de conexiones; **el límite de cuerpo no existe de hecho**: `_max_cuerpo` se declara
  (`servidor_web.fa:73`) y se puede fijar (`:225`), pero **nunca se lee**. Sin protección contra
  slowloris, sin `Expect: 100-continue`, sin compresión
  (`gzip`/`br`), sin rangos (`Range`) para estáticos, sin ETag/`Last-Modified`, sin
  `HEAD` correcto, sin `OPTIONS` automático, sin límite de cabeceras.
- **Arreglo**: es una lista de tareas concretas en `servidor_web.fa` + 2-3 nativos nuevos
  (`_gzip`, `_mtime`).

#### B5. Sin router decente
- Hoy: `_registrar_ruta` con `{param}` y `_resolver_ruta` (`:231`), más middlewares sin orden
  declarado ni `next()` bien definido.
- Falta: grupos/prefijos, comodines, prioridad determinista, rutas por método + 405, montaje de
  sub-apps, generación de URL desde el nombre de ruta, y **tipado** de params.
- **Arreglo**: rediseñar el router como árbol (segmento estático > param > comodín) y exponer
  `ruta("nombre")` para construir URLs.

#### B6. Sin observabilidad
- No hay request-id, log estructurado por request (método, ruta, status, ms), métricas
  (latencia p50/p95, requests/s), healthcheck estándar, ni trazas.
- **Arreglo**: `std/servidor` emite un evento por request a un handler configurable;
  `_log_escribir` ya existe; agregar `std/metricas` con contadores/histogramas.

### 3.2 Datos y estado

#### B7. SQLite sin capa de datos
- **Qué hay**: `abrir/cerrar/ejecutar/consultar/consultar_una/*_con_parametros/ultimo_id/tablas/
  columnas/transaccion/confirmar/deshacer`. Todo 1:1 sobre `rusqlite`.
- **Qué falta y bloquea una web**:
  - **Pool de conexiones** (hoy una `Connection` global compartida; SQLite en modo WAL + pool de
    lectores es lo que hace falta).
  - **Migraciones** versionadas (`migraciones/001_*.sql` + tabla `_migraciones`).
  - **Query builder / mapeo a clases**: hoy devolvés `Arreglo<Mapa>` y accedés con strings.
  - **Transacciones con scope** (`con_transaccion(db, func(tx) { ... })`) y rollback por error.
  - **Tipos**: fechas (`TIMESTAMP`), blobs, decimales exactos (¡Forja ya tiene `Exacto`!).
  - **Errores útiles**: código SQLite, constraint violada, tabla/columna inexistente.
- **Arreglo**: `std/datos` (nuevo) + nativos `_sqlite_pool_*` si hace falta; migraciones en `.sql`.
- **Aceptación**: CRUD de una tabla con migración, transacción que revierte y 100 conexiones
  concurrentes sin "database is locked".

#### B8. Redis no existe
- **Qué falta**: cliente RESP (conexión, auth, select, `GET/SET/EX/DEL/INCR/HGETALL/LPUSH/BRPOP`,
  pub/sub), pool de conexiones, serialización de valores, y una capa de caché de alto nivel
  (`cachear(clave, ttl, func()`), invalidación por patrón).
- **Impacto sin Redis**: no hay sesiones distribuidas, ni rate limiting, ni colas de trabajos,
  ni caché compartida entre procesos.
- **Arreglo**: `std/redis` en `.fa` sobre sockets (RESP es texto, se puede hacer hoy) + 2 nativos
  (`_redis_pool_*`) para no reimplementar framing y timeouts. **No hace falta un crate Rust
  nuevo**: se puede escribir en Forja, pero necesita `_socket_fijar_timeout` y reconexión.
- **Aceptación**: `SET`/`GET` contra un Redis real en CI (docker), pub/sub con 2 procesos.

#### B9. Sin sesiones, auth ni CSRF
- **Qué hay**: `_hash_password`/`_verify_password` (bcrypt), HMAC-SHA256, PBKDF2, scrypt,
  parseo de cookies (`servidor_web.fa:112`).
- **Qué falta**: cookie firmada (`HMAC` sobre `sid`), store de sesiones (memoria o Redis),
  expiración/rotación, `HttpOnly`/`SameSite`/`Secure`, CSRF token por sesión, rate limiting de
  login, recuperación de contraseña, y un `std/auth` con `registrar/iniciar_sesion/cerrar_sesion`.
- **Por qué es bloqueante**: cualquier "página real" con login se apoya en esto.
- **Aceptación**: flujo registro → login → sesión persistida en Redis → logout, con test de
  cookie robada/reutilizada.

#### B10. Sin trabajos en segundo plano
- No hay cola de trabajos, reintentos, ni scheduler. `hilo {}` y `canal()` existen, pero no hay
  una abstracción de worker persistente.
- **Arreglo**: `std/trabajos` (cola en Redis con `BRPOP` + `reintentar`), y un runner
  `forja trabajos` en el CLI.

### 3.3 Frontend web / GUI web

#### B11. La GUI web es un canvas: no es una página
- **Causa**: `library/wasm-gui/src/lib.rs:2` — "Renderiza interfaces Forja en un `<canvas>`
  HTML5 usando Canvas 2D API".
- **Consecuencias directas** (todas verificadas por lectura):
  - Sin DOM: **cero accesibilidad** (un lector de pantalla ve un canvas vacío), sin selección de
    texto, sin zoom de navegador, sin "buscar en la página".
  - Sin SEO: los buscadores no ven nada.
  - Sin CSS: cada estilo se dibuja a mano; no hay `:hover`, `@media`, `prefers-color-scheme`.
  - **Sin formularios**: clickear `entrada_texto` escribe en la consola
    (`wasm-gui/src/lib.rs:1382-1391`). No hay teclado, ni foco, ni validación, ni autocompletado.
  - Sin scroll ni resize: `grep scroll|wheel|resize` no devuelve nada.
  - Sin enlaces/URL: no hay router, ni historial, ni compartir un link a una pantalla.
- **Arreglo**: ver §4.6. Dos caminos posibles (decisión D1), recomendado **DOM isomórfico**.

#### B12. La GUI web tiene su propio modelo de layout, con 10 widgets
- **No reusa nada**: `wasm-gui` define **su propio** `enum Layout` (`wasm-gui/src/lib.rs:101`)
  con 10 variantes (Column, Row, PilaZ, Label, VariableLabel, Button, EntradaTexto, Title,
  Spacer, Unimplemented), mientras que el nativo tiene **115 variantes**
  (`gui-rt/src/gui_nativa.rs:439`). El resto de los widgets nativos simplemente no existen en
  web; el AST→layout del navegador es otro convertidor independiente.
- **Impacto**: cada widget nuevo hay que hacerlo dos veces, y una app de escritorio no se puede
  "publicar en web" sin reescribir la pantalla.
- **Arreglo**: un solo `Layout` (el de `gui-rt`) y dos renderers: nativo (xilem) y web (DOM).
  Si se sigue con canvas, además de B11 hay que portar 105 widgets para llegar al mismo punto.

#### B13. Sin SSR, sin hidratación, sin assets
- No hay forma de devolver HTML generado en el servidor con los datos ya puestos (importante para
  SEO y primer render), ni de hidratar ese HTML con la app wasm, ni un pipeline de CSS/JS
  (minificar, hashear, servir con caché).
- **Arreglo**: `std/plantillas` (interpolación con escape automático de HTML) + `forja web build`
  que empaquete `wasm-gui` + assets.

#### B14. Sin i18n ni rutas localizadas
- No hay catálogo de textos ni helper de traducción. Para una web real suele hacer falta.

### 3.4 GUI de escritorio (lo que pidió mejorar)

#### B15. La API de GUI son "nombres mágicos"
- `library/gui/gui.fa` es una capa de **funciones identidad**: `columna(hijos...) { retornar hijos }`
  (`gui.fa:13`), `boton(...) { retornar texto }` (`:341`). La UI no se construye con esos valores:
  la construye el **evaluador/transpilador** interpretando el *nombre* de la función en el AST.
- **Consecuencias**:
  - No se puede componer: `variable f = boton_relleno; f("x", cb)` no hace nada.
  - No se puede generar UI desde datos (bucle que arma filas) salvo que el evaluador lo permita
    caso por caso.
  - El tipado es decorativo (`-> Widget` devuelve el texto de entrada).
  - IDE no puede autocompletar de verdad: no hay valores.
- **Arreglo**: que `Layout`/`Widget` sean **valores reales** en el runtime (una variante de
  `ValorFast`/`ValorGUI`), y que `gui.fa` construya esos valores. Es el cambio de diseño más
  importante para que la GUI sea programable.
- Complementos que faltan (no existen o son stubs):
  - **Estado**: hay `VariableAlmacen` reactivo, pero sin validación de formularios, ni formularios
    como unidad, ni "submit".
  - **Eventos**: solo clic y (nativo) teclado por widget; no hay eventos de ventana (resize, focus,
    close), ni timer/animación declarativa, ni drag&drop real.
  - **Listas grandes**: `ListaMaterial`/`TablaDatosMaterial` sin virtualización comprobada.
  - **Navegación**: hay `Navigator`/rutas, pero sin URL ni historial (en escritorio importa menos).
  - **Testing de UI**: no hay snapshots ni forma de testear un layout sin abrir ventana.
  - **Accesibilidad**: hay `AccessibilityManager` (roles, anuncios) pero no está verificado
    contra un lector real.
  - **Inspector/devtools**: no hay nada tipo "ver árbol de widgets y estado" en vivo.

### 3.5 Lenguaje, runtime y tooling que bloquean lo anterior

#### B16. Tres intérpretes con dialectos distintos
- VM: 206 nativos + ~40 builtins (`vm_fast/builtins.rs`).
- GUI nativa: `gui-rt/src/evaluador.rs` (2.108 líneas, ~60 nativos, su propio SQLite).
- GUI web: evaluador propio (~10 construcciones, solo `escribir`).
- **Impacto**: cualquier programa "que anda en la VM" puede no andar en la GUI o en la web, y
  cada feature hay que implementarla 3 veces. Es la causa raíz de que la web no pueda usar la
  stdlib.
- **Arreglo**: un solo evaluador compartido (el de la VM) expuesto como librería; la GUI solo
  aporta el **host** (dibujar, eventos). La web igual, con host DOM.

#### B17. `async/await` es ficticio
- `ejemplos/async_await/*.fa` llaman `.unir()` sobre valores comunes; `std/concurrencia.fa`
  documenta "async con thread pool real" pero `paralelo_for` (línea 40) es un `mientras`
  secuencial.
- **Impacto**: el servidor no puede tener un modelo de I/O no bloqueante; todo es bloqueante con
  hilos.
- **Arreglo**: o bien (a) asumir "hilos + pool" como modelo oficial (más simple y suficiente para
  empezar), o (b) implementar `asincrono`/`esperar` de verdad (futures + executor). Recomendación:
  (a) ahora, (b) después (D3).

#### B18. Sin dev server ni formato de proyecto para web
- `forja new` genera `main.fa` + `forja.json` (sin template web, `paquetes.rs:26`).
- No hay `forja web dev` (watch + rebuild + reload), ni `forja web build` (bundle), ni `.env`.
- **Arreglo**: plantillas (`forja new --web`), dev server con watch sobre `.fa`, recarga del
  navegador por WebSocket, y `forja web build`.

#### B19. Sin lockfile ni aislamiento de dependencias
- `forja add` escribe en `forja.json`; no hay `forja.lock` con hashes. Para una web real
  (reproducibilidad de build) hace falta.

#### B20. Sin tests de comportamiento en stdlib
- 47 módulos, 0 tests en `.fa`; el smoke solo importa.
- **Arreglo**: `forja test` ya funciona (`@test` + `asegurar`); escribir tests por módulo y
  hacerlos obligatorios en CI. Es lo más barato y lo que más rápido sube la calidad percibida.

---

## 4. Arquitectura objetivo

### 4.1 Modelo de ejecución
```
        ┌─────────────────────────────┐
        │  forja-runtime (1 intérprete)│  ← VM actual, con natives
        └──────────────┬──────────────┘
          host: CLI    │    host: GUI nativa    host: web (DOM)
```
- Un `trait Anfitrion` (ya existe) para: imprimir, leer, reloj, aleatorio, red, FS.
- La GUI implementa `Anfitrion` y expone `render(arbol, eventos)`.
- La web implementa `Anfitrion` sobre DOM y expone lo mismo.
- **Regla**: todo nativo nuevo (SQLite, TLS, Redis…) se implementa **una vez**, en el host del
  proceso, no en el evaluador de cada GUI.

### 4.2 `std/servidor` (reemplaza `servidor_web`)
```fa
importar std/servidor

variable app = Servidor.nuevo()

app.ruta("GET", "/", inicio)
app.ruta("GET", "/tareas/:id", tarea)
app.grupo("/api", funcion(g) {
    g.ruta("POST", "/tareas", crear_tarea)
})
app.estaticos("/publico", "publico")
app.middleware(logger)
app.middleware(sesion(almacen_redis))

app.escuchar(8080)        # bloqueante, con pool de N hilos
```
Requisitos de implementación: lectura completa, keep-alive, timeouts, límites, TLS opcional,
pool de hilos, router determinista, errores 500 con log, `HEAD`/`OPTIONS`.

### 4.3 `std/datos` (nuevo)
```fa
importar std/datos

variable db = datos.pool("app.db", conexiones: 8)   # WAL
datos.migrar(db, "migraciones")                     # 001_*.sql en orden
datos.con_transaccion(db, funcion(tx) {
    tx.ejecutar("INSERT INTO tareas (titulo) VALUES (?)", [titulo])
})
variable filas = tx.consultar("SELECT * FROM tareas WHERE hecha = 0")
```
Más adelante: mapeo a clase (`Tarea.mapear(fila)`), y un mini query builder
(`datos.tabla("tareas").donde("hecha", falso).limite(20)`).

### 4.4 `std/redis` (nuevo)
```fa
variable r = redis.conectar("127.0.0.1", 6379)
r.fijar("clave", "valor", expira: 60)
r.incr("contador")
r.publicar("canal", "mensaje")
variable suscripcion = r.suscribir("canal")
```
Con pool, reconexión y timeouts. Base para sesiones, caché y colas.

### 4.5 `std/sesion` + `std/auth`
Cookie `sid` firmada con HMAC, store en Redis, CSRF por sesión, `hash_password` ya disponible.
API: `sesion.buscar(almacen, id)`, `sesion.crear(almacen, datos)`, `sesion.invalidar(almacen, id)`,
`auth.requiere_login(sesion)`.

### 4.6 UI web: dos caminos (decisión D1)

**Camino A (recomendado) — DOM isomórfico**
- `Layout` → HTML real (`div`, `button`, `input`, `table`…), con clases y CSS propio (o Tailwind
  prearmado).
- Estado reactivo existente (`VariableAlmacen`) enganchado a atributos/texto del DOM.
- `wasm-gui` pasa a ser un pequeño renderer DOM (~200 líneas) + el evaluador compartido.
- Ventajas: accesibilidad, SEO (con SSR), formularios nativos, CSS, texto seleccionable.
- Costo: hay que escribir la capa de render (~1-2 semanas) y un CSS base.

**Camino B — seguir con canvas**
- Solo si el objetivo son apps tipo dashboard/kiosco, no páginas web.
- Costo: portar 105 widgets y aun así sin a11y/SEO/formularios.

**SSR (independiente del camino)**: `std/plantillas` + `forja web build --ssr` que ejecuta el
mismo código en el servidor y devuelve HTML; el wasm hidrata. Esto es lo que hace que una página
sea indexable.

### 4.7 Dev y deploy
- `forja web dev`: compila a wasm, sirve estáticos, watch de `.fa`, recarga por WebSocket.
- `forja web build`: bundle (`app.wasm` + `app.js` glue + CSS + assets con hash).
- `forja web deploy`: guía + Dockerfile multi-stage (ya hay `Dockerfile` de CI, sirve de base).
- El backend se despliega como `.exe` nativo (`forja aot`) o con la VM embebida.

---

## 5. Roadmap por fases

Estimaciones en días-dev de una persona con el repo caliente.

### Fase 0 — Red de seguridad (3 días) ← hacer primero, sin excepción
- [ ] Tests `.fa` por módulo de stdlib con `@test` (empezar por: `servidor_web`, `cliente_http`,
      `sqlite`, `json`, `url`, `fecha`, `crypto`, `colecciones`). **Ya se puede hoy**: `forja test`
      funciona.
- [ ] CI: `forja test library/std/tests/` + smoke existente.
- [ ] Test de regresión del bug `claves_mapa` (importar y **llamar** cada función pública).
- **Aceptación**: cualquier función pública de un módulo probado falla en CI si se rompe.
- **Por qué primero**: sin esto, cada fase siguiente reintroduce el mismo tipo de bug.

### Fase 1 — Servidor HTTP usable (8-10 días)
- [ ] B1: arreglar `claves_mapa` (1 h).
- [x] B2 (a): lectura completa del request (headers + `Content-Length`).
- [x] B2 (b): timeouts por conexión.
- [x] B2 (c): primitiva de **estado compartido** (N11). `std/compartido` da celdas escalares;
      un pool de hilos que comparta *objetos* sigue pendiente (los hilos copian el montículo).
- [x] B2 (d): keep-alive + límite de cabeceras (N12). El **pool de hilos** sigue pendiente: con
      `hilo {}` cada conexión clonaría el montículo, así que primero hace falta compartir objetos.
- [ ] B2 (e): pool de hilos por conexión (bloqueado por lo anterior; mientras tanto el
      servidor sigue atendiendo una conexión por vez).
- [x] B5: router determinista con grupos, 405, URL por nombre. Comodín `*` final captura el
      resto. Prioridad por cantidad de segmentos literales (no por orden de registro).
- [x] B4: estáticos con `ETag`/`304`/`HEAD`/MIME y protección de traversal. **Falta** `Range`,
      gzip y binarios (necesitan `_socket_enviar_binario`); los binarios responden 415 hoy.
- [x] B6: log por request (`método ruta -> código (ms, #n)`), `/salud` y `/metricas` bajo
      `habilitar_salud()` (no se exponen por defecto).
- **Aceptación**: 
  - 1000 requests concurrentes (`hey`/`wrk`) sin errores y con latencia estable.
  - Subida de un archivo de 10 MB (aunque sea a memoria) sin truncar.
  - `curl -I`, `curl -X OPTIONS`, `curl -H "Range: bytes=0-10"` correctos.
- **Riesgo**: si el pool de hilos usa `hilo {}` con el evaluador tree-walking actual, verificar
  que el estado compartido (`VariableAlmacen`) sea thread-safe (ya usa `Arc<RwLock>`).

### Fase 2 — Datos (10-12 días)
- [x] `std/datos`: migraciones idempotentes, transacciones con scope, helpers de consulta.
      **Falta**: pool real (WAL + N conexiones) y mapeo fila→clase.
- [ ] Nativos nuevos si hacen falta (`_sqlite_pool_abrir/ejecutar/consultar`).
- [x] `std/redis`: cliente RESP + pub/sub básico (`publicar` + `recibir_crudo`). **Falta** pool y
      reconexión.
- [ ] `std/cache` (opcional, encima de Redis).
- [x] `std/trabajos` (B10): cola con reintentos y dead letter sobre listas de Redis.
- **Aceptación**:
  - CRUD con migración aplicada dos veces (idempotente).
  - 50 hilos haciendo `INSERT` sin `database is locked`.
  - `SET/GET/INCR/EXPIRE` contra Redis en docker; pub/sub entre 2 procesos.
- **Decisión**: ¿Redis se implementa en `.fa` o en Rust? Recomendación: en `.fa` (RESP es
  texto), con `_socket_*` actuales; solo agregar nativo si el rendimiento no alcanza.

### Fase 3 — Sesiones, auth y seguridad (6-8 días)
- [x] Cookies firmadas + `std/sesion` (store en memoria con expiración; **falta** el store en Redis) +
      rotación de `sid`.
- [x] `std/auth`: registro, login, logout, verificación de contraseña, rate limit
      (`std/auth` + `std/limitador`).
- [x] Store de sesiones en Redis (`sesion.almacen_redis`): sesiones compartidas entre procesos,
      con TTL como red de seguridad. `cantidad()` no aplica (hace falta `SCAN`).
- [x] CSRF token + verificación en `POST` (`token_csrf`/`csrf_valido`).
- [x] Cabeceras de seguridad por defecto (`CSP`, `X-Frame-Options`, `Referrer-Policy`, HSTS).
- **Aceptación**: flujo completo con test e2e (curl/script) y checklist de seguridad.

### Fase 4 — UI web real (15-20 días, la fase grande)
- [ ] Decidir D1 (DOM vs canvas). Recomendado: DOM.
- [ ] `wasm-gui`: renderer DOM + CSS base + eventos (click, input, submit, keydown, resize,
      scroll, history/URL).
- [ ] Un solo evaluador (B16): `wasm-gui` y `gui-rt` usan el de la VM.
- [x] `std/plantillas` (escape automático, condicionales y bucles; SSR ya sirve HTML con datos).
      **Falta** la hidratación del wasm, que depende del renderer DOM.
- [ ] `forja web dev` (watch + reload) y `forja web build` (bundle).
- **Aceptación**:
  - La misma app funciona en escritorio y navegador.
  - Formulario de login real: escribe, envía, valida, muestra error, navega sin recargar, y
    funciona sin JS (SSR) en el primer render.
  - Lighthouse: accesibilidad ≥ 90, SEO ≥ 90 en una página de ejemplo.

### Fase 5 — Producción (8-10 días)
- [x] Métricas y healthcheck (`/salud`, `/metricas`); falta request-id y `/version`.
- [ ] Logs estructurados y request-id.
- [ ] Config por entorno (`.env`/`forja.yaml` + secretos), sin recompilar.
- [ ] Graceful shutdown (`Ctrl+C` → terminar requests en curso).
- [x] `forja web build`: paquete con `Dockerfile` + `ejecutar.sh` + README. Falta el binario de Forja dentro de la imagen (hoy se copia a mano).
- **Aceptación**: despliegue reproducible de la app demo, con reinicio sin perder requests.

### Fase 6 — GUI de escritorio (10-15 días, en paralelo a 4)
- [ ] B15: `Layout`/`Widget` como valores reales (composable, generable desde datos).
- [ ] Eventos de ventana y ciclo de vida; timers; animaciones declarativas.
- [ ] Formularios: validación, submit, estados de error.
- [ ] Testing de UI: snapshots de layout (sin abrir ventana) + inspector.
- [ ] Virtualización de listas/tablas grandes.
- **Aceptación**: una pantalla de "lista de tareas" armada desde un `Arreglo` de datos, con
  formulario validado y test de snapshot.

**Total**: ~60-73 días-dev (≈3 meses a tiempo completo) para tener web + GUI de producto.
Solo el "hola mundo web funcional con BD y login" (fases 0-3) son ~27-33 días.

---

## 6. Mejoras concretas de GUI (priorizadas)

| # | Mejora | Por qué | Archivos |
|---|---|---|---|
| 1 | `Widget` como valor real, no nombre mágico | sin esto no hay UI programable ni testeable | `library/gui/gui.fa`, `gui-rt/src/evaluador.rs` |
| 2 | Un solo intérprete | hoy 3 dialectos; la mitad de los bugs son divergencias | `gui-rt/src/evaluador.rs`, `wasm-gui/src/lib.rs` |
| 3 | Formularios como unidad (validación, submit, errores) | es lo que hace falta en cualquier pantalla real | nuevo `gui-rt/src/formularios.rs` |
| 4 | Eventos de ventana + timers + animación declarativa | hoy solo click/teclado por widget | `gui_nativa.rs` |
| 5 | Virtualización de listas y tablas | listas de 10k filas se cuelgan | `gui_nativa.rs` |
| 6 | Snapshots de layout + `forja gui test` | hoy no se puede testear UI | nuevo |
| 7 | Inspector en vivo (árbol + estado + tiempos) | debug de UI sin adivinar | nuevo |
| 8 | Accesibilidad verificada (roles, foco, teclado) | requisito legal en muchos lados | `accessibility.rs` |
| 9 | CSS/tema unificado entre escritorio y web | hoy son dos sistemas de estilo | `gui-theme` |
| 10 | i18n de textos de widgets | `FAB`, `Calendar` muestran inglés | `gui-theme` |

---

## 7. Decisiones a tomar (y recomendación)

| # | Decisión | Opciones | Recomendación |
|---|---|---|---|
| D1 | UI web | canvas vs DOM | **DOM**; canvas solo si el producto es 100% dashboard/kiosco |
| D2 | Modelo de concurrencia | hilos+pool vs async real | **hilos+pool** ahora (B17), async después |
| D3 | Redis | nativo Rust vs `.fa` sobre sockets | **`.fa`** (RESP es texto); nativo solo si no rinde |
| D4 | Datos | SQLite vs Postgres | **SQLite** para empezar (ya hay soporte), Postgres como Fase 7 |
| D5 | Plantillas | strings vs motor | **motor mínimo con escape automático** (seguridad) |
| D6 | SSR | no / opcional / siempre | **opcional por ruta** (SEO donde importa) |
| D7 | Deploy | binario AOT vs VM | **AOT** (ya funciona) con fallback VM |

---

## 8. Riesgos

1. **El bug de `claves_mapa` es síntoma, no enfermedad**: sin tests de stdlib (Fase 0) cualquier
   fase siguiente se rompe igual. No saltear la Fase 0.
2. **Tres intérpretes**: si no se unifican (B16), el costo de cada feature web ×2 (escritorio y
   navegador) y el 90% de los widgets web seguirán sin existir.
3. **Canvas → DOM**: reescribir el renderer es trabajo real, pero es la única forma de tener
   accesibilidad, formularios y SEO. Hacerlo antes de sumar widgets.
4. **Concurrencia**: el evaluador tree-walking con `Arc<RwLock>` puede convertirse en cuello de
   botella con 100 requests; medir antes de optimizar y considerar mover el estado compartido a
   estructuras sin lock (arena por request).
5. **TLS**: sin servidor TLS no hay HTTPS ni HTTP/2 real; es un requisito de cualquier sitio
   público. `rustls` ya está en el árbol de dependencias (cliente), reutilizarlo.

---

## 9. Fuera de alcance (para no vender humo)

- Compilar `.fa` → wasm nativo (backend wasm): no hace falta para servir una web; la VM en wasm
  alcanza para el frontend. Sí hace falta para apps web sin servidor.
- Postgres/MySQL: recién después de cerrar SQLite + migraciones.
- GraphQL, gRPC, colas tipo Kafka: no son necesarios para "una página real".
- Multi-tenant, sharding, i18n avanzado: producto, no plataforma.

---

## 10. Anexo: cómo reproducir la medición

```bash
# 1) El servidor web no arranca (bug claves_mapa)
cat > /tmp/t_web.fa <<'EOF'
importar "std/servidor_web"
funcion main() {
    variable servidor = nuevo ServidorWeb(8799)
    servidor_web.registrar_obtener(servidor, "/", func(req, res) {
        servidor_web.html(res, "<h1>hola</h1>")
    })
    servidor_web.iniciar(servidor)
}
main()
EOF
forja run /tmp/t_web.fa     # → "función no definida: 'claves_mapa'"

# 2) SQLite sí funciona
#    (abrir/crear/insertar/consultar con parámetros)

# 3) Hilos reales
forja run ejemplos/concurrencia/126_hilo_simple.fa

# 4) Cobertura de widgets web vs nativos
grep -c 'Layout::' library/wasm-gui/src/lib.rs          # 38 usos, 10 variantes
# variantes nativas: 115 (enum Layout en gui-rt/src/gui_nativa.rs:439)

# 5) Nativos por intérprete
grep -c 'registrar(' compiler/forja-native/src/**/*.rs   # 206
grep -c '"' library/gui-rt/src/evaluador.rs              # ~60 nombres
grep -c '"' library/wasm-gui/src/lib.rs                  # ~10

# 6) Sin tests de comportamiento de stdlib
ls library/std/*.fa | wc -l                              # 47 módulos
grep -rl '@test' library/std | wc -l                     # 0

# 7) El smoke test solo importa
sed -n 1,40p tests/stdlib/stdlib_smoke.rs
```
