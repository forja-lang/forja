# Parches pendientes

Cambios verificados que **no** se dejaron aplicados porque el archivo pertenece a
otra sesión en curso (regla 3 de `COORDINACION-SESIONES.md`: un escritor por
archivo, commit por archivo). El árbol estaba tomado por la sesión dueña de
`src/backend_llvm/**` mientras se escribió esto.

## `handoff5-1.2-tabla-de-literales.patch`

- **Qué**: `handoff5.md` §1.2, la parte segura. Extrae `tipo_de_literal()` — la
  tabla literales → `T` que estaba copiada en cuatro sitios (`tipo_por_forma`,
  `inferir_campo::tipo_valor`, `inferir_retorno::tipo_valor` y
  `tipo_estatico`). Las dos copias con fallthrough se reemplazan por una llamada
  a la tabla; las seis ramas de `inferir_retorno` se borran porque su `_` ya
  delega en `tipo_estatico`, que usa la misma tabla. `tipo_estatico` **no** se
  tocó.
- **Dónde**: `src/backend_llvm/lowering.rs`.
- **Estado**: `cargo test -p forja --test llvm_backend` → 38/38. Verificado a
  mano contra la VM en `176_comando` (igual), `400_exacto` (igual) y los cuatro
  excluidos de siempre (`164`, `167`, `168`, `185`, siguen difiriendo igual).
- **Aplicar** (después de que la sesión dueña commitee su trabajo):

  ```bash
  git apply --check plans/parches/handoff5-1.2-tabla-de-literales.patch
  git apply       plans/parches/handoff5-1.2-tabla-de-literales.patch
  cargo test --offline -p forja --test llvm_backend
  ```

  Se generó contra el `lowering.rs` del 2026-11-17 16:51. Si no aplica, son tres
  sitios: copiar `tipo_de_literal` (está entera en el parche), hacer que
  `tipo_por_forma` devuelva `tipo_de_literal(e)`, y borrar las ramas de literales
  de los dos `tipo_valor` y de `inferir_retorno`.

## Lo que se midió y **no** conviene hacer: unificar el contagio aritmético

`handoff5.md` §1.2 también propone una única `tipo_probable` para
`tipo_con_pistas` (el pre-escaneo) y `tipo_estatico`. Se probó y **cambia
resultados**:

| Versión | `176_comando` (nativo) | VM |
|---|---|---|
| sin unificar (hoy) | `tras deshacer: 17 / 12 / 12` | idéntico |
| `tipo_con_pistas` con la regla completa (`Exacto` > `Decimal` > `Texto`) | `143 / 143 / 1430` | `17 / 12 / 12` |

La causa: cuando una pista dice `Texto` y la operación no es `Suma`, el código
de hoy cae a `tipo_estatico`, que durante el pre-escaneo **no conoce las
variables** y por eso responde `Entero`; la regla "completa" responde `Texto` y
la casilla pasa a concatenar (`143` + `0`). O sea: la "divergencia" entre las dos
funciones era deliberada y conservadora, no un descuido.

Conclusión: la parte de §1.2 que vale la pena es la tabla de literales (el
parche de arriba). El contagio aritmético **no** se unifica sin valores con tag
(`handoff4` §3.1); es otra entrada para la tabla de §1.3.

## Sobre `tipo_arg`

`handoff5.md` §1.2 dice que `tipo_arg` y `tipo_con_pistas` "inferían aritmética
por separado". No es así: `tipo_arg` sólo mira la forma del argumento
(`tipo_por_forma`), la variable de la que sale y los retornos anotados. Darle
contagio aritmético es justo el experimento de §1.3 que ya se midió como ruido
(mejora `185`, empeora `167`), así que no se hizo.
