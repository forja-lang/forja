# 🔨 Forja — Referencia Completa de Comandos

## Compilar el compilador

```bash
# Debug
cargo build

# Release (recomendado para benchmarks y uso diario)
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

Siempre usar `cargo run --release --bin forja -- <comando>` para ejecución optimizada.

### `forja <archivo.fa>` — Ejecutar en ForjaFast (default)

Si el primer argumento termina en `.fa`, ejecuta automáticamente en **ForjaFast** 🏆:

```bash
cargo run --release --bin forja -- examples/hola_mundo.fa
cargo run --release --bin forja -- examples/fib.fa
```

### `forja run|ejecutar|correr <archivo> [--vm fast|vm|jit] [--asm]`

Compila y ejecuta en la VM seleccionada. No necesitás Rust.

```bash
# ForjaFast (default) — 🏆 recomendado
cargo run --release --bin forja -- run examples/hola_mundo.fa

# VM Original
cargo run --release --bin forja -- run examples/clases.fa --vm vm

# VM JIT (Direct Threading)
cargo run --release --bin forja -- run examples/funciones.fa --vm jit

# Assembly nativo (⚡ más rápido, requiere gcc)
cargo run --release --bin forja -- run examples/hola_mundo.fa --asm
```

### `forja build|compilar|construir <archivo> -o <salida>` — Ejecutable autónomo

Genera un `.exe` que contiene la VM + bytecode incrustado.

```bash
cargo run --release --bin forja -- build examples/hola_mundo.fa -o hola.exe
# ✅ Ejecutable generado: hola.exe (1234 bytes)
./hola.exe
# → ¡Hola, mundo desde Forja!
```

### `forja build-asm|compilar-asm|asm <archivo> [--target <arch>] [-o <salida>]` — Assembly nativo (⚡ más rápido)

Compila directamente a assembly x86-64 o ARM64 + `gcc -O2`. Velocidad nativa.

```bash
# Mínimo: detecta plataforma actual automáticamente
cargo run --release --bin forja -- build-asm examples/hola_mundo.fa

# Con nombre de salida
cargo run --release --bin forja -- build-asm examples/hola_mundo.fa -o programa.exe

# Especificar arquitectura destino
cargo run --release --bin forja -- build-asm examples/hola_mundo.fa --target arm64 -o programa

# Compilar manualmente el .s generado
cargo run --release --bin forja -- build-asm examples/hola_mundo.fa --target x86_64-linux -o prog
gcc -O2 -o prog prog.s
```

Targets disponibles:

| Flag | Arquitectura | Convención |
|------|-------------|------------|
| *(ninguno)* | Detección automática | Según SO y CPU |
| `--target x86_64-windows` | x86-64 | Microsoft x64 (RCX, RDX, R8, R9) |
| `--target x86_64-linux` | x86-64 | System V (RDI, RSI, RDX, RCX) |
| `--target arm64` | ARM64 AArch64 | X0..X7, stp/ldp, cbz |

### `forja transpile|t|transpilar|transpilador <archivo> [-o <salida>]` — Transpilar a Rust

```bash
cargo run --release --bin forja -- transpile examples/hola_mundo.fa
cargo run --release --bin forja -- t examples/clases.fa -o salida.rs
```

### `forja repl|interactivo [--vm fast|vm|jit]` — Modo interactivo

Intérprete línea por línea. Las variables persisten entre líneas.

```bash
cargo run --release --bin forja -- repl
# 🔨 Forja v0.3.0 — Escribí 'salir' para terminar
# > variable x = 5
# > x = x + 10
# > escribir(x)
# 15
# > salir
# 👋 ¡Hasta luego!
```

Seleccionar VM para el REPL:
```bash
cargo run --release --bin forja -- repl --vm fast  # ForjaFast 🏆 (default)
cargo run --release --bin forja -- repl --vm vm    # VM Original
cargo run --release --bin forja -- repl --vm jit   # VM JIT (Direct Threading)
```

### `forja medir|bench|medicion|benchmark <archivo> [--iters N] [--vm fast|vm|jit|todas] [--asm]`

Mide tiempos de ejecución: cold (primera ejecución) + hot (promedio de N iteraciones).

```bash
# Medir en ForjaFast (default)
cargo run --release --bin forja -- medir examples/hola_mundo.fa --iters 100

# Medir en todas las VMs
cargo run --release --bin forja -- medir benchmarks/speed_comparison.fa --iters 50 --vm todas

# Medir solo en VM Original
cargo run --release --bin forja -- medir examples/fib.fa --iters 100 --vm vm

# Medir en ASM nativo (requiere gcc)
cargo run --release --bin forja -- medir benchmarks/speed_comparison.fa --asm --iters 10
```

### `forja diagrama|grafico|diagram <archivo>` — Generar diagrama HTML

Genera un HTML interactivo con el árbol AST del código:

```bash
cargo run --release --bin forja -- diagrama examples/hola_mundo.fa
# Genera: examples/hola_mundo.html
```

### `forja fmt|formatear|format <archivo>` — Formatear código

Aplica formato consistente al código Forja (indentación 4 espacios):

```bash
cargo run --release --bin forja -- fmt examples/desorden.fa
```

### `forja new|nuevo|crear <nombre>` — Crear nuevo proyecto

```bash
cargo run --release --bin forja -- nuevo mi_programa
# ✅ Proyecto 'mi_programa' creado
# cd mi_programa && forja run main.fa

# Estructura generada:
#   mi_programa/
#     main.fa
#     forja.json
#     modulos/
```

### `forja init|iniciar` — Inicializar proyecto en directorio actual

```bash
cargo run --release --bin forja -- init
# Crea main.fa, forja.json y modulos/ en el directorio actual
```

### `forja learn|aprender` — Tutorial interactivo

```bash
cargo run --release --bin forja -- learn
# 🎓 Forja — Aprendé a programar
# Lección 1: Mostrar mensajes
# ...
```

### `forja explain|explicar <palabra>` — Explicar un concepto

```bash
cargo run --release --bin forja -- explicar escribir
cargo run --release --bin forja -- explain funcion
cargo run --release --bin forja -- explicar clase
```

### `forja keywords|palabras|lista` — Listar palabras clave

```bash
cargo run --release --bin forja -- keywords
# 📚 Palabras clave de Forja
#
#   PALABRA         QUÉ HACE
#   ─────────────── ───────────────────────────────
#   escribir        Muestra mensajes en pantalla
#   variable/var    Declara una variable (mutable)
#   ...
```

### `forja highlight|color|colorear <archivo>` — Colorear código en terminal

```bash
cargo run --release --bin forja -- highlight examples/hola_mundo.fa
# Muestra el código con resaltado de sintaxis ANSI
```

### `forja doc|documentar <archivo>` — Generar documentación desde AST

```bash
cargo run --release --bin forja -- doc examples/clases.fa
```

### `forja help|ayuda|--help|-h [tema]` — Ayuda

```bash
cargo run --release --bin forja -- ayuda
cargo run --release --bin forja -- help si
cargo run --release --bin forja -- --help
```

---

## Ejemplos disponibles

```bash
# Básicos
cargo run --release --bin forja -- run examples/01_hola.fa
cargo run --release --bin forja -- run examples/02_variables.fa
cargo run --release --bin forja -- run examples/03_tipos.fa
cargo run --release --bin forja -- run examples/04_operaciones.fa
cargo run --release --bin forja -- run examples/05_condicionales.fa
cargo run --release --bin forja -- run examples/06_bucles.fa
cargo run --release --bin forja -- run examples/07_funciones.fa
cargo run --release --bin forja -- run examples/08_arrays.fa
cargo run --release --bin forja -- run examples/09_strings.fa

# Intermedios
cargo run --release --bin forja -- run examples/10_clases.fa         # POO completa
cargo run --release --bin forja -- run examples/11_mapas.fa          # Diccionarios
cargo run --release --bin forja -- run examples/12_input.fa          # Entrada de usuario
cargo run --release --bin forja -- run examples/13_errores.fa        # Manejo de errores

# Avanzados
cargo run --release --bin forja -- run examples/14_adivina.fa        # Juego: adivina el número
cargo run --release --bin forja -- run examples/15_calculadora.fa    # Calculadora interactiva

# Conceptos
cargo run --release --bin forja -- run examples/ownership.fa         # Ownership y préstamos
cargo run --release --bin forja -- run examples/poo_simple.fa        # POO sin constructor
cargo run --release --bin forja -- run examples/poo_test.fa          # Tests de POO
```

---

## Benchmarks

### Benchmarks integrados (cargo bench bins)

Los benchmarks se ejecutan como bins independientes con `cargo run --release --bin <nombre>`:

```bash
# JIT Nativo vs ForjaFast vs Rust
cargo run --release --bin bench-jit

# JIT Nativo 100k iteraciones (vs ForjaFast, Python, Rust)
cargo run --release --bin bench-jit-100k

# VM Original vs JIT(DT)
cargo run --release --bin bench-vms

# Todas las VMs (cold + hot, completo)
cargo run --release --bin bench-forjafast

# Rust nativo (baseline con black_box)
cargo run --release --bin bench-rust-native

# ForjaFast vs Python
cargo run --release --bin bench-clean

# Completo: Forja vs Rust vs Python vs Go
cargo run --release --bin bench-completo

# Optimizaciones CPython-style en todas las VMs
cargo run --release --bin bench-cpython-opt

# Forja vs Python (comparativa detallada)
cargo run --release --bin bench-vs-python
```

### Benchmarks con ASM nativo

```bash
# Medir un .fa compilado a ASM nativo (requiere gcc)
cargo run --release --bin forja -- medir benchmarks/leibniz_10m.fa --asm --iters 5
cargo run --release --bin forja -- medir benchmarks/speed_comparison.fa --asm --iters 10
```

---

## Proyectos con módulos

```bash
# Ejecutar proyecto con imports desde el directorio raíz
cargo run --release --bin forja -- run main.fa

# El módulo 'importar "modulos/matematica"' busca:
#   ./modulos/matematica.fa
```

⚠️ **Seguridad:** Las rutas con `..` (path traversal) son rechazadas automáticamente.

---

## Compilar el Rust generado

```bash
# Si usaste transpile, el .rs se compila con rustc
rustc -O examples/hola_mundo.rs
./hola_mundo
```
