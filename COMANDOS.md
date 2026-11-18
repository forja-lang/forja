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

### `forja run|ejecutar|correr <archivo>`

Compila y ejecuta con el motor y el destino elegidos en `forja.yaml` (la CLI no usa flags):

| Clave | Valores | Qué hace |
|---|---|---|
| `motor` | `rapido` (default) \| `jit` | Motor de ejecución |
| `destino` | `maquina` (default) \| `ensamblador` \| `nativo` | VM, ASM nativo o GUI/AOT |
| `matematica_rapida` | `verdadero` \| `falso` | Aritmética sin checks (sólo benchmarks) |
| `perfil` | `''` \| `recoger` \| `usar` | PGO |
| `recarga` | `verdadero` \| `falso` | Reinicia la app cuando cambia un `.fa` (nativo y apps GUI) |
| `estado` | ruta a un `.json` \| `''` | Variables del store con las que arranca una app GUI |

```bash
cargo run --release --bin forja -- run ejemplos/fundamentos/01_hola.fa

# Motor JIT: `motor: jit` en forja.yaml
cargo run --release --bin forja -- run ejemplos/fundamentos/07_funciones.fa

# Assembly nativo (requiere gcc): `destino: ensamblador` en forja.yaml
cargo run --release --bin forja -- run ejemplos/fundamentos/01_hola.fa
```

> **Apps GUI (`importar gui`)**: la VM no dibuja ventanas. Comportamiento:
> - `destino: maquina` (default): compila al vuelo a un ejecutable con el fuente
>   incrustado y lo corre con **`forja-rt-gui`** (el runtime que se instala con
>   Forja). No necesita Rust ni Cargo.
>   El programa **no se transpila**: el runtime interpreta el `.fa`.
> - `destino: nativo` (**avanzado, no recomendado**): genera un proyecto Cargo
>   temporal que depende por ruta de `library/gui-rt` y compila con cargo
>   (necesita el **checkout del repo** y Rust; el primer build compila Xilem/wgpu,
>   ~10-15 min, y el programa se transpila a Rust). Sólo para obtener un binario
>   Rust propio o para Android/web.
>
> Si `forja-rt-gui` no está instalado, el mensaje explica cómo obtenerlo.

### `forja build|compilar|construir <archivo> [salida]` — Ejecutable autónomo

Genera un `.exe` que contiene el runtime + el programa incrustado:

- apps de consola: runtime `forja-rt` + **bytecode** de la VM.
- apps GUI (`importar gui`): runtime `forja-rt-gui` + **fuente Forja** (lo evalúa el
  runtime gráfico, sin compilar Xilem por app). Requiere `forja-rt-gui` junto al
  `forja` instalado (`cargo build -p forja-rt --features gui` desde el checkout).

```bash
cargo run --release --bin forja -- compilar ejemplos/fundamentos/01_hola.fa hola.exe
# ✅ ⚡ Ejecutable generado: hola.exe (1234 bytes de bytecode)
./hola.exe
# → ¡Hola, mundo desde Forja!
```

### `forja build-asm|compilar-asm|asm <archivo> [salida]` — Assembly nativo (⚡ más rápido)

Compila directamente a assembly x86-64 o ARM64 + `gcc -O2`. Velocidad nativa.
La arquitectura destino sale de `arquitectura` en `forja.yaml`.

> **Alcance:** resuelve los `importar` y poda lo inalcanzable igual que `forja aot`,
> así que los módulos de la stdlib escritos **sólo en Forja** (por ejemplo
> `matematica.mcd`) funcionan. Las funciones de la stdlib que dependen de una
> **primitiva nativa** (`_env`, `_json_parsear`, `_socket_*`, …) no tienen
> equivalente en este backend: la compilación se rechaza con el nombre de la
> primitiva y la alternativa a usar. Para programas con stdlib completa, usá
> `forja compilar` (ejecutable autónomo con bytecode).

```bash
# Detecta la plataforma actual automáticamente
cargo run --release --bin forja -- asm ejemplos/fundamentos/01_hola.fa

# Con nombre de salida (segundo argumento posicional)
cargo run --release --bin forja -- asm ejemplos/fundamentos/01_hola.fa programa.exe

# Arquitectura destino: `arquitectura: arm64` en forja.yaml
cargo run --release --bin forja -- asm ejemplos/fundamentos/01_hola.fa programa
```

Targets disponibles (clave `arquitectura`):

| Valor | Arquitectura | Convención |
|------|-------------|------------|
| `''` | Detección automática | Según SO y CPU |
| `x86_64-windows` | x86-64 | Microsoft x64 (RCX, RDX, R8, R9) |
| `x86_64-linux` | x86-64 | System V (RDI, RSI, RDX, RCX) |
| `arm64` | ARM64 AArch64 | X0..X7, stp/ldp, cbz |
| `riscv64` | RISC-V 64 (RV64GC) | A0..A7, sd/ld, beqz/bnez, ecall |

### `forja aot|llvm <archivo>` — LLVM IR + ejecutable nativo

Baja el programa a LLVM IR (`runtime.c` + `llc`) o genera el `.ll`. Alcance:
Forja puro **más** las primitivas de archivo, entorno y sistema que implementa
`runtime.c` (mismo contrato que la VM: `-1`/`nulo` en vez de abortar):

- `std/archivo`: leer, escribir, agregar, existe, eliminar, tamaño.
- `std/env`: `existe_variable`, `establecer`, `eliminar`, `obtener_variable`;
  las que parsean (`obtener_entero`, `obtener_booleano`) todavía no.
- `std/sistema`: SO, directorio actual y temporal, hostname, usuario, cpus,
  `tiempo_ms`, `salir`, `variable_entorno`.

Cualquier otra primitiva (red, hilos, GUI, JSON nativo) se rechaza en
compilación con un mensaje que sugiere `forja compilar` o `forja run`.

### `forja transpile|t|transpilar|transpilador <archivo> [directorio]` — Transpilar a Rust

```bash
cargo run --release --bin forja -- transpilar ejemplos/fundamentos/01_hola.fa
cargo run --release --bin forja -- t ejemplos/clases.fa salida_rs
```

> Para apps GUI (`importar gui`) no genera un único `.rs`: crea un **proyecto Cargo**
> (`<nombre>_rs/`) con `src/lib.rs` y las rutas a `forja`/`forja-gui-rt` del
> checkout, y lo compila con cargo. El AST viaja como datos (`fn programa()`),
> igual que en la ruta `destino: nativo`.

### `forja repl|interactivo [archivo.fa]` — Modo interactivo

Intérprete línea por línea. Las variables persisten entre líneas. El motor sale de
`motor` en `forja.yaml`.

```bash
cargo run --release --bin forja -- repl
# Forja — Escribí 'salir' para terminar
# > variable x = 5
# > x = x + 10
# > escribir(x)
# 15
# > /ayuda      (lista de comandos: /motor, /configurar, /variables, /recargar, /salir)
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
#     forja.json    (manifiesto: nombre, versión, módulos, dependencias)
#     forja.yaml    (configuración de la CLI)
#     modulos/
```

### `forja init|iniciar` — Inicializar proyecto en directorio actual

```bash
cargo run --release --bin forja -- init
# Crea main.fa, forja.json, forja.yaml y modulos/ en el directorio actual
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
cargo run --release --bin forja -- medir 100 ejemplos/fundamentos/06_bucles.fa
cargo run --release --bin forja -- medir 10 ejemplos/algoritmos/40_fibonacci.fa
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
