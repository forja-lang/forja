# Estado actual y correcciones pendientes

Documento de handoff para otro agente. Fecha: 2026-11-09.
Repositorio: `C:\Users\gaucho\forja`.
Rama: `main`.
Últimos commits relevantes:

- `2cede471` — Android GUI real, macOS FFI, cobertura ASM y CI.
- `4ecc67a3`, `16b01ca3`, `eedcda0b` — reorganización, tests y cobertura.

---

## 1. Resumen ejecutivo

El árbol **compila y los tests locales pasan**, pero el CI **no está verde** y el
worktree tiene cambios sin commitear de otra sesión/agente.

Estado local verificado ahora:

```text
cargo test --locked              → 1852 pasados, 0 fallidos, 7 ignorados
cargo xtask verify -p forja      → FALLA en cargo fmt --check
scripts/asm-coverage.sh          → 100.00% (con exclusiones LCOV)
cargo check --workspace          → OK (verificar de nuevo)
```

Estado del CI más reciente (`42543233847`, commit `2cede471`):

- Falla **Cobertura ASM (100%)**.
- Falla **Build y Test (macos-latest)**.
- Android: `aarch64` pasó; el resto seguía en cola al momento de escribir esto.
- El resto de jobs pasó.

---

## 2. Fallo 1 — Cobertura ASM en Linux: 99.95 %

### Síntoma

En Windows/local el script da 100%; en Linux/CI da **99.95%**.

Líneas faltantes reportadas por el LCOV del CI:

```text
1857
1861
```

### Causa raíz

`src/compiler_asm.rs`, función `compilar_texto_operando`:

```rust
fn compilar_texto_operando(&mut self, expr: &Expresion) {
    let a = self.arch;
    self.compilar_expresion_asm(expr);
    if !self.expr_es_texto(expr) {
        self.emit_line(a.mov_reg_reg(a.arg_regs()[0], a.ret_reg()));
        let ss = a.shadow_space();
        if ss > 0 {
            self.emit_line(a.sub_sp(ss));      // 1857
        }
        self.emit_line(a.call("forja_entero_a_texto"));
        if ss > 0 {
            self.emit_line(a.add_sp(ss));      // 1861
        }
    }
}
```

En Linux `X86_64Linux::shadow_space()` devuelve 0, así que las ramas `ss > 0`
nunca se ejecutan. En Windows sí, por eso local da 100%.

### Fix recomendado

Añadir un test de caja blanca que fuerce el target Windows:

```rust
#[test]
fn cubre_texto_operando_con_shadow_space() {
    let mut c = CompilerAsm::with_target(TargetArch::X86_64Windows);
    c.compilar_texto_operando(&Expresion::LiteralNumero(1));
}
```

Ya hay un test `cubre_caja_blanca_final` en `src/compiler_asm.rs`; agregar ahí
la llamada con `with_target(X86_64Windows)`.

Además, las líneas 1802 y 1815 están marcadas `// LCOV_EXCL_LINE` porque son
llaves de cierre que LLVM no atribuye. El script `scripts/asm-coverage.sh` las
filtra leyendo el fuente, pero el reporte todavía las muestra como missed. No
volver a poner el umbral en 94: el script debe seguir exigiendo 100.

### Validación

```bash
PYTHON="C:/Users/gaucho/AppData/Local/Python/pythoncore-3.15-64/python.exe" \
  ASM_COVERAGE_THRESHOLD=100 bash scripts/asm-coverage.sh
```

Debe imprimir `100.00%` y salir 0.

---

## 3. Fallo 2 — macOS: FFI automática

### Síntoma

Job `Build y Test (macos-latest)` falla en:

```text
tests/corpus/lexico_sintaxis_corpus.rs::ffi_enlace_automatico_sin_resolver_de_imports
```

Resultado observado:

```text
left:  ["falso", "nulo"]
right: ["verdadero", "4"]
```

### Código actual

El test tiene tres ramas:

- Windows: `kernel32.dll`
- macOS: `libSystem.B.dylib`
- resto: `libc.so.6`

La rama macOS ya se agregó y está commiteada, pero el runtime no resuelve
`getpid`/`strlen` con `libSystem.B.dylib` en el runner.

### Fix recomendado

1. Probar la ruta absoluta:

```rust
"importar externa \"/usr/lib/libSystem.B.dylib\"\n..."
```

2. Si el cargador no acepta rutas absolutas, revisar `compiler/forja-native/src/ffi.rs`
   y el cargador de `importar externa` para macOS (`dlopen`/`dlsym`).
3. Si macOS no soporta FFI automática por diseño, no silenciar: marcar el test
   con `#[cfg(not(target_os = "macos"))]` y documentar la limitación en
   `docs/arquitectura/ffi.md`.

No usar `libc.so.6` en macOS.

---

## 4. Fallo 3 — Android

### Estado

Se corrigió el commit anterior que intentaba compilar el **CLI raíz** con
`--features gui`. El paquete correcto es `forja-android-rt`, que ya incluye
`forja-gui-rt` y por lo tanto la GUI.

`scripts/build-android.sh` ahora ejecuta:

```bash
cargo build -p forja-android-rt --target "$target" --profile "$profile"
```

Sin `--features gui` en el raíz.

### Pendiente

- Confirmar los 4 targets Android en el CI (`aarch64`, `x86_64`, `armv7`,
  `i686`).
- El job `aarch64` pasó; faltan los otros tres.
- Si falla un target, revisar `toolchain-android.sh`, NDK r27 y
  `CARGO_TARGET_*_LINKER`.

---

## 5. Fallo 4 — `cargo fmt --check` local

`cargo xtask verify -p forja` falla en el primer paso:

```text
Diff in compiler/forja-syntax/src/parser/declaraciones.rs
```

Hay cambios **sin commitear** en el submódulo `compiler/forja-syntax` (firma de
`parsear_declaracion_variable`, `parsear_contratos`, etc.) que no están
formateados.

### Fix

```bash
cd compiler/forja-syntax
cargo fmt
```

Luego revisar que no cambie lógica y commitear **solo si** el cambio es
intencional. Si no lo es, descartarlo:

```bash
git -C compiler/forja-syntax diff
```

---

## 6. Cambios sin commitear detectados

Al cierre de esta sesión, `git status` muestra cambios de otra sesión/agente:

```text
CONVENCIONES.md
Cargo.lock
README.md
compiler/forja-bytecode/Cargo.toml
compiler/forja-driver/Cargo.toml
compiler/forja-driver/src/aot.rs
compiler/forja-driver/src/cache.rs
compiler/forja-errors/Cargo.toml
compiler/forja-fmt/Cargo.toml
compiler/forja-fmt/src/lib.rs
compiler/forja-syntax (submódulo)
...
```

No se commitearon para no mezclar trabajo ajeno. Antes de tocar nada:

```bash
git status --short
git diff
git -C compiler/forja-syntax diff
```

---

## 7. Workflow de actualización automática de submódulos

Se agregó `.github/workflows/submodules.yml`.

- Corre diario a las 04:17 UTC y manualmente.
- `git submodule update --remote --merge --recursive`.
- Commitea y pushea solo `.gitmodules` y punteros de submódulos.
- Evita tener que actualizar punteros a mano.

Verificar que la rama `main` permita push del bot o ajustar permisos.

---

## 8. CI actual: qué jobs ya están bien

Pasaron en el run `42543233847`:

- `Detectar cambios`
- `Elegir runner`
- `Métricas de documentación`
- `Miri`
- `Auditoría RUSTSEC`
- `Formato y Clippy`
- `Check por crate` (syntax, types, ir, bytecode, runtime, native, registry, vm, cli)
- `Matriz de features` (cli, vm sin/con features, native sin features, lsp, wasm, AOT)
- `Tests del workspace`
- `Build y Test (ubuntu-latest)`
- `Build y Test (windows-latest)`
- `Android (aarch64-linux-android)`

Fallan:

- `Cobertura ASM (100%)`
- `Build y Test (macos-latest)`

---

## 9. Orden recomendado para el próximo agente

1. `git status` y decidir qué cambios ajenos conservar.
2. `cargo fmt` en `compiler/forja-syntax`.
3. Agregar test de `compilar_texto_operando` con target Windows y verificar
   cobertura 100 local.
4. Corregir FFI macOS (`libSystem`/`dlopen`) o documentar exclusión.
5. Revisar los 3 Android restantes.
6. Ejecutar:

```bash
cargo test --locked
cargo xtask verify -p forja
PYTHON="C:/Users/gaucho/AppData/Local/Python/pythoncore-3.15-64/python.exe" ASM_COVERAGE_THRESHOLD=100 bash scripts/asm-coverage.sh
```

7. Un solo commit con todas las correcciones; no commits por cada fallo.
8. Push y esperar CI completo.

---

## 10. Archivos clave

| Archivo | Motivo |
|---|---|
| `src/compiler_asm.rs` | cobertura ASM, `compilar_texto_operando`, peephole |
| `scripts/asm-coverage.sh` | umbral 100 y filtro `LCOV_EXCL_LINE` |
| `tests/corpus/lexico_sintaxis_corpus.rs` | FFI macOS |
| `scripts/build-android.sh` | build Android GUI real |
| `.github/workflows/ci.yml` | jobs crates/features/miri/android/cobertura |
| `.github/workflows/submodules.yml` | actualización automática de punteros |
| `compiler/forja-syntax/` | cambios sin formatear |
| `compiler/forja/Cargo.toml` | `doctest = false` |

---

## 11. Comandos útiles

```bash
# Estado
git status --short
git submodule status

# Tests
cargo test --locked
cargo test --locked --test codegen
cargo test --locked -p forja --lib compiler_asm

# Verificación
cargo xtask verify -p forja
cargo check --workspace --locked
cargo doc -p forja --no-deps --locked

# Cobertura
PYTHON="C:/Users/gaucho/AppData/Local/Python/pythoncore-3.15-64/python.exe" \
  ASM_COVERAGE_THRESHOLD=100 bash scripts/asm-coverage.sh

# Android
bash scripts/build-android.sh aarch64-linux-android release
bash scripts/build-android.sh x86_64-linux-android release
bash scripts/build-android.sh armv7-linux-androideabi release
bash scripts/build-android.sh i686-linux-android release
```
