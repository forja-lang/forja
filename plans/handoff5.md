# Handoff 5 — por mejorar (deuda, proceso y trampas)

Fecha: 2026-11-16 00:05. Complemento de `plans/handoff4.md` (lo pendiente
funcional). Esto es lo que *funciona* pero está mal, es frágil, o nos hizo
perder tiempo. Repo: `C:\Users\gaucho\forja`, rama `main`.

---

## 1. Deuda de diseño en el backend nativo

### 1.1 Hay dos mecanismos para saber si un valor es un arreglo

- `ForjaArreglo.magia` (`FORJA_ARREGLO_MAGIA`, un campo extra en la struct) +
  `forja_arreglo_validar()` leyendo ese campo.
- El registro `puntero → tag` (`forja_registrar` / `forja_tag_de`) que se agregó
  después.

Hoy conviven y hacen lo mismo con distinta precisión: la `magia` solo distingue
arreglos; el registro distingue texto/arreglo (y mañana mapa). **Unificar en el
registro** y borrar el campo `magia` (y el `%ForjaArreglo = { i64, i64, i64, ptr }`
se queda como estaba: la magia era la palabra 3 del struct → ojo con el layout y
con los GEP del lowering).

### 1.2 Dos implementaciones de "tipo probable", con reglas distintas

- `tipo_arg()` en `evidencia_de_llamadas` (tipa parámetros por las llamadas).
- `tipo_con_pistas()` (pre-escaneo del cuerpo, con pistas de locales/parámetros).

Las dos inferíian aritmética (`Decimal` contagia, `Texto`+`Suma` concatena) por
separado; ya divergieron una vez. Unificarlas en **una** función pura
`tipo_probable(expr, pistas, retornos)` y que ambas la llamen.

### 1.3 La inferencia estática tocó su techo (medido)

Intentos de arreglar la familia "aritmética dinámica" **con inferencia** y su
resultado medido:

| Intento | Efecto |
|---|---|
| tipo de expresiones aritméticas como evidencia | `185` mejora, `167` empeora |
| sembrar los parámetros tipados en el pre-escaneo | `185` mejora, `167` empeora (varianza 3.07 vs 3) |
| revertir ambas | al revés |

Conclusión: **no insistir con heurísticas**; los ejemplos se turnan. Lo que falta
es el modelo de valores con tag (`handoff4.md` §3.1). Dejar esto escrito evita que
la próxima persona repita el ciclo.

### 1.4 Un `forja_concat` que trata `NULL` como cadena vacía

`forja_concat(NULL, x)` devuelve `x`; la VM concatena `"nulo"` (`"x" + nulo` =
`"xnulo"`). No se cambió porque `nulo` y `0` todavía comparten representación
(§3.1 del otro documento); cuando exista `T::Dinamico` hay que corregirlo.

### 1.5 `forja_a_texto_dinamico` y compañía: protocolo de tag implícito

Las ops `forja_*_dinamico` asumen "i64 que cae en el registro = objeto"; eso vale
mientras los escalares no se registren. **Si alguien registra decimales o
booleanos en el registro, todo esto se rompe en silencio.** Anotarlo en el código
(ya está el comentario de `FORJA_T_TEXTO`), pero conviene un `debug_assert`
(`FORJA_DEBUG_TAGS=1`) que verifique que una caja nunca se registra como objeto.

---

## 2. Las 11 limitaciones que el código reconoce (`grep -rn "shortcut:"`)

| Archivo:línea | Limitación | Qué haría falta |
|---|---|---|
| `expresiones.rs:67` | `Exacto` solo en programas autocontenidos | soporte de `__int128` en FFI y mapas |
| `expresiones.rs:1038` | por canal nativo viaja un `i64` | canales heterogéneos (requiere valores con tag) |
| `expresiones.rs:1108` | retorno de closure asumido `i64` | guardar la firma en el objeto closure |
| `expresiones.rs:1134` | llamadas indirectas asumen firma `i64(i64, …)` | ídem, con la firma en el valor |
| `expresiones.rs:2654` | `switch` de despacho lista todas las clases que declaran el método | despacho por tabla de vtables |
| `sentencias.rs:588` | solo precondiciones; `asegura` no se inyecta | postcondiciones en la ruta nativa |
| `lowering.rs:593` | mapa/enum/clase/función concatenados con Texto | representación de texto por clase (hoy hay `derive(Mostrar)`) |
| `lowering.rs:883` | sin genéricos en la representación: se asume el uso | especialización real (§3.2 del otro doc) |
| `lowering.rs:1987` | la evidencia no distingue la clase del método | clave `Clase::metodo` en `evidencia` |
| `lowering.rs:2635` | `Entero` y punteros comparten representación al unificar retornos | valores con tag |
| `lowering.rs:2663` | `inferir_campo` mira todas las asignaciones del programa | inferencia por receptor |

Orden sugerido: `1987` (barato y mejora la inferencia), `1108`/`1134` (cierran
closures), `588` (postcondiciones), el resto cuando haya valores con tag.

---

## 3. El arnés de paridad

### 3.1 Bien, pero con dos trampas

- `--update` **reescribe los motivos** de las exclusiones: si alguien lo corre,
  se pierden los motivos medidos y quedan rutas peladas. Mejor: `--update` solo
  agrega líneas nuevas con `TODO: motivo` y no toca las existentes.
- La lista de exclusiones es a la vez "aceptado" y "no probado": conviene una
  columna `motivo` verificable (p. ej. `# VM-truncamiento`) para poder auditar.

### 3.2 Falta en CI

`scripts/paridad-native-vm.sh` corre a mano. Debería ser un job (el repo ya tiene
`scripts/check-perf-regression.sh` como precedente): si suben "difieren" o
aparece un "revienta", que falle el PR.

### 3.3 `forja aot` miente con el código de salida

Cae a bytecode y sale 0 (el único indicio es el texto). Para un script o CI es
imposible de detectar. **Propuesta**: `--nativo-obligatorio` o código de salida
distinto, y que el fallback avise por stderr siempre.

### 3.4 La caché nativa está bien; el árbol moviéndose no

Verificado: `compilar_llvm_nativo_cache` hashea `fuente + RUNTIME_C + variante`
(`src/tools/forja-cli/src/commands/ejecucion.rs:71-75`), así que un cambio de
runtime sí invalida la caché. **Pero** si otra sesión compila el `forja.exe`
mientras uno mide, el binario del script y el que uno cree tener pueden no ser el
mismo, y los números de paridad dejan de ser comparables (pasó dos veces hoy).
Antes de medir: `git status --short`, `git log --oneline -3` y comprobar la fecha
de `target/release/forja.exe`.

### 3.5 Tests

- `tests/llvm_backend.rs` tiene 38 tests y **2** tocan el camino dinámico
  (`casilla_con_numero_se_comporta_como_la_vm`,
  `indice_fuera_de_rango_no_aborta_y_la_casilla_crece`).
- No hay test de: registro de tags con textos, `inciertos`, despacho por tag en
  impresión, `nulo` vs `0`.
- Ni un test que corra `scripts/paridad-native-vm.sh` en CI (§3.2).

---

## 4. Proceso (esto nos costó trabajo real)

### 4.1 Un worktree, varias sesiones

Hoy escribieron 4 sesiones en el mismo árbol. Consecuencias reales de esta
jornada:

- Una sesión commiteó el trabajo de otra (`6a5d7a17` es mío, autor `lococoi`).
- Un `git stash pop` fusionó cambios de dos sesiones en el mismo archivo.
- Dos sesiones implementaron **la misma** inferencia de genéricos en
  `src/monomorph.rs` a la vez (una lo dejó desactivado con `TEMP-PI`).
- `cargo build` falló varias veces con *Acceso denegado* por el `.exe` en uso.

Regla mínima mientras no haya worktrees: cada sesión declara su área en
`plans/COORDINACION-SESIONES.md` y **commitea por archivo**
(`git add <paths>`, nunca `commit -a`), porque `commit -a` barre autoría ajena.

### 4.2 `plans/` no está versionado

Los handoffs, las decisiones y la bitácora viven en `.gitignore`: un `clean -xfd`
los borra. Decisión pendiente (§10 de `DECISIONES-PENDIENTES.md`): sacarlo del
`.gitignore` o mover estos documentos a `docs/` (que tampoco está versionado).

### 4.3 Documentos desactualizados

- `plans/DECISIONES-PENDIENTES.md` dice `142 coinciden … 3 revientan`: hoy es
  `147 … 0 revientan` (comparable) y varias de sus 10 decisiones ya están
  implementadas (8-B caché, 5/6 arnés y exclusiones, 9-bis crashes).
- `plans/handoff2.md` habla de 263 ejemplos; hay 293.
- `plans/TRASPASO-BACKEND-NATIVO.md` sigue siendo la historia larga: no
  contradecirlo, pero no leerlo como estado actual.

---

## 5. Calidad del código del backend

- `lowering.rs` tiene **2.864 líneas** y **3.058** `expresiones.rs`. El paso
  `compilar` mezcla registro de firmas, clases, globals, cuerpos y ensamblado del
  módulo. Extraer: `firmas.rs`, `clases.rs`, `modulo.rs` (sin cambiar semántica,
  con el arnés de paridad como red).
- `T`/`V` se comparan con `==` en decenas de sitios y `T::Int` es a la vez "tipo
  entero" y "no sé": por eso la inferencia miente en silencio (ver §1.3 y
  `nulo`/`0`). Un `T::Desconocido` explícito, separado de `T::Int`, haría
  evidente dónde falta información — y es el paso previo natural a
  `T::Dinamico`.
- En `runtime.c` conviven arena con `calloc` por bloque, registro con `malloc`,
  y mapas con linear probing: está bien para ahora, pero **no hay liberación**
  (ni falta: no hay GC). Documentar en el encabezado el modelo de memoria (ya
  está a medias).
- `runtime.c` se edita como binario en algunas herramientas (tiene un `\0` y
  CRLF): `grep` necesita `-a`; los scripts de edición deben leer/escribir bytes
  con `utf-8` explícito. Anotarlo evita sustos (`git diff` lo muestra como
  `Bin`).

---

## 6. Ideas que valen la pena (no urgentes)

1. **`forja verify`**: correr el mismo ejemplo por VM y nativo y comparar, con un
   `--explicar` que muestre la primera línea que difiere. Hoy lo hace el script,
   pero sin decir *por qué*.
2. **Modo `--trace-tipos`**: volcar por stderr el tipo decidido para cada
   variable y por qué (anotado / evidencia / forma / defecto). Ahorraría las
   horas que se fueron en adivinar por qué un parámetro quedó `Entero`.
3. **Tabla de paridad en el README**: `coinciden / difieren / no bajan` por
   categoría, generada por el script (`--markdown`), para que el número publicado
   no envejezca como el de `DECISIONES-PENDIENTES.md`.
4. **`scripts/paridad-native-vm.sh --solo <patrón>`** para iterar un ejemplo sin
   recorrer los 293 (hoy se hace a mano con `aot` + `diff`).

---

## 7. Trampas concretas que hicieron perder tiempo (para no repetirlas)

1. `forja eje cutar`/`forja ejecutar` usa la VM; el nativo es `forja aot` o
   `destino: auto`. Comparar contra el binario sin mirar el mensaje
   `✅ AOT nativo:` mide el intérprete contra sí mismo.
2. Un `.exe` de ejemplo puede quedar colgado (`recibir()` bloqueado) y bloquear
   el siguiente `cargo build` con *Acceso denegado*: `ps -W | grep forja`.
3. `cargo --locked` explota cuando otra sesión toca `Cargo.toml`/`Cargo.lock`:
   usar `--offline`.
4. `timeout` de coreutils **no existe** en `cmd.exe`: los scripts de paridad
   necesitan Git Bash.
5. Un ejemplo nuevo (`.fa`) cambia los números de paridad aunque no se toque el
   backend: comparar siempre la misma lista de ejemplos o anotar el total.
6. `nvim`/editores con BOM + CRLF en `runtime.c` rompen `clang -w` de formas
   raras; compilar el runtime suelto (`clang -c -w -o /tmp/rt.o`) es la
   verificación más rápida.
