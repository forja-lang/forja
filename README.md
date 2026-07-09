# Forja (fa)

Forja es un lenguaje de programación educativo con palabras clave en español, diseñado para enseñar conceptos de sistemas sin la complejidad sintáctica de Rust. El lenguaje incluye JIT nativo x86-64, múltiples implementaciones de VM y targets de compilación a ensamblador y LLVM IR.

[![Docs](https://img.shields.io/badge/docs-forja--lang.github.io/docs-blue)](https://forja-lang.github.io/docs)
[![VS Code](https://img.shields.io/badge/vscode-extensio%CC%81n-007ACC?logo=visualstudiocode)](https://github.com/forja-lang/vscode)
[![Examples](https://img.shields.io/badge/examples-256%2B-brightgreen)](https://github.com/forja-lang/examples)
[![Benchmarks](https://img.shields.io/badge/benchmarks-resultados-orange)](https://github.com/forja-lang/benchmarks)
[![Patches](https://img.shields.io/badge/patches-xilem%2Fmasonry-lightgrey)](https://github.com/forja-lang/patches)

## Palabras Clave del Lenguaje

### Declaraciones

| Palabra | Alias | Descripción |
|---------|-------|-------------|
| `variable` | `var` | Declaración de variable mutable. Puede reasignarse. |
| `constante` | `const` | Declaración de constante inmutable. No se puede reasignar. |
| `funcion` | `fun` | Definición de función. Crea una unidad ejecutable. |
| `clase` | - | Definición de clase. Crea un tipo con campos y métodos. |
| `constructor` | - | Método inicializador de clase. Se ejecuta automáticamente al instanciar. |
| `tipo` | - | Define un tipo algebraico (enum). |

### Control de Flujo

| Palabra | Alias | Descripción |
|---------|-------|-------------|
| `si` | - | Rama condicional. Ejecuta el bloque si la condición es verdadera. |
| `sino` | - | Rama alternativa para `si`. |
| `mientras` | - | Bucle while. Repite mientras la condición sea verdadera. |
| `para` | - | Bucle for con sintaxis: inicialización; condición; incremento. |
| `repetir` | - | Bucle de repetición fija. Ejecuta exactamente N veces. |
| `retornar` | - | Retorna valor desde función. |
| `coincidir` | - | Expresión de pattern matching. |
| `caso` | - | Rama de patrón dentro de `coincidir`. |
| `otro` | `_` | Patrón comodín/por defecto en `coincidir`. |

### Programación Orientada a Objetos

| Palabra | Descripción |
|---------|-------------|
| `nuevo` | Crea una instancia de clase. Invoca el constructor. |
| `este` | Referencia al objeto actual (equivalente a `self`/`this`). |
| `importar` | Importa declaraciones desde otro archivo de módulo. |

### Tipos y Literales

| Palabra | Descripción |
|---------|-------------|
| `Texto` | Anotación de tipo string. |
| `Entero` | Anotación de tipo entero (i64). |
| `Decimal` | Anotación de tipo flotante (f64). |
| `Booleano` | Anotación de tipo booleano. |
| `Nulo` | Valor literal nulo/ausente. |
| `verdadero` | Literal booleano verdadero. |
| `falso` | Literal booleano falso. |

### Concurrencia

| Palabra | Descripción |
|---------|-------------|
| `hilo` | Crea un nuevo hilo. Devuelve handle del hilo. |
| `canal` | Crea canal de comunicación. Devuelve (emisor, receptor). |
| `enviar` | Envía valor por canal. |
| `recibir` | Recibe valor del canal. |
| `unir` | Une el hilo, esperando su finalización. |
| `seleccionar` | Select sobre múltiples canales (estilo Go). |
| `tiempo` | Cláusula de timeout dentro de `seleccionar`. |

### Rasgos y Genéricos

| Palabra | Descripción |
|---------|-------------|
| `rasgo` | Define un trait (interfaz). |
| `implementa` | Implementa un trait para una clase. |
| `donde` | Restricción de bound de trait en parámetros genéricos. |

### Manejo de Errores

| Palabra | Alternativa | Descripción |
|---------|-------------|-------------|
| `Resultado` | - | Tipo Result<T, E> para manejo de errores. |
| `Ok` | - | Variante de éxito de Result. |
| `Error` | - | Variante de error de Result. |
| `Opcion` | - | Tipo Option<T> para valores opcionales. |
| `Some` | - | Variante Some de Option. |
| `Ninguno` | `Nulo` | Variante None de Option. |

### Atributos

| Atributo | Descripción |
|----------|-------------|
| `@test` | Marca una función como test. |
| `@derive(T)` | Auto-implementa trait T para una clase. |

### Funciones Builtin

| Función | Descripción |
|---------|-------------|
| `escribir(expr)` | Imprime expresión a stdout. |
| `leer()` | Lee línea desde stdin. Devuelve Texto. |

### Operadores

| Operador | Descripción |
|----------|-------------|
| `+` | Suma o concatenación de strings. |
| `-` | Resta. |
| `*` | Multiplicación. |
| `/` | División (entera). |
| `%` | Módulo (resto). |
| `==` | Comparación de igualdad. |
| `!=` | Comparación de desigualdad. |
| `>` | Mayor que. |
| `<` | Menor que. |
| `>=` | Mayor o igual que. |
| `<=` | Menor o igual que. |
| `\|\|` | OR lógico. |
| `&&` | AND lógico. |
| `!` | NOT lógico. |
| `no` | Sintaxis alternativa para NOT lógico. |
| `&` | Crea referencia (presta valor). |

## Comandos de la CLI

Los comandos se ejecutan mediante `cargo run --release --bin forja -- <comando>`. La ejecución directa usa `forja <archivo.fa>` para correr con ForjaFast VM.

### `forja <archivo.fa>`

Ejecuta un archivo Forja directamente en la VM ForjaFast (motor por defecto).

```
forja examples/01_hola.fa
```

### `forja run [OPCIONES] <archivo>`

Ejecuta en la VM o backend especificado.

**Opciones:**
- `--vm <vm>`: Selección de VM: `fast` (ForjaFast, por defecto), `vm` (VM original), `jit` (JIT nativo)
- `--asm`: Compila a ensamblador nativo vía gcc (requiere gcc)
- `--native`: Ejecuta con GUI nativa (requiere `--features gui`)
- `--debug`, `--console`: Mantiene visible la consola
- `--no-debug`: Oculta la consola (subsistema GUI Windows)

```
forja run examples/main.fa                    # ForjaFast por defecto
forja run examples/main.fa --vm vm            # VM original
forja run examples/main.fa --vm jit           # JIT nativo
forja run examples/main.fa --asm              # Ensamblador nativo
forja run examples/gui.fa --native            # GUI nativa
```

### `forja build [OPCIONES] <archivo>`

Genera un ejecutable autónomo con VM y bytecode incrustados, o incrusta el código fuente para ejecución GUI nativa.

**Opciones:**
- `-o <ruta>`: Ruta del ejecutable de salida
- `--no-debug`: Oculta la consola (Windows)
- `--debug`, `--console`: Mantiene visible la consola (modo debug)

```
forja build examples/main.fa -o programa.exe
forja build examples/gui.fa -o app.exe --no-debug
```

### `forja build-asm [OPCIONES] <archivo>`

Compila a ensamblador nativo (x86-64 o ARM64). Genera archivo `.s` y llama a gcc.

**Opciones:**
- `--target <arquitectura>`: Arquitectura objetivo:
  - `x86_64-windows`: Convención de llamadas Windows x64
  - `x86_64-linux`: Convención System V
  - `arm64`: ARM64 AArch64
- `-o <ruta>`: Ruta del ejecutable de salida

```
forja build-asm examples/main.fa                     # Auto-detecta plataforma
forja build-asm examples/main.fa --target arm64      # Target ARM64
forja build-asm examples/main.fa -o programa         # Nombre de salida personalizado
```

### `forja build-llvm [OPCIONES] <archivo>`

Genera LLVM IR para compilación con `llc`.

**Opciones:**
- `-o <ruta>`: Ruta del archivo `.ll` de salida

```
forja build-llvm examples/main.fa -o salida.ll
```

### `forja transpile [OPCIONES] <archivo>`

Transpila código Forja a código Rust equivalente. Crea un proyecto Cargo completo.

**Opciones:**
- `-o <dir>`: Nombre del directorio de salida (por defecto: `<nombre>_rs`)

```
forja transpile examples/main.fa
forja transpile examples/main.fa -o mi_proyecto
```

### `forja test [archivo]`

Ejecuta tests marcados con anotación `@test`. Compila cada test a código nativo vía rustc.

```
forja test examples/test.fa
forja test                              # Ejecuta todos los tests en examples/
```

### `forja bench [OPCIONES] <archivo>`

Mide tiempos de ejecución con medición cold (primera ejecución) y hot (promedio de N iteraciones).

**Opciones:**
- `--iters <n>`: Número de iteraciones para promedio hot (por defecto: 100)
- `--vm <vm>`: VM a medir: `fast`, `vm`, `jit`, o `todas` (por defecto: todas)
- `--asm`: Mide ensamblador nativo en lugar de VMs

```
forja bench examples/main.fa --iters 100
forja bench examples/main.fa --vm fast
forja bench examples/main.fa --asm --iters 10
```

### `forja repl [OPCIONES]`

Inicia modo REPL interactivo con estado persistente entre líneas.

**Opciones:**
- `--vm <vm>`: VM a usar: `fast` (por defecto), `vm`, o `jit`

```
forja repl
forja repl --vm vm
```

### `forja fmt <archivo>`

Formatea código fuente con indentación consistente (4 espacios).

```
forja fmt examples/main.fa
```

### `forja diagram <archivo>`

Genera visualización HTML del AST.

```
forja diagram examples/main.fa
forja diagram examples/main.fa -o diagrama.html
```

### `forja doc [OPCIONES] <archivo>`

Genera documentación HTML desde doc comments (`///`).

**Opciones:**
- `-o <dir>`: Directorio de salida

```
forja doc examples/main.fa -o docs/
```

### `forja highlight <archivo>`

Muestra código fuente con resaltado de sintaxis ANSI en la terminal.

```
forja highlight examples/main.fa
```

### `forja new <nombre>`

Crea un nuevo proyecto con estructura estándar.

```
forja new mi_programa
```

Crea:
- `mi_programa/main.fa`
- `mi_programa/forja.json`
- `mi_programa/modulos/`

### `forja init`

Inicializa un proyecto Forja en el directorio actual.

```
forja init
```

### `forja learn`

Inicia tutorial interactivo.

```
forja learn
```

### `forja explain <palabra>`

Explica una palabra clave o concepto.

```
forja explain variable
forja explain funcion
forja explain rasgo
```

### `forja keywords`

Lista todas las palabras clave del lenguaje.

```
forja keywords
```

## Stack Tecnológico

| Componente | Tecnología |
|------------|------------|
| Lenguaje | Rust (edition 2021) |
| Compilador | Rust puro (sin dependencias externas para núcleo) |
| REPL | rustyline |
| JIT Nativo | Generación de código x86-64 en memoria |
| GUI | Framework UI reactivo xilem (feature opcional) |
| WASM | wasm-bindgen |
| LLVM Backend | Generación de texto LLVM IR (sin bindings a libllvm) |

## Arquitectura

El pipeline de compilación consiste en:

1. **Lexer** (`src/lexer.rs`): Tokeniza texto fuente
2. **Parser** (`src/parser.rs`): Parsing descendente recursivo con precedencia
3. **Type Checker** (`src/semantics.rs`): Valida tipos, ownership, traits y genéricos
4. **Optimizer** (`src/optimizer.rs`): Constant folding, dead code elimination
5. **Múltiples backends**: ForjaFast VM, JIT nativo, ensamblador, LLVM IR

## Implementaciones de VM

| VM | Archivo | Técnica | Rendimiento Relativo |
|----|---------|---------|---------------------|
| ForjaVM Original | src/vm.rs | Stack-based con tagged enums | 1x (línea base) |
| ForjaFast | src/vm_fast.rs | NaN tagging, stack caching, superinstrucciones | ~4.8x más rápido |
| JIT Nativo | src/jit.rs | Generación de código máquina x86-64 | ~62x más rápido |
| Ensamblador Nativo | src/compiler_asm.rs | gcc -O2 | ~437x más rápido |
| LLVM | src/compiler_llvm.rs | llc -O2 | ~500x más rápido |

## Características del Lenguaje

- **Interpolación de strings**: `"Hola ${nombre}, tienes ${edad} años"`
- **Result/Opcion con `?`**: Propagación automática de errores
- **Rasgos e implementaciones**: Polimorfismo basado en interfaces
- **Genéricos**: Polimorfismo paramétrico con sintaxis `<T>`
- **Pattern matching exhaustivo**: Verificación de cobertura en tiempo de compilación
- **Concurrencia**: Hilos, canales, select con timeout
- **JIT nativo**: Generación de código x86-64 sin dependencias externas
- **Múltiples targets de compilación**: Ensamblador, LLVM IR, ejecutables autónomos
- **Playground WASM**: Ejecución en navegador

## 📜 Design by Contract

Forja soporta **Design by Contract** (Diseño por Contrato) con precondiciones, postcondiciones e invariantes de clase.

```forja
funcion dividir(a: Entero, b: Entero) -> Entero
    requiere b != 0, "No se puede dividir por cero"
    asegura resultado <= a
{
    retornar a / b
}
```

### Keywords

| Palabra | Propósito | Ejemplo |
|---------|-----------|---------|
| `requiere` | Precondición | `requiere x > 0, "mensaje"` |
| `asegura` | Postcondición | `asegura resultado > 0` |
| `siempre` | Invariante de clase | `siempre saldo >= 0` |
| `resultado` | Valor de retorno en postcondición | `asegura resultado > 0` |
| `anterior()` | Valor previo a la ejecución | `asegura x == anterior(x) + 1` |

### Modos

- **Debug** (default): Los contratos se verifican en runtime
- **Release**: Los contratos se eliminan (`--release`, `--no-contratos`)

### Ejemplos

Ver [`examples/500_contratos.fa`](examples/500_contratos.fa) para un ejemplo completo de contratos exitosos,
y [`examples/501_contratos_error.fa`](examples/501_contratos_error.fa) para un ejemplo de contratos que fallan.

## 🎨 GUI - Material You Expressive

Forja incluye una librería de componentes UI con diseño **Material Design 3 (Material You)**.
Incluye 200+ componentes responsivos con tema dinámico, iconos vectoriales y modo oscuro.

📚 **[Documentación completa](https://github.com/forja-lang/docs)**
🚀 **[Guía de inicio rápido](https://github.com/forja-lang/docs)**

### Componentes principales
- **Botones**: 14 variantes (Filled, Tonal, Outlined, Text, Elevated, FAB, Icon, Segmented, Chips)
- **Inputs**: TextField, Select, Sliders, Switches, Date/Time Pickers
- **Navegación**: NavigationBar, TopAppBar, Tabs, Drawer, SearchBar
- **Feedback**: Dialogs, BottomSheets, Snackbar, Tooltips, Menús
- **Layout**: Flex, Grid, Flow, Responsive (Compact/Medium/Expanded)
- **Gráficos**: Line, Bar, Pie, Donut, Gauge, Sparkline
- **Expressive**: Glassmorphism, Gradientes, Glow

### Tema
```bash
# Claro (por defecto)
forja-gui ejemplo.fa

# Oscuro
forja-gui --dark ejemplo.fa

# Auto (detecta sistema)
forja-gui --auto-tema ejemplo.fa

# Color personalizado
forja-gui --tema #FF5722 ejemplo.fa
```

## Instalación

```bash
git clone https://github.com/forja-lang/forja.git
cd forja

cargo build --release                 # Binario principal únicamente
cargo build --release --features all  # Todas las features (GUI, LSP)
cargo build --release --features gui  # Soporte GUI
cargo build --release --features lsp  # Soporte LSP
```

## Cross-compilation para Android

Forja compila a Android (ARM64, x86_64, ARM32, x86) con un solo comando. El script detecta el NDK automáticamente e instala los targets de Rust que falten.

```bash
bash scripts/build-android.sh              # Todos los targets (release)
bash scripts/build-android.sh aarch64-linux-android  # Solo ARM64
```

O con `make`:

```bash
make android-all       # Todos los targets
make android-arm64     # Solo ARM64
```

El NDK se busca en `$ANDROID_NDK_HOME`, `$ANDROID_HOME/ndk/`, y las rutas por defecto de cada SO. Si no está instalado, el script muestra cómo hacerlo.

## Licencia

Forja está licenciado bajo la **GNU General Public License v3.0 (GPLv3)** con términos adicionales sobre la marca registrada.

- **Código fuente**: GPLv3 - puedes usar, estudiar, modificar y redistribuir siempre que las modificaciones sigan la misma licencia.
- **Marca "Forja"**: El nombre y logo son marca registrada. Queda prohibido usarlos para promocionar forks o productos derivados sin autorización.
- **Programas creados con Forja**: Pueden usar cualquier licencia que desees. El copyleft de GPL solo se aplica al compilador/intérprete, no a tu código.

Ver [LICENSE.md](LICENSE.md) para términos completos.

[Code of Conduct](CODE_OF_CONDUCT.md) | [Security Policy](SECURITY.md)