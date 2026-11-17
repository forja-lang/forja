# Handoff 2 — backend nativo y paridad con la VM

Fecha: 2026-11-12. Repo: `C:\Users\gaucho\forja`, rama `feat/llvm-nativo`.
Documentos hermanos: `plans/TRASPASO-BACKEND-NATIVO.md` (historia larga),
`plans/DECISIONES-PENDIENTES.md` (las 10 decisiones de producto), este handoff
(estado + cómo seguir).

> `plans/` está en `.gitignore`. Si querés versionar estos documentos:
> `git add -f plans/`. Ojo: `*.txt` también está ignorado (por eso
> `scripts/paridad-exclusiones` no lleva extensión).

---

## 1. Estado actual, medido (no de memoria)

```bash
cargo build --release -p forja-cli
bash scripts/paridad-native-vm.sh
```

Salida hoy (commit `a4abea9c`):

```
nativo vs VM: 145 coinciden · 3 difieren (sin excluir) · 128 no bajan a nativo
              · 5 no terminan en 20s · 2 salen con error explícito · 0 revientan
```

Los 3 que difieren son `101`, `104` y `105` (genéricos sin argumentos de tipo: ver
§4.2). Las exclusiones viejas de aritmética `144`, `149`, `164`, `167`, `168`,
`185` ya no son “aritmética de tramo”: `144` y `167` **coinciden**; en `149`,
`164`, `168` y `185` el que está mal es la VM (§4.2-bis).

Y de los tests:

| Suite | Resultado |
|---|---|
| `cargo test -p forja --test llvm_backend` | **38/38** |
| `cargo test -p forja --lib` | 127 |
| `cargo test -p forja --test corpus` | 299 |
| `cargo test -p forja-vm -p forja-cli -p forja-syntax` | 25 + 42 + 49 |
| clippy / fmt de mis archivos | limpios |

Ya **no revienta ninguno**: los 3 segfaults (`147`, `149`, `196`) se arreglaron
resolviendo el tipo **por valor en runtime** (ver §4.1). El «no bajan» subió de
101 a 126 porque la otra sesión agregó ejemplos nuevos (`ejemplos/`).

---

## 2. Antes de tocar nada (gotchas que cuestan horas)

1. **Otra sesión trabaja en el mismo worktree y commitea en la misma rama**, a
   veces con `git commit -a` y a veces con commits `chore: update …`. Antes de
   empezar: `git status --short` y `git log --oneline -10`. No mezcles sus
   archivos (`installer/`, `packaging/`, `src/transpiler.rs`, `library/*`,
   `compiler/forja-interprete`, la demo web) con los tuyos.
2. **`forja aot` cae a bytecode en silencio y sale con 0.** El único indicio es el
   mensaje `✅ AOT nativo: <ruta>`. Cualquier script de comparación que se fíe del
   código de salida está midiendo el intérprete contra sí mismo.
3. **`cargo --locked` falla** cuando la otra sesión toca `Cargo.toml`/`Cargo.lock`.
   Usá `cargo --offline` (y `--locked` sólo si el lock está quieto).
4. **`cargo`/`sccache`/`rustc` a veces quedan “no aplicable al toolchain”** unos
   minutos por la otra sesión: reintentá; si no, `cargo +stable-x86_64-pc-windows-msvc`.
5. **`forja.exe` bloqueado** al compilar: quedó un proceso vivo (un ejemplo con
   `recibir()` bloqueado). `ps -W | grep forja`.
6. `plans/` y `docs/` no están versionados; los números de benchmark publicados
   pueden estar viejos (la otra sesión compila en paralelo).

---

## 3. Qué se cerró en esta serie (39 → 9 diferencias)

Todo en `src/backend_llvm/` + `src/lib.rs` + `src/monomorph.rs` +
`compiler/forja-ir/src/optimizer/passes/dce.rs`. Commits propios (entremezclados
con los de la otra sesión):

```
df42051b  despacho dinámico por tag de clase (rasgos) + inferencia por forma
7c4a68b2  campos dinámicos, arr[i] = v, evidencia de tipos por llamada
79c2f88b  Booleano como tipo propio (verdadero/falso)
de8300cb  arreglos que arrancan vacíos y Resultado/Opcion como la VM
641fbd2c  mapas con valores de texto (tag por entrada)
a530709c  repr de Decimal (la más corta que vuelve al double)
1ac1720b  main() no se ejecuta solo
19f5ebad  arreglos de Decimal, texto mezclado como la VM, evidencia con locales
d93c85df  un slot declarado `nulo` se lee como Nulo
b6640be4  el retorno sin anotar también sale de un local
2c0ab0a1  las llamadas usan las versiones monomorfizadas
2466215b  inferir_campo resuelve parámetros y locales del método
0a7bc6f8  contratos (`requiere`) e `imprimir` como builtin
4827ca45  @derive(Igual) y comparación de enums por tag
69465665  @derive(Mostrar) y evidencia de constructores por clase
b23fcaac  Booleano gana a Entero al unificar retornos de despacho
c8757243  test: método booleano por despacho dinámico
c6428d6d  la evidencia de parámetros gana a la forma (2 vueltas)
4249aa24  payload de Resultado según la firma
d5e00625  monomorfización de los cuerpos genéricos (punto fijo)
0b0a179d  caché de ejecutables nativos + script de paridad
3a559a94  los campos toman el tipo de los parámetros del constructor
```

Infraestructura nueva que hay que usar:

- **`scripts/paridad-native-vm.sh`** — compara los 263 ejemplos (nativo vs VM) y
  clasifica: coincide / difiere / no baja / no termina / error explícito / crash.
  Falla sólo por diferencias no excluidas o por crashes.
  - `--verbose` lista cada caso; `--update` regenera la lista de exclusiones.
- **`scripts/paridad-exclusiones`** — las diferencias aceptadas, con motivo (9
  entradas). Editable a mano.
- **Caché de ejecutables nativos** (`compilar_llvm_nativo_cache`): `destino: auto`
  compila a `target/forja-cache/<hash>` → 6,2 s → 78 ms en la segunda corrida.
- **ASan funciona** con el clang de `llvm-mingw` (`clang -fsanitize=address`).
  Receta para diagnosticar un crash nativo:
  ```bash
  target/release/forja.exe llvm ejemplos/x.fa /tmp/x.ll
  clang -fsanitize=address -g -O0 -Wno-override-module -o /tmp/x.exe /tmp/x.ll src/backend_llvm/runtime.c
  /tmp/x.exe      # pila con la función y la línea del IR
  ```

---

## 4. Lo que queda, con causa y con lo ya descartado

### 4.1 Los 3 segfaults — **CERRADO** (commit `6a5d7a17`)

Diagnóstico con ASan y patrón único: **el programa guarda un número (o un
texto/mapa) donde la inferencia cree que va un arreglo, y la VM lo tolera
devolviendo `nulo`**.

Lo que se hizo, después de descartar los parches de más abajo: **resolver el
tipo por el valor en runtime**, como la VM.

- `runtime.c`: registro `puntero → tag` (`FORJA_T_TEXTO` / `FORJA_T_ARREGLO`).
  Los arreglos se registran en `forja_arreglo_nuevo`, los textos del heap en
  `forja_texto_alloc` y los literales desde `@forja_init`
  (`forja_registrar_texto`).
- `forja_indice_dinamico` / `forja_longitud_dinamica` /
  `forja_imprimir_dinamico` / `forja_a_texto_dinamico`: arreglo → casilla,
  texto → carácter/strlen, otra cosa → `nulo`/0/imprime el número. Eso es
  exactamente la VM (`longitud(6)`→0, `6[0]`→nulo, `escribir(6)`→`6`).
- El lowering usa esos caminos cuando el tipo del valor es **una suposición**
  (`Lowering::incierto`: elemento de un arreglo que vive en un campo, algo que
  salió de ahí, o el retorno de una closure).
- `forja_arreglo_validar` ahora pregunta el registro (`nulo` si la casilla no es
  un arreglo) en vez de abortar: los 3 ejemplos **coinciden** con la VM.
- Semántica de índices alineada con la VM: fuera de rango → `nulo` (antes
  abortaba, y eso hacía fallar `132`) y `arr[i] = v` más allá del final alarga
  con huecos nulos (`150`).

Intentos viejos, medidos y **revertidos** (quedan como historia, no repetir):

1. `Index` sobre un valor de tipo `Entero` → devolver `nulo`: pierde 4 ejemplos.
2. Chequeo “puntero < 4 KiB → nulo”: rompe `197_dijkstra`.
3. `longitud` con `phi`: movía el fallo a otro ejemplo.

La lección: el chequeo de plausibilidad en el lowering mueve el problema; el tag
del objeto lo resuelve. **Nunca desreferenciar un i64 sin preguntar el tag.**

### 4.2 Genéricos usados sin argumentos de tipo — **CERRADO** (`a4abea9c`)

`nuevo Par(1, "uno")`, `nuevo Cola()`, `identidad(42)`: la clase/función genérica
no se monomorfiza, así que `T` no tiene tipo concreto. El backend lo resolvía
como `Entero` y **convertía** los valores (`frente()` → 0, `potencia(termino, …)`
→ 0, la cola perdía los textos).

Lo que se hizo (sin trucos):

- `x: T` → la casilla es `i64` y el argumento viaja con sus **bits** (un texto es
  un puntero): `params_dinamicos` + `como_slot` en las llamadas (funciones,
  métodos y constructores).
- `-> T` → el `retornar` devuelve los bits tal cual (`ret_dinamico`), y quien
  llama sabe que el valor no tiene tipo estático (`incierto`) → se imprime y se
  concatena por su **tag** en runtime (lo de `6a5d7a17`).
- Evidencia de tipos: una cuenta (`(x - 1) / (x + 1)`) o un ternario también
  tipan un argumento; `Decimal` contagia.
- El pre-escaneo de cada cuerpo arranca con las pistas de **sus** parámetros y no
  arrastra nada del anterior.

Quedan `101`, `104` y `105`, y **sólo** al imprimir un `Booleano` o un `Decimal`
que sale de una casilla `T`: el `i64` no distingue `verdadero` de 1 ni un double
de un entero. Eso pide el **valor encajado** (`{tipo, bits}`):

- prototipo 1 y 2 en `git stash list` ("experimento cajas tipado dinamico",
  "intento 2 del modelo de cajas"). Ya tienen `forja_caja`, el tag
  `FORJA_T_CAJA`, y los desencajados (`forja_a_entero_dinamico`,
  `forja_a_decimal_dinamico`, `forja_verdad_dinamico`).
- Por qué no está: encajar/desencajar toca **todos** los constructos (arreglos,
  mapas, enums, comparaciones, aritmética) y cada sitio nuevo destapaba otro
  (arreglos guardan i64 crudos, `coincidir` compara i64, …). Hacerlo a medias
  arregla 3 ejemplos y rompe otros 3. Es un cambio de representación de valores,
  no un parche: hacelo con el árbol quieto y una sola sesión.

### 4.2-bis Diferencias donde **la VM es la que está mal**

Antes de “arreglar” estas, medir la VM: el nativo da el resultado correcto.

| Ejemplo | Nativo | VM |
|---|---|---|
| `149_tabla_hash` | inserta las 5 claves (bucket 0) | no inserta nada (buckets vacíos) |
| `164_ecuacion_segundo_grado` | `0 ± 0.866i` | `0 ± 0i` (pierde la parte imaginaria) |
| `168_regresion_lineal` | 59 / 79 (pendiente real) | 51 / 51 (constante: pierde la pendiente) |
| `185_ffi_matematicas` | `log(10) = 2.3025…` | `0` |

### 4.3 `72_genericos`

Necesita inferir el retorno de `retornar este.campo` **y** acotar `inferir_campo`
por clase en el mismo cambio. Medido cuatro veces: arregla `72`, rompe `176`
(`ComandoRestar.valor_anterior` acaba `Texto` y el despacho de `deshacer` unifica
a texto). El hilo pendiente es entender por qué ese campo aparece `Texto`.

### 4.4 `107_generico_multiple`

`combinar_pares<T, U, V>(par1: Par, par2: Par)`: los parámetros de tipo declarados
no se usan en la firma, así que no hay de dónde inferir la especialización.

### 4.5 `184_ffi_simple`

FFI de `rand`/reloj: no determinista, no puede coincidir. Está en las exclusiones.

### 4.6 `canales` y `playground_ejemplos`

**La VM es la que falla**: `async_await/canales.fa` no termina en el intérprete
(2+ min) y `test_ejemplos_playground_ejecutan` se cuelga. La ruta nativa corre
`canales` y termina. Si se toca el intérprete, empezar por ahí.

### 4.7 Los 101 que no bajan a nativo

Por volumen (aproximado, ver `target/paridad/*.aot`):

| Falta | Ejemplos | Coste |
|---|---|---|
| `leer()` (stdin) | ~11 | bajo (stdlib de C) |
| `_archivo_*` (FS) | ~4 | bajo |
| `escribir_linea` | ~3 | bajo |
| `seleccionar` (multi-canal) | ~7 | medio |
| `unir` de hilos | ~6 | medio |
| red/sockets/HTTP | ~10 | alto (sandbox, timeouts) |
| GUI (requiere host de widgets) | ~40 | fuera del alcance del backend |

---

## 5. Decisiones de producto pendientes

Están en `plans/DECISIONES-PENDIENTES.md` (10 puntos, con opciones y mi
recomendación). Las que ya están implementadas: **8-B** (caché nativa) y **5/6**
(arnés reproducible + exclusiones). Las que faltan decidir:

1. ¿Paridad exacta con la VM o “nunca mentir” (error explícito)? → **B**.
2. Aritmética dinámica: ¿flujo por tramo (2–3 días) o documentar?
3. `72`: ¿invertir en el diagnóstico o dejarlo?
4. `107`: ¿heurística por uso, error explícito o dejarlo?
7. Qué parte de la stdlib nativa se implementa (mi orden: `leer`/archivos →
   `seleccionar` → red).
10. Versionar `plans/` y separar ramas de las dos sesiones.

---

## 6. Cómo verificar cualquier cambio

```bash
# 1. Compilar
cargo build --offline --release -p forja-cli

# 2. Paridad (lo más importante: compara 263 ejemplos nativo vs VM)
bash scripts/paridad-native-vm.sh                 # falla por diffs no excluidas
bash scripts/paridad-native-vm.sh --verbose       # caso por caso

# 3. Tests
cargo test --offline -p forja --test llvm_backend
cargo test --offline -p forja --lib
cargo test --offline -p forja --test corpus
cargo test --offline -p forja-vm -p forja-cli -p forja-syntax
cargo clippy --offline -p forja --lib

# 4. Un ejemplo a mano
target/release/forja.exe llvm ejemplos/x.fa /tmp/x.ll     # IR
target/release/forja.exe aot  ejemplos/x.fa /tmp/x        # nativo (ojo: fallback)
/tmp/x.exe
target/release/forja.exe ejecutar ejemplos/x.fa           # VM
```

**Regla de oro**: cualquier cambio en `src/backend_llvm/` se valida con el script
de paridad *antes* de commitear. Las cifras de referencia son
`142 / 0 / 101 / 4 / 6 / 3` (coinciden / difieren / no bajan / no terminan /
error / revientan). Si un cambio sube “difieren” o baja “coinciden”, no vale.

---

## 7. Mapa de los archivos que importan

| Archivo | Qué hay |
|---|---|
| `src/backend_llvm/lowering.rs` | tipos (`T`/`V`), tabla de clases (campos, métodos, `tag`, `generica`), `RUNTIME` (declaraciones C), `como`/`como_slot`/`casilla_a_valor`, `tipo_estatico`, inferencia por forma (`inferir_campo`, `inferir_parametro`, `inferir_retorno`, `evidencia_de_llamadas`) y el paso `compilar` |
| `src/backend_llvm/lowering/expresiones.rs` | expresiones: aritmética, comparaciones, llamadas, despacho dinámico (`abrir_despacho`/`cerrar_despacho`), closures, hilos, canales, mapas, arreglos, enums + `coincidir`, objetos, `?` |
| `src/backend_llvm/lowering/sentencias.rs` | sentencias, `funcion`/`metodo`, contratos |
| `src/backend_llvm/runtime.c` | runtime: arena, texto, arreglos, mapas, canales, `Exacto` (`__int128`) |
| `src/lib.rs` | `compilar_a_llvm*`, `podar_inalcanzable` (DCE previo), `monomorphize_programa`, `memoizar_puras` |
| `src/monomorph.rs` | monomorfización de funciones y clases (punto fijo, cuerpos) |
| `src/tools/forja-cli/src/commands/ejecucion.rs` | compilación nativa con clang (`-O3 -flto`) y caché |
| `src/tools/forja-cli/src/commands/aot.rs` | `forja aot` (con el fallback silencioso a bytecode) |
| `tests/llvm_backend.rs` | 36 tests que compilan y ejecutan con clang |

Cada limitación está anotada en el código como comentario `shortcut:` con el
motivo y cuándo mejorarla (`grep -rn "shortcut:" src/backend_llvm/`).
