# Traspaso: backend LLVM nativo y rendimiento

Última actualización: 2026-11-11.
Repositorio: `C:\Users\gaucho\forja` (rama `main`, trabajo en la rama `feat/llvm-nativo` al principio).
Documentos hermanos: `plans/PLAN-VENCER-A-RUST.md` (auditoría + plan por fases),
`plans/ESTADO-BACKEND-LLVM-LEGACY.md` (historia del legacy y su reemplazo),
`plans/BUGS-BACKEND-LLVM.md` (los dos bugs viejos, ya resueltos).

> **Ojo**: `plans/` está en `.gitignore`. Este documento y sus hermanos viven sólo
> en el working tree; copialos si necesitás versionarlos.

---

## 1. Resumen ejecutivo

En esta serie se hizo, en orden:

1. **Se ejecutó `plans/ESTADO-Y-CORRECCIONES.md`**: cobertura ASM al 100 %, arreglo
   de los tests de FFI en macOS, formato del submódulo `forja-syntax`. CI en verde.
2. **Se escribió el plan para ganarle a Rust** (`PLAN-VENCER-A-RUST.md`) con una
   auditoría del intérprete y mediciones por benchmark.
3. **Se reemplazó el backend LLVM**: se migró todo el lowering a un backend nuevo
   con IR válido de entrada (punteros opacos, valores tipados, globals reales) y
   **se eliminó `src/backend_llvm/legacy.rs`**.
4. **Los 10 benchmarks de la competencia corren nativos**, y **9 de 10 quedan en
   paridad o mejor que `rustc -O`** (ratio 0,19× – 1,04×; el peor, `vectores` sin
   reserva, 1,51×).
5. **Arranque de la CLI**: de ~400 ms a ~47 ms por ejecución.
6. **Tres bugs de pipeline que afectaban a la VM y al backend nativo** (parser de
   literales Exacto, const-prop que borraba el tipo declarado, arreglos de textos
   leídos como enteros). El ejemplo `ejemplos/exactitud/400_exacto.fa` corre
   nativo con **salida idéntica a la VM**.
7. **Infraestructura de medición**: banco de competencias vendorizado, script de
   medición con baseline versionada y job de CI que falla si un caso deja de
   compilar nativo o el ratio empeora.
8. **Despacho dinámico nativo**: los objetos llevan el tag de su clase en la
   palabra 0 y las llamadas sobre campos/parámetros sin anotar se resuelven con un
   `switch` sobre las clases que declaran el método. Con eso los patrones con
   rasgos corren nativos y dan **salida idéntica a la VM**: `174_estrategia`,
   `175_decorador`, `178_adaptador`, `91_rasgo_simple`, `93_rasgo_polimorfismo`.
   Para que el tipado acompañe se agregó inferencia de tipos de campos y
   parámetros sin anotar (cuerpo y llamadas), y la repr de arreglo al concatenar
   con texto (`"a = " + arr` → `a = [1,2,3]`, antes salía la dirección).
10. **Segunda pasada sobre objetos y arreglos** (§3.9): campos de receptores sin
   tipo estático (lectura y escritura), `p.metodo(x)` como sentencia,
   `arr[i] = v` (escribía siempre en 0) y `arr[longitud(arr)] = v` que crece como
   la VM, y error explícito para literales de arreglo heterogéneos.
11. **`Booleano` como tipo propio** (`T::Bool`/`V::Bool`, §3.9): `verdadero` y
   `falso` se imprimen y se concatenan como en la VM.
12. **Arreglos que arrancan vacíos** (§3.10): al empujar/escribir un texto se
    recuerda el tipo y se marca en el runtime. `Resultado`/`Opcion` se imprimen
    como la VM.
13. **Mapas con valores de texto** (§3.11): tag por entrada y
    `escribir(m[clave])` como la VM.
14. **Repr de `Decimal`** (§3.12): la más corta que vuelve al mismo `double`.
15. **Arreglos de `Decimal`** (§3.14), texto mezclado en aritmética como la VM, y
    evidencia por llamada con variables locales (un conflicto ya no se pisa).
16. **`@derive(Mostrar)`** (§3.15) y evidencia de constructores por clase.
17. **`@derive(Igual)`** (§3.16): `==` campo por campo, y comparación de enums por
    tag + payload.
18. **Precondiciones (`requiere`)** verificadas nativamente (§3.17) e `imprimir`
    como builtin.
19. **Evidencia de parámetros en dos vueltas** y prioridad sobre la forma (§3.19).
20. **Payload de `Resultado` desde la firma** (§3.20).
21. **Monomorfización de los cuerpos genéricos** con punto fijo (§3.21).
19. **Monomorfización de funciones** aplicada a los usos (§3.18): `identidad` y
    compañía. Las clases genéricas quedan pendientes.
20. **`s[i]`**, locales que se ensanchan desde un literal, clases genéricas con
    una sola instanciación y arreglos de textos por parámetro (§3.19).

Estado actual verificado: `llvm_backend` 17/17, `forja --lib` 127,
`forja-vm` 40, `forja-cli` 24, `forja-syntax` 49, `cargo fmt --check` limpio,
clippy 0 warnings en `forja`.

---

## 2. Mapa del backend nativo

| Archivo | Qué hace |
|---|---|
| `src/backend_llvm.rs` | API pública `compilar_a_llvm_texto()`, `RUNTIME_C` (el runtime C embebido), tipos (`LlvmTipo`), layouts de enum/clase, intrínsecas de GC, pipeline de optimización. |
| `src/backend_llvm/lowering.rs` | Núcleo del lowering: tipos del modelo (`T`/`V`), tabla de enums, clases (campos, métodos y **tag de clase**), variantes, `RUNTIME` (declaraciones nativas), conversiones (`como`/`como_slot`/`desde_slot`), paso de programa (`compilar`), `primer_retorno`, inferencia de tipos de campos/parámetros (`inferir_campo`, `inferir_parametro`, `inferir_retorno`), `nombres_usados_en_funciones`. |
| `src/backend_llvm/lowering/expresiones.rs` | Expresiones: literales, aritmética (Entero/Decimal/Exacto/Texto), comparaciones, llamadas (usuario → builtins → FFI → indirectas), **despacho dinámico por tag** (`despacho_dinamico`), closures, hilos, canales, mapas, arreglos, enums + `coincidir`, objetos, `?`. |
| `src/backend_llvm/lowering/sentencias.rs` | Sentencias: variables, asignaciones, control de flujo, `retornar`, bucles, y las funciones/métodos del programa. |
| `src/backend_llvm/runtime.c` | Runtime: arena, tiempo, texto, impresión, arreglos, mapas, canales/hilos, `Exacto` (`__int128`) y `esperar`. |
| `tests/llvm_backend.rs` | 17 tests que **compilan y ejecutan** con clang lo que el backend soporta. |

El IR se compila con `clang -O3 -flto` junto a `runtime.c` (ver
`compilar_llvm_a` en `src/tools/forja-cli/src/commands/ejecucion.rs`).

---

## 3. Lo hecho, por bloque

### 3.1 CI en verde (plan `ESTADO-Y-CORRECCIONES.md`)

- **Cobertura ASM 100 %**: test de caja blanca que compila
  `compilar_texto_operando` con `TargetArch::X86_64Windows` (en Linux las ramas de
  shadow space no se ejecutan). Verificado: `100.00%`, sólo quedan las 2 líneas
  `// LCOV_EXCL_LINE` (1802/1815) que el script excluye.
- **macOS FFI**: los tests de la CLI usaban `libc.so.6` fijo; ahora eligen
  `kernel32.dll` / `libSystem.B.dylib` / `libc.so.6` (`libc_sistema()` en
  `src/tools/forja-cli/tests/cli/flujo.rs`).
- **Formato**: `cargo fmt` en `compiler/forja-syntax`.
- Commit `3fea8d33`. CI `42545646818` quedó **completamente verde**.

### 3.2 Backend LLVM: migración completa (commit `78a2242e` y siguientes)

Se eliminó `src/backend_llvm/legacy.rs` (emitía `i64*`, rompía con globales y se
parcheaba con `normalizar_llvm`, que también se borró). El backend nuevo soporta:

funciones y recursión · globals reales (`@g_x`, sólo los que usan funciones) ·
control de flujo completo · texto (concat + builtins) · arreglos · mapas ·
enums (`tipo X = A(..) | B(..)`) con `coincidir` · clases con constructor,
campos y métodos · FFI (`funcion externa`) · punteros a función · closures con
captura · hilos y canales · `Resultado`/`Opcion` con `?` · **`Exacto`**
(BigDecimal) · impresión de arreglos/mapas · `reservar`/`repetir_arreglo`.

Cada constructo no soportado devuelve un **error explícito** con qué hacer (nunca
genera código que miente). Ejemplo de mensaje: dispatch dinámico.

### 3.3 Runtime C (`runtime.c`)

- **Arena** de bloques de 1 MB (bump allocator, alineado a 16 B); los pedidos
  grandes van a `calloc`. La memoria no se libera durante la corrida (no hay GC en
  la ruta nativa; antes tampoco se liberaba).
- **Tiempo** de alta resolución (`QueryPerformanceCounter` en Windows): con
  `GetTickCount64` los benchmarks internos mentían (15 ms de granularidad).
- **Texto**: concat, `a_texto`, `a_entero`, `longitud`, `indice_de`, `subcadena`,
  comparaciones, `dividir` (split), `recortar`, mayúsculas/minúsculas, impresión.
- **Arreglos** `{ len, cap, tipo, datos }` con `tipo` (0 = enteros, 1 = textos),
  `empujar` (con camino rápido inlineado en el lowering), `reservar`,
  `repetir_arreglo`, `imprimir_arreglo`.
- **Mapas**: tabla abierta con dos espacios de claves (i64 y texto), hasher
  propio, `imprimir_mapa`.
- **Canales e hilos**: cola FIFO con mutex + condición; `forja_canal_intentar`
  (`try_recibir`), `forja_esperar`, `CreateThread`/`pthread_create`.
- **Exacto**: coeficiente `__int128` + escala, con la semántica de la VM
  (homogeneizar, mul con suma de escalas, div a 20 dígitos, formato `1.50`).

### 3.4 Rendimiento (banco de competencias)

- `benches/competencias/`: los 10 programas `.rs` y `.fa`, las **variantes justas**
  de Rust en `rust_escala/` (`black_box` en cada lectura) y `stress5m.fa`,
  `vectores_reserva.fa`.
- `scripts/bench-competencias.sh`: compila ambos lados, mide pared (mínimo de N),
  imprime la tabla, compara con `benches/competencias/baseline.json` (margen 2,5×,
  por el ruido del runner) y **falla si AOT degrada a bytecode** o si un caso no
  compila. Limpia su carpeta de salida (un binario bytecode viejo de 5,9 MB se
  estaba midiendo como nativo).
- Job `bench-competencias` en `.github/workflows/ci.yml`.

**Tabla actual** (pared, mínimo de 3; Forja nativo vs `rustc -O`):

| caso | forja | rust | ratio |
|---|---|---|---|
| erroresanidados | 261 ms | 1392 ms | **0,19×** |
| stress (5M) | 118 ms | 279 ms | **0,42×** |
| overflowpanic | 51 ms | 84 ms | **0,61×** |
| memcache | 264 ms | 282 ms | **0,94×** |
| hash | 387 ms | 382 ms | 1,01× |
| concurrencia | 48 ms | 47 ms | 1,02× |
| dispatch | 66 ms | 64 ms | 1,03× |
| parseo | 546 ms | 531 ms | 1,03× |
| ffi | 47 ms | 45 ms | 1,04× |
| vectores | 113 ms | 75 ms | 1,51× |

Extras medidos: `fib(42)` 49 ms con memoización vs 839 ms de Rust (**0,06×**);
suma de 16,7 M en un `mientras (i < longitud(arr))` 6 ms vs 6,6 ms (paridad, el
chequeo de rango se elide solo); `vectores` con `reservar` 65 ms vs 62 ms
(`with_capacity`) = 0,97×.

La VM, para comparar, estaba entre 8,7× y 812× en los mismos programas.

### 3.5 Arranque de la CLI

- `forja ayuda` tardaba **401 ms**: en Windows se copiaba el binario (9 MB) a
  `%TEMP%` y se re-ejecutaba **en cada arranque** (`selfrun::shadow_copia`). Ahora
  sólo para los comandos que reemplazan el binario
  (`selfrun::comando_reemplaza_binario`).
- `forja ejecutar` intentaba `clang -O3` en cada corrida si el fuente **no contenía
  la subcadena `importar`** (~200 ms). Ahora es opt-in: `destino: auto` en
  `forja.yaml`.
- Resultado: **400 ms → 47 ms**; `concurrencia.fa` 449 → 66 ms; `ffi.fa` 446 → 58 ms.

### 3.6 Bugs compartidos arreglados (afectaban a la VM **y** al nativo)

1. **Parser: `variable x: Exacto = 0.5 + 42` perdía el operando derecho**
   (`compiler/forja-syntax` `b30adaf`). El atajo de "literal Exacto desde los
   dígitos" se usaba aunque hubiera una expresión completa. Ahora sólo si el
   literal *es* la expresión.
2. **Parser: literales decimales dentro del inicializador Exacto**
   (`4df9a11`): `contexto_exacto` hace que `0.9` salga del lexer con
   `(coeff, escala)` en vez de pasar por f64 (`1 - 0.9` daba `0.0999999999`).
3. **Const-prop borraba el tipo declarado** (`f2b69f11`,
   `compiler/forja-ir/src/optimizer/passes/const_prop.rs`): ponía `tipo: None`, así
   que `variable frac: Exacto = 456` llegaba como entero y `frac / 1000` daba 0.
4. **Arreglos de textos leídos como enteros** (`191ac03c`): `partes[0]` de un
   `texto.dividir(".")` es un puntero; ahora `Index` devuelve `Texto` (con
   `inttoptr`) y `tipo_estatico` lo refleja. Se rastrea por variable
   (`arreglos_texto`).
5. **Sentencias de nivel superior fuera de orden** (`98b6693b`): todas las
   inicializaciones se emitían antes que las sentencias, así que
   `variable duracion = ahora() - inicio` corría antes del bucle medido.
6. **Nombre de variable vs global del módulo** (`f2a4e82f`): gana la declaración
   del programa; antes el tipo salía de la stdlib.
7. **Construcción de `Resultado`/`Opcion` y payloads de enum**: `Ok`/`Error`/`Algo`
   construyen la variante, `?` propaga devolviendo el mismo valor, y el payload se
   enlaza con su tipo declarado (antes un `Texto` se leía como entero).

### 3.7 Otros

- **Memoización** (`memoizar_puras`): acepta cualquier aridad, extrae bien los
  parámetros (`%p0) {` ya no deja el `)`), emite punteros opacos.
- **`forja medir`** resolvía imports con la variante equivocada y fallaba con
  «función no definida: tiempo_ms»; ahora usa `compilar_pipeline_completa_desde`.
- **`forja aot` / `forja llvm`** resuelven `importar` y **podan lo inalcanzable**
  (`podar_inalcanzable` en `src/lib.rs`): la stdlib entera se inlinea y sus módulos
  sin usar chocan con el backend (usan funciones nativas sin tipo declarado). Con
  la poda, sólo se baja lo que el programa realmente llama. `_sistema_*` mapea al
  runtime (`forja_tiempo_ms`).
- **Métodos de primitivos** como azúcar: `arr.empujar(x)`, `s.longitud()`,
  `n.absoluto()` → `empujar(arr, x)`, … Las funciones del programa tienen
  prioridad sobre los builtins (una función del usuario llamada `dividir` ya no se
  confunde con el split).
- **`verificar_rangos`** en `forja.yaml` (default `verdadero`) + **elisión
  automática** del chequeo en `mientras (i < longitud(arr))`; `FORJA_SIN_RANGOS=1`
  como override.

### 3.8 Despacho dinámico nativo (2026-11-11)

Los programas con rasgos guardan objetos de varias clases en un campo o parámetro
sin anotar (`contexto.estrategia = nuevo OrdenamientoBurbuja()`), así que el
método no se puede resolver en compilación. Se resolvió **sin vtables**:

- **`%Clase = type { i64, <campos> }`**: la palabra 0 del objeto es el **tag** de
  su clase (1..N en orden alfabético; 0 queda para nulo). `indice_campo` suma 1 y
  `instanciar` escribe el tag al crear el objeto.
- **`despacho_dinamico`** (`expresiones.rs`): cuando el receptor no tiene tipo
  estático, carga el tag y hace un `switch` sobre las clases del programa que
  declaran ese método. Cada rama llama al método concreto con los argumentos ya
  convertidos a su firma real (los argumentos se evalúan una sola vez, antes del
  `switch`); los casos «objeto nulo» y «clase sin ese método» abortan con
  mensaje. Los tipos de retorno de las implementaciones se unifican con
  `unificar_retorno` (`Entero` y los tipos con representación de puntero
  comparten `i64`); `Decimal`/`Exacto`/`Nulo` mezclados piden anotación.
- **`implementa Rasgo para Clase`**: los métodos se registran en la clase (misma
  tabla y misma emisión que los suyos) y el DCE de `podar_inalcanzable` también
  recorre esos cuerpos (antes los podaba y la llamada quedaba sin definir).
- **`self.a.b(x)`**: el parser lo entrega como el método `"a.b"` sobre `self`; el
  lowering lo reescribe al acceso al campo y la llamada real (dinámica).

Para que el tipado acompañe (antes imprimían `0`, que era el puntero convertido a
entero), se agregó inferencia de tipos **sin anotar**, toda local y en
`lowering.rs`:

| Qué | Regla |
|---|---|
| Parámetro | si se indexa (`p[i]`) o se le empuja/reserva → `Arreglo`; si se concatena con un literal de texto (`"" + p`) **y no escapa** (no se retorna ni se guarda) → `Texto`; si no, lo que llegue en las llamadas (`nuevo Nodo("x")` → ese parámetro es `Texto`) |
| Campo de clase | tipo de lo que se le asigna en el programa (`este.campo = "x"` → `Texto`, `= nuevo C()` → esa clase); si dos clases homónimas lo llenan con tipos distintos queda `Entero` |
| Retorno de función/método | se unifican **todas** las ramas de `retornar`; `retornar p` usa el tipo del parámetro y `retornar f(x)` / `retornar este.x.m()` el de la llamada |
| `este.campo` como expresión | `tipo_estatico` mira el layout de la clase |

También se agregó `forja_arreglo_a_texto` en el runtime: concatenar un arreglo con
texto da su repr `[1,2,3]` (lo mismo que `escribir(arreglo)` y que la VM) en vez
de la dirección. Y `@main` ya no llama dos veces a `forja_main_usuario` cuando el
nivel superior del programa ya llama a `main()`.

### 3.9 `Booleano` como tipo propio

`Tipo::Booleano` se bajaba como `Entero`, así que `verdadero` se imprimía `1`. Se
agregó `T::Bool` (mismo `i64`, como `Entero`) y `V::Bool`:

- literales, comparaciones (texto / `Exacto` / `Decimal` / enteros), `y`/`o` y
  `no` producen `Bool`;
- `escribir` usa `forja_imprimir_booleano` (que ya estaba) y concatenar con texto
  usa `forja_booleano_a_texto` (nuevo en `runtime.c`);
- `como` deja pasar `Bool`↔`Entero` sin instrucciones y `unificar_retorno` los
  trata igual (los dos son un slot).

Queda un hueco: un `Booleano` que viaja por un parámetro o un campo **sin anotar**
sigue llegando como `Entero` (la inferencia no deduce `Bool` de un parámetro), así
que `escribir(este.flag)` puede imprimir `1`. Afecta a `191` y `32`.

### 3.10 Arreglos que arrancan vacíos y `Resultado`/`Opcion`

- Un `variable xs = []` que después se llena (`xs.empujar("texto")` o
  `xs[i] = "texto"`, también sobre campos: `este.elementos`) ahora se recuerda
  como arreglo **de textos**: sus casillas se leen con `inttoptr` y además se
  marca su `tipo` en el runtime (`getelementptr` a la palabra 2 de
  `%ForjaArreglo`), así que `escribir(xs)` y `"x" + xs` muestran los textos.
- `Resultado`/`Opcion` se imprimen `<Resultado>` / `<Opcion>` (lo que hace la VM)
  tanto en `escribir` como al concatenar; antes salía la dirección.

### 3.11 Mapas con valores de texto

La casilla de un mapa es un `i64` sin tipo, así que un valor `Texto` se guardaba
con `a_entero` y `persona["nombre"]` daba `0`. Ahora `ForjaPar` lleva
`valor_texto` (lo escribe el mismo `fijar`, con un argumento más) y el valor viaja
como puntero (`como_slot`). Para imprimir sin adivinar, el runtime expone
`forja_mapa_valor_a_texto_i64` / `_texto`, que devuelven el texto tal cual o el
número formateado según el tag; `escribir(m[clave])` los usa. Concatenar
(`"x" + m[k]`) sigue viendo la dirección: el `V` no recuerda de dónde vino.

### 3.12 Decimales con la repr de la VM

`forja_decimal_a_texto` usaba `%g` (6 dígitos): `0.8584073465` salía `0.858407`.
Ahora busca la representación más corta que vuelve al mismo `double` (lo mismo
que `f64::to_string()` de la VM): `%.1g`, `%.2g`, … hasta que `strtod` devuelve
el valor, con `%.17g` como techo. Eso arregla la comparación de salida en los
ejemplos con `Decimal` (`161`, `162`).

### 3.13 `main()` no se ejecuta solo

El entry point nativo (`@main`) llamaba a `forja_main_usuario` cuando el programa
declaraba `funcion main()` aunque nadie la llamara: la VM no lo hace. Se sacó esa
llamada (y el chequeo de "ya la llama el nivel superior"), así que nativo y VM
ejecutan lo mismo.

### 3.14 Arreglos de `Decimal`, texto mezclado y tipificación con variables

- **Arreglos de `Decimal`**: la casilla es un `i64`, así que el double viaja
  bit-casteado (y los enteros del mismo literal, convertidos). Se rastrea por
  variable/campo (`arreglos_dec`) igual que los arreglos de textos, y la lectura
  vuelve como `double` — también por el camino rápido de rango demostrado, que
  antes devolvía `Entero` siempre (`casilla_a_valor`).
- **Texto mezclado con números**: `t - 1`, `t * 2` → `nulo`; `7 == t` → `falso`,
  `7 != t` → `verdadero`; dos `Texto` se siguen comparando como texto. Es lo que
  hace la VM; antes se convertía el texto a número (`"abc" == 5` daba igualdad).
  Arregla `76` y `79`.
- **Parámetros que además se concatenan**: si el parámetro se usa en aritmética
  (`2 * a`) no se tipa `Texto` aunque también aparezca en un `escribir` — era el
  caso de `164`.
- **Evidencia por llamada con variables locales**: `variable horas = 2.5` …
  `predecir(horas)` ahora cuenta como evidencia (`Decimal`), y un argumento con
  tipos distintos en distintas llamadas queda **en conflicto** y no se rellena
  con el último (antes todas las llamadas a `presentar_animal` se resolvían con
  la última clase y `93` imprimía el método de la vaca).

### 3.15 `@derive(Mostrar)` y genéricos sin resolver

- **`@derive(Mostrar)`** (encontrado en el AST como `Atributo { nombre: "derive",
  argumentos: ["Mostrar"] }`): `escribir(obj)` y la concatenación formatean
  `Clase { campo: valor, … }` como la VM (`clase_a_texto`, recursivo hasta 4
  niveles; un campo de otra clase sin derivar va como `<Clase>`). Un enum
  concatenado va como `<Nombre>`. Arregla `121` y `97`.
- **Genéricos sin resolver**: `Tipo::Parametro(_)` en un parámetro o retorno ya
  no se fuerza a `Entero` (`de_tipo` devuelve `None`), así manda la evidencia de
  las llamadas; pero un `-> T` **sin** evidencia se queda en `Entero` (inferido
  de un `retornar nulo` dejaba la función `void` y el cuerpo devolvía un valor:
  `144` dejó de compilar hasta agregar esa guarda).
- **Evidencia de constructores por clase** (`constructor@Clase`): todos los
  constructores se llaman igual, así que antes se pisaban entre clases y ningún
  parámetro de constructor se tipaba.

### 3.16 `@derive(Igual)` y comparación de enums

- **`@derive(Igual)`** (`Igual`/`Igualdad`/`Eq`): `p1 == p2` compara **campo por
  campo** (texto con `forja_texto_igual`, `Decimal` con `fcmp oeq`, `Exacto` con
  `forja_exacto_comparar`, clases derivadas recursivamente) y `!=` niega el
  resultado. Antes comparaba punteros y daba siempre `falso`.
- **Dos valores del mismo `tipo` (enum)**: `==`/`!=` comparan el tag y las
  palabras del payload (dos `Rojo` eran dos objetos distintos → `falso`).

### 3.17 Contratos nativos y `imprimir` como builtin

- **`requiere` (precondiciones)**: al entrar a la función/método se verifica cada
  condición; si falla imprime `Error en ejecución: contrato violado:
  precondición no se cumple: <mensaje>` y aborta, igual que la VM. Arregla `500`.
  Las `asegura` (postcondiciones) quedan anotadas como `shortcut:`: habría que
  inyectarlas antes de cada `retornar`.
- **`imprimir` es un builtin** en la VM (imprime el valor y un salto, como
  `escribir`), no la función de `std/io` (que agrega su propio `
`): la ruta
  nativa ahora lo trata igual, así que un `imprimir(...)` no deja una línea en
  blanco de más.

### 3.18 Monomorfización: los usos pasan a la versión especializada

`monomorphize_programa` generaba las especializaciones (`identidad_Texto`) pero
**nadie renombraba los usos**: la llamada iba a la genérica (todo `i64`) y la
especialización quedaba muerta y la podaba el DCE. Se agregó
`Monomorphizer::aplicar_instantiations`, que recorre el AST y renombra
`LlamadaFuncion` cuando el argumento permite inferir el tipo. Arregla `101`.

Para las **clases** genéricas no alcanza: habría que renombrar también las
anotaciones `Tipo::Clase` y evitar que el DCE pode la clase genérica
(`shortcut:` anotado). Además se mejoró `inferir_campo`: ahora resuelve el valor
asignado a un campo cuando es un parámetro o un local con tipo (`pistas_de` usa
`preescanear_tipos` + `tipos_params`), que es lo que hace falta para los campos
de una clase genérica especializada.

### 3.19 Tipos de los parámetros: evidencia en dos vueltas

`tipos_params` prioriza ahora la **evidencia** (qué tipo llegó en las llamadas)
sobre la forma en que el cuerpo usa el parámetro (`validar_email(email)` se llama
con un `Texto` aunque el cuerpo lo indexe con `email[i]`). Y la pasada de
evidencia hace **dos vueltas**, sembrando cada cuerpo con lo que ya sabe de sus
parámetros, para que el tipo atraviese una llamada intermedia
(`validar_usuario("Ana", 25, "ana@email.com")` → `validar_email(email)`).

Con eso `87_resultado_validacion` valida bien y sólo le queda una diferencia: el
**payload de `Ok`/`Error`** se imprime como dirección (ver §6.2.1, punto 2).

### 3.20 Payload de `Resultado` según la firma

El preludio asume `Ok`/`Algo` = `Entero` y `Error` = `Texto`, así que
`funcion buscar() -> Resultado<Texto, Texto>` … `coincidir (r) { caso Ok(v) → v }`
enlazaba `v` como entero (y `"x: " + v` imprimía la dirección). Ahora:

- al registrar una función se guarda `Resultado<Ok, Error>` anotado
  (`resultado_fn`);
- `variable r = f()` recuerda los tipos del payload de ese `Resultado`
  (`resultado_var`, también a nivel superior);
- `coincidir (r)` enlaza `Ok`/`Algo` con el tipo `Ok` y `Error` con el tipo `Error`
  de la firma, y el preludio queda como respaldo.

Arregla `87_resultado_validacion`. La limitación que sigue: un `Resultado`
**sin anotar** (o pasado por parámetro) sigue con `Entero`/`Texto`.

### 3.21 Monomorfización de los cuerpos (genéricos)

`specialize_funcion`/`specialize_clase` sustituían los tipos de las **firmas** y
dejaban el cuerpo tal cual, así que un método de `Par<A, B>` que hace
`nuevo Par(este.segundo, este.primero)` seguía instanciando el `Par` genérico
(todo `i64`). Ahora:

- `specialize()` es un **punto fijo** (hasta 24 vueltas): las instanciaciones que
  aparecen al especializar un cuerpo se especializan también;
- `especializar_cuerpo` reescribe los `nuevo Generico(args)` cuando los tipos de
  los argumentos se conocen en ese contexto (`este.campo`, parámetros, literales),
  registrando la instanciación nueva;
- `campos_concretos` deduce el tipo de un campo **sin anotar** desde el
  constructor (`este.primero = primero` con `constructor(primero: A)`), que es lo
  que permite resolver esos argumentos;
- si el retorno anotado es un genérico sin argumentos (`-> Par`) y el cuerpo
  devuelve una `Instanciacion`, el retorno pasa a ser la especialización.

Arregla `105_generico_par`. Queda `107` (`combinar_pares<T, U, V>` declara
parámetros de tipo que **no usa** en su firma: sin ellos no hay forma de saber qué
especialización pedir) y `72` (necesita el retorno de `retornar este.campo`, ver
§6.2.1).

### 3.22 Indexar un texto, locales que se ensanchan y clases genéricas

- **`s[i]` nativo** (`forja_texto_indice`): un texto de un carácter, `nulo` si el
  índice se pasa. Antes devolvía error; arregla `87` y destrabó `192` y `153`.
- **Locales que arrancan en un literal**: `variable suma = 0` que después recibe
  un `Decimal` se tipa `Decimal`; si el inicial es una variable o una llamada, su
  tipo manda (así `163_raiz_cuadrada` no cambia de aritmética).
- **Clases genéricas**: se renombran las instanciaciones (`nuevo Caja("Hola")` →
  `Caja_Texto`), los métodos de la clase especializada pierden el parámetro de
  tipo, y el DCE ya no poda la plantilla (eso rompía `variable p: Par`). Una
  anotación `p: Par` con una sola especialización resuelve a ella. Arregla `104`.
- **Arreglos de textos por parámetro**: si la llamada pasa un arreglo de textos
  (literal, o una variable local que lo es), las casillas de ese parámetro se
  leen como punteros (`marcar_params_texto` + `evidencia_texto`). Arregla `192`.
- **Anotación de plantilla vs evidencia**: `p: Par` (clase genérica) cede ante la
  evidencia de la llamada cuando esta da una especialización concreta.
- **Un slot declarado `nulo`** se lee como `Nulo` (no como `0`), así que
  `escribir("x: " + dato)` imprime `nulo` como la VM. Arregla `28`.
- **`retornar <local>`**: el tipo de retorno sin anotación también sale de un
  local del cuerpo (`retornar moda` con `moda` un arreglo). Arregla `197`.
  (Se probó también inferirlo de `retornar este.campo`, que arreglaba `72` pero
  cambiaba el retorno de métodos como `Comando.deshacer` en `176`: **revertido**.)

### 3.20 Lo que se sumó encima del despacho (2026-11-11, segunda pasada)

- **Campos de un receptor sin tipo estático**: `nodo.siguiente` sobre una variable
  sin anotar se resuelve con el mismo `switch` por tag; cada rama usa el índice
  de campo de su clase. También la **escritura** (`despacho_campo_fijar`). El
  prólogo/epílogo del switch quedó compartido (`abrir_despacho` /
  `cerrar_despacho`, struct `Despacho`).
- **`p.metodo(x)` como sentencia**: el parser la entrega como la *función*
  `"p.metodo"` (sólo las expresiones arman `LlamadaMetodo`); si no existe tal
  función, el lowering la reescribe al método sobre el receptor (y de ahí cae en
  el despacho dinámico si hace falta).
- **`arr[i] = v` escribía en el índice 0**: `ArregloFijar` recibía el `Index`
  completo y el lowering hacía un `load` de `arr[i]` (con su chequeo de rango) y
  guardaba en 0. Ahora escribe en `i`. Además `arr[longitud(arr)] = v` **crece**,
  como en la VM (`forja_arreglo_fijar` delega en `forja_empujar`).
- **Evidencia de tipos por llamada**: en una pasada previa se anota qué tipo
  (no-`Entero`, y sólo si todas las llamadas coinciden) llega a cada parámetro;
  es lo que tipa los parámetros sin anotar que sólo se usan para guardarse
  (`constructor(nombre)` con `nuevo Producto("Laptop", 999)`).
- **Arreglos heterogéneos**: un literal que mezcla textos y números ahora da
  error explícito. Antes compilaba y guardaba el puntero en una casilla de
  entero (`[3, 4, "+", 5, "*"]` daba `140697162827090` en vez de 25).

Verificación cruzada (nativo vs `forja ejecutar`, sobre los 263 ejemplos de
`ejemplos/`): no hubo **ningún caso nuevo** en la lista de diferencias ni ningún
segfault nuevo; 12 ejemplos que antes no bajaban ahora compilan nativos
(`141`, `142`, `145`, `146`, `148`, `176`, `182`, `190`, `195`, `167`, `187`,
`191`, `77`) y los patrones con rasgos (`174`, `175`, `176`, `178`) y
`91`/`93`/`77` dan salida byte-idéntica a la VM.

Después de `T::Bool` (§3.9), los arreglos de textos + `Resultado`/`Opcion`
(§3.10), los mapas con valores de texto (§3.11), la repr de `Decimal` (§3.12) y
sacar la llamada automática a `main()` (§3.13), las diferencias quedaron en
**23** de 263. Casi todas son la misma causa: un valor (`Texto`, `Decimal`,
`Bool`) que viaja por un parámetro, campo o payload sin anotar y se pierde
(`144`, `164`, `167`, `168`, `169`). Además: clases genéricas con **varias**
instanciaciones (`105`, `107`, `72`), FFI de `rand`/reloj (`184`),
`canales` (`async_await`) y `87` (un parámetro que se indexa se tipa `Arreglo` y
resulta ser un `Texto`).

---

## 4. Cómo verificar y cómo medir

```bash
# Todo el estado (lo que corre el CI)
cargo fmt --all -- --check
cargo clippy -p forja --lib            # 0 warnings
cargo test --locked -p forja --lib     # 127
cargo test --locked -p forja --test llvm_backend   # 17 (compila con clang)
cargo test --locked -p forja-vm -p forja-cli       # 40 + 24
cargo test --locked -p forja-syntax                # 49

# Verificación completa que usa el repo
cargo xtask verify -p forja            # fmt + clippy + tests + métricas

# Cobertura del backend ASM (sigue exigiendo 100 %)
PYTHON="C:/Users/gaucho/AppData/Local/Python/pythoncore-3.15-64/python.exe" \
  ASM_COVERAGE_THRESHOLD=100 bash scripts/asm-coverage.sh

# Banco de competencias (nativo vs Rust)
cargo build --locked --release -p forja-cli
REPETICIONES=3 bash scripts/bench-competencias.sh            # tabla + comparación
REPETICIONES=3 bash scripts/bench-competencias.sh --update   # regenerar baseline
```

### Recetas puntuales

```bash
# IR de un programa
target/release/forja.exe llvm ejemplos/exactitud/400_exacto.fa /tmp/x.ll

# Ejecutable nativo (falla con error si no puede bajar algo)
target/release/forja.exe aot ejemplos/async_await/canales.fa /tmp/canales

# Aceptar el fallback al intérprete (por defecto es un error)
FORJA_AOT_FALLBACK=1 target/release/forja.exe aot programa.fa salida

# Desactivar el chequeo de rangos (vectoriza mejor; UB si te pasás)
FORJA_SIN_RANGOS=1 target/release/forja.exe aot programa.fa salida
# o `verificar_rangos: falso` en forja.yaml
```

### Cómo agregar un test del backend

En `tests/llvm_backend.rs`: `ejecutar_nativo(fuente)` compila con clang y devuelve
la salida (o `None` si no hay clang en PATH). Todos los tests siguen ese patrón:

```rust
#[test]
fn mi_caso() {
    let fuente = r#"escribir(1 + 1)"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "2");
    }
}
```

---

## 5. Limitaciones conocidas (y dónde están anotadas)

Todas figuran como comentario `shortcut:` en el código:

| Limitación | Dónde | Nota |
|---|---|---|
| **`Booleano` por parámetro/campo sin anotar** | `lowering.rs` (`inferir_parametro`, `inferir_campo`) | La inferencia no deduce `Bool`, así que un booleano que viaja por un parámetro/campo sin anotar llega como `Entero` y se imprime `1`. |
| **`"x" + m[clave]`** con valor de texto | `expresiones.rs` (`como`) | El `V` de un `Index` sobre mapa no recuerda si la casilla era texto: concatenar imprime la dirección. `escribir(m[clave])` sí usa el tag (§3.11). |
| **Valores sin anotar que pierden su tipo** (`Texto`/`Decimal`/`Bool` por parámetro, campo o payload) | `lowering.rs` (inferencia por forma) | La inferencia mira la forma de la expresión, no el tipo del slot: un `0.5` que entra por un parámetro sin anotar se guarda como entero. Arreglo de fondo: tipificación interprocedural. |
| **Límites del dispatch dinámico** | `expresiones.rs` (`despacho_dinamico`) | Las implementaciones tienen que compartir la representación del retorno (`Entero` y punteros se unifican; `Decimal`/`Exacto`/`Nulo` piden anotación) y el `switch` crece con cada implementación. |
| **Inferencia de tipos sin anotar** es local | `lowering.rs` (`inferir_campo`, `inferir_parametro`, `inferir_retorno`) | Un campo que sólo se llena desde fuera con un tipo que no ve, o un parámetro que sólo se reenvía, quedan como `Entero`. |
| **Indexar un Texto** (`s[i]`) no está soportado | `expresiones.rs` (`Index`) | Devuelve error explicable (`subcadena`/`dividir`). Antes compilaba y leía basura. |
| **Por un canal viaja un `i64`**: los textos se reciben como puntero | `expresiones.rs` (`recibir`) | Upgrade: slot con tag por valor. |
| **`Resultado<T, E>` asume `Ok`=Entero, `Error`=Texto** al enlazar | `lowering.rs` (preludio) | La representación no lleva los genéricos; upgrade: propagarlos desde la firma. |
| **`seleccionar`** (select multi-canal) no está soportado | `expresiones.rs` | Devuelve error explícito. |
| **`Exacto % y lógicos** no soportados | `expresiones.rs` | Devuelve error. |
| **`/` y `%` enteros**: el divisor 0 aborta con mensaje (LLVM no lo define) | `expresiones.rs` | |
| **Callbacks/closures**: la llamada indirecta asume firma `i64(i64, …)` | `expresiones.rs` | Una closure que devuelve texto requiere guardar la firma. |
| **Arreglos heterogéneos**: el `tipo` del arreglo es uno solo (0/1) | `runtime.c` | Mezclar textos y enteros en el mismo arreglo rompe la impresión. |
| **Cadenas de objetos > ~100k** desbordan la pila en la **VM** (mark recursivo del GC) | `gc_bridge.rs` (VM) | El nativo no tiene ese problema; `stress.fa` usa 100k para poder correr en ambos. |
| **`forja ejecutar` usa la VM por defecto** | `config.rs` | `destino: nativo`/`auto` elige la ruta nativa (paga ~200 ms de clang por corrida). |

---

## 6. Lo que queda (priorizado)

### 6.1 Corto plazo

1. ~~`corpus::bd_crea_nulo` rojo~~ **hecho**: el caso ahora es
   `bd_crea_conexion` (`BD("sqlite:memoria")` → `falso`) y `bd_sin_argumento`
   fija el error. Corpus 299/299.
2. **Re-medir la tabla** cuando el árbol esté quieto: la otra sesión compila en
   paralelo y los tiempos salen inflados (se ve `parseo` a 9 s en vez de 0,5 s).
3. **Revisar el worktree**: hay ~25 archivos modificados de la otra sesión
   (españolización, bumps). No mezclar.
4. **`tests/playground_ejemplos.rs::test_ejemplos_playground_ejecutan` se cuelga**
   (más de 40 min, sin timeout) al ejecutar los 18 snippets de
   `docs/src/components/Playground.astro` en la VM. No viene del backend nativo
   (ese test no toca el `.ll`): parece un snippet con un bucle infinito o una
   lectura bloqueante. `docs/` no está versionado, así que puede ser de la otra
   sesión. Reproducir snippet por snippet.

### 6.2 Medio plazo

4. ~~Dispatch dinámico nativo~~ **hecho** (§3.8 y §3.9): métodos y campos de
   receptores sin tipo estático. Lo que queda del tema: `escribir` de una clase
   (devuelve error) y los tipos de retorno que sólo se ven por las llamadas.
5. ~~`T::Bool`~~ **hecho** (§3.9): `verdadero`/`falso` como la VM. Lo que queda
   es la inferencia de `Bool` por parámetro/campo sin anotar.
6. **Diferencias restantes con la VM (9)**, de las cuales **una no es del
   nativo**: `async_await/canales.fa` **cuelga en la VM** (más de 2 minutos, el
   intérprete no termina ese programa), mientras la ruta nativa lo corre y
   termina. Las **8 reales** son:: casi todas son la misma causa —
   un valor (`Texto`, `Decimal`, `Bool`) que viaja por un parámetro, campo o
   payload sin anotar y se pierde (`164`, `168`, `169`, `110`, `73`, `76`, `79`,
   `144`), más genéricos que devuelven `0` (`101`, `104`, `105`, `107`, `72`),
   `derive`/`Mostrar` (`121`, `123`, `97`, `98`), `contratos` con líneas de más
   (`500`), FFI de `rand`/reloj (`184`), `canales` (`async_await`) y dos de GUI
   (`201`, `202`). El arreglo de fondo es la **tipificación interprocedural**
   (llevar el tipo del valor junto con el slot, no inferirlo por forma).
7. **`vectores` 1,51×**: crecimiento incremental de arreglos. Ideas: caché del
   puntero `datos` y de `len`/`cap` por bucle (hoisting), o `empujar` con
   `realloc` in-place (el arena lo impide). El patrón con `reservar` ya está en
   0,97×.
8. **`seleccionar`** nativo (esperar varios canales): con las condvars actuales
   hace falta un `forja_seleccionar(canales[], n)` con un mutex/condición
   compartida o espera con timeout corto.
9. **`forja ejecutar` con `destino: nativo` por defecto** (o `auto`): decisión de
   producto. Ganancia: 10–50× en programas largos; costo: ~200 ms de clang por
   corrida.

### 6.2.1 Diseño del paso que queda: tipificación interprocedural

Las 11 diferencias reales comparten causa: el tipo de un *slot* (parámetro,
local, campo, retorno, payload) se decide por la **forma** de la expresión que se
le asigna, no por el tipo del valor que realmente circula. Para cerrarlas:

1. **Una pasada de inferencia** sobre el AST, antes del backend, que asigne un
   `T` a cada slot y lo itere hasta el punto fijo:
   - fuente: literales, `Instanciacion`, `Index` (tipo del elemento del arreglo),
     `LlamadaFuncion`/`LlamadaMetodo` (tipo de retorno del callee), concatenación
     → `Texto`, comparación/lógicos → `Bool`, aritmética → *join* de operandos.
   - unión (`join`): `Decimal` gana a `Entero`, `Texto` a `Entero`; dos tipos que
     no comparten representación → `Entero` (con aviso) en vez de adivinar.
   - los parámetros se unen con los tipos de **sus argumentos en todas las
     llamadas**, y los campos con todas las asignaciones — que es lo que hoy
     hacen a mano `inferir_parametro`, `inferir_campo` y `evidencia_de_llamadas`.
2. **`join` de aritmética con la VM**: hoy la ruta nativa decide `Entero` vs
   `Decimal` por el *slot*; la VM lo hace por el *valor* en el momento de operar
   (`7 / 2` = 3 en las dos, pero `x` que arranca `0` y se suma a un `Decimal` es
   `Decimal` en la VM y `Entero` acá). El `join` del punto 1 reproduce eso sin
   cambiar los benchmarks (los bucles numéricos quedan en `Entero`).
3. **Payload de `Resultado`/`Opcion`**: guardar el `T` del payload en el *valor*
   (un tercer campo de la casilla) o especializar el preludio por uso
   (`Opcion<Bool>`), en vez de asumir `Entero`/`Texto`.
4. Retirar entonces `inferir_parametro`/`inferir_campo`/`evidencia_de_llamadas`
   (unas ~400 líneas repartidas) y quedarse con la tabla de la pasada.

**Experimento hecho y medido (2026-11-12)**: se escribió esa pasada
(`src/backend_llvm/lowering/inferencia.rs`, ~380 líneas: tabla por slot con punto
fijo de 12 vueltas, `join` de tipos, propagación por argumentos/asignaciones/
`retornar`) y se conectó en dos modos:

- como **fuente principal** (antes de las heurísticas por forma): arreglaba 0
  ejemplos y **rompía 2** (`149_tabla_hash`, `gui/404_login_test`) porque un
  parámetro que recibe `Entero` y `Texto` en llamadas distintas quedaba `Texto`
  para todas;
- como **último respaldo** (después de las heurísticas): resultado **idéntico**
  al estado actual (12 diferencias), o sea que las 11 reales no son “falta de una
  tabla de tipos”: son **sensibilidad al flujo** (`163` vs `168` se contradicen
  con un `join` estático), **tags de payload** y **especialización múltiple**.

Se **eliminó** el módulo: no aportaba y sumaba ~380 líneas al pipeline. El
siguiente intento tiene que atacar esos tres puntos concretos, no reemplazar la
inferencia por forma:
- flujo: tipar el slot **por tramo** (int antes del `si`, Dec dentro), o aceptar
  que la ruta nativa fije el tipo en el primer uso (documentado);
- payload: un tercer campo de la casilla de enum con el `T` del payload;
- especialización: resolver `p: Par` por **uso** (el primer método que se llama
  sobre `p` determina la especialización) en vez de por anotación. Probado también
  (2026-11-12): promover a `Texto` los campos de la clase genérica cuando todas
  sus especializaciones los guardan como `Texto` salvo alguna `Entero` — no
  arregló `105`/`72` y **rompió** `107` (el cuerpo de un método genérico es
  compartido, así que renombrar sus `nuevo X(...)` requiere duplicar el cuerpo
  por especialización, que es monomorfización de verdad).
- retorno de `retornar este.campo` sin anotación: **medido cuatro veces y
  descartado**. La cuarta, ya con los cuerpos genéricos monomorfizados y con
  `inferir_campo` acotado por clase (prefiriendo las asignaciones propias y
  mirando el resto del programa sólo si no hay ninguna), arregla `72` y vuelve a
  romper `176`: `ComandoRestar.valor_anterior` acaba `Texto` y su `deshacer()`
  devuelve texto, con lo que el despacho de `deshacer` unifica a `Texto` y
  `valor_actual + cambio` concatena (`143` en vez de `17`). Falta entender por qué
  `inferir_campo` ve un `Texto` ahí (el único candidato es el parámetro `valor`);
  sin eso, la inferencia del retorno no puede entrar. **Estado: revertido**. Ya con la monomorfización de cuerpos hecha, registrando los campos
  antes que las firmas de los métodos arregla `72` **pero rompe `176`**: el campo
  acaba tipado por una asignación desde *otra* clase (`inferir_campo` mira todas
  las asignaciones al mismo nombre de campo en el programa, que es lo que arregla
  `93`), y `Comando.deshacer` pasa a devolver `Texto` donde el programa suma. El
  próximo intento tiene que **acotar `inferir_campo` por clase al mismo tiempo**
  que agrega la inferencia del retorno (las dos cosas juntas, no una).

Lo demás que queda del plan original sigue igual: `vectores` 1,51×,
`seleccionar` nativo, `destino: nativo` por defecto y el cuelgue de
`test_ejemplos_playground_ejecutan` (que también es de la VM).

### 6.3 Largo plazo (el intérprete)

La Fase 1 de `PLAN-VENCER-A-RUST.md` quedó **sin hacer** y con dos resultados
negativos medidos (documentados ahí): aplanar el despacho con un
`familia_de(opcode)` es **18 % más lento** (LLVM ya fusiona los 11 `match`), y
mover `max_inst`/`maybe_gc` a cada 64 instrucciones no cambia nada. Los 27 ns por
opcode son **trabajo** (chequeos dinámicos, indirecciones al heap): mejorarlo pide
otro diseño (registros, IR tipado o JIT), no micro-optimizar el bucle.

### 6.4 Fuera del alcance de esta serie (CI/CD)

Del worktree anterior quedaron rojos en `CD / Release` (no los toqué):
- `forja-wasm` en wasm32: enlaza `dlsym`/`dlerror` (gatear el `extern "C"` de
  `compiler/forja-native/src/ffi.rs` con `not(target_arch = "wasm32")` + stubs).
- `scripts/build-aar.sh --build-only` sale con 1 sin mensaje.
- Empaquetado Unix/Windows: busca binarios `forja-rt`/`forja-rt-gui`.

---

## 7. Notas de proceso (importante)

- **Otra sesión estuvo trabajando en el mismo worktree todo el tiempo**: renombró
  medio árbol al español, bumpea submódulos y commitea con `git commit -a`. Varios
  de **mis cambios fueron barridos dentro de sus commits** (no aparecen con mi
  autoría). Antes de tocar nada: `git status --short` y `git log --oneline -10`.
- **`plans/` y `docs/` están en `.gitignore`**: este documento, la bitácora del
  plan y las mejoras de documentación del sitio **no están versionados**.
- **`plans/PLAN-VENCER-A-RUST.md`** tiene la bitácora completa por fecha de todas
  las mediciones (incluidos los resultados negativos) y el análisis por benchmark.
- **El árbol se rompe solo**: mientras trabajaba, la otra sesión dejó sin compilar
  `TablaSimbolos::len`, `FormaCampos/Forma::len`, `Location::Stack`,
  `Severity::Aviso`, `Semver::parse`, `anterior_stack`, `ProfileManager::load`,
  `ForjaFast::reset`, `color::RESET`, `ConstructorSsa::new`, `MmapRegion.len`. Se
  arreglaron los que bloqueaban; si volvés a ver esos errores, son de ese lado.
- **`target/bench-competencias/`**: borralo antes de medir (el script ya lo hace).
  Un ejecutable bytecode viejo puede tapar al nativo y dar números del intérprete.
- **`forja.exe` bloqueado** al compilar (`Acceso denegado`): quedó un proceso vivo
  (por ejemplo un ejemplo con `recibir()` bloqueado). `ps -W | grep forja` o
  renombrar el `.exe` y volver a compilar.

---

## 8. Referencia rápida de commits

| Commit | Qué |
|---|---|
| `3fea8d33` | Cobertura ASM 100 %, FFI macOS en tests de CLI (CI verde) |
| `78a2242e` | Backend nativo completo; se borra `legacy.rs` |
| `103b7894` | Arena de memoria en el runtime |
| `d7a1cbce` | `reservar(arreglo, n)` nativo y en la VM |
| `2d62762c` | `Resultado`/`Opcion`, `?`, payloads tipados |
| `98b6693b` | Sentencias de nivel superior en orden + medición honesta |
| `f2a4e82f` | Orden de inicialización y tipo de `-exacto` |
| `7b1aeb2b` | `Exacto` habilitado (i128 + escala) |
| `47be97dc` | Stdlib en la ruta LLVM, métodos de primitivos |
| `b54b5390` | `try_recibir`, `esperar`, canales por clase |
| `c11aa30f` | Stdlib en AOT con poda de lo inalcanzable |
| `0cb26b58` | `empujar` inline, memo N-args, bench en CI |
| `d82782ea` | Mensaje claro para dispatch dinámico |
| `1b719967` | Parser: `Exacto` con operando derecho (submódulo `b30adaf`) |
| `f2b69f11` | Const-prop conserva el tipo declarado |
| `191ac03c` | Arreglos de textos + `400_exacto.fa` idéntico a la VM |
| `df42051b` | Despacho dinámico por tag de clase, inferencia de tipos sin anotar, repr de arreglo al concatenar, `s[i]` con error explícito, corpus `BD` |
