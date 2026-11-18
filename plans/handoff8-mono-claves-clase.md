# Handoff 8 — merge de `native/cajas` + arreglos que destapó

Fecha: 2026-11-17 (sesión paralela a la de GUI y a la de `forja-native`).
Continúa `handoff4.md` §3.2, `handoff7.md` §3 y `handoff5.md`.

## Merge

`native/cajas` estaba 2 commits adelante de `main`:

- `7ec9ed22` fix(native): la clave de parametro/retorno dinamico lleva la clase
  (`params_dinamicos`/`metodos_param` cualificados por clase; cierra 101/104/105).
- `1821cbcf` feat(native): builtins de entrada e introspeccion (recupera
  `leer`, `tipo`, `unir_elementos`, `escribir_linea`, `_mapa_*`).

Mergeado con `--no-ff` en `0b2cc95c`. Único conflicto: `scripts/paridad-exclusiones`
(ambos lados agregaron entradas) → se conservaron las dos. El WIP de las otras
sesiones se guardó en stash y se devolvió con `pop --index` después del merge.

`native/atajos` (worktree `C:/Users/gaucho/forja-shortcuts`) también mergeado
después, en `bcb671f4` (sin conflictos): `a502dcfb` evidencia por clase+método,
`80a3cbde` `asegura` antes de cada retorno, `13798a3c` firma de closures,
`135cc66a` test `nulo` vs `0`, `d6824c56` `FORJA_DEBUG_TAGS`. Verificado con la
misma batería: paridad igual (154 · 0 · 116 · 0 revientan), `llvm_backend` 48/48,
`lib` 129, `corpus` 299.

## Arreglos de esta sesión (`fef2883d`)

Al habilitarse los builtins nuevos, 12 ejemplos más bajaron a nativo y
aparecieron cuatro bugs (uno con **segfault**):

1. `runtime.c` — `forja_subcadena` tomaba el 3er argumento como índice final; la
   VM lo toma como longitud. Toda `std/texto` estaba cortando un byte.
2. `longitud(x)` con `x` de tipo Entero leía `len` como arreglo: con
   `variable p = partes[i]` el «arreglo» era un texto y devolvía sus primeros
   bytes (`envolver`). Ahora usa `forja_longitud_dinamica` (por tag).
3. `tipo_builtin` no conocía `_mapa_pares/_mapa_claves/_mapa_valores`
   (Arreglo) ni `unir_elementos/reemplazar/invertir` (Texto), y
   `inferir_retorno` no consultaba builtins: `pares()` se tipaba Entero →
   `inttoptr` sobre un arreglo → **segfault** en `fusionar` (`732`).
4. `m[clave] = v` con clave que no es Texto estático guardaba los bits del
   puntero como clave numérica y se perdían las entradas. Nuevos
   `forja_mapa_{fijar,obtener}_dinamico` (deciden por tag).
5. Un parámetro que sólo se usa como aguja de `contiene(texto, ch)` no se veía
   como Texto: `_es_digito` recibía el texto como número (segfault en `709`).

## GUI: de mentira silenciosa a error explícito (§3.5 de `handoff4.md`)

`forja aot` sobre una app GUI compilaba, salía con 0 y **no imprimía nada** (la
VM tampoco corre: no hay host de widgets). Ahora `cmd_aot` detecta
`importar gui` y falla con mensaje y código 1, señalando `forja ejecutar`
(`forja-rt-gui`) o `forja compilar`. Los 3 ejemplos GUI pasan de «difieren» a
«no bajan» (skipped) y la exclusión de `900_storybook` se fue (ya no aplica).

## Medido (árbol con este commit, release)

```
nativo vs VM: 154 coinciden · 0 difieren · 119 no bajan · 2 no terminan · 3 error · 0 revientan
```

Tests: `llvm_backend` 40/40 · `lib` 129 · `corpus` 299 · `forja-vm`/`forja-cli`/
`forja-syntax` OK · `rustfmt --check` y `clippy --lib` limpios en lo tocado.

Exclusiones nuevas (con motivo medido): `709_texto` (la VM pega un `nulo` al
texto envuelto: `...español.nulo`) y `153_ahorcado` (elige palabra con
`tiempo_actual()`, no determinista; coincidió una vez por casualidad).

## Benchmarks re-medidos (§3.6) — hay una regresión vieja

`scripts/bench-competencias.sh` (3 reps, mínimo, árbol quieto):

| caso | forja | rust | ratio | baseline 2026-11-09 |
|---|---|---|---|---|
| ffi | 38 | 37 | 1.03x | 1.02x |
| dispatch | 52 | 52 | 1.00x | 1.03x |
| **parseo** | **1893** | **522** | **3.63x** | **1.03x (546 ms)** |
| memcache | 248 | 255 | 0.97x | 1.04x |
| vectores | 85 | 122 | 0.70x | 1.51x (0.97x con `reservar`) |
| overflowpanic | 66 | 66 | 1.00x | 1.01x |
| hash | 354 | 262 | 1.35x | 0.94x |
| erroresanidados | 355 | 1283 | 0.28x | 0.61x |
| concurrencia | 40 | 41 | 0.98x | 0.42x |
| stress | 122 | 256 | 0.48x | 1.51x |

**`parseo` es 3.4x más lento que la baseline** (Rust se mide igual: 522 vs 531 ms,
así que no es la máquina). Perfilado a mano con el mismo binario:

- construir los textos (`forja_concat` + 2 `a_texto`) = **1.42 s de 1.75 s**;
  `subcadena` sólo 0,15 s; `indice_de` despreciable.
- **no es el registro de tags**: compilando sin `forja_registrar` en
  `forja_texto_alloc` el caso tarda lo mismo (1849 ms vs 1750 ms).
- escala casi lineal (250k→182 ms, 500k→389, 1M→846, 2M→1750).

La regresión está entre la baseline (2026-11-09, `98b6693b`) y hoy, en el camino
de alloc/texto; no la introdujo este merge (no toca `forja_concat`/`a_texto`).
**No actualicé `baseline.json`**: publicarla dejaría el 3.6x como «normal» y el
job de CI (`Bench competencias`) es justamente el que avisa. Siguiente paso
sugerido: bisecar `98b6693b..HEAD` sobre `runtime.c` con este perfil (3 min por
commit) y mirar `forja_alloc`/`forja_texto_alloc`.

## Lo que sigue rojo/abierto

- **`a_texto(mapa)` imprime la dirección** (shortcut documentado en `como()`).
  La VM da `{"a":10}`. Es el `shortcut:` más visible que queda.
- `nulo` vs `0`: `r[b]` da `0` donde la VM da `nulo` (handoff4 §3.1, sigue
  pendiente el modelo de valores con tag).
- `212`-style dobles índices siguen dependiendo del tag en runtime.
- `main` está pusheado (`460fe98f`). Las ramas `native/*` quedaron ya dentro.
