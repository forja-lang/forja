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

`native/atajos` (worktree `C:/Users/gaucho/forja-shortcuts`) **sigue sin mergear**:
5 commits de backend nativo más (`a502dcfb` evidencia por clase+método,
`80a3cbde` `asegura` antes de cada retorno, `13798a3c` firma de closures,
`135cc66a` test `nulo` vs `0`, `d6824c56` `FORJA_DEBUG_TAGS`). Decisión de quien
lleve esa sesión: mergear o no.

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

## Medido (árbol con este commit, release)

```
nativo vs VM: 154 coinciden · 0 difieren · 116 no bajan · 2 no terminan · 6 error · 0 revientan
```

Tests: `llvm_backend` 40/40 · `lib` 129 · `corpus` 299 · `forja-vm`/`forja-cli`/
`forja-syntax` OK · `rustfmt --check` y `clippy --lib` limpios en lo tocado.

Exclusiones nuevas (con motivo medido): `709_texto` (la VM pega un `nulo` al
texto envuelto: `...español.nulo`) y `153_ahorcado` (elige palabra con
`tiempo_actual()`, no determinista; coincidió una vez por casualidad).

## Lo que sigue rojo/abierto

- **`a_texto(mapa)` imprime la dirección** (shortcut documentado en `como()`).
  La VM da `{"a":10}`. Es el `shortcut:` más visible que queda.
- `nulo` vs `0`: `r[b]` da `0` donde la VM da `nulo` (handoff4 §3.1, sigue
  pendiente el modelo de valores con tag).
- `212`-style dobles índices siguen dependiendo del tag en runtime.
- `native/atajos` sin mergear (ver arriba).
- `origin/main` está 26 commits atrás: nadie pusheó.
