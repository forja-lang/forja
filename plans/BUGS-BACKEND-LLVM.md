# Bugs del backend LLVM (legacy) encontrados al revisar `fib` con memo

Fecha: 2026-11-09. Archivo auditado: `src/backend_llvm/legacy.rs`
(camino único: `compilar_a_llvm_texto` → `forja::compilar_a_llvm` → `forja llvm`,
`forja medir`, AOT).

Contexto: `forja medir fib-r.fa` imprimía `140700345580752` en lugar de
`267914296`. La memoización (`memoizar_puras`) estaba bien: lo que fallaba era
la impresión. Ese bug ya está corregido (ver §0). Los dos de abajo son
preexistentes y quedan pendientes.

---

## 0. Corregido en esta sesión (referencia)

`escribir("x = " + n)` emitía `add i64 (ptrtoint "%s"), n` y lo imprimía con
`%lld`: mostraba `dirección + n` como número. Ahora `escribir` elige el formato
(`%s` / `%lld`) y una concatenación textual se imprime parte por parte, con
`texto_vars` para recordar qué identificadores guardan `Texto`.

Límite conocido (deliberado): sólo se arregla la **impresión**. El valor
sigue siendo un `i64` sin tipo, lo que deja vivo el bug §1.

---

## 1. `Texto + Entero` fuera de `escribir` sigue siendo una suma de punteros

Estado actual: **RESUELTO (2026-11-09)** por el backend nuevo: `Texto` es `ptr`
y `+` con un lado texto llama a `forja_concat`. Test:
`tests/llvm_backend.rs::texto_mas_entero_concatena_en_variables_y_retornos`.

`suma` en `expr`/`bin` (y en `generar_expr_con_resultado`) siempre emite
`add i64`. Con `Texto` representado como `ptrtoint i64`, cualquier
concatenación que no vaya directo a `escribir` produce basura:

```forja
funcion main() {
    variable a = 5
    variable s = "x = " + a   // s = puntero_de_str + 5
    escribir(s)               // imprime un número enorme
}
```

Reproducción:

```
forja llvm hola.fa && clang hola.ll -o hola.exe && ./hola.exe
```

Alcance: `variable s = ...`, `retornar "..." + n`, argumentos de funciones,
campos de clase, `+` anidado en expresiones no impresas.

Opciones (de menor a mayor esfuerzo):

1. **No soportarlo explícitamente**: rechazar en `TypeChecker`/backend la
   concatenación fuera de `escribir` con un error claro en español. 10 líneas,
   elimina la mentira silenciosa. Es la opción que sigue el resto del backend
   (que ya devuelve `Err` para `LiteralExacto`, `si` en argumentos, etc.).
2. Concat real: `declare i8* @forja_concat(i64, i64)` + conversión de entero a
   texto (`forja_entero_a_texto` ya existe en el backend ASM,
   `src/compiler_asm.rs:1848` y `:2033`). Hay que portar esas dos rutinas a IR y
   decidir el tipo de cada operando con un `expr_es_texto` equivalente, además
   de propagar la marca de `Texto` a través de parámetros y retornos.
3. El arreglo de fondo es el backend nuevo (`src/backend_llvm.rs`), que sí
   tiene tipos (`LlvmType`). El legacy no debería seguir creciendo (lo dice su
   propia cabecera).

---

## 2. Variables de nivel superior sin `main` generan IR inválido

Estado actual: **RESUELTO (2026-11-09)**: los globales son
`@g_x = internal global` y su inicialización va dentro de `@forja_init`. Test:
`tests/llvm_backend.rs::globales_sin_main_generan_ir_valido`.

`decl()` se ejecuta antes de que exista `@main`, así que un `alloca`/`store`
de nivel superior se emite **fuera** de cualquier función; después `auto_main`
vuelve a emitir las mismas declaraciones dentro de `main`. Resultado: clang
rechaza el `.ll`.

Reproducción:

```forja
variable a = 5
escribir("x = " + a)
```

```
forja llvm c.fa
clang c.ll -o c.exe
# c.ll:5:3: error: expected top-level entity
#     5 |   store i64 5, i64* %a.a
```

IR emitido (extracto):

```llvm
declare void @forja_contract_error(i8*)
  store i64 5, i64* %a.a      ; <- nivel superior, inválido
define i32 @main() {
  entry:
  %a.a = alloca i64
  store i64 5, i64* %a.a      ; <- duplicado
```

Notas: con `funcion main()` explícito no aparece. `fib-r.fa` no lo dispara
porque no tiene variables globales.

Arreglo mínimo: en `compile()`, buffear la salida de `decl()` y, si no hay
`main`, emitir sólo lo que no sea `Declaracion::Variable`/`Asignacion` de nivel
superior (auto_main ya las reemite), o abrir `@main` antes del bucle y cerrarlo
al final. Cuidado: hoy `auto_main` (`legacy.rs:1546`) re-emite **todas** las
declaraciones, así que hay que verificar que no se dupliquen las `Funcion`.

---

## Verificación sugerida para cerrar §1 y §2

Agregar al final de `src/backend_llvm.rs` (tests del backend legacy):

- `"x = " + 5` asignado a variable e impreso → `x = 5`
- `"a" + 1 + "b"` en `retornar` → texto correcto
- programa sin `main` con variable de nivel superior → el IR contiene un solo
  `alloca` y compila con clang

Los tres casos de §0 ya se probaron a mano (`fib(42) = 267914296`,
`hola Ana3`, `solo int 5`) y `cargo test -p forja` está verde.
