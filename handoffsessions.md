# Sesiones — bitácora compartida

Este archivo lo escriben **varios agentes a la vez** en el mismo worktree. Es el
punto único para saber *quién hizo qué*, *qué está medido* y *qué está tomado*,
antes de tocar nada.

## Reglas (leelas antes de escribir)

1. **Agregá tu entrada al principio de la sección «Registro»**, no reescribas las
   de otros. Si una entrada ajena está mal, agregá una corrección fechada; no la
   borres.
2. **Una entrada por sesión**: fecha, rama/worktree, qué medíste, qué
   commiteaste, qué dejaste abierto.
3. **Los números van con la fecha y el árbol con el que se midieron.** Sin eso no
   sirven (el árbol se mueve cada pocos minutos).
4. **No commitees archivos ajenos.** `git add` sólo de lo que tocaste. El árbol
   puede tener 50+ archivos modificados de otras sesiones.
5. **Antes de editar un archivo que otra sesión puede estar tocando**
   (`src/backend_llvm/**`, `src/monomorph.rs`, `library/**`, `ejemplos/**`,
   `plans/**`): mirá la tabla «Tomado» y, si no está, anotalo vos.
6. **Nada se commitea sin pasar el arnés** (`scripts/paridad-native-vm.sh`) y los
   tests de abajo. Un segfault nunca es aceptable.

## Cómo medir (orden fijo)

```bash
cargo build --offline --release -p forja-cli
bash scripts/paridad-native-vm.sh            # nativo vs VM sobre ejemplos/
cargo test --offline -p forja --test llvm_backend
cargo test --offline -p forja --lib
cargo test --offline -p forja --test corpus
cargo test --offline -p forja-cli
rustfmt --edition 2021 --check src/backend_llvm/**/*.rs
```

En Windows/`cmd.exe` (para `bg_run`) hay que prefijar el PATH de Git Bash:

```
set "PATH=C:\Program Files\Git\usr\bin;C:\Windows\System32;%PATH%" && cd /d C:\Users\gaucho\forja && bash scripts/paridad-native-vm.sh
```

## Estado verificado

Última medición: **2026-11-17 21:28**, `main` en `f88595cb` (el `557a9981` de la
primera versión de esta línea es posterior), árbol con 35 archivos sucios de las
otras sesiones, release de las 21:27. La midió la sesión de deuda de más abajo;
re-medir si el backend cambió después.

```
nativo vs VM: 154 coinciden · 0 difieren · 119 no bajan · 2 no terminan · 3 error · 0 revientan
```

| Suite | Resultado |
|---|---|
| `llvm_backend` | 48/48 |
| `lib` | 129 |
| `corpus` | 299 (2 ignorados) |
| `forja-cli` / `forja-vm` / `forja-syntax` | OK |

Merges ya dentro de `main`: `native/cajas` (`0b2cc95c`) y `native/atajos`
(`bcb671f4`). `origin/main` al día.

> **Corrección 2026-11-17 23:05** (sesión `native/cajas`, al final de este
> archivo). `main` ya está en `557a9981` y esta medición **no se reproduce desde
> un árbol limpio**: con `ejemplos` en el mismo `58b106ca` y árbol limpio, el
> arnés da `150 · 0 · 95 · 4 · 4 · 0` (`00614f6b`, 22:47, release sin
> recompilar). La brecha (154/119 vs 150/95) sólo puede venir del árbol sucio
> con el que se midió o de la versión sin commitear del arnés. Re-medir con el
> árbol quieto antes de citar cualquiera de los dos números.

## Tomado ahora

| Zona | Quién | Desde |
|---|---|---|
| (libre) | — | — |

## Pendientes (con dueño y medición)

| Tema | Estado | Dónde se explica |
|---|---|---|
| Modelo de valores con tag (`nulo` vs `0`, `72`, `a_texto(mapa)`) | **bloqueado por decisión de producto** (paridad vs «nunca mentir») | `plans/handoff4.md` §3.1, `plans/DECISIONES-PENDIENTES.md` §1 |
| `parseo` 3.63x vs baseline 2026-11-09 | **medido y perfilado, sin arreglar**: 1.42 s de 1.75 s es construir textos; **no** es el registro de tags | `plans/handoff8-mono-claves-clase.md` |
| Lo que no baja a nativo (119): GUI (~40, ahora falla explícito), red/SQLite/JSON/TOML, `seleccionar` | abierto | `plans/handoff4.md` §3.3 |
| La VM cuelga en `canales` y en un test del playground | abierto | `plans/handoff4.md` §3.4 |
| CI: falla `Métricas de documentación (auto)` (el resto queda `skipped`) | abierto, ajeno al backend | `gh run view <id>` |
| `arreglos_texto`/`arreglos_dec` con claves sin clase (bloquea la inferencia por variable de clases) | documentado, desconectado | `plans/handoff8…` («Intentado y revertido») |

## Registro

### 2026-11-18 — carril L3: corpus dorado y ejemplos (worktree `forja-corpus`, rama `corpus/dorado`)
Commits: `f9f174c2` (superproyecto) + 12 en `ejemplos` (`ae78db1` … `1d37f72`, rama
`corpus/dorado` del submódulo).

- **Medido** (2026-11-18 00:26, `2a6f63fc` + estos cambios, árbol con los 2
  archivos del carril, `ejemplos` en el puntero nuevo, release de 00:26):
  `150 coinciden · 0 difieren · 95 no bajan · 4 no terminan · 4 error · 0 revientan`
  — igual que la referencia de `00614f6b`, o sea los ejemplos reparados no
  mueven la paridad. `golden_runner` **verde** (2/2) sin `FORJA_UPDATE_GOLDEN`.
  `llvm_backend` 48, `lib` 129, `corpus` 299 en verde.
- **L3:T1 no era lo que decía el plan**: los 11 goldens «viejos» no estaban
  viejos, eran **ejemplos corruptos**. El formateador (`587f05c` y los
  «normalizados» posteriores) partió asignaciones y retornos:
  `resultado = resultado * 2` → tres sentencias sueltas; `retornar resultado` →
  `retornar` + `resultado`; `este.cola.siguiente = nuevo` → dos líneas; el rango
  `[0..n-2]` quedó en `2`; y en `79` borró un `escribir` entero.
  **No se regeneró ningún golden** salvo añadir el de `162` (no estaba
  commiteado). Se arreglaron 11 ejemplos y **los goldens viejos volvieron a
  pasar tal cual**: `66`, `79`, `139`, `142`, `143`, `150`, `162`, `166`, `190`,
  `193`, `198`.
- **L3:T2**: `juegos/158_contador_palabras.fa` llamaba a `mayuscula()`, que no
  existe: ahora usa `letras[k].a_mayusculas()` y corre.
- **Bugs de VM/parser encontrados** (van a L2/L1, con repro mínimo):
  1. **Mutación anidada perdida**: `este.cola.siguiente = nuevo` no persiste
     (con `variable c = este.cola; c.siguiente = nuevo; este.cola = c` sí).
     Repro: dos clases `N{v,sig}`/`L{cola,cabeza}` y `l.add(1..3)`; el recorrido
     se corta en el primero. Rompía `142` (por eso ese ejemplo usa la local).
  2. **No hay rangos**: `a[0..=2]` compila pero devuelve vacío (`longitud` = 0) y
     `a[0..2]` no parsea; el lexer no tiene token de rango. Rompía `143`/`150`
     (recorte de pila y la línea `Heap:`), hoy hechos con bucles.
  3. **Interpolación → concatenación con precedencia rota** (ya consumado en los
     ejemplos): `"${i + 1}"` se volvió `"" + i + 1`, que imprime `01`.
- **`cargo fmt --check -p forja` falla por archivos ajenos** a este carril
  (`src/transpiler.rs`, `tests/codegen/ir_cobertura_corpus.rs`): ya estaba así en
  `main`; no lo toqué.
- **Abierto**: los tres bugs de arriba siguen en `main` (los ejemplos los
  esquivan); `142`/`143`/`150` se pueden devolver a su forma original cuando se
  arreglen. Los otros ejemplos con `self.` (`145`, `146`, `147`, `108`) funcionan
  tal cual (`self` es alias válido).
- **Zona tomada**: ninguna al cerrar. `ejemplos/**` queda en la rama
  `corpus/dorado` del submódulo; `tests/golden/**` sólo suma `162_aprox_e.out`.

### 2026-11-18 — merge de ramas nativas + bugs destapados (rama `main`)
Commits: `0b2cc95c` (merge `native/cajas`), `bcb671f4` (merge `native/atajos`),
`fef2883d`, `557a9981`, `20d3b120`.

- Mergeé `native/cajas` (`7ec9ed22` claves por clase, `1821cbcf` builtins de
  entrada/introspección) y `native/atajos` (`a502dcfb` evidencia por
  clase+método, `80a3cbde` `asegura` por retorno, `13798a3c` firma de closures,
  `d6824c56` `FORJA_DEBUG_TAGS`). Único conflicto: `scripts/paridad-exclusiones`
  (se conservaron ambos lados). El WIP de otras sesiones se guardó en stash y se
  devolvió con `pop --index`.
- Arreglé 5 bugs que aparecieron al bajar 12 ejemplos más a nativo: `subcadena`
  (el runtime tomaba índice final, la VM toma longitud), `longitud(x)` sobre un
  Entero que en realidad era texto, `tipo_builtin` sin `_mapa_*`/`unir_elementos`
  (segfault en `fusionar`), claves de mapa dinámicas (`forja_mapa_*_dinamico`), y
  evidencia de `contiene(texto, aguja)` (segfault en `_es_digito`).
- `forja aot` sobre una app GUI ahora **falla explícito** en vez de generar un
  `.exe` mudo (§3.5).
- Re-medí los benchmarks: sólo `parseo` está mal (3.63x) y es de antes.
- Exclusiones nuevas con motivo medido: `709_texto` (la VM pega un `nulo`),
  `153_ahorcado` (`tiempo_actual()`, no determinista); se retiró `900_storybook`.

### Antes del 2026-11-18
Resumido de `plans/handoff2.md` … `handoff7.md` (no reescribir, están completos):

- Backend nativo desde cero hasta 145–154 ejemplos que coinciden con la VM.
- `6a5d7a17`: **cero segfaults** — registro `puntero → tag` en el runtime y ops
  dinámicas (`forja_indice_dinamico`, `forja_longitud_dinamica`,
  `forja_imprimir_dinamico`, `forja_a_texto_dinamico`).
- Monomorfización reactivada (`src/lib.rs`) y claves de
  `params_dinamicos`/`metodos_param` cualificadas por clase.
- GUI: runtime prebuilt (`forja-rt-gui`), `forja ejecutar` rearma el `.exe`,
  módulos propios en apps GUI.
- `forja aot` dejó de caer a bytecode en silencio (sale 1 con
  `FORJA_AOT_NATIVO_OBLIGATORIO=1`, y avisa siempre).

### Plantilla para tu entrada

```markdown
### AAAA-MM-DD — título corto (rama o worktree)
Commits: `<sha>`, `<sha>`

- Qué medíste (comando + números + con qué árbol).
- Qué cambiaste y por qué (una línea por cosa).
- Qué dejaste abierto y dónde está explicado.
- Qué archivos/zona tomaste (para que otro no se pise).
```

---

### 2026-11-18 — deuda de `plans/handoff5.md` + arnés de paridad (rama `main`)

Commits propios: `3b720e6b`, `eafe0d98`, `fe4129e1`, `42100f17`, `57e94a84`,
`45cf4690`, `4065ed9d`, `0bd48d7f`, `f88595cb`, `1186f3d8`, `f3413555`.
De backend, en la rama `native/atajos` (mergeada en `bcb671f4`, no por mí):
`a502dcfb`, `80a3cbde`, `13798a3c`, `135cc66a`, `d6824c56`.

**Medido** (2026-11-17 21:28, `f88595cb`, release 21:27, 35 archivos sucios
ajenos): `154 coinciden · 0 difieren · 119 no bajan · 2 no terminan · 3 error ·
0 revientan`. `llvm_backend` 48/48, `lib` 129, `corpus` 299. Es el número de
«Estado verificado»; ojo con atribuirlo al commit equivocado, el árbol se movió
después.

**Backend** (`handoff5` §2, hecho en un worktree propio para no pisar
`src/backend_llvm/**`, que estaba tomado):

- `1987`: la evidencia de tipos de los parámetros se indexa `Clase::metodo` (y
  cae a la clave pelada si el receptor no tiene tipo estático). Dos clases con un
  método homónimo ya no se contaminan.
- `588`: `asegura` se verifica antes de **cada** retorno (explícito e implícito),
  con `resultado` como local y `anterior(expr)` capturado al entrar; mismo
  mensaje y `exit 1` que la VM. +4 tests.
- `1108`/`1134`: la firma viaja con la closure (`firmas_closure`) y con el
  puntero a función (`variable op = sumar`). +1 test.
- `1.5`: `FORJA_DEBUG_TAGS=1` aborta si al registro de objetos entra algo que no
  sea texto o arreglo. §5: el encabezado de `runtime.c` documenta el modelo de
  memoria completo. +1 test.
- Test que fija el estado de `nulo` vs `0` (§1.4 sigue **abierto**: un `Texto`
  nulo concatena "nulo", pero `variable n = nulo` se tipa `Entero` e imprime
  "0"; se cierra con valores con tag).

**Arnés de paridad** (archivos míos, `scripts/` + `.github/`):

- `--update` ya no reescribe `paridad-exclusiones`: agrega lo que falta con
  `TODO: motivo` y respeta los motivos multilínea existentes.
- Job `paridad-native-vm` en CI: falla si algo difiere fuera de exclusiones o
  revienta.
- `forja aot` avisa siempre por stderr que generó el fallback y, con
  `FORJA_AOT_NATIVO_OBLIGATORIO=1`, sale con 1 en vez de caer a bytecode.
- La corrida imprime commit, archivos sucios, ruta/fecha del binario y cantidad
  de ejemplos antes de medir.

**Trampas nuevas (esto costó dos corridas inválidas seguidas):**

1. Si otra sesión corre `cargo build` mientras medís, cargo reemplaza
   `target/release/forja.exe`: 122 de los 175 "no bajan" de una corrida eran
   `line 86: ...forja.exe: No such file or directory`. El arnés ahora **copia**
   el binario a `target/paridad/<fecha>-<pid>/forja` y mide contra la copia.
2. Dos corridas simultáneas compartían `target/paridad` y cada `rm -rf` borraba
   los `.aot` de la otra (mitad de los ejemplos como "no bajan", más
   `grep: ... No such file or directory`). Ahora cada corrida usa su propio
   directorio (override `PARIDAD_OUT`) y el CI sube `target/paridad/**/*.aot`.
3. `bash` invocado desde `cmd.exe` resuelve al bash de **WSL**
   (`C:\Windows\System32ash.exe`): ve rutas `/mnt/c/...` y `uname -s = Linux`,
   así que el arnés busca `forja` sin `.exe` y falla con "Falta …". Para
   `bg_run`/scripts hay que usar `"C:\Program Files\Gitinash.exe"`.
4. `git worktree add` no trae `patches/` ni los submódulos: el build falla con
   `patch location … does not contain packages matching masonry_winit`. Receta
   en `plans/handoff8.md` §0.
   - **Corrección 2026-11-17** (sesión `native/cajas`): desde `00614f6b`
     `patches/` es **submódulo** (antes era un directorio gitignoreado, y la
     receta era copiarlo a mano). `git submodule update --init` no alcanza si el
     SHA registrado no está pusheado: traerlo del clon local del árbol principal
     (`git -C patches fetch <ruta-al-principal>/patches <sha>` y
     `git -C patches checkout --detach <sha>`). `plans/` tampoco viaja al
     worktree (está gitignoreado): copiarlo si hace falta leer los handoffs.
5. HEAD llegó a **no compilar** `forja-cli` (llamaba a `forja::compilar_a_asm_desde`,
   que sólo existía sin commitear). Lo dejé compilando en `4065ed9d`, un
   checkpoint de los 66 archivos que estaban sueltos. Moraleja: si tu cambio
   hace falta para que el árbol compile, commitealo antes de cerrar la sesión.

**Abierto** (no lo toqué, a propósito):

- `handoff5` §5: partir `lowering.rs` (2.864 líneas) en `firmas.rs` /
  `clases.rs` / `modulo.rs`. Es refactor grande sobre el archivo que la sesión
  `native` está moviendo: hacerlo con el árbol quieto.
- `handoff5` §1.4 / `handoff4` §3.1: valores con tag (decisión de producto).
- La inferencia estática está en su techo: unificar el contagio aritmético de
  `tipo_estatico` y `tipo_con_pistas` **cambia resultados** (medido: `176_comando`
  pasa de `17/12/12` a `143/143/1430`). Queda un parche sólo con la tabla de
  literales en `plans/parches/` (`handoff5-1.2-tabla-de-literales.patch`).

**Zona tomada**: ninguna, la sesión terminó. El worktree `forja-shortcuts` se
borró después del merge.

---

### 2026-11-17 23:05 — backend nativo: cajas descartadas, `101/104/105` y builtins (worktree `forja-native`, rama `native/cajas`)

Commits propios: `7ec9ed22` (claves por clase), `1821cbcf` (builtins de
entrada/introspección), `136c92e0` (`leer()` recorta), `290e630d` (handoff8
versionado), `00614f6b` (merge de `main`). Los dos primeros ya los mergeó la
sesión de arriba en `0b2cc95c`; lo único que aporta mi rama sobre `557a9981` es
`136c92e0` (12 líneas en `runtime.c`) más el doc, así que el merge real es
**fast-forward** (`git merge --ff-only native/cajas`).

**Trabajé en un worktree aparte** (`C:\Users\gaucho\forja-native`, rama
`native/cajas`, creado en `377f5bd4` con los submódulos fijados al SHA
registrado): el árbol principal tenía 108 archivos sucios ajenos y
`monomorphize_programa` desactivado con `TEMP-PI`, así que medir paridad ahí no
valía. Receta de recreación en `plans/handoff8.md` §0.

**Medido** (2026-11-17 22:47, `00614f6b` = `main` `557a9981` + el recorte de
`leer`; árbol limpio, `ejemplos` en `58b106ca`, release de 22:47:36 y `cargo
build` posterior sin recompilar, o sea el binario es el del commit):

```
nativo vs VM: 150 coinciden · 0 difieren · 95 no bajan · 4 no terminan · 4 error · 0 revientan
llvm_backend 48/48 · lib 129 · corpus 299 · frontera_crates 2/2 · clippy y fmt limpios
```

Contra el `154 · 0 · 119` de «Estado verificado» — ver la corrección de arriba:
la diferencia no se explica por el árbol (mismo `ejemplos`), sino por lo sucio de
esa corrida o por su arnés sin commitear. No citar ninguno de los dos sin
re-medir.

**Backend — lo que cambió de criterio:**

- `101/104/105` **no necesitan el modelo de cajas** del `handoff7` §3. La causa
  era otra: `params_dinamicos` y `metodos_param` se indexaban por nombre de
  método (`nuevo:0`, `obtener`), así que el `Caja<T>` genérico (que sobrevive a
  la monomorfización) marcaba como «dinámico» el constructor y el `-> T` de
  `Caja_Texto`/`Caja_Booleano`. Efectos: el constructor recibía `ptrtoint` donde
  la firma especializada esperaba `ptr` (clang rechazaba el IR y `aot` caía a
  bytecode **en silencio**) y `obtener()` se imprimía por bits (`1` vs
  `verdadero`). La clave ahora lleva la clase. La monomorfización ya propaga el
  tipo concreto a campos, parámetros y retornos (`specialize_clase` +
  `campos_concretos`), así que el encajado de escalares no hace falta **para
  genéricos**.
- Builtins recuperados de `stash@{2}`: `leer`, `escribir_linea`,
  `unir_elementos`, `tipo`, `_mapa_claves/_mapa_valores/_mapa_pares`, más lo que
  los sostiene (evidencia de tipo por aritmética y ternario, pistas de tipo
  frescas por cuerpo, `ret_dinamico` en métodos). Al portarlos con
  `git apply -3` hay que **borrar los duplicados** de `metodos_param`/
  `ret_dinamico` en el `struct` y en el constructor: el stash es anterior a
  `6a5d7a17`/`a4abea9c` y esos hunks ya estaban.
- Dos diferencias con la VM que aparecieron al probarlos: `leer()` sin tipo de
  retorno dejaba el `nulo` de fin de entrada como `0` (ahora `tipo_builtin` dice
  `Texto` y `forja_concat` une un texto nulo como `"nulo"`), y `tipo(v)` creía el
  tipo estático de un `arr[i]` —una suposición— así que `aplanar` no veía los
  arreglos anidados (ahora sólo `Booleano`/`Decimal` usan la vía estática).
- `136c92e0`: `forja_leer_linea` recorta la línea y trata la vacía (o de sólo
  espacios) como fin de entrada, igual que `Opcode::LeerLinea`. Probado con
  **stdin por tubería** contra `forja ejecutar` (con datos, vacío y línea en
  blanco), no sólo con la entrada vacía del arnés: `  x  ` → `x`, línea vacía →
  `nulo`, EOF → `nulo`.
- Exclusiones nuevas, con motivo medido: `211_colecciones`, `test_basico` y
  `test_sobrecarga` (las dos últimas porque `escribir_linea` de la **VM** imprime
  tres líneas por llamada; el nativo imprime una).

**Abierto y dónde está:**

- Los 95 que no bajan: separar por causa real (`forja llvm ejemplo.fa /tmp/x`).
  `146_arbol_avl` y `186_gestion_memoria` salen con error explícito de runtime
  («acceso a `valor` sobre un objeto nulo»), sin investigar. Y la decisión §2 del
  `handoff7` (paridad vs «nunca mentir»), que es de producto.
- **El encajado de escalares ya no tiene como justificante a los genéricos**: le
  queda sólo `nulo` en una casilla tipada (`211_colecciones`: `encontrar()`
  imprime `0` donde la VM imprime `nulo`). Medir cuántos ejemplos lo piden antes
  de escribirlo; los stashes `{0}`/`{1}` sirven de punto de partida.
- `golden_runner` falla en `main` con **11 goldens commiteados viejos** (`66`,
  `139`, `142`, `143`, `150`, `169`, `170`, `190`, `193`, `198`, `400`); el árbol
  principal ya los regeneró sin commitear. No es del backend: el diff de
  `66_busqueda_lineal` es una línea de salida del programa.
- **`cargo test -p forja` no puede terminar**: se cuelga dentro de
  `tests/runtime.rs` (los `concurrencia_stress::f12_t2_*` imprimen `FAILED` pero
  el binario nunca sale: hilos bloqueados en `recibir()`, el runner no puede
  cerrar el proceso) y también en
  `ir_emision_paridad_funcional_con_ast` (misma familia, casos de canal). El tope
  `fijar_max_inst(10_000_000_000)` no los corta: **bloquean**, no giran. Para
  correr el resto:
  `cargo test --offline -p forja -- --skip ir_emision_paridad_funcional_con_ast`.
- `juegos/158_contador_palabras.fa` llama a `mayuscula()`, que no existe en
  ningún lado (ni en el ejemplo, ni en la VM, ni en la stdlib). Es del submódulo
  `ejemplos`.
- Inconsistencia de la VM: `Opcode::LeerLinea` recorta y la línea vacía es nulo;
  `Uop::LeerLinea` no recorta y devuelve texto vacío. Copié la primera (es la que
  usa `forja ejecutar`); conviene decidir cuál es la buena.

**Zona tomada**: ninguna, la sesión terminó. `src/backend_llvm/**`,
`scripts/paridad-exclusiones` y `tests/llvm_backend.rs` quedan libres.

### 2026-11-18 00:25 — vm/cuelgues: `Ninguno` y `leer()` (rama `vm/cuelgues`)
Commits: `c02ad23b` (+ `compiler/forja-types` `746a9f7`), `f68330fc`

- Medido: `cargo test --offline -p forja --test corpus` → 299 · `--test runtime`
  → **102 pasan, 0 fallan, 1 ignored en 0,52 s** · `--lib` → 129 ·
  `--test codegen ir_emision_paridad_funcional_con_ast` → ok (0,02 s).
  Todas con el árbol de la rama, commit `c02ad23b`, 2026-11-18 00:20.
  `bash scripts/paridad-native-vm.sh` (Git Bash, árbol limpio en `1a58b470`,
  binario release 00:27) → **151 coinciden · 0 difieren · 94 no bajan ·
  4 no terminan · 4 error · 0 revientan** (base `150`).
- Cambió:
  - `compiler/forja-types`: los dos checkers registran las variantes del
    prelude al construirse. `retornar Ninguno` (el parser lo entrega como
    `Identificador`) ya no es «variable no declarada» en las rutas que no
    inyectan el prelude (`compilar_a_llvm_programa`, `analizar`, `transpilar`,
    `compilar_modulo`). Arregla la fase de análisis de `83`, `84`, `88`, `103`,
    `109`; `forja llvm` de `83_option_simple` y `103_generico_pila` genera IR y
    `83` compila con `aot` con salida idéntica a la VM.
  - `compiler/forja-vm`: `Opcode::LeerLinea` y `Uop::LeerLinea` comparten
    `texto_de_lectura()`: recorta; **sólo** el fin de entrada es `nulo`; la
    línea en blanco es texto vacío. Medido con `forja ejecutar` y stdin por
    tubería: `  x  ` → `x`, línea en blanco → `[]`, EOF → `nulo`.
- **L2:T2 y L2:T3 no tenían trabajo**: en `2a6f63fc` (base de la rama) los
  `f12_t2_*` ya pasan (con `unir()` y tope de 60 s en el arnés) y
  `ir_emision_paridad_funcional_con_ast` termina. La premisa del plan venía de
  una medición anterior a los fixes de `ops_concurrencia`/`ops_metodos`.
- Abierto y dónde está (no es de este carril): ver los PEDIDO de abajo.
- `cargo fmt --check -p forja` **falla en `src/transpiler.rs`** (código previo,
  sin tocar acá); `cargo fmt --check -p forja-vm -p forja-types` limpio.
  `--test codegen` tiene 1 fallo previo (`ir_cobertura_corpus`: `206`, `305`,
  `306` no compilan) reproducido con el `forja-types` viejo.
- Zona tomada: ninguna. `compiler/forja-types`, `compiler/forja-vm`,
  `tests/runtime/**` y `tests/codegen/ir_emision.rs`.

### PEDIDO L2 → L1 — 2026-11-18 00:25
- Archivo: `src/backend_llvm/lowering.rs` §3.4 (preludio) y
  `lowering/expresiones.rs` (`construir_variante_con`).
- Problema medido: `forja llvm ejemplos/resultados/84_option_desempaquetar.fa`
  → `variante 'Alguno' desconocida`. El parser canoniza `caso Algo(v)` a
  `Alguno`, pero la tabla del preludio del lowering sólo registra `"Algo"`.
  Igual en `88_option_combinar.fa`.
- Bloquea: la parte nativa de L2:T1 (`84`/`88` ya pasan el análisis).

### PEDIDO L2 → L1 — 2026-11-18 00:25 (informativo, ya es L1:T1/T2)
- `103_generico_pila.fa`: ya pasa el front-end; `forja aot` falla en lowering
  con IR inválido (`'%v14' defined with type 'ptr' but expected 'i64'`), familia
  del grupo I.
- `109_generico_opcion.fa`: ya pasa el front-end; `forja llvm` falla con
  «`coincidir` espera un valor de `tipo` (enum), no un Entero» (grupo D).

### PEDIDO L2 → L4 — 2026-11-18 00:25
- Función: `forja_leer_linea` en `src/backend_llvm/runtime.c`.
- Semántica que fija la VM (medida): recortar la línea (`  x  ` → `x`); línea
  en blanco → texto vacío; **fin de entrada → `nulo`**.
- Hoy en nativo: `  x  ` → `  x  ` (no recorta) y en EOF imprime `0`
  (`escribir(leer())` → `0`; con la VM → `nulo`).
- Ejemplo para verificar: `variable x = leer()` + `escribir(x)` con stdin
  vacío, y `escribir("[" + leer() + "]")` con `  x  `.
- Bloquea: paridad de `leer()` entre las dos rutas (L2:T4).
### 2026-11-18 14:10 — L1 (native/lowering): backend nativo, grupos I, D, C y G (rama `native/lowering`)
Commits: `d5b3882f`, `29b2dc69`, `2aa53c90`, `9209305f`, `b7232880`

- Medido: `scripts/paridad-native-vm.sh` → **171 coinciden · 0 difieren · 67 no bajan ·
  5 no terminan · 4 error · 0 revientan** (árbol con `b7232880`, 14:05, limpio).
  Base del plan (`00614f6b`): 150 · 0 · 95 · 4 · 4 · 0.
- Cambió:
  - `retornar nulo` en una rama ya no deja el `define` en `void` (193, 165, 166);
    global `variable g = nulo` se tipa por su primera asignación (171); `alloca`
    avanza el contador de etiquetas (201_login_gui).
  - `coincidir` sobre un enum metido en una casilla `Entero` (111, 112, 113, 114,
    115, 177, 179, 152) y sobre enteros con patrones literales / `caso x` (600);
    payload de `Error` de tipo enum en un arreglo (89, que además hacía segfault).
  - `hilo { … } .unir()`: `forja_hilo_lanzar` devuelve handle, `forja_hilo_unir`
    lo espera y da los bits del `retornar`; `unir()` sobre algo que no es un hilo
    devuelve el valor (126, 127, 128, 129, 130, 173, 200).
  - `enviar`/`recibir` sin tipo estático (los bits viajan como i64, antes un
    texto enviado se convertía con `a_entero`), azúcar de método para `funcion`
    libres (`r1.es_ok()`), `reemplazar` nativo (80), `a_codigo`/`a_caracter`
    (170), `Alguno` como alias de `Algo` (183), retorno de método tipado como su
    `tipo`/mapa/canal (200).
- Abierto:
  - **Bug de front-end (para L2)**: hay declaraciones que desaparecen antes del
    lowering. Repros mínimos: `target/vM.fa` (de 71_rasgos: rasgo con `-> Texto`
    + `variable mensaje = p.aterrizar()`) y `target/v214c.fa` (de 214: el cuerpo
    de `probar_base64` accede a `dec1.valor`); en los dos, `decls` ya no trae la
    `clase`/`funcion` al llegar al backend ("clase `Pajaro` desconocida", "función
    `probar_base64` no declarada"). Afecta también a `199_forja_desde_forja`
    (`clase Token`).
  - `220_servidor_h2`: llamadas con nombre de módulo (`servidor_h2.registrar_x`)
    no soportadas nativamente.
  - `143_pila`, `194_enums_tuplas`, `102_generico_intercambiar`: arreglo
    heterogéneo + destructuring → es el encajado de escalares (§9, bloqueado).
    Medido: al quitar la prohibición de arreglos heterogéneos, `143` imprime `*`
    donde la VM imprime `25`; por eso la prohibición sigue en su lugar.
  - `158`, `159`, `156`, `185`, `214`: ejemplos rotos (`mayuscula`, `caracter`,
    `abs`, `anon` no existen en ningún lado; el fuente de `185` está corrupto) →
    son de L3.
  - `302`, `403`, `404_login_test`: apps GUI, `aot` las rechaza por diseño.
  - `216_concurrencia`: usa `es_ok` sin importar `std/resultado` (la VM también
    devuelve `nulo`: ejemplo roto, de L3).
- **Zona tomada por L1 fuera de su tabla**: `src/backend_llvm/runtime.c` (era de
  L4, que no llegó a arrancar). Lo que se agregó ahí: handle de hilo +
  `forja_hilo_unir`, `forja_reemplazar`, `forja_a_codigo`/`forja_a_caracter` y el
  registro de handles vivos. Nada de lo existente cambió de semántica (el intento
  de que `forja_a_texto_dinamico(NULL)` diera "nulo" se revirtió: el 0 entero
  también llega como puntero nulo y rompía 33 ejemplos).

### 2026-11-18 14:40 — integración de los cuatro carriles (rama `integracion/final`)
Commits: merges `10870674` (native/lowering), `c136b71d` (native/runtime), `vm/cuelgues`,
`corpus/dorado`; `a75c0d3d`, `a93737c4`

- Medido (árbol limpio, release de 14:08): `bash scripts/paridad-native-vm.sh` →
  **178 coinciden · 0 difieren · 57 no bajan · 5 no terminan · 4 error · 0 revientan**
  (base del plan `00614f6b`: 150 · 0 · 95 · 4 · 4 · 0; L1 solo: 171). `llvm_backend` 55,
  `llvm_runtime` 2, `lib` 129, `corpus` 299, `runtime` 102, clippy `--lib` sin errores.
  `cargo fmt --check -p forja` sigue con los 2 diffs previos (`src/transpiler.rs`,
  `tests/codegen/ir_cobertura_corpus.rs`), ajenos a estos cambios.
- Integración: `runtime.c` se resolvió **a mano**. Queda la implementación de hilo de L1
  (`forja_hilo_lanzar` devuelve handle + registro de handles vivos) y se descartó el par
  `forja_hilo_lanzar_unible`/`forja_hilo_unir` del carril L4, que duplicaba. Encima va el
  fix de rendimiento de L4 (los textos de la arena no entran de a uno al registro de tags).
  El handle de hilo se reserva con `malloc`: con el registro por rango, un handle en la
  arena se clasificaba como texto.
- Cambió (lo que faltaba después de L1/L2/L3):
  - `[v] + arr` bajaba como suma de enteros (`ptrtoint`+`add`) y reventaba al
    desreferenciar: primitiva `forja_arreglo_concat` + rama en `binaria_valores_con`
    (sólo arreglo+arreglo; `texto + arreglo` sigue por la vía de texto). Arregla
    `avanzado/103_generico_pila` (coincide con la VM) y los dos `revientan` que aparecían
    al juntar los carriles (`141_lista_enlazada`, `197_dijkstra`).
  - `forja_leer_linea` recorta los espacios como `texto_de_lectura` de la VM: `  x  ` → `x`,
    línea en blanco → texto vacío, sólo el fin de entrada es `nulo` (era el PEDIDO L2 → L4).
    Medido con stdin por tubería en las dos rutas.
  - `podar_inalcanzable` no escaneaba el cuerpo de `main` si nadie la llamaba desde el nivel
    superior: la función se conservaba pero sus clases se podaban antes del backend. Ahora
    `main` es raíz. `forja llvm` de `71_rasgos`, `214_codificacion` y `199_forja_desde_forja`
    ya genera IR; `71` además compila AOT y coincide (+1). Las otras dos siguen sin bajar
    por primitivas sin equivalente (`base64_*`, y el caso de `199`), con error explícito.
  - Ejemplos reparados (rama `corpus/ejemplos-cierre`, puntero en `ejemplos`): `156`
    (asignaciones partidas por el formatter + `factorial` local + `absoluto`), `159`
    (`caracter` → `a_caracter`; sigue sin bajar por el **bug de nombres no ASCII** de abajo),
    `185` (reconstruido desde `9074678`: las asignaciones de Newton/Taylor/Mercator se habían
    vuelto `anon(...)`), `216` (`importar "std/resultado"` + `es_ok(r1)`).
  - `paridad-exclusiones`: `109` y `216` (encajado de escalares, ya conocido) y `185`
    (la VM trunca la aritmética Decimal de `potencia` y da 0 en la serie de `log`; el nativo
    da el valor correcto). Cada línea con su motivo medido.
- **Abierto y dónde está**:
  - `bench-competencias`: `parseo` quedó en 542-568 ms (1.05-1.11x de Rust) en tres corridas
    con el árbol quieto, contra 2160 ms antes del fix. **`baseline.json` no se actualizó**: las
    corridas de `dispatch`/`concurrencia`/`ffi` varían ±50 % entre corridas (42-94 ms el mismo
    binario), así que hornear este momento era ruido. Las tres corridas dieron "ratios dentro de
    la baseline" con la vieja.
  - **Nombres no ASCII en el IR**: una función con `ñ` (`generar_contraseña` en `159`) emite
    `@generar_contraseña` sin comillas y clang rechaza el `.ll` (`expected '(' in function
    argument list`). Afecta a cualquier ejemplo con acentos en identificadores; el arreglo
    (comillar o sanear el símbolo en todos los puntos de emisión) queda fuera de este cierre.
  - `216`: además del encajado, `tipo(canal)` da "objeto" en el nativo y "texto" en la VM, e
    imprimir un `Resultado` sale como puntero.
  - `214`/`199`: primitivas nativas sin equivalente (`_base64_*` y la de `199`), grupo B del plan.
  - `109`: campo sin tipo que guarda un texto (encajado de escalares, §9, bloqueado).
- Zona tomada al cerrar: ninguna; las ramas de carril quedan en `origin/` (`native/lowering`,
  `native/runtime`, `corpus/dorado`, `corpus/ejemplos-cierre`) y `vm/cuelgues` quedó local.

### 2026-11-18 16:10 — cierre de lo que la primera integración dejó abierto
Commits: `46b210ce`, `f4f40226`

- Medido (árbol limpio, `ejemplos` en `bf02c32`): `scripts/paridad-native-vm.sh` →
  **177 coinciden · 0 difieren · 56 no bajan · 6 no terminan · 4 error · 0 revientan**.
  `llvm_backend` 56 (55 + el nuevo), `llvm_runtime` 2, `lib` 129, `corpus` 299, `runtime` 102,
  clippy sin errores. `bench-competencias` (máquina quieta, sin otros carriles):
  `parseo` **562 ms · 1.05x** de Rust (baseline 546 ms · 1.03), resto dentro de ±10 % de la
  baseline y el script da "✅ ratios dentro de la baseline".
- Cambió:
  - **Identificadores con acento** (bug real, no un ejemplo roto): `variable año` emitía
    `%año.0 = alloca i64` y clang rechazaba el `.ll`; igual `funcion generar_contraseña`.
    `sanear_identificadores` (en las dos entradas del backend LLVM) cambia cada carácter no
    ASCII **de un identificador** por `_` antes de parsear, dejando intactas cadenas y
    comentarios (los emojis y acentos de los mensajes se imprimen igual). Test:
    `llvm_backend::identificadores_con_acento_bajan_a_nativo`. Con esto `159_generador_contraseñas`
    baja a nativo (queda excluido de paridad: arma la clave con `tiempo_actual()`).
  - `128_hilo_multiple` a exclusiones con motivo medido (intercalado de hilos, como 129/130/173;
    el contenido coincide y el diff cambia entre corridas).
- **`baseline.json` sigue sin actualizar, a propósito**: el `--update` midió `rust` de `parseo`
  en 630 ms (corrida contaminada) y dejaba un ratio 0.89 que no es el real; la referencia
  `2026-11-09` (546 ms · 1.03) sigue siendo válida y el árbol actual pasa contra ella.
- Abierto (con repro y sin dueño todavía):
  - `tipo(objeto)` y la repr de objetos/enums: la VM devuelve `"objeto"` y imprime `<Caja>` /
    `<Resultado>` / `[<CanalEmisor>,<CanalReceptor>]`; el nativo devuelve `"entero"` y el
    puntero. Es una feature de introspección (tag de objeto + tabla de nombres), no un bug de
    una línea. Afecta las dos divergencias de `216`.
  - `214`/`199`: primitivas nativas sin equivalente (`_base64_*` y la de `199`), grupo B del plan.
  - `109`: campo sin tipo que guarda un texto (encajado de escalares, §9, bloqueado por producto).
  - El árbol principal tiene 30 `.fa` sin commitear de otra sesión (`ejemplos/stdlib/*`,
    `fundamentos/720*`, `721*`) que inflan la medición de paridad hecha ahí (293 vs 263
    ejemplos): las cifras de arriba son con el corpus versionado.
