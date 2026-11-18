# HANDOFF 9 — runtime nativo, paridad y qué queda

> **Alcance**: el backend nativo/LLVM (`src/backend_llvm/**`), el runtime C, la
> paridad nativo↔VM sobre `ejemplos/` y lo que quedó abierto del
> `plans/PLAN-SUBAGENTES.md` (carriles L1–L4 cerrados; L5 sigue pendiente).
> La pila web/GUI tiene su propio handoff: `plans/HANDOFF-WEB-Y-GUI.md`.
> La bitácora cronológica completa está en `handoffsessions.md`.

**Estado al escribir esto**: `main` = `13a3e38f`, árbol medido en el worktree
`../forja-integracion` (ver §8: **no medir en `main`**).

---

## 1. Estado en una pantalla

| Métrica | Base del plan (`00614f6b`) | Ahora |
|---|---|---|
| Paridad `ejemplos/` | 150 · 0 dif · 95 no bajan · 4 no terminan · 4 error · 0 revientan | **187 · 0 dif · 53 no bajan · 3 no terminan · 4 error · 0 revientan** |
| `--test llvm_backend` | 48 | **56** |
| `--test llvm_runtime` | — (nuevo) | **3** |
| `--lib` | 129 | **129** |
| `--test corpus` | 299 | **299** |
| `--test runtime` | colgaba | **102 (0.5 s)** |
| `parseo` (bench) | 546 ms · 1.03x | **~570 ms · 1.05x** (antes del fix: 2160 ms) |
| `cargo clippy -p forja --lib` | — | limpio |

`cargo fmt --check -p forja` **sigue fallando por 2 archivos ajenos**
(`src/transpiler.rs`, `tests/codegen/ir_cobertura_corpus.rs`): ya estaba así en
la base; no lo tocamos.

**Todo lo que sigue está mergeado y pusheado en `main`**, y los 14 gitlinks de
submódulo resuelven desde un clon limpio (ver §8.3).

---

## 2. Cómo verificar (comandos exactos)

```bash
# Gate completo (compila release, paridad y tests). ~12 min.
bash C:/tmp/gate.sh           # ojo: adentro está GATE_DIR; ver §8.1

# A mano, en el worktree de integración:
cd ../forja-integracion
cargo build --offline --release -p forja-cli
bash scripts/paridad-native-vm.sh
cargo test --offline -p forja --test llvm_backend
cargo test --offline -p forja --test llvm_runtime
cargo test --offline -p forja --lib
cargo test --offline -p forja --test corpus
cargo test --offline -p forja --test runtime
cargo test --offline -p forja --test golden_runner   # lee la VM, no el nativo
cargo clippy --offline -p forja --lib
```

Referencia de paridad que **no** puede empeorar: `0 difieren` (fuera de
`scripts/paridad-exclusiones`), `0 revientan`, y "coinciden" no baja de 187.

---

## 3. Qué se hizo (por área, con el commit que lo explica)

### 3.1 Integración de los cuatro carriles
`native/lowering` (L1), `native/runtime` (L4), `vm/cuelgues` (L2) y
`corpus/dorado` (L3) están mergeados en `main`. El único conflicto real fue
`runtime.c`: se resolvió **a mano** quedándose con la implementación de hilo de
L1 (`forja_hilo_lanzar` devuelve handle + registro de handles vivos) y
descartando el par duplicado que había hecho L4. Los submódulos (`forja-types`,
`ejemplos`) se trajeron de los worktrees de cada carril.

### 3.2 Runtime nativo (`src/backend_llvm/runtime.c`)
- **Hilos unibles**: `forja_hilo_lanzar` devuelve handle; `forja_hilo_unir`
  espera y devuelve los bits del `retornar` del cuerpo. `unir()` sobre algo que
  no es un hilo devuelve el valor (como la VM).
- **Rendimiento de `parseo` (3.63x → 1.05x)**: cada texto nuevo entraba al
  *registro de tags* (12 M altas en 2 M iteraciones, tabla de ~33 M entradas).
  Ahora un puntero de la **arena** es texto por rango de bloque (con caché del
  último bloque) y el registro queda para arreglos, mapas, textos grandes
  (>256 KB, fuera de arena) y literales. `forja_tag_de` = registro → canal →
  arena → escalar.
- **Canales**: llevan una cabecera con magia `0xFF…` (imposible en un texto
  UTF-8) para que `forja_tag_de` los distinga sin tabla por instancia;
  `tipo(canal)` → `"objeto"` y su repr `[<CanalEmisor>,<CanalReceptor>]`, como
  la VM.
- **`texto[i]` sin alocar**: los 256 textos de un carácter se cachean (un
  recorrido `texto[i]` alocaba uno por vuelta).
- **`leer()`**: recorta los espacios como `texto_de_lectura` de la VM (`  x  ` →
  `x`, línea en blanco → texto vacío, sólo EOF → `nulo`).
- **Primitivas nuevas** (mismo contrato que `forja-native`): `forja_arreglo_concat`,
  `forja_base64_codificar/decodificar`, `forja_hex_decodificar`,
  `forja_archivo_copiar`, `forja_fecha_desde_timestamp`, `forja_sha256`, y
  `forja_guardar_args`/`forja_argumentos` (`main` ahora recibe `argc`/`argv`).

### 3.3 Lowering (`src/backend_llvm/lowering.rs` + `lowering/*.rs`)
- **Poda (DCE) de programa**: `main` es raíz aunque nadie la llame, y se
  recorren las variantes que faltaban (`ArregloFijar`, `Referencia`,
  `CanalNuevo`, `Si` como expresión, `Seleccionar`). Era la causa de "clase X
  desconocida" con la clase declarada en el mismo archivo.
- **Identificadores con acento**: `variable año` emitía `%año.0` y clang
  rechazaba el `.ll`. `sanear_identificadores` (en las dos entradas del backend)
  cambia cada carácter no ASCII **de un identificador** por `_`; las cadenas y
  los comentarios quedan intactos.
- **`x == nulo`**: comparaba con la regla "texto contra número" y daba siempre
  `falso`. Ahora compara los bits con 0 (clave para reconocer EOF).
- **`[v] + arr`**: bajaba como suma de enteros (`ptrtoint`+`add`) y reventaba.
  Ahora `forja_arreglo_concat` (sólo arreglo+arreglo; `texto + arreglo` sigue
  por la vía de texto).
- **Modelo de objetos**: `tipo(v)` responde como la VM (clase/enum → `"objeto"`,
  canal → `"arreglo"`, evaluando el valor **una sola vez**); `-> Resultado` /
  `-> Opcion` sin genéricos se tipan como los enums del preludio (por eso
  `variable r = con_timeout_async(...)` imprime `<Resultado>`).
- **`r.valor` / `r.error`** sobre un `Resultado` en variable: payload de la
  variante y `nulo` si no corresponde, con el tipo de la firma.
- **`variable x, y = f()`**: reparte casillas cuando el arreglo tiene tipo
  conocido; con `-> auto` (mixto) se sigue rechazando (ver §5.1).

### 3.4 VM (`compiler/forja-vm`, `forja-bytecode`, `forja-ir`)
- `Ninguno` es variante del preludio (ya no "variable no declarada").
- Una sola semántica de `leer()` en las dos rutas (`texto_de_lectura`).
- **`escribir_linea` imprimía tres saltos**: los generadores de bytecode e IR
  agregaban un `"\n"` sobre `Imprimir`, que ya cierra la línea. Ahora uno.

### 3.5 Ejemplos (`ejemplos/`, submdódulo)
Rama `corpus/dorado` + `corpus/ejemplos-cierre` (ambas en `origin`):
11 ejemplos que el formateador había partido (`resultado` / `llamada` en dos
líneas), `158`, `156`, `159` (`a_caracter`), `185` (reconstruido desde
`9074678`), `216` (`std/resultado` + `es_ok(r1)`), y `151`/`152`/`159` cortan
con `salir(0)` cuando no hay entrada (`pedir_numero_o_fin`).

---

## 4. Dónde tocar cada cosa (mapa rápido)

| Quiero… | Archivo | Nota |
|---|---|---|
| Añadir una primitiva nativa | `runtime.c` (función) + `lowering.rs` (`const RUNTIME`, firma `tipo (params)`) + `lowering/expresiones.rs` (`NATIVAS_STDLIB`, nombre `_x` → función C y tipos) | test en `tests/llvm_runtime.rs` (driver en C) |
| Cambiar la semántica de un builtin | `lowering/expresiones.rs` (`fn builtin`) | mirar `fn imprimir`, `tipo`, `longitud`, `Index` |
| Toque de tipos / "incierto" | `Lowering::tipo_estatico`, `incierto`, `como`, `desde_slot` | `inciertos` = nombre de variable cuyo tipo estático miente |
| Poda del programa | `src/lib.rs` → `podar_inalcanzable` | toda `Expresion`/`Declaracion` nueva hay que agregarla ahí |
| Repr / `escribir` de un valor | `lowering/expresiones.rs` → `fn imprimir` | por tipo estático; la vía dinámica es `forja_imprimir_dinamico` |
| Nombres/acentos | `src/lib.rs` → `sanear_identificadores` | se aplica en `compilar_a_llvm` y en `programa_para_backend_nativo` |
| Hilos/canales | `runtime.c` (§ hilo/canal) | el handle de hilo va con `malloc`, **fuera** de la arena |

**Regla del registro de tags** (`forja_tag_de`): registro (arreglos, mapas,
textos >256 KB, literales) → canal (cabecera mágica) → arena = texto → 0 =
escalar. Cualquier objeto nuevo que se guarde como `i64` y pueda llegar a una
ruta dinámica tiene que entrar en esa cadena, o se clasificará como texto.

---

## 5. Lo que queda abierto (con causa medida y por dónde atacarlo)

### 5.1 Encajado de escalares — **bloqueado por producto**
Un slot nativo no lleva tipo: `nulo` se vuelve `0`, `falso` se vuelve `0` y un
`Decimal` guardado en una casilla "Entero" se trunca. La VM decide por el valor
del momento. Afecta:

| Ejemplo | Síntoma |
|---|---|
| `109_generico_opcion` | un campo sin tipo que guarda un texto se imprime `0` (excluido) |
| `102_generico_intercambiar` | casilla de arreglo mixto: `(0, Activo)` / `(100, 3)` en vez de `(falso, …)` / `(100, 3.14)` |
| `143_pila`, `194_enums_tuplas` | "arreglo heterogéneo (textos y números en el mismo literal)" — la prohibición sigue puesta: al quitarla, `143` imprime `*` donde la VM imprime `25` |
| `194_expresiones_regulares` | `patron[j]` sobre un parámetro sin tipo devuelve un entero y la comparación da `falso`; con ese cambio el programa termina pero imprime distinto |
| `146_arbol_avl`, `186_gestion_memoria` | `nulo.campo` aborta ("acceso a `valor` sobre un objeto nulo") / el despacho dinámico no encuentra el método; la VM devuelve nulo |

**Probado y revertido** (no repetir sin cuidado): marcar todos los parámetros
sin tipo como `inciertos` arregla el cuelgue de `194` pero rompe `162_aprox_e` y
`400_exacto_debug` (imprimen Decimal/Exacto como entero).

**Decisión pendiente** (`DECISIONES-PENDIENTES.md` §1 y `handoff7` §2):
¿paridad exacta con la VM o "nunca mentir"? Hasta que se decida, no se toca.

### 5.2 `seleccionar` — 7 ejemplos, implementación nueva (no bloqueada)
`116`, `117`, `118`, `119`, `120`, `73_seleccionar`, `160_cronometro`: el
lowering corta con "`seleccionar` no está soportado por la ruta LLVM".
Camino sugerido: en `runtime.c`, `forja_canal_intentar` ya existe; implementar un
`forja_seleccionar(ptr arreglo_de_canales, i64 timeout)` que haga polling con
`Sleep`/`nanosleep` corto (1 ms) y devuelva el índice del canal listo (o -1 por
timeout / default). Después, el lowering de `Expresion::Seleccionar` arma el
arreglo y despacha por índice. Riesgo: `forja_canal_intentar` no tiene mutex
global, así que el polling es por canal; es correcto aunque no óptimo.

### 5.3 Primitivas fuera de alcance del plan (~10 ejemplos)
- **Red** (`200`, `202`, `203`, `204`, `218`, `221`, `219_websocket`, `217`):
  `_socket_tcp_conectar`, `_socket_cerrar`, `_compartido_nuevo` — es un cliente
  y servidor TCP/UDP con epoll/select, proyecto aparte.
- **JSON** (`209`): `_json_parsear` devuelve un valor dinámico; conviene
  implementarlo en C devolviendo mapa/arreglo de Forja.
- **XHB** (3 ejemplos de `otros/`): `_xhb_parsear`, formato propio.
- **`220_servidor_h2`**: llamadas con nombre de módulo (`servidor_h2.x`) que el
  backend nativo no resuelve.

### 5.4 GUI — 28 ejemplos, por diseño
`aot` rechaza apps GUI ("❌ AOT nativo no soporta apps GUI"). No es un bug:
la GUI corre sobre `gui-rt`/`gui-dom`, no sobre el binario autónomo.

### 5.5 Menores
- **`189_benchmark_manual`**: la **VM** tarda >60 s (el nativo falla en 35 s).
- **`501_contratos_error`**: esperado (el contrato falla en ambas rutas; el arnés
  sólo compara salidas con rc 0).
- **`baseline.json`**: no se regeneró. Los `rust_ms` bailan entre corridas
  (ffi 45 → 104 ms), así que hornear un momento deja ratios irreales; los
  tiempos de Forja sí son estables. Regenerar en CI o con la máquina quieta.
- **`paridad-exclusiones`**: 28 entradas, todas con motivo medido. Revisar sobre
  todo las de hilos (128/129/130/173: intercalado) y reloj (208/210/213/153).

---

## 6. Exclusiones actuales (28) y por qué

Grupos: hilos con intercalado no determinista (128, 129, 130, 173, `async_await/*`),
reloj/aleatorio (153, 208, 210, 213, 159), `nulo` vs `0` / §9 (109, 185),
la VM tarda o cuelga (`189`, `async_await/canales`), aviso del compilador que la
VM imprime y el binario no (`200`), y GUI (`900_storybook`, `201_login_gui`).
Cada línea del archivo tiene el motivo medido: **no agregar entradas sin medir**.

---

## 7. Decisiones de diseño que conviene no deshacer

1. **Nothing se libera**: la arena no se libera; el handle de hilo y los textos
   de un carácter son las únicas cosas fuera de esa regla (el handle se libera
   al unir; el caché de caracteres es estático).
2. **Nada de tablas por instancia en el camino caliente**: ya costó 3.6x en
   `parseo`. Si hace falta distinguir objetos, usar cabecera mágica (como los
   canales) o un campo que ya exista (el tag de clase), nunca un `HashMap` por
   objeto.
3. **Los `\r\n` del repo son mixtos**: al hacer reemplazos con scripts, usar
   índices o `split('\n')`, no patrones con `\n` fijos.
4. **`forja_hilo_lanzar` ya no es "detached"**: el cuerpo devuelve `i64` y el
   handle se une. Si se toca el lowering de `hilo {}`, mantener ese contrato.

---

## 8. Trampas del entorno (esto ahorra horas)

**8.1 cmd vs git-bash.** `bg_run` ejecuta con `cmd.exe`: `cd /c/...` y `tail` no
existen. Para scripts bash usar `"C:\Program Files\Git\bin\bash.exe" ruta.sh` y
rutas `C:\...`. Un script lanzado desde cmd con `bash` a secas usa el **WSL**
(`/mnt/c/...`) y falla.

**8.2 Las herramientas de edición resuelven rutas relativas al cwd de la
sesión** (el árbol `main`), no al worktree del carril. Usar **rutas absolutas**
siempre que se edite en `../forja-*`.

**8.3 Submódulos.** Cada worktree tiene su propio clon de submódulo
(`.git/worktrees/<wt>/modules/...`); los commits no se ven entre worktrees. Para
integrar: `git -C <sub> fetch <ruta del otro worktree> <sha>`. Los 4 pines que
faltaban en `origin` (`forja-syntax`, `library/std`, `library/wasm`,
`library/android`) ya se pushearon.

**8.4 Medir en `../forja-integracion`, no en `main`.** El árbol `main` tiene
**30 `.fa` sin commitear** de otra sesión (`ejemplos/stdlib/*`,
`fundamentos/720*`, `721*`) que inflan la paridad (293 vs 263 ejemplos).
Antes de medir: `git status` limpio en el worktree y `ejemplos` en el puntero de
`HEAD`.

**8.5 Tests en Windows.** Los tests que compilan C en un directorio temporal
necesitan un nombre único por test (reloj del sistema tiene resolución de ms):
usar un contador atómico (ver `tests/llvm_runtime.rs`).

**8.6 El gate compila primero.** `paridad-native-vm.sh` usa
`target/release/forja.exe`: si se mergeó código sin recompilar, se mide el
binario viejo (pasó dos veces).

---

## 9. Pendiente de L5 (no lo hizo este carril)

- Medición limpia final en `main` sin archivos sucios (bloqueada por los 30
  `.fa` ajenos: decidir si se commitean o se borran).
- CI: el job `Métricas de documentación (auto)` sigue fallando.
- Versionar `plans/*.md` con `git add -f`.
- Regenerar `baseline.json` con la máquina quieta.
