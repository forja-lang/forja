# Forja — Referencia Completa de Comandos

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
cargo run --release --bin forja -- ejemplos/fundamentos/01_hola.fa
cargo run --release --bin forja -- ejemplos/algoritmos/40_fibonacci.fa
```

### `forja run|ejecutar|correr <archivo> [--vm fast|vm|jit] [--asm] [--native] [--fast-math]`

Compila y ejecuta en la VM seleccionada. No necesitás Rust.

```bash
# ForjaFast (default) — 🏆 recomendado
cargo run --release --bin forja -- run ejemplos/fundamentos/01_hola.fa

# VM Original
cargo run --release --bin forja -- run ejemplos/fundamentos/10_clases.fa --vm vm

# VM JIT (Direct Threading)
cargo run --release --bin forja -- run ejemplos/fundamentos/07_funciones.fa --vm jit

# Assembly nativo (⚡ más rápido, requiere gcc)
cargo run --release --bin forja -- run ejemplos/fundamentos/01_hola.fa --asm

# Fast-Math: omite verificaciones aritméticas (solo con --vm fast)
# - En programas 100% Decimal se activa SOLO automáticamente (sin flag):
#   omite el check de división por cero float (x / 0.0 → inf en vez de nulo)
# - Con --fast-math se fuerza además sin checks de tipo (modo unsafe,
#   puede producir valores indefinidos en código con tipos mixtos)
cargo run --release --bin forja -- run ejemplos/algoritmos/40_fibonacci.fa --fast-math
```

> **Apps GUI (`importar "gui"`) con `--native`**: generan un proyecto Cargo temporal que
> depende por ruta de `library/gui-rt`, así que necesitan el **checkout del repo** y
> Rust instalado (el primer build compila todo Xilem/wgpu en release y tarda varios
> minutos). La app se abre sola al terminar.

### `forja build|compilar|construir <archivo> -o <salida>` — Ejecutable autónomo

Genera un `.exe` que contiene la VM + bytecode incrustado.

```bash
cargo run --release --bin forja -- build ejemplos/fundamentos/01_hola.fa -o hola.exe
# ✅ Ejecutable generado: hola.exe (1234 bytes)
./hola.exe
# → ¡Hola, mundo desde Forja!
```

### `forja build-asm|compilar-asm|asm <archivo> [--target <arch>] [-o <salida>]` — Assembly nativo (⚡ más rápido)

Compila directamente a assembly x86-64 o ARM64 + `gcc -O2`. Velocidad nativa.

```bash
# Mínimo: detecta plataforma actual automáticamente
cargo run --release --bin forja -- build-asm ejemplos/fundamentos/01_hola.fa

# Con nombre de salida
cargo run --release --bin forja -- build-asm ejemplos/fundamentos/01_hola.fa -o programa.exe

# Especificar arquitectura destino
cargo run --release --bin forja -- build-asm ejemplos/fundamentos/01_hola.fa --target arm64 -o programa

# Compilar manualmente el .s generado
cargo run --release --bin forja -- build-asm ejemplos/fundamentos/01_hola.fa --target x86_64-linux -o prog
gcc -O2 -o prog prog.s

# Target cruzado: el .s queda en disco y lo ensambla el toolchain cruzado
cargo run --release --bin forja -- build-asm ejemplos/fundamentos/01_hola.fa --target riscv64 -o prog
riscv64-linux-gnu-gcc -O2 -o prog prog.s
```

Targets disponibles:

| Flag | Arquitectura | Convención |
|------|-------------|------------|
| *(ninguno)* | Detección automática | Según SO y CPU |
| `--target x86_64-windows` | x86-64 | Microsoft x64 (RCX, RDX, R8, R9) |
| `--target x86_64-linux` | x86-64 | System V (RDI, RSI, RDX, RCX) |
| `--target arm64` | ARM64 AArch64 | X0..X7, stp/ldp, cbz |
| `--target riscv64` | RISC-V 64 (RV64GC) | A0..A7, sd/ld, beqz/bnez, ecall |

### `forja transpile|t|transpilar|transpilador <archivo> [-o <salida>]` — Transpilar a Rust

```bash
cargo run --release --bin forja -- transpile ejemplos/fundamentos/01_hola.fa
cargo run --release --bin forja -- t ejemplos/clases.fa -o salida.rs
```

> Para apps GUI (`importar "gui"`) no genera un único `.rs`: crea un **proyecto Cargo**
> (`<nombre>_rs/`) con `src/lib.rs` + `src/programa.json` y las rutas a
> `forja`/`forja-gui-rt` del checkout, y lo compila con cargo. El AST viaja como datos
> (`fn programa()`), igual que en la ruta `--native`.

### `forja repl|interactivo [--vm fast|vm|jit]` — Modo interactivo

Intérprete línea por línea. Las variables persisten entre líneas.

```bash
cargo run --release --bin forja -- repl
# Forja — Escribí 'salir' para terminar
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

### `forja medir|bench|medicion|benchmark [iteraciones] <archivo.fa>`

Mide tiempos de ejecución: cold (primera ejecución) + hot (promedio de N iteraciones).
El primer argumento numérico son las iteraciones (por defecto 1). Sin flags: el motor
(`motor`), el backend ASM (`destino: ensamblador`) y `matematica_rapida` salen de `forja.yaml`.

```bash
# Medir en el motor configurado, 100 iteraciones
cargo run --release --bin forja -- medir 100 ejemplos/fundamentos/01_hola.fa

# Una sola iteración
cargo run --release --bin forja -- medir ejemplos/fundamentos/01_hola.fa

# Todas las VMs: poner `motor: todas` en forja.yaml
cargo run --release --bin forja -- medir 50 ejemplos/fundamentos/06_bucles.fa

# ASM nativo (requiere gcc): `destino: ensamblador` en forja.yaml
cargo run --release --bin forja -- medir 10 ejemplos/fundamentos/06_bucles.fa

# ForjaFast con fast-math: `matematica_rapida: verdadero` en forja.yaml
cargo run --release --bin forja -- medir 50 ejemplos/algoritmos/40_fibonacci.fa
```

### `forja diagrama|grafico|diagram <archivo>` — Generar diagrama HTML

Genera un HTML interactivo con el árbol AST del código:

```bash
cargo run --release --bin forja -- diagrama ejemplos/fundamentos/01_hola.fa
# Genera: ejemplos/hola_mundo.html
```

### `forja fmt|formatear|format <archivo>` — Formatear código

Aplica formato consistente al código Forja (indentación 4 espacios):

```bash
cargo run --release --bin forja -- fmt ejemplos/fundamentos/01_hola.fa
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
cargo run --release --bin forja -- highlight ejemplos/fundamentos/01_hola.fa
# Muestra el código con resaltado de sintaxis ANSI
```

### `forja doc|documentar <archivo>` — Generar documentación desde AST

```bash
cargo run --release --bin forja -- doc ejemplos/fundamentos/10_clases.fa
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
cargo run --release --bin forja -- run ejemplos/fundamentos/01_hola.fa
cargo run --release --bin forja -- run ejemplos/fundamentos/02_variables.fa
cargo run --release --bin forja -- run ejemplos/fundamentos/03_tipos.fa
cargo run --release --bin forja -- run ejemplos/fundamentos/04_operaciones.fa
cargo run --release --bin forja -- run ejemplos/fundamentos/05_condicionales.fa
cargo run --release --bin forja -- run ejemplos/fundamentos/06_bucles.fa
cargo run --release --bin forja -- run ejemplos/fundamentos/07_funciones.fa
cargo run --release --bin forja -- run ejemplos/fundamentos/08_arrays.fa
cargo run --release --bin forja -- run ejemplos/fundamentos/09_strings.fa

# Intermedios
cargo run --release --bin forja -- run ejemplos/fundamentos/10_clases.fa         # POO completa
cargo run --release --bin forja -- run ejemplos/fundamentos/11_mapas.fa          # Diccionarios
cargo run --release --bin forja -- run ejemplos/fundamentos/12_input.fa          # Entrada de usuario
cargo run --release --bin forja -- run ejemplos/fundamentos/13_errores.fa        # Manejo de errores

# Avanzados
cargo run --release --bin forja -- run ejemplos/fundamentos/14_adivina.fa        # Juego: adivina el número
cargo run --release --bin forja -- run ejemplos/fundamentos/15_calculadora.fa    # Calculadora interactiva

# Conceptos
cargo run --release --bin forja -- run ejemplos/ownership/181_ownership.fa         # Ownership y préstamos
```

---

## Benchmarks

Los benchmarks propios están en `benches/` y se ejecutan con Cargo:

```bash
# Puerta de rendimiento (ratios normalizados contra benches/baseline.json)
cargo bench --bench perf-regresion

# Tiempos absolutos de compilación y VM (criterion)
cargo bench --bench compilar-vm
```

Para medir un programa concreto:

```bash
cargo run --release --bin forja -- medir ejemplos/fundamentos/06_bucles.fa --iters 100
cargo run --release --bin forja -- medir ejemplos/algoritmos/40_fibonacci.fa --asm --iters 10
```

## Proyectos con módulos

```bash
# Ejecutar proyecto con imports desde el directorio raíz
cargo run --release --bin forja -- run main.fa

# El módulo 'importar modulos/matematica' busca:
#   ./modulos/matematica.fa
```

⚠️ **Seguridad:** Las rutas con `..` (path traversal) son rechazadas automáticamente.

---

## Compilar el Rust generado

```bash
# Si usaste transpile, el .rs se compila con rustc
rustc -O ejemplos/hola_mundo.rs
./hola_mundo
```
