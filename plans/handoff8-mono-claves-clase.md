# Handoff 8 — monomorfización reactivada y claves con clase

Fecha: 2026-11-17 (sesión paralela a la de GUI/paridad y a la del backend nativo).
Sigue a `handoff4.md` §3.2 y a `handoff7.md` §3. **No** reemplaza a ninguno.

> El árbol está sucio con el WIP de dos sesiones más y hay commits nuevos
> (`845e6d23`, `eafe0d98`, `fe4129e1`, `3b720e6b`). **Nada de esto está
> commiteado**: el diff de `lowering.rs` mezcla mis hunks con los de otra sesión
> (refactor `tipo_de_literal` / `contagio_aritmetico` / `inferir_campo`).

## Medido

```
antes (handoff7): 145 coinciden · 3 difieren · 128 no bajan · 5 no terminan · 2 error · 0 revientan
ahora:            148 coinciden · 0 difieren · 128 no bajan · 2 no terminan · 5 error · 0 revientan
```

`cargo test -p forja --test llvm_backend` 38/38 · `--lib` 128 · `rustfmt --check` y
`clippy --lib` limpios en los archivos tocados.

## Cambios

1. `src/lib.rs` — **monomorfización reactivada**: se borró el `return` de
   `TEMP-PI` y la copia muerta `monomorphize_programa_original`. Sin esto toda
   medición de paridad era contra un nativo sin especializar.
2. `src/backend_llvm/lowering.rs` — `params_dinamicos` se indexa
   `metodo@Clase:indice` (antes `metodo:indice`). El `T` de la clase genérica ya
   no contagia a su especialización: el call site encajaba `i64` donde la firma
   especializada espera `ptr` (clang fallaba y el ejemplo caía a bytecode).
3. `src/backend_llvm/lowering.rs` — `metodos_param` se indexa `Clase::metodo` y
   `expresiones.rs::incierto()` consulta la clase del receptor (con respaldo por
   nombre cuando el receptor no tiene tipo). Antes el `-> T` de la clase genérica
   marcaba como «sin tipo» al método de la especialización y se ignoraba el tipo
   estático: `Booleano: 1` en vez de `verdadero`.
4. `src/backend_llvm/lowering.rs` — los `campos` de una clase entran en la tabla
   antes de calcular sus métodos.
5. `src/monomorph.rs` — `buscar_usos` reconoce la forma sentencia
   `obj.metodo(args)` (llega como `LlamadaFuncion { nombre: "obj.metodo" }`, no
   como `LlamadaMetodo`). Es lo que faltaba para que `recolectar_por_uso` viera
   algo, pero **sigue sin conectarse**: ver abajo.

## Efecto

`101`, `104`, `105`, `107`, `144` coinciden exactamente con la VM (5 de los 6 que
eran diff o fallo de clang). `72` sigue difiriendo (`0` donde la VM imprime
`mundo`).

## Intentado y revertido (no repetir a ciegas)

- **Inferencia por variable (`recolectar_por_uso`)**: con el arreglo de
  `buscar_usos` ya funciona, especializa `Cola` en `Cola_Texto` y `Cola_Entero`,
  y `144` **revienta** (segfault en `Cola_Entero_frente`): `arreglos_texto` /
  `arreglos_dec` / `arreglos_arreglo` se indexan con claves sin clase
  (`"este.elementos"`, nombre de variable), así que el marcado de una
  especialización se aplica a la otra. Para reactivarlo hay que cualificar
  `clave_arreglo` (y los `insert` de variables/parámetros) por clase.
- **`clase_actual` para tipar `este.campo`** (arreglaba `72`): rompe `176_comando`
  (`143` vs `17`); el campo `historial` pasa a tiparse Arreglo también en el
  cuerpo y cambia el camino estático/dinámico. Revertido.

## Pendiente que este trabajo deja más cerca

- `72` (`retornar este.campo` sin anotación) necesita el tipo del campo en la
  inferencia del retorno **sin** cambiar la tipificación del cuerpo — o el modelo
  de valores de `handoff4.md` §3.1.
- `201_gui_ventana` sigue siendo la «mentira silenciosa» de `handoff4.md` §3.5.
- `scripts/paridad-native-vm.sh` estaba **sintácticamente roto** (comilla sin
  cerrar en el resumen de fallos) mientras otra sesión lo editaba; y dos paridades
  simultáneas chocan en `target/paridad` (una borra los `.out` de la otra). Medir
  con el árbol quieto y una sola corrida.
