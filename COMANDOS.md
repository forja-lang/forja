# Comandos del Compilador Forja (fa)

## 1. Compilar el compilador (generar el binario)

```bash
# Primero asegurate que la instalación de LLVM MinGW terminó
# Luego:
cargo build --target x86_64-pc-windows-gnu

# Para release (más rápido):
cargo build --release --target x86_64-pc-windows-gnu
```

## 2. Ejecutar tests

```bash
# Todos los tests (39 tests: lexer 12, parser 10, semantics 7, transpiler 10)
cargo test --target x86_64-pc-windows-gnu

# Tests de un módulo específico
cargo test --target x86_64-pc-windows-gnu -- lexer
cargo test --target x86_64-pc-windows-gnu -- parser
cargo test --target x86_64-pc-windows-gnu -- semantics
cargo test --target x86_64-pc-windows-gnu -- transpiler
```

## 3. Transpilar un archivo .fa a .rs

```bash
# Compila el compilador primero (cargo build)
# Luego ejecutá:

# Desde el directorio raíz del proyecto:
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/hola_mundo.fa

# Esto genera: examples/hola_mundo.rs

# Con salida personalizada:
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/hola_mundo.fa -o salida.rs

# Con errores en JSON:
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/clases.fa --json-errors
```

## 4. Ejemplos disponibles

```bash
# 1) Hola Mundo básico
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/hola_mundo.fa

# 2) Variables y mutabilidad
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/variables.fa

# 3) Condicionales si/sino
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/condicionales.fa

# 4) Bucles (mientras, para, repetir)
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/bucles.fa

# 5) POO - Clases, métodos, instanciación
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/clases.fa

# 6) Ownership y préstamos
.\target\x86_64-pc-windows-gnu\debug\forja.exe examples/ownership.fa
```

## 5. Compilar el Rust generado

Una vez que `forja` genera un `.rs`, lo compilás con:

```bash
rustc examples/hola_mundo.rs
# o
cargo build --target x86_64-pc-windows-gnu
```
