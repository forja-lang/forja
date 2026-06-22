# 🔨 Forja — Referencia Completa de Comandos

## Compilar el compilador

```bash
# Debug
cargo build

# Release (recomendado)
cargo build --release

# En Windows con LLVM MinGW
cargo build --target x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

## Ejecutar tests

```bash
# Todos los tests
cargo test

# Tests por módulo
cargo test -- lexer
cargo test -- parser
cargo test -- semantics
cargo test -- transpiler
cargo test -- bytecode
cargo test -- vm
```

---

## Comandos del CLI

### `forja <archivo.fa>` — Transpilar a Rust (default)

Si el primer argumento termina en `.fa`, transpila automáticamente:

```bash
# Genera <nombre>.rs automáticamente
forja examples/hola_mundo.fa

# Con salida personalizada
forja transpile examples/hola_mundo.fa -o salida.rs

# Errores en JSON (ideal para IDEs)
forja transpile examples/complejo.fa --json-errors
```

### `forja ejecutar|run|correr <archivo>` — Ejecutar en VM

Compila y ejecuta el bytecode en la Máquina Virtual. No necesitás Rust.

```bash
forja run examples/hola_mundo.fa
forja ejecutar examples/clases.fa
forja correr examples/funciones.fa
```

### `forja compilar|build|construir <archivo> -o <salida>` — Ejecutable autónomo

Genera un `.exe` que contiene la VM + bytecode incrustado.

```bash
forja build examples/hola_mundo.fa -o hola.exe
# ✅ Ejecutable generado: hola.exe (1234 bytes)
./hola.exe
# → ¡Hola, mundo desde Forja!
```

### `forja compilar-asm|build-asm|asm <archivo> [--target <arch>] [-o <salida>]` — Assembly nativo (⚡ más rápido)

Compila directamente a assembly x86-64 o ARM64 + `gcc -O2`. Velocidad nativa.

```bash
# Mínimo: detecta plataforma actual automáticamente
forja build-asm examples/hola_mundo.fa

# Con nombre de salida
forja build-asm examples/hola_mundo.fa -o programa.exe

# Especificar arquitectura destino
forja build-asm examples/hola_mundo.fa --target arm64 -o programa

# Compilar manualmente el .s generado
forja build-asm examples/hola_mundo.fa --target x86_64-linux -o prog
gcc -O2 -o prog prog.s
```

Targets disponibles:

| Flag | Arquitectura | Convención |
|------|-------------|------------|
| *(ninguno)* | Detección automática | Según SO y CPU |
| `--target x86_64-windows` | x86-64 | Microsoft x64 (RCX, RDX, R8, R9) |
| `--target x86_64-linux` | x86-64 | System V (RDI, RSI, RDX, RCX) |
| `--target arm64` | ARM64 AArch64 | X0..X7, stp/ldp, cbz |

### `forja transpile|t|transpilar|transpilador <archivo> [-o <salida>]` — Transpilar a Rust explícitamente

```bash
forja transpile examples/hola_mundo.fa
forja t examples/clases.fa -o salida.rs
```

### `forja repl` — Modo interactivo

Intérprete línea por línea. Las variables persisten entre líneas.

```bash
forja repl
# 🔨 Forja v0.2.0 — Escribí 'salir' para terminar
# > variable x = 5
# > x = x + 10
# > escribir(x)
# 15
# > salir
# 👋 ¡Hasta luego!
```

### `forja diagram|grafico|diagram <archivo>` — Generar diagram HTML

Genera un HTML interactivo con el árbol AST del código:

```bash
forja diagram examples/hola_mundo.fa
# Genera: examples/hola_mundo.html
```

### `forja formatear|fmt|format <archivo>` — Formatear código

Aplica formato consistente al código Forja (indentación 4 espacios):

```bash
forja fmt examples/desorden.fa
```

### `forja nuevo|new|crear <nombre>` — Crear nuevo proyecto

```bash
forja nuevo mi_programa
# ✅ Proyecto 'mi_programa' creado
# cd mi_programa && forja run main.fa

# Estructura generada:
#   mi_programa/
#     main.fa
#     forja.json
#     modulos/
```

### `forja iniciar|init` — Inicializar proyecto en directorio actual

```bash
forja init
# Crea main.fa, forja.json y modulos/ en el directorio actual
```

### `forja aprender|learn` — Tutorial interactivo

```bash
forja learn
# 🎓 Forja — Aprendé a programar
# Lección 1: Mostrar mensajes
# ...
```

### `forja explicar|explain <palabra>` — Explicar un concepto

```bash
forja explicar escribir
forja explain funcion
forja explicar clase
```

### `forja palabras|keywords|lista` — Listar palabras clave

```bash
forja keywords
# 📚 Palabras clave de Forja
#
#   PALABRA         QUÉ HACE
#   ─────────────── ───────────────────────────────
#   escribir        Muestra mensajes en pantalla
#   variable/var    Declara una variable (mutable)
#   ...
```

### `forja colorear|highlight|color <archivo>` — Colorear código en terminal

```bash
forja highlight examples/hola_mundo.fa
# Muestra el código con resaltado de sintaxis ANSI
```

### `forja documentar|doc <archivo>` — Generar documentación desde AST

```bash
forja doc examples/clases.fa
```

### `forja ayuda|help|--help|-h [tema]` — Ayuda

```bash
forja ayuda
forja help si
forja --help
```

---

## Ejemplos disponibles

```bash
# Básicos
forja run examples/01_hola.fa
forja run examples/02_variables.fa
forja run examples/03_tipos.fa
forja run examples/04_operaciones.fa
forja run examples/05_condicionales.fa
forja run examples/06_bucles.fa
forja run examples/07_funciones.fa
forja run examples/08_arrays.fa
forja run examples/09_strings.fa

# Intermedios
forja run examples/10_clases.fa         # POO completa
forja run examples/11_mapas.fa          # Diccionarios
forja run examples/12_input.fa          # Entrada de usuario
forja run examples/13_errores.fa        # Manejo de errores

# Avanzados
forja run examples/14_adivina.fa        # Juego: adivina el número
forja run examples/15_calculadora.fa    # Calculadora interactiva

# Conceptos
forja run examples/ownership.fa         # Ownership y préstamos
forja run examples/poo_simple.fa        # POO sin constructor
forja run examples/poo_test.fa          # Tests de POO
```

---

## Proyectos con módulos

```bash
# Ejecutar proyecto con imports desde el directorio raíz
forja run main.fa

# El módulo 'importar "modulos/matematica"' busca:
#   ./modulos/matematica.fa
```

⚠️ **Seguridad:** Las rutas con `..` (path traversal) son rechazadas automáticamente.

---

## Compilar el Rust generado

```bash
# Si usaste transpile, el .rs se compila con rustc
rustc examples/hola_mundo.rs
./hola_mundo
```

---

## Benchmarks

```bash
cargo run --release --bin bench-fa           # Benchmark Forja vs Rust
cargo run --release --bin bench-vm           # Benchmark VM
cargo run --release --bin bench-compare      # Forja vs Python
cargo run --release --bin bench-vms          # Comparar las 4 VMs
cargo run --release --bin bench-jit          # Benchmark JIT
cargo run --release --bin bench-jit-100k     # JIT con 100k iteraciones
cargo run --release --bin bench-completo     # Benchmark completo
cargo run --release --bin bench-cpython-opt  # Forja optimizado vs CPython
```
