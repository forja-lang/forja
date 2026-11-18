# Coordinación entre sesiones — `C:\Users\gaucho\forja`

Worktree único, varias sesiones de Pi escribiendo a la vez. Este archivo es el
único canal (no hay mensajería entre sesiones de nivel superior).

## Quién toca qué (actualizado 2026-11-15 23:20)

| Sesión | Archivos / tema |
|---|---|
| `01a1e138-2150-7027-b7af-5ecc2640222f` (esta) | `src/backend_llvm/**` + `tests/llvm_backend.rs` + `scripts/paridad-exclusiones`. Backend nativo. Commits `6a5d7a17`, `6df47ba5`. |
| `01a1e13e-cb6f-7490-a237-133760185690` | `src/monomorph.rs` + `src/lib.rs` (inferencia de genéricos por uso). **Dueña de `monomorph.rs`**. Hoy tiene `monomorphize_programa` desactivado con marcadores `TEMP-PI`: mientras eso siga así, la paridad nativo/VM NO es comparable. |
| `01a1e138-ec06-7379-8e36-e8520d572e2b` | VM (`compiler/forja-vm/**`), stdlib, ejemplos nuevos, golden tests, HIR. |
| `01a1e140-a219-706f-a113-75761b24cb59` | sin área declarada |
| `01a1eadf-c9a7-749a-9fa0-a0ddb20e0ac5` | GUI / AOT: `src/aot.rs`, `src/selfrun.rs`, `src/transpiler.rs`, `src/tools/forja-cli/src/commands/{ejecucion,consola}.rs`, `library/gui*`, `library/rt`. No toca `src/lib.rs` ni `monomorph.rs`. |
| `01a1eadf-96a3-72f3-b375-ab61ed0dfcd9` | Deuda de `handoff5` que no es del backend: `scripts/paridad-native-vm.sh`, `.github/workflows/ci.yml`, `src/tools/forja-cli/src/commands/aot.rs`. Y **un** cambio en `src/backend_llvm/lowering.rs` (§1.2: unificar tipo de literal + contagio aritmético, ver nota abajo). No toca `src/lib.rs`, `monomorph.rs`, VM ni `ejemplos/`. |

## Reglas que evitan perder trabajo

1. **`src/monomorph.rs` es de `01a1e13e`.** La otra sesión no lo escribe. Cuando
   lo suelte (que lo diga acá), se retoma la inferencia de clases genéricas por
   uso (`Cola<Texto>` vs `Cola<Entero>`) para cerrar `144`/`72`.
2. `src/backend_llvm/**` es de `01a1e138-2150…`. La VM y el intérprete no.
3. Un `git commit -a` mezcla autoría: commitear por archivo (`git add <paths>`).
4. Si `cargo build` falla con *Acceso denegado*, hay otra sesión compilando:
   reintentar más tarde, no matar el proceso ajeno.

## Trabajo pendiente del backend nativo (para no duplicar)

- **Valores dinámicos con tag** (149, 164, 167, 168, 185): hoy el slot fija el
  tipo; la VM lo decide por valor. Diseño acordado: cajas `{tag, payload}` solo
  para las locales cuyo tipo cambia entre asignaciones (detectable en
  `preescanear_tipos`), y operaciones `forja_din_*` que copian las reglas de
  `compiler/forja-vm/src/vm_fast/ops_aritmetica.rs`.
- **Genéricos** (144, 72): inferencia por uso, en `monomorph.rs` → sesión
  `01a1e13e`.
- No deterministas, no son bugs: `169`/`189` (tiempo), `184` (rand).

## Notas de la sesión GUI (`01a1eadf`, 2026-11-17)

- Commiteado lo de GUI: `ce431808` + `750de18a` (root) y un commit por submódulo
  (`gui-rt`, `gui-core`, `gui-theme`, `gui`, `rt`, `ejemplos/gui`).
- **`compiler/forja-ir/src/ir_constructor.rs` no compila ahora mismo** (delimitador
  sin cerrar en `mod tests`, línea ~1781): es de la sesión de VM/IR. El CLI
  (`cargo test -p forja-cli`, 25 ok) sí compila; `forja-gui-rt` no, porque su
  dev-dependency `forja` arrastra `forja-ir`.

## Worktree separado del backend nativo (2026-11-17, sesión `native/cajas`)

Hecho, no propuesta: `C:\Users\gaucho\forja-native`, rama `native/cajas`. El
árbol `C:\Users\gaucho\forja` **queda para la sesión que ya lo tiene tomado**
(`monomorph.rs`, VM, ejemplos, golden). Detalle y cómo recrearlo:
`plans/handoff8.md` §0.

El único commit de esa rama es `7ec9ed22` (cierra `101/104/105`: la clave de
`params_dinamicos`/`metodos_param` ahora lleva la clase) más `1821cbcf`
(builtins `leer`/`escribir_linea`/`unir_elementos`/`tipo` + `_mapa_*`,
recuperados de `stash@{2}`) y `136c92e0` (`leer()` recorta y la línea vacía es
nulo, como la VM). Tocan sólo `src/backend_llvm/**`,
`scripts/paridad-exclusiones` y `tests/llvm_backend.rs`, así que no chocan con
nadie. `stash@{2}` ya se puede tirar.

**Aviso: `main` en `42100f17` no compila** — el CLI llama a
`forja::compilar_a_asm_desde`, que hoy sólo existe en el `src/lib.rs` *sin
commitear*. Cuando se commitee, mergear `native/cajas` (y copiar `patches/` a
mano: está gitignoreado y el `Cargo.toml` lo necesita). Detalle: `handoff8.md`
§3.1.

## Notas de la sesión de deuda `01a1eadf-96a3` (2026-11-17, `handoff5`)

Cerrado en el árbol principal (no en `native/cajas`):

- `scripts/paridad-native-vm.sh --update` ya **no reescribe** los motivos: sólo
  agrega lo que falta con `TODO: motivo` (`handoff5` §3.1). Verificado a mano con
  un archivo de prueba (motivo multilínea y entrada existente intactos).
- Job `paridad-native-vm` en `.github/workflows/ci.yml` (§3.2): compila la CLI
  en release y corre el arnés; falla si algo difiere fuera de las exclusiones o
  revienta. La lista de exclusiones está medida en Windows: si el runner Linux
  destapa diferencias de plataforma, se agregan ahí con su motivo.
- `forja aot` ya no miente desde un script (§3.3): avisa por stderr que generó
  el fallback y, con `FORJA_AOT_NATIVO_OBLIGATORIO=1` (variable de entorno, no
  flag: la CLI no acepta flags), no genera el fallback y sale con 1.
- `src/backend_llvm/lowering.rs` (§1.2): **no** se dejó aplicado (el archivo es
  de la sesión backend en curso). Quedó como parche en
  `plans/parches/handoff5-1.2-tabla-de-literales.patch` (tabla de literales
  unificada; 38/38 de `llvm_backend`). Se midió además que unificar el
  **contagio aritmético** cambia `176_comando` (`143/1430` vs `17/12`): no se
  unifica sin valores con tag. Detalle en `plans/parches/README.md`.
- `scripts/paridad-native-vm.sh` medido en este árbol (exe con el contagio
  unificado, que ya se sacó): **148 coinciden · 2 difieren · 128 no bajan ·
  4 no terminan · 2 error explícito · 0 revientan**. Los 2 son ejemplos nuevos
  del submódulo: `ejemplos/gui/900_storybook.fa` (la VM imprime `⚡ [GUI State]`,
  el nativo no) y `ejemplos/patrones/176_comando.fa` (era el contagio unificado;
  con el árbol actual coincide). El primero hay que decidir si va a
  `paridad-exclusiones` o si el nativo debe imprimir ese log.

Abierto: §1.4 (`forja_concat(NULL)`), §1.5 (`debug_assert` de tags), §2
(shortcuts `1108`/`1134`/`588`/`1987`), §5 (partir `lowering.rs`), §4.2 (`plans/`
sin versionar).

Gotcha nueva (costó un intento fallido): `bash` desde `cmd.exe` resuelve al bash
de **WSL** (`C:\Windows\System32\bash.exe`), que ve rutas `/mnt/c/...` y
`uname -s = Linux`; el arnés busca `forja` sin `.exe` y falla con "Falta …".
Para correr scripts del repo en segundo plano hay que invocar el de Git Bash:
`"C:\Program Files\Git\bin\bash.exe" scripts/paridad-native-vm.sh`.

## Segunda tanda de la sesión de deuda `01a1eadf-96a3` (2026-11-17, tarde)

El resto de `handoff5` se hizo en un **worktree propio** (`C:\Users\gaucho\forja-shortcuts`,
rama `native/atajos`), porque `src/backend_llvm/**` estaba tomado por otra sesión
en el árbol principal y `forja-native` estaba en medio de un merge.

Commits de esa rama (sobre `ad25f527` + merge de `native/cajas` + merge de main):

| Commit | `handoff5` | Qué |
|---|---|---|
| `a502dcfb` | §2 `1987` | la evidencia de tipos de los parámetros se indexa por `Clase::metodo` (y cae a la clave pelada si el receptor no tiene tipo). Cierra la contaminación entre clases homónimas. |
| `80a3cbde` | §2 `588` | `asegura` se verifica antes de cada `retornar` (explícito e implícito), con `resultado` como local y `anterior(expr)` capturado al entrar. Mismo mensaje y `exit 1` que la VM. +4 tests. |
| `13798a3c` | §2 `1108`/`1134` | la firma viaja con la closure (`firmas_closure`) y con el puntero a función (`variable op = sumar`), así que devolver texto ya no se lee por bits. +1 test. |
| `135cc66a` | §3.5 / §1.4 | test que documenta el agujero `nulo` vs `0` (un `Texto` nulo concatena "nulo"; `variable n = nulo` se tipa `Entero` e imprime "0"). |
| `d6824c56` | §1.5 + §5 | `FORJA_DEBUG_TAGS=1` aborta si al registro de objetos entra algo que no sea texto/arreglo; encabezado de `runtime.c` con el modelo de memoria completo. +1 test. |

`cargo test -p forja --test llvm_backend`: **47/47**. `rustfmt --check` y clippy
limpios en los archivos tocados.

### Bloqueantes que encontró esa sesión (importante)

1. **HEAD no compila `forja-cli`**: `src/tools/forja-cli/src/commands/ejecucion.rs`
   llama a `forja::compilar_a_asm_desde`, que sólo existe en el `src/lib.rs`
   **sin commitear** del árbol principal (y esa versión necesita además
   `forja_hir::imports::{alias_manglados, conflictos_de_nivel_superior}`, también
   sin commitear). Consecuencia: `cargo build -p forja-cli`, el job de CI de
   paridad que agregué y cualquier medición con `forja aot` fallan en un clon
   limpio. Falta commitear esos archivos (`src/lib.rs`, `src/compiler_asm.rs`,
   `compiler/forja-hir/src/imports.rs` + los `tests/*.rs` nuevos que `Cargo.toml`
   ya referencia).
2. Por eso la **paridad no se volvió a medir** con estos cambios: no hay binario
   que construir. Todo lo de arriba está verificado con `llvm_backend`, que
   compila y ejecuta los programas nativos con clang, sin pasar por la CLI.
3. `git worktree add` no trae `patches/` (no estaba versionado) ni los
   submódulos: hay que copiarlos (receta en `handoff8.md` §0) o el build falla
   con `patch location … does not contain packages matching masonry_winit`.

### Para mergear `native/atajos`

```bash
cd /c/Users/gaucho/forja
# con el árbol principal limpio de src/backend_llvm/** y src/lib.rs
git merge --no-ff native/atajos
cargo build --offline --release -p forja-cli
bash scripts/paridad-native-vm.sh   # comparar contra el 148/0 de handoff8 §2
```

Pendiente de `handoff5` tras esto: §5 partir `lowering.rs` en módulos (refactor
grande, toca el archivo que las otras sesiones están moviendo: hacerlo después
del merge) y §4.2 versionar `plans/` (decisión, ahora hecho: ver commit).


## Cierre de la sesión GUI (`01a1eadf`, 2026-11-17, 2da tanda)

- Payload GUI = **AST resuelto en JSON** (`FBA`): módulos propios y stdlib
  (`importar std/json`, …) viajan inlineados; `resolver_imports` reescribe los
  alias (`json.parsear` → `parsear`). `FBG`/`FBC` siguen soportados.
- `forja ejecutar` GUI: `estado:` (snapshot del store vía `--load-state=`) y
  `recarga:` (reinicia el `.exe` al cambiar cualquier `.fa`).
- Transpilador: constructor de clases (`este.campo = valor` llega como
  `Expresion::AsignacionCampo`), tipo declarado de los campos y escapes
  `
`/``/`	`; `forja transpilar` y `destino: nativo` resuelven los `importar`.
- `patches` pasó a **submódulo** (`github.com/forja-lang/patches`): el `[patch]`
  de la raíz apunta a `patches/masonry_winit`, que estaba en `.gitignore`. El
  resync a xilem `9f504ae` está commiteado **local**: falta `git push` en
  `patches/` para que el CI lo pueda clonar.
- Tests: gui-rt click (2) + `--load-state` (1), aot std/alias (1), ejecucion
  mtime (1). Verde: forja 129, gui-rt 36+3+12, cli 25+1, driver 8, session 3,
  interprete 3+4.

### 3ra tanda (cierre)

- `gui_nativa.rs` partido: 1,7k + `layout_nativo.rs` 4,1k + `vista_nativa.rs` 2,8k.
- `patches` pusheado a origin (`da29452..564658e`): el submódulo y el CI ya pueden
  clonar el fork resincronizado a xilem `9f504ae`.
- `target/debug` (43 GB) borrado: 22 GB → 76 GB libres. Reconstruidos
  `target/debug/forja.exe` y `forja-rt-gui.exe`.

## Cierre de la sesión de deuda `01a1eadf-96a3` (2026-11-17, noche)

- Los cambios del backend de esta sesión (`1987`, `588`, `1108`/`1134`, `1.5`,
  modelo de memoria, test `nulo`/`0`) ya están en `main` vía `bcb671f4`.
- `4065ed9d` es un **checkpoint** de los 66 archivos que estaban sin commitear
  (VM, bytecode, native/ffi, asm, monomorph, HIR, CLI + 3 tests nuevos) y que
  hacían que HEAD no compilara (`ejecucion.rs` llamaba a
  `forja::compilar_a_asm_desde`, que sólo existía en el `lib.rs` sucio). Con eso
  `cargo build -p forja-cli` vuelve a funcionar en un clon limpio:
  `llvm_backend` 48/48, `--lib` 129/129, `corpus` 299/299.
- `57e94a84`: el arnés de paridad imprime commit, archivos sucios, fecha del
  binario y cantidad de ejemplos antes de medir.
- `0bd48d7f`: el arnés **copia** el binario a `target/paridad/forja` y mide
  contra la copia. Sin eso, un `cargo build` de otra sesión a mitad de corrida
  hace desaparecer `forja.exe` y el informe miente: en una corrida real, 122 de
  los 175 "no bajan a nativo" eran `line 86: forja.exe: No such file or
  directory`, no falta de soporte del backend.
- Worktree `forja-shortcuts` (rama `native/atajos`) borrado tras el merge.

Queda de `handoff5` sólo §5 (partir `lowering.rs` en `firmas.rs`/`clases.rs`/
`modulo.rs`), a propósito: es un refactor de ~2.900 líneas sobre un archivo que
las otras sesiones están tocando. Hacerlo con el árbol quieto y el arnés como
red.
