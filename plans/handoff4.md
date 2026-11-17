# Handoff 4 — todo lo pendiente del backend nativo

Fecha: 2026-11-15 23:59. Repo: `C:\Users\gaucho\forja`, rama `main` (la rama
`feat/llvm-nativo` ya está fusionada: `main` == `feat/llvm-nativo`).
Documentos hermanos: `plans/handoff2.md` (historia y gotchas), `handoff5.md`
(por mejorar / deuda), `DECISIONES-PENDIENTES.md` (las decisiones de producto),
`COORDINACION-SESIONES.md` (reparto entre sesiones).

> `plans/` está en `.gitignore`: estos documentos no se versionan.

---

## 0. Antes de escribir una línea

**Hay varias sesiones de Pi trabajando en este worktree a la vez.** Antes de
tocar nada:

```bash
git status --short          # ¿archivos ajenos en vuelo?
git log --oneline -5        # ¿commits nuevos de otras sesiones?
grep -n "TEMP-PI" src/lib.rs src/monomorph.rs   # ¿alguien desactivó algo?
```

Si `monomorphize_programa` tiene un `TEMP-PI ... desactivado`, la paridad **no
es comparable**: los ejemplos genéricos van a diferir por eso y no por tu
cambio. Ese marcador lo pone y lo saca la sesión dueña de `src/monomorph.rs`
(ver `COORDINACION-SESIONES.md`).

Gotchas de entorno que ya costaron horas:

1. `bg_run` ejecuta con `cmd.exe`: hay que prefijar el `PATH` de Git Bash o el
   script no encuentra `dirname`/`uname`:
   ```
   set "PATH=C:\Program Files\Git\usr\bin;C:\Windows\System32;%PATH%" && cd /d C:\Users\gaucho\forja && bash scripts/paridad-native-vm.sh
   ```
2. `cargo build` puede fallar con *Acceso denegado* si otra sesión está
   compilando o corriendo `target/release/forja.exe`: reintentar, no matar el
   proceso ajeno.
3. `forja aot` cae a bytecode **en silencio** y sale con 0. El único indicio es
   `✅ AOT nativo: <ruta>`; si dice `AOT nativo no disponible (...)`, ese ejemplo
   no bajó y cualquier comparación de salida es el intérprete contra sí mismo.
4. `plans/` y `*.txt` están ignorados por git.

---

## 1. Estado medido hoy

Último estado **comparable** (monomorfización activa, todo lo de la §2 aplicado):

```
nativo vs VM: 147 coinciden · 0 difieren · 126 no bajan a nativo
              · 1 no termina · 5 error explícito · 0 revientan
```

Estado **ahora mismo** (con `monomorphize_programa` desactivado por la sesión
`01a1e13e`, y 30 ejemplos nuevos que agregó otra sesión — 293 `.fa` en total):

```
nativo vs VM: 145 coinciden · 6 difieren · 128 no bajan
              · 2 no terminan · 2 error explícito · 0 revientan
```

Las 6 diferencias de esa corrida, y de quién son:

| Ejemplo | Causa |
|---|---|
| `101_generico_identidad`, `104_generico_caja`, `105_generico_par` | monomorfización desactivada (`TEMP-PI`). No son del backend. |
| `201_gui_ventana`, `202_login`, `900_storybook` | **pendiente real**: el nativo compila, sale 0 y **no imprime nada**, mientras la VM imprime. GUI sin host de widgets = mentira silenciosa (ver §3.5). |

De los 263 ejemplos de `handoff2.md` se pasó a 293: hay ejemplos nuevos
(`720_closures`, `721_azucar_sintactica`, `731/732/733…`, `185_ffi_matematicas`,
`900_storybook`) que agregan casos y también exponen lo que falta.

Tests (todos verdes hoy): `llvm_backend` 38/38, `lib` 127, `corpus` 299,
`forja-vm`/`forja-cli`/`forja-syntax` OK, clippy y `rustfmt --check` limpios en
`src/backend_llvm/**`.

---

## 2. Qué quedó cerrado (no rehacer)

| Commit | Qué |
|---|---|
| `6a5d7a17` | **Cero segfaults.** Registro `puntero → tag` (texto/arreglo) en el runtime; los literales de texto se registran en `@forja_init`; `forja_indice_dinamico`, `forja_longitud_dinamica`, `forja_imprimir_dinamico`, `forja_a_texto_dinamico` deciden por el valor; `forja_arreglo_validar` devuelve `nulo` si la casilla no es arreglo. Índices fuera de rango → `nulo`; `arr[i] = v` alarga con huecos. `Decimal` se imprime como el `Display` de Rust (sin exponente). Inferencia: `func(x){…}` → `T::Closure`, `p["k"]` → `T::Mapa`, retorno anotado como evidencia. |
| `6df47ba5` | El pre-escaneo de tipos (`tipos_pre`) no se filtra entre cuerpos. |
| `a4abea9c`, `377f5bd4` | (otra sesión) `-> T` / `x: T` de genéricos sin especializar; exclusiones con motivo medido. |

Ejemplos que pasaron de reventar o diferir a coincidir exactamente con la VM:
`147`, `196`, `132`, `150`, `162`, `720`; `149` y `185` ya no revientan.

Arquitectura del registro (lo que hay que entender antes de tocar):

- `runtime.c`: `forja_registrar(p, tag)` mantiene un mapa abierto `puntero → tag`
  (`FORJA_T_TEXTO`, `FORJA_T_ARREGLO`). Todo texto de heap se registra en
  `forja_texto_alloc`; los literales, desde `@forja_init`. **Lo no registrado es
  un escalar.**
- `lowering`: `self.inciertos: HashSet<String>` marca las variables cuyo tipo es
  una suposición; `fn incierto(&self, e)` decide si un acceso va por el runtime.
  El camino estático (GEP + load inlineado) queda para los tipos seguros: los
  bucles calientes no pagan nada.

---

## 3. Pendiente, en orden de valor

### 3.1 El modelo de valores con tag (bloquea casi todo lo demás)

**Síntoma**: quedan 5 diferencias que salen todas de lo mismo — la VM decide el
tipo **por el valor, en cada operación**; el nativo lo fija **por slot**.

```
149_tabla_hash   `h * 31 + clave[i]` deja `h` como Texto en la VM (y ahí pierde
                 las inserciones); el nativo lo fija Entero y sí inserta.
164/168/185      la VM trunca (log(10)=0, pendiente 51 para todo); el nativo da
                 el valor "correcto". Parámetro sin anotar que recibe Decimal.
167              la casilla se ensancha a Decimal; la VM divide entero/entero.
```

**Y el caso que apareció al intentar la stdlib nativa** (§3.3): `nulo` y `0`
comparten representación, así que el nativo imprime `0` donde la VM imprime
`nulo` (lo rompen `12_input`, `211_colecciones`, `732_colecciones`).

Semántica de la VM, medida (probe con `forja ejecutar`):

```
nulo + 1 → "nulo1"        nulo * 2 → "nulonulo"    "ab" - 1 → "ab-1"
7 / 2 → 3                 7 / 2.0 → 3.5            7.5 % 2 → 0
verdadero + 1 → "verdadero1"   -nulo → nulo
tipo(1)=entero  tipo(1.5)=flotante  tipo([])=arreglo  tipo({})=mapa  tipo(nulo)=nulo
leer() al final de stdin → nulo
```

**Diseño acordado** (no implementado):

1. `runtime.c`: `ForjaDin { int64_t tag; int64_t payload; }` con
   `NULO/ENTERO/DECIMAL/TEXTO/BOOLEANO/ARREGLO/MAPA`. Los objetos ya están
   registrados, así que texto/arreglo/mapa son el puntero; los escalares llevan
   el payload en la caja.
2. Operaciones `forja_din_*` (sumar, restar, multiplicar, dividir, modulo,
   negativo, comparar, igualdad, a_texto, imprimir, longitud, indice, a_entero,
   a_decimal) copiando las reglas de la tabla de arriba.
3. `lowering`: `T::Dinamico` (y `V::Dinamico`) para las variables cuyo tipo
   cambia entre asignaciones. Eso se detecta en `preescanear_tipos`: si una
   asignación aporta un tipo distinto al del inicializador, la casilla es
   dinámica. `binaria`, `imprimir`, `longitud`, `Index` y `como` despachan a
   `forja_din_*` cuando un operando es dinámico.
4. Aceptación: `149`, `164`, `167`, `168`, `185` coinciden; `12_input`,
   `211_colecciones` y `732_colecciones` también; `nulo` se imprime `nulo`;
   paridad sin exclusiones de aritmética y sin revientan; benchmark
   `scripts/bench-competencias.sh` sin caída en los bucles estáticos (los
   dinámicos sí van a ser más lentos: eso es correcto).

**Coste estimado**: ~1 día. **Prerrequisito**: árbol quieto en
`src/backend_llvm/**` (ver `COORDINACION-SESIONES.md`).

> Alternativa explícita si se decide no hacerlo: `DECISIONES-PENDIENTES.md` §1-B
> ("nunca mentir") = esas operaciones fallan con error explícito en vez de dar
> otro número. Hoy están como diferencia aceptada (§4).

### 3.2 Genéricos (144, 72, 107)

- `144_cola`: la **misma** clase se usa con `Texto` (en `simulacion_banco`) y con
  `Entero` (en `main`). Hay que especializar **por variable**, no por clase
  (`Cola_Texto` y `Cola_Entero`) y renombrar el `nuevo Cola()` de cada cuerpo.
- `72_genericos`: además, `retornar este.campo` sin anotación (§4.3 de
  `handoff2.md`).
- `107_generico_multiple`: parámetros de tipo que la firma no usa.

Hay código ya escrito y **sin usar** en `src/monomorph.rs:460`
(`recolectar_por_uso`, `buscar_bindings`, `buscar_usos`, campo
`var_especializada` + `scope`): infiere por variable con clave `cuerpo::variable`
y renombra el constructor. Se validó a mano: con él, `107` coincidía y `144`
pasaba a necesitar la inferencia por variable (que es lo que implementa). No se
pudo commitear porque la sesión dueña del archivo lo estaba editando; **hay que
reconciliarlo con su versión** (`recoger_retornos`, `ret_dinamico`) o borrarlo.

Aceptación: `144`, `72` y `107` coinciden con la VM y salen de
`scripts/paridad-exclusiones`.

### 3.3 Stdlib nativa (los ~128 que no bajan)

Por volumen (de `target/paridad/*.aot`, contado el 2026-11-15):

| Falta | Ejemplos | Coste | Nota |
|---|---|---|---|
| `leer()` (stdin) | 10 | bajo | `fgetc`/`getline` en el runtime |
| `_mapa_claves` / `_mapa_valores` / `_mapa_pares` | 9 | bajo | iterar `ForjaMapa`, claves ordenadas (ya hay `forja_mapa_*`) |
| `tipo(v)` | 3 | bajo | estático si el tipo se conoce; si no, por tag |
| `escribir_linea` | 3 | bajo | no existe en la stdlib: la VM la trata como función desconocida (imprime el argumento + 2 saltos). *Ojo*: implementarla la hace diferir |
| `unir_elementos` | 2 | bajo | join por tag |
| `seleccionar` (multi-canal) | 7 | medio | concurrencia |
| `_archivo_*` (FS) | 4 | bajo | |
| red / sockets / HTTP / SQLite / JSON / TOML / hash | ~20 | alto | sandbox y timeouts |
| GUI | ~40 | fuera de alcance | necesita host de widgets (§3.5) |

**Ya se implementó y se revirtió** (medido): `leer`, `tipo`, `unir_elementos`,
`escribir_linea`, `_mapa_claves/valores/pares` bajan de 148 a 143 coincidencias
**y** 8 diferencias + 1 segfault, porque al ejecutarse aparece `nulo` impreso
como `0`. Conclusión: **§3.1 primero**, después esto es casi gratis.

### 3.4 La VM es la que falla

- `async_await/canales.fa`: el intérprete no termina (2+ min); la ruta nativa sí.
- `test_ejemplos_playground_ejecutan` se cuelga.

Si alguien toca el intérprete, empezar por ahí.

### 3.5 GUI: mentira silenciosa (nuevo, y es un bug)

`201_gui_ventana`, `202_login` y `900_storybook` **compilan nativos, salen con 0 y
no imprimen nada**; la VM imprime los textos. La ruta nativa no tiene host de
widgets: o se implementa un host mínimo (imprimir lo que el GUI escribiría) o
esos ejemplos **fallan con error explícito** (`Nunca mentir`, §1-B). Hoy hacen lo
peor: silencio.

### 3.6 Rendimiento

- Re-medir `scripts/bench-competencias.sh` con el árbol quieto (la tabla
  publicada puede estar vieja).
- `vectores` estaba en 1,51× (0,97× con `reservar`). Candidatos: hoisting de
  `len`/`cap` por bucle, `realloc` in-place fuera del arena.
- Medir el coste del camino dinámico **después** de §3.1 (solo debe tocar
  casillas dinámicas).

### 3.7 Decisiones de producto que siguen abiertas

De `plans/DECISIONES-PENDIENTES.md` (los números son los de ese documento):

1. **1** — ¿paridad exacta con la VM o "nunca mentir"? Define todo lo demás.
2. **2** — aritmética dinámica: ¿modelo de valores (§3.1) o documentar?
3. **7** — qué stdlib nativa se implementa (orden propuesto: `leer`/archivos →
   `seleccionar` → red).
4. **10** — versionar `plans/` y **separar ramas/worktrees** por sesión.

---

## 4. Diferencias aceptadas hoy (`scripts/paridad-exclusiones`, 11 entradas)

Cada una tiene el motivo medido, no "pendiente":

| Ejemplo | Motivo |
|---|---|
| `149_tabla_hash` | la VM pierde las inserciones (buckets vacíos); el nativo sí inserta |
| `164_ecuacion_segundo_grado` | la VM pierde la parte imaginaria (`0 ± 0i`) |
| `167_estadistica` | la casilla se ensancha a Decimal y la VM divide entero/entero |
| `168_regresion_lineal` | la VM pierde la pendiente (51 para todo) |
| `185_ffi_matematicas` | la VM trunca la serie (`log(10) = 0`) |
| `107_generico_multiple` | parámetros de tipo que la firma no usa |
| `72_genericos` | retorno de `este.campo` sin anotación |
| `169`, `189` | no deterministas (tiempo) — no son bugs |
| `184_ffi_simple` | rand/reloj: no determinista |
| `canales` | la VM no termina |

Reglas de la lista: **una entrada = un motivo medido**, nunca "es difícil". Se
revisa entera después de §3.1 y §3.2 (`149/164/167/168/185` y `107/72` deberían
poder salir).

---

## 5. Cómo verificar (siempre en este orden)

```bash
# 1. compilar
cargo build --offline --release -p forja-cli

# 2. paridad (el juez real: 293 ejemplos, nativo vs VM)
bash scripts/paridad-native-vm.sh                 # falla por diffs no excluidas o crashes
bash scripts/paridad-native-vm.sh --verbose        # caso por caso
bash scripts/paridad-native-vm.sh --update         # rehace la lista de exclusiones

# 3. tests
cargo test --offline -p forja --test llvm_backend
cargo test --offline -p forja --lib
cargo test --offline -p forja --test corpus
cargo test --offline -p forja-vm -p forja-cli -p forja-syntax
cargo clippy --offline -p forja --lib
rustfmt --edition 2021 --check src/backend_llvm/lowering.rs \
    src/backend_llvm/lowering/expresiones.rs src/backend_llvm/lowering/sentencias.rs \
    tests/llvm_backend.rs

# 4. un ejemplo a mano
target/release/forja.exe llvm ejemplos/algoritmos/147_grafo_adyacencia.fa /tmp/x.ll
target/release/forja.exe aot  ejemplos/algoritmos/147_grafo_adyacencia.fa /tmp/x
/tmp/x.exe
target/release/forja.exe ejecutar ejemplos/algoritmos/147_grafo_adyacencia.fa   # VM
```

**Regla de oro**: cualquier cambio en `src/backend_llvm/**` se valida con el
script de paridad *antes* de commitear, y sin subir "difieren" ni bajar
"coinciden". Un segfault nunca es aceptable, ni siquiera con "nunca mentir".

Para diagnosticar una caída nativa:

```bash
target/release/forja.exe llvm ejemplos/x.fa /tmp/x.ll
clang -fsanitize=address -g -O0 -w -Wno-override-module -o /tmp/x.exe /tmp/x.ll src/backend_llvm/runtime.c
/tmp/x.exe     # pila con la función exacta
```

---

## 6. Mapa de archivos

| Archivo | Qué hay |
|---|---|
| `src/backend_llvm/runtime.c` | arena, registro de tags (`forja_registrar`, `forja_tag_de`), texto, arreglos (`%ForjaArreglo` con `magia`), mapas, canales, `Exacto`, impresión, ops dinámicas |
| `src/backend_llvm/lowering.rs` | `T`/`V`, tabla de clases, `RUNTIME` (declaraciones C), `como`/`como_slot`/`casilla_a_valor`, `tipo_estatico`, `tipo_con_pistas`, `preescanear_tipos`, inferencia (`inferir_campo`, `inferir_parametro`, `inferir_retorno`, `evidencia_de_llamadas`), paso `compilar` |
| `src/backend_llvm/lowering/expresiones.rs` | expresiones: aritmética, comparaciones, llamadas, builtins, `incierto()` + `elemento_de_campo_contenedor()`, despacho dinámico, closures (`crear_closure`), hilos, canales, mapas, arreglos, enums + `coincidir`, objetos, `?` |
| `src/backend_llvm/lowering/sentencias.rs` | sentencias, `funcion`/`metodo`, contratos, declaración de variables (tipo + `tipos_pre`) |
| `src/lib.rs` | `compilar_a_llvm*`, `podar_inalcanzable`, `monomorphize_programa`, `memoizar_puras` |
| `src/monomorph.rs` | especialización de funciones y clases (punto fijo) + `recolectar_por_uso` (sin usar, ver §3.2) |
| `src/tools/forja-cli/src/commands/ejecucion.rs` | compilación nativa con clang (`-O3 -flto`), caché por hash |
| `src/tools/forja-cli/src/commands/aot.rs` | `forja aot` (con el fallback silencioso a bytecode) |
| `tests/llvm_backend.rs` | 38 tests que compilan y ejecutan con clang |
| `scripts/paridad-native-vm.sh` + `scripts/paridad-exclusiones` | el arnés y las diferencias aceptadas |

`grep -rn "shortcut:" src/backend_llvm/` lista las 11 limitaciones que el código
reconoce de sí mismo (ver `handoff5.md`).
