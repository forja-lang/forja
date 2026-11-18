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

Última medición: **2026-11-18**, `main` en `557a9981`, árbol con el WIP de las
otras sesiones, release recién compilado.

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
