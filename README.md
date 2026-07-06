# Forja (fa)

Forja is an educational programming language with Spanish keywords, designed to teach systems programming concepts without the syntactic complexity of Rust. The language features a native x86-64 JIT, multiple VM implementations, and compilation targets including assembly and LLVM IR.

## Language Keywords

### Declarations

| Keyword | Alias | Description |
|---------|-------|-------------|
| `variable` | `var` | Mutable variable binding. The binding may be reassigned. |
| `constante` | `const` | Immutable constant binding. Cannot be reassigned. |
| `funcion` | `fun` | Function definition. Creates a callable unit. |
| `clase` | - | Class definition. Creates a type with fields and methods. |
| `constructor` | - | Class initializer method. Called automatically on instantiation. |
| `tipo` | - | Defines an algebraic data type (enum). |

### Control Flow

| Keyword | Alias | Description |
|---------|-------|-------------|
| `si` | - | Conditional branch. Executes block if condition is truthy. |
| `sino` | - | Alternative branch for `si`. |
| `mientras` | - | While loop. Repeats while condition is truthy. |
| `para` | - | For loop with initialization; condition; increment syntax. |
| `repetir` | - | Fixed-count loop. Executes exactly N times. |
| `retornar` | - | Return from function with optional value. |
| `coincidir` | - | Pattern matching expression. |
| `caso` | - | Pattern branch within `coincidir`. |
| `otro` | `_` | Wildcard/default pattern in `coincidir`. |

### Object-Oriented Programming

| Keyword | Description |
|---------|-------------|
| `nuevo` | Creates a new instance of a class. Invokes the constructor. |
| `este` | Reference to the current object instance (equivalent to `self` or `this`). |
| `importar` | Imports declarations from another module file. |

### Types and Literals

| Keyword | Description |
|---------|-------------|
| `Texto` | String type annotation. |
| `Entero` | Integer (i64) type annotation. |
| `Decimal` | Floating-point (f64) type annotation. |
| `Booleano` | Boolean type annotation. |
| `Nulo` | Null/missing value literal. |
| `verdadero` | Boolean true literal. |
| `falso` | Boolean false literal. |

### Concurrency

| Keyword | Description |
|---------|-------------|
| `hilo` | Spawns a new thread. Returns a thread handle. |
| `canal` | Creates a channel for thread communication. Returns (sender, receiver). |
| `enviar` | Sends a value on a channel. |
| `recibir` | Receives a value from a channel. |
| `unir` | Joins a thread, waiting for completion. |
| `seleccionar` | Selects over multiple channels (Go-style). |
| `tiempo` | Timeout clause within `seleccionar`. |

### Traits and Generics

| Keyword | Description |
|---------|-------------|
| `rasgo` | Defines a trait (interface). |
| `implementa` | Implements a trait for a class. |
| `donde` | Trait bound constraint on generic parameters. |

### Error Handling

| Keyword | Alternative | Description |
|---------|-------------|-------------|
| `Resultado` | - | Result<T, E> type for error handling. |
| `Ok` | - | Success variant of Result. |
| `Error` | - | Error variant of Result. |
| `Opcion` | - | Option<T> type for optional values. |
| `Some` | - | Some variant of Option. |
| `Ninguno` | `Nulo` | None variant of Option. |

### Attributes

| Attribute | Description |
|-----------|-------------|
| `@test` | Marks a function as a test. |
| `@derive(T)` | Auto-implements trait T for a class. |

### Builtins

| Function | Description |
|----------|-------------|
| `escribir(expr)` | Prints expression to stdout. |
| `leer()` | Reads a line from stdin. Returns Texto. |

### Operators

| Operator | Description |
|----------|-------------|
| `+` | Addition or string concatenation. |
| `-` | Subtraction. |
| `*` | Multiplication. |
| `/` | Division (integer division). |
| `%` | Modulo (remainder). |
| `==` | Equality comparison. |
| `!=` | Inequality comparison. |
| `>` | Greater than. |
| `<` | Less than. |
| `>=` | Greater than or equal. |
| `<=` | Less than or equal. |
| `&&` | Logical AND. |
| `||` | Logical OR. |
| `!` | Logical NOT. |
| `no` | Alternative syntax for logical NOT. |
| `&` | Creates a reference (borrows value). |

## CLI Commands

Commands are invoked via `cargo run --release --bin forja -- <command>`. Direct execution uses `forja <file.fa>` to run with ForjaFast VM.

### `forja <file.fa>`

Execute a Forja source file directly on the ForjaFast VM (default execution engine).

```
forja examples/01_hola.fa
```

### `forja run [OPTIONS] <file>`

Execute on a specified VM or backend.

**Options:**
- `--vm <vm>`: VM selection: `fast` (ForjaFast, default), `vm` (original VM), `jit` (native JIT)
- `--asm`: Compile to native assembly via gcc (requires gcc installed)
- `--native`: Run with native GUI (requires `--features gui`)
- `--debug`, `--console`: Keep console window visible
- `--no-debug`: Hide console window (Windows GUI subsystem)

```
forja run examples/main.fa                    # ForjaFast default
forja run examples/main.fa --vm vm            # Original VM
forja run examples/main.fa --vm jit           # Native JIT
forja run examples/main.fa --asm              # Native assembly
forja run examples/gui.fa --native            # Native GUI
```

### `forja build [OPTIONS] <file>`

Generate a self-contained executable with embedded VM and bytecode, or embed GUI source for native GUI execution.

**Options:**
- `-o, --output <path>`: Output executable path
- `--no-debug`: Hide console window (Windows)
- `--debug`, `--console`: Keep console window visible (debug mode)

```
forja build examples/main.fa -o program.exe
forja build examples/gui.fa -o app.exe --no-debug
```

### `forja build-asm [OPTIONS] <file>`

Compile to native assembly (x86-64 or ARM64). Generates `.s` file and calls gcc.

**Options:**
- `--target <arch>`: Target architecture. Options:
  - `x86_64-windows`: Windows x64 calling convention
  - `x86_64-linux`: System V calling convention
  - `arm64`: ARM64 AArch64
- `-o, --output <path>`: Output executable path

```
forja build-asm examples/main.fa                     # Auto-detect platform
forja build-asm examples/main.fa --target arm64      # ARM64 target
forja build-asm examples/main.fa -o program          # Custom output name
```

### `forja build-llvm [OPTIONS] <file>`

Generate LLVM IR for compilation with `llc`.

**Options:**
- `-o, --output <path>`: Output `.ll` file path

```
forja build-llvm examples/main.fa -o output.ll
```

### `forja transpile [OPTIONS] <file>`

Transpile Forja source to equivalent Rust code. Creates a complete Cargo project.

**Options:**
- `-o, --output <dir>`: Output directory name (default: `<name>_rs`)

```
forja transpile examples/main.fa
forja transpile examples/main.fa -o my_project
```

### `forja test [file]`

Execute tests marked with `@test` annotation. Compiles each test to native code via rustc.

```
forja test examples/test.fa
forja test                              # Run all tests in examples/
```

### `forja bench [OPTIONS] <file>`

Benchmark execution with cold-run and hot-run measurements.

**Options:**
- `--iters <n>`: Number of iterations for hot-run average (default: 100)
- `--vm <vm>`: VM to benchmark: `fast`, `vm`, `jit`, or `todas` (default: todas)
- `--asm`: Benchmark native assembly instead of VMs

```
forja bench examples/main.fa --iters 100
forja bench examples/main.fa --vm fast
forja bench examples/main.fa --asm --iters 10
```

### `forja repl [OPTIONS]`

Start interactive REPL mode with persistent state between lines.

**Options:**
- `--vm <vm>`: VM to use: `fast` (default), `vm`, or `jit`

```
forja repl
forja repl --vm vm
```

### `forja fmt <file>`

Format source code with consistent indentation (4 spaces).

```
forja fmt examples/main.fa
```

### `forja diagram <file>`

Generate HTML visualization of the AST.

```
forja diagram examples/main.fa
forja diagram examples/main.fa -o diagram.html
```

### `forja doc [OPTIONS] <file>`

Generate HTML documentation from doc comments (`///`).

**Options:**
- `-o, --output <dir>`: Output directory

```
forja doc examples/main.fa -o docs/
```

### `forja highlight <file>`

Display source code with ANSI syntax highlighting in terminal.

```
forja highlight examples/main.fa
```

### `forja new <name>`

Create a new project with standard structure.

```
forja new my_project
```

Creates:
- `my_project/main.fa`
- `my_project/forja.json`
- `my_project/modulos/` directory

### `forja init`

Initialize a Forja project in the current directory.

```
forja init
```

### `forja learn`

Start interactive tutorial.

```
forja learn
```

### `forja explain <word>`

Explain a keyword or concept.

```
forja explain variable
forja explain funcion
forja explain rasgo
```

### `forja keywords`

List all language keywords with brief descriptions.

```
forja keywords
```

## Technology Stack

| Component | Technology |
|-----------|------------|
| Language | Rust (edition 2021) |
| Compiler | Pure Rust (no external dependencies for core) |
| REPL | rustyline |
| JIT Engine | x86-64 machine code generation in memory |
| GUI | xilem reactive UI framework (optional feature) |
| WASM | wasm-bindgen |
| LLVM Backend | LLVM IR text generation (no libllvm bindings) |

## Architecture

The compiler pipeline consists of:

1. **Lexer** (`src/lexer.rs`): Tokenizes source text
2. **Parser** (`src/parser.rs`): Recursive descent with precedence parsing
3. **Type Checker** (`src/semantics.rs`): Validates types, ownership, traits, and generics
4. **Optimizer** (`src/optimizer.rs`): Constant folding, dead code elimination
5. **Multiple backends**: ForjaFast VM, native JIT, assembly, LLVM IR

## VM Implementations

| VM | File | Technique | Relative Performance |
|----|------|-----------|---------------------|
| ForjaVM Original | src/vm.rs | Stack-based with tagged enums | 1x (baseline) |
| ForjaFast | src/vm_fast.rs | NaN tagging, stack caching, superinstructions | ~4.8x faster |
| JIT Native | src/jit.rs | x86-64 machine code generation | ~62x faster |
| Native ASM | src/compiler_asm.rs | gcc -O2 | ~437x faster |
| LLVM | src/compiler_llvm.rs | llc -O2 | ~500x faster |

## Language Features

- **String interpolation**: `"Hola ${nombre}, tienes ${edad} años"`
- **Result/Option with `?`**: Automatic error propagation
- **Traits and implementations**: Interface-based polymorphism
- **Generics**: Parametric polymorphism with `<T>` syntax
- **Exhaustive pattern matching**: Compile-time coverage verification
- **Concurrency**: Threads, channels, select with timeout
- **Native JIT**: x86-64 code generation without external dependencies
- **Multiple compilation targets**: Assembly, LLVM IR, self-contained executables
- **WASM playground**: Browser-based execution

## Installation

```bash
git clone https://github.com/lococoi/forja.git
cd forja

cargo build --release                 # Main binary only
cargo build --release --features all  # All features (GUI, LSP)
cargo build --release --features gui  # GUI support
cargo build --release --features lsp  # LSP support
```

## License

Source-Available License. See LICENSE.md for terms.