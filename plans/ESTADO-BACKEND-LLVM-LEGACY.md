# Estado del backend LLVM: ¿por qué seguimos usando el legacy?

Fecha de revisión: 2026-11-09.
Archivos revisados: `src/backend_llvm.rs`, `src/backend_llvm/legacy.rs`, `src/lib.rs`,
`src/tools/forja-cli/src/commands/{run.rs,consola.rs}`, `src/aot.rs`, `tests/llvm_backend.rs`,
`docs/src/pages/arquitectura/llvm.astro`.
Bug pendiente relacionado: `plans/BUGS-BACKEND-LLVM.md`.

---

## 1. Respuesta corta

**Sí, seguimos usando el legacy. No está "retenido por compatibilidad": es hoy el único
motor de LLVM que existe.**

Todo lo que produce IR LLVM de un programa Forja real pasa por
`src/backend_llvm/legacy.rs`. El backend nuevo (`src/backend_llvm.rs`) son piezas sueltas
(tipos, instrucciones, GC), sin capacidad de compilar un AST.

---

## 2. Qué hay en cada archivo

| Archivo | Líneas | Qué es | ¿Compila programas? |
|---|---|---|---|
| `src/backend_llvm/legacy.rs` | 1577 | Lowering real AST → IR textual. Emite sintaxis LLVM vieja (`i64*`, punteros tipados). | **Sí, es el único** |
| `src/backend_llvm.rs` | 1194 | Andamiaje: `LlvmType`, `LlvmInstruction`, `LlvmModule`, enum/struct layout, intrínsecas GC, passes, tests. Emite `ptr` opaco. | **No** |

En `src/backend_llvm.rs` no existe **ninguna** función que reciba
`&[Declaracion]` ni `&Expr`. Solo hay constructores de piezas (`new_temp`,
`register_class`, `add_string_literal`, `inject_gc_intrinsics`,
`to_llvm_ir_with_gc`). Su propio encabezado lo admite (líneas 1-8):

> "Todavía **NO implementa lowering AST→LLVM** … El lowering real AST→LLVM hoy vive en
> el módulo interno/legacy".

Y `legacy.rs` también lo admite (líneas 1-15):

> "Es el ÚNICO módulo con lowering real AST→LLVM … usarlo para nuevos desarrollos,
> no agregar funcionalidades aquí."

---

## 3. El único puente público apunta al legacy

```rust
// src/backend_llvm.rs:1190
#[path = "backend_llvm/legacy.rs"]
pub mod legado;

pub fn compilar_a_llvm_texto(declaraciones: &[crate::ast::Declaracion]) -> Result<String, String> {
    let mut backend = legado::LlvmBackend::new("", "forja_module");   // ← legacy
    backend.compile(declaraciones)?;
    Ok(backend.emit_ir())
}
```

Esa función es la única entrada al backend y solo la construye sobre `legado`.

---

## 4. Quién consume ese camino

Todos entran por `forja::compilar_a_llvm` (`src/lib.rs:1646`), que hace
lexer → parser → type checker → borrow checker → mono → optimizador AST →
`backend_llvm::compilar_a_llvm_texto` (o sea, legacy).

| Comando / API | Dónde | Notas |
|---|---|---|
| `forja llvm archivo.fa [salida.ll]` | `consola.rs:460` | Genera el `.ll` para inspeccionar |
| `forja compilar archivo.fa` | `consola.rs:72` → `compilar_llvm_a` | **Primer intento** de la ruta nativa; si falla, cae al AOT de bytecode |
| `forja ejecutar` (modo automático) | `run.rs:135` → `ejecutar_llvm_nativo` | **Antes** que el ASM propio; si falla el clang, prueba ASM y después bytecode |
| REPL | `consola.rs:72` | El mismo `compilar_llvm_a` |
| `compilar_a_llvm_desde_ir` | `lib.rs:515` | Ni siquiera usa el `forja-ir`: saca el AST embebido y va al legacy |

Nota: `src/aot.rs` **no** usa LLVM. Genera un ejecutable con bytecode embebido
(`forja-driver::aot`). La documentación dice lo contrario (ver §7).

---

## 5. Capa extra: parches en texto sobre la salida del legacy

`src/lib.rs` no devuelve el IR crudo. Le aplica dos post-procesos de ~200 líneas:

| Función | Línea | Qué hace |
|---|---|---|
| `normalizar_llvm()` | `lib.rs:1797` | Mueve instrucciones de nivel superior dentro de `main`, saca strings globales, borra instrucciones después de `ret`, renumera `%N` por función, corrige `%%label` y la firma de `main` |
| `memoizar_puras()` | `lib.rs:1692` | Cachea funciones puras recursivas de `i64 -> i64` |

Que esto exista es la prueba de que el legacy emite IR inválido de fábrica. No es un
backend: es una tirita encima de otra tirita, y está incompleta (ver §6.2).

---

## 6. Estado real de los bugs del plan (reproducidos con `target/release/forja.exe`)

### 6.1 §1 — `Texto + Entero` fuera de `escribir`: **ABIERTO**

Entrada:
```forja
funcion main() {
    variable a = 5
    variable s = "x = " + a
    escribir(s)
}
```

IR generado (extracto real):
```llvm
%v1 = ptrtoint i8* %v0 to i64     ; dirección de "x = "
%v2 = load i64, i64* %a.a         ; 5
%v3 = add i64 %v1, %v2            ; puntero + 5  ← basura
store i64 %v3, i64* %a.s
```

Imprime un número enorme. En `escribir` sí funciona (se arregló solo el caso de
impresión, §0 del plan); en cualquier otro sitio miente en silencio.

Alcance: asignaciones, `retornar`, argumentos, campos de clase, `+` anidado.

### 6.2 §2 — Variables de nivel superior sin `main`: **ABIERTO**

Entrada:
```forja
variable a = 5
escribir("x = " + a)
```

IR generado (extracto real):
```llvm
declare void @forja_contract_error(i8*)
  store i64 5, i64* %a.a     ; ← nivel superior, inválido
define i32 @main() {
  %a.a = alloca i64
  store i64 5, i64* %a.a     ; ← duplicado
```

`clang c.ll` falla:
```
c.ll:5:3: error: expected top-level entity
```

Causa: `normalizar_llvm` solo mueve líneas que empiezan con `%` o `call `
(`lib.rs:1818`), no `store` ni `alloca`. Además se duplican los strings globales
(`@.str.0..2` y `@.str.3..5`) porque `decl()` emite y `auto_main` reemite.

---

## 7. Documentación desactualizada

`docs/src/pages/arquitectura/llvm.astro` afirma:

> "el AOT con LLVM usa `llc` + `clang`"

Falso hoy: `src/aot.rs` produce bytecode embebido. El LLVM solo aparece en
`forja llvm`, en el primer intento de `forja compilar` y en el primer intento de
`forja ejecutar`.

---

## 8. Por qué no se migró (diagnóstico)

1. **El backend nuevo no fue un reemplazo, fue un andamiaje paralelo.** Implementa
   tipos, layout de structs/enums, intrínsecas GC y un `LlvmModule` que **duplica** lo
   que `forja-ir` ya hace. El lowering que falta no es AST → `LlvmModule`: si se quiere
   LLVM de verdad, el camino correcto es **`forja-ir` → LLVM**.
2. **Los dos backends son incompatibles de convivencia**: el legacy emite punteros
   tipados (`i64*`) y el nuevo `ptr` opaco. Hay que elegir, no mezclar.
3. **No hay presión para cerrarlo** porque el legacy "funciona" en programas simples.
   Pero funciona **mintiendo**: imprime números en vez de dar error (§6.1).
4. Cerrar §1/§2 nunca se hizo, y cada bug nuevo se tapa con otro parche de texto en
   `normalizar_llvm`.

---

## 9. Qué hacer (de menor a mayor esfuerzo)

### Paso 1 — 10 líneas, hoy
Rechazar la concatenación fuera de `escribir` con un error claro en español
(opción 1 del plan, §1). Convierte una mentira silenciosa en un mensaje de error.
Es coherente con el resto del backend, que ya devuelve `Err` para `LiteralExacto`,
`si` en argumentos, etc.

### Paso 2 — cerrar §2
Dos alternativas:
- **(a)** En `normalizar_llvm`, mover también `alloca`/`store` de nivel superior y
  evitar globals duplicados. Rápido, pero sigue siendo un parche de texto.
- **(b)** En `compile()` del legacy, buffear `decl()` y, si no hay `main`, emitir solo
  lo que no sea variable/asignación de nivel superior (auto_main ya las reemite).
  Es el arreglo en origen. **Ojo**: afecta también a `compilar_a_llvm_desde_ir`, que
  no pasa por `normalizar_llvm`; hay que verificar que no se dupliquen las `Funcion`.

### Paso 3 — el arreglo de fondo
Dejar de invertir en `legacy.rs`. Escribir el lowering **`forja-ir` → LLVM** y hacer
que `compilar_a_llvm` consuma el IR. `src/backend_llvm.rs` sirve como catálogo de
tipos e intrínsecas GC, poco más. Mientras tanto, el backend completo y correcto sigue
siendo el ASM propio (`src/compiler_asm/`).

---

## 10. Tests que faltan (del plan, siguen sin existir)

Agregar a `tests/llvm_backend.rs`:

- `"x = " + 5` en variable e impreso → `x = 5`
- `"a" + 1 + "b"` en `retornar` → texto correcto
- programa sin `main` con variable de nivel superior → un solo `alloca` y compila con clang

Los tres fallarían hoy.

---

## 11. RESUELTO (2026-11-09): migración al backend nuevo

`src/backend_llvm/legacy.rs` **ya no existe**. Todo el lowering AST → LLVM vive
en `src/backend_llvm/lowering.rs` (+ `lowering/{expresiones,sentencias}.rs`) y
`compilar_a_llvm_texto` lo usa directamente. `normalizar_llvm` (el parche de
texto de §5) quedó fuera del camino: el IR nuevo es válido de entrada.

Qué bajó el backend nuevo (antes no existía en ninguna ruta):

| Capacidad | legacy | nuevo |
|---|---|---|
| Globals con `@g_x = internal global` + init en `@forja_init` | ✗ (IR inválido) | ✓ |
| Punteros opacos (`ptr`) | ✗ (`i64*`) | ✓ |
| Valores tipados (`Texto`=ptr, `Decimal`=double) | ✗ (todo i64) | ✓ |
| `"x = " + n` concatena (bug §1) | ✗ (sumaba punteros) | ✓ |
| `@main` C correcto (llama a init y al main del usuario) | ✗ | ✓ |
| Arreglos (`[1,2,3]`, `arr[i]`, `empujar`, `longitud`) | ✗ | ✓ (runtime C) |
| Clases: constructor, campos, métodos | ✗ (no-op) | ✓ |
| FFI (`funcion externa`) con firmas reales | parcial | ✓ |
| Punteros a función (dispatch por tabla) | ✗ | ✓ |
| Mapas, enums, closures, hilos | ✗ | **error explícito** (no mienten) |
| Runtime de soporte | inline disperso | `src/backend_llvm/runtime.c` |

`forja aot` compila **7 de los 10 benchmarks** de
`C:\Users\gaucho\Desktop\competencias` (antes: 0). Tiempos de pared (mínimo de 3,
mismo host, `clang -O3 -flto`):

| benchmark | VM (antes) | nativo (ahora) | Rust `-O` | ratio vs Rust |
|---|---|---|---|---|
| ffi | 446 ms | **38 ms** | 39 ms | 0,97× |
| dispatch (10M) | 8 000 ms | **61 ms** | 52 ms | 1,17× |
| parseo (2M) | 7 469 ms | **1 035 ms** | 489 ms | 2,12× |
| memcache (4096²) | 8 858 ms | **287 ms** | 78 ms | 3,68× |
| vectores | 584 ms | **104 ms** | 63 ms | 1,65× |
| overflowpanic (50M) | 22 298 ms | **38 ms**¹ | 38 ms¹ | 1,00× |
| fib(42) | (memoiza) | **764 ms** | 839 ms | **0,91×** |

¹ el original de Rust es DCE'd (100 ns); la variante justa mide 27 ms.

Pendientes (con dueño en `PLAN-VENCER-A-RUST.md`): mapas, enums y closures en la
ruta nativa; memoización sobre el IR nuevo; y elusión de bounds checks
(`memcache` es el único que sigue lejos, por los chequeos por celda).

## 12. Segunda tanda (2026-11-09): mapas y enums

Se agregaron al backend nativo:

- **Mapas** (`{}`, `m[k] = v`, `m[k]`, `longitud(m)`) con `runtime.c`: tabla
  abierta con dos espacios de claves (enteros y texto), hasher propio
  (`0x9E3779B97F4A7C15` para i64, FNV-1a para texto). `shortcut:` `m[1]` y
  `m["1"]` son claves distintas (en la VM son la misma porque la clave siempre
  es `String`); upgrade: unificar con clave canónica.
- **Enums** (`tipo X = A(i64) | B(Texto, i64)`) con layout
  `%Enum.X = type { i64 tag, i64… }` y `coincidir` como cadena de comparaciones
  de tag con binding de payloads.

Estado final: **9 de los 10 benchmarks compilan y corren nativo**.

| benchmark | VM | nativo | Rust | ratio |
|---|---|---|---|---|
| ffi | 446 ms | 37 ms | 37 ms | **1,00×** |
| dispatch | 8 000 ms | 59 ms | 51 ms | 1,16× |
| parseo | 7 469 ms | 1 024 ms | 488 ms | 2,10× |
| memcache | 8 858 ms | 305 ms | 89 ms | 3,43× |
| vectores | 584 ms | 102 ms | 62 ms | 1,65× |
| overflowpanic | 22 298 ms | 38 ms | 37 ms | 1,03× |
| hash | 1 857 ms | 310 ms | 284 ms | 1,09× |
| erroresanidados | 30 993 ms | 1 103 ms | 1 342 ms | **0,82×** |
| fib(42) | — | 764 ms | 839 ms | **0,91×** |
| concurrencia | 6 ms (50k/hilo) | pendiente (hilos) | 451 µs | — |

## 13. Tercera tanda (2026-11-09): hilos, canales y cierre 10/10

Agregado al backend nativo:

- **Hilos** (`hilo { ... }`): el bloque se baja a una función
  `@hilo.N(ptr %entorno)`; las variables visibles se copian por valor a un
  arreglo que viaja como entorno. El lanzamiento usa `CreateThread` (Windows) /
  `pthread_create` (Unix) desde `runtime.c`.
- **Canales** (`canal()`, `enviar`, `recibir`): cola FIFO con mutex + condición.
  El canal es simétrico (emisor y receptor son el mismo handle), así que
  `variable e, r = canal()` registra ambos nombres sobre el mismo puntero.
- `variable a, b = canal()` en la ruta LLVM (antes: error).
- `obj.metodo(args)` cuando llega como `LlamadaFuncion` con nombre punteado.
- `noalias` en el retorno de `forja_alloc` (habilitar LICM/vectorización).
- `FORJA_SIN_RANGOS=1`: acceso a arreglos sin chequeo de rango (equivalente a
  `get_unchecked`), para poder vectorizar bucles.

### Tabla final: 10 de 10 benchmarks nativos

Tiempos de pared, mínimo de 5 corridas, mismo host:

| benchmark | VM (antes) | nativo | Rust `-O` | ratio |
|---|---|---|---|---|
| ffi | 446 ms | 50 ms | 54 ms | **0,93×** |
| dispatch | 8 000 ms | 76 ms | 69 ms | 1,10× |
| parseo | 7 469 ms | 1 165 ms | 553 ms | 2,11× |
| memcache | 8 858 ms | 325 ms | 93 ms¹ | 3,49×¹ |
| vectores | 584 ms | 123 ms | 77 ms | 1,60× |
| overflowpanic | 22 298 ms | 48 ms | 47 ms¹ | 1,02× |
| hash | 1 857 ms | 324 ms | 293 ms | 1,11× |
| erroresanidados | 30 993 ms | 1 135 ms | 1 409 ms | **0,81×** |
| concurrencia | 449 ms (VM, 50k/hilo) | 52 ms | 48 ms | 1,08× |
| fib(42) sin memo | — | 764 ms | 839 ms | **0,91×** |
| fib(42) con memo | — | **49 ms** | 839 ms | **0,06×** |

¹ `memcache.rs` y `overflowpanic.rs` originales quedan **optimizados por LLVM**
(las sumas sobre `vec![1; N]` se pliegan): sus 93/47 ms no ejecutan el trabajo.
La variante "justa" de `overflowpanic` mide 27 ms (mismo orden que Forja).
Para `memcache`, la variante justa de Rust mide 111-121 ms de pared y sus
recorridos internos 7 ms (posible plegado parcial); el reparto medido en Forja
es relleno 62 ms + pasada por filas 62 ms + pasada por columnas 265 ms, y el
relleno se debe a que Forja construye el arreglo con 16,7 M de `empujar`
(Rust usa `vec![1; N]`, un solo `memset`).

## 14. Cuarta tanda (2026-11-09): closures y variantes sin payload

- **Closures** (`func(x) { ... }`): se baja a `@closure.N(ptr %entorno, params…)`
  igual que los hilos, y el valor es un objeto `{ fn, entorno }`. Las variables
  locales se capturan por valor; los globals se leen en vivo (`@g_x`), que es lo
  correcto para un closure que se ejecuta más tarde.
- **Hilos**: capturan *todas* las variables visibles por valor (snapshot al
  lanzar), como hace la VM. Antes capturaban sólo locales y las carreras leían
  globals ya modificados (`507` en vez de `365` en el test).
- **Variantes sin payload** (`Rojo`, `Pendiente`): se construyen al referenciarlas
  (`Expresion::Identificador`) y su tipo se infiere como el enum.

Con esto la ruta nativa cubre: funciones y recursión, globals, control de flujo,
texto (concat/builtins), arreglos, mapas, enums + `coincidir`, clases con
ctor/campos/métodos, FFI, punteros a función, closures, hilos y canales.
Sólo quedan fuera cosas puntuales que devuelven error explícito (`escribir` de
agregados, `Exacto`, `seleccionar`, `?`).

### Ratios finales (pared, mínimo de 5)

| benchmark | nativo | Rust `-O` | ratio |
|---|---|---|---|
| ffi | 49 ms | 48 ms | 1,02× |
| dispatch | 73 ms | 64 ms | 1,14× |
| parseo | 1 196 ms | 567 ms | 2,11× |
| vectores | 121 ms | 76 ms | 1,59× |
| overflowpanic | 48 ms | 48 ms | 1,00× |
| erroresanidados | 1 184 ms | 1 397 ms | **0,85×** |
| hash | 320 ms | 344 ms | **0,93×** |
| concurrencia | 54 ms | 51 ms | 1,06× |
| fib(42) con memo | 49 ms | 839 ms | **0,06×** |

## 15. Quinta tanda (2026-11-09): pendientes cerrados

- **`escribir` de agregados**: los arreglos llevan un `tipo` de elemento en el
  layout del runtime (`%ForjaArreglo = { len, cap, tipo, datos }`), así que
  `escribir([1,2,3])` imprime `[1,2,3]` y `escribir(["uno","dos"])` imprime
  `[uno,dos]`. Los mapas imprimen `{clave: valor, …}` respetando claves enteras
  y de texto. Los elementos de texto viajan como `ptrtoint` en la casilla i64
  (antes se convertían con `forja_a_entero`, que los dejaba en 0).
- **Primitivo de relleno**: `repetir_arreglo(valor, n)` (alias
  `arreglo_relleno`) crea un arreglo de n copias en un solo recorrido, el
  equivalente nativo de `vec![v; N]`.
- **Memoización**: sigue aplicándose sobre el IR nuevo (`fib(42)` 49 ms; con
  `memoizacion: falso` 764 ms), sin cambios adicionales.

Efecto medido en `memcache`, mismo programa con el relleno cambiado por
`repetir_arreglo(1, filas*cols)`:

| versión | pared |
|---|---|
| relleno con 16,7 M de `empujar` | 324 ms |
| relleno con `repetir_arreglo` | **286 ms** |

La comparación con Rust sigue sin ser válida en este benchmark: su
`vec![1; N]` deja que LLVM pliegue las sumas (mide 93 ms sin ejecutar los
recorridos).

## 16. Sexta tanda (2026-11-09): variables de nivel superior sin global

Medido con instrumentación por fases en `memcache`: el bucle por filas costaba
63 ms contra 5,7 ms de Rust. Causa: **toda variable de nivel superior era un
`@global`**, así que un acumulador como `suma_hit` hacía `load`/`store` a memoria
en cada iteración y LLVM no podía promoverlo a registro ni vectorizar.

Fix: sólo se emiten como `@global` las variables que aparecen dentro de un
cuerpo de función/método/closure/hilo (`nombres_usados_en_funciones`, un
recorrido del AST). El resto vive en un `alloca` de `@forja_init`.

También se midió la variante justa de `memcache` en Rust (relleno con
`std::hint::black_box`, para que LLVM no pliegue las sumas): **116 ms** contra
**289 ms** de Forja con `repetir_arreglo` (2,49×). El resto de la brecha está en
que los bucles de Forja no se vectorizan igual (chequeo de rango por acceso,
`datos` cargado de la estructura del arreglo).

## 17. Séptima tanda (2026-11-09): chequeo de rango configurable y elidible

- **`verificar_rangos`** es una clave de `forja.yaml` (default `verdadero`).
  Desactivarla quita el chequeo por acceso y deja vectorizar; el env
  `FORJA_SIN_RANGOS=1` sigue funcionando como override para pruebas.
- **Elisión automática** en el patrón idiomático
  `mientras (i < longitud(arr)) { … arr[i] … }`: `arr[i]` está en rango por
  construcción, así que no se emite chequeo y el bucle se vectoriza. Efecto
  medido: suma de 16,7 M elementos en **6 ms** contra 6,6 ms de Rust (antes 16 ms).
- **Reloj de alta resolución** en el runtime nativo (`QueryPerformanceCounter`):
  `GetTickCount64` tiene ~15 ms de granularidad y hacía inservibles los tiempos
  internos de los benchmarks.
- `repetir_arreglo` también existe en la **VM** (builtin `BuiltinFast::RepetirArreglo`),
  así los programas que lo usan corren en ambos motores.
