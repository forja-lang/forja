<div align="center">

<img src="https://raw.githubusercontent.com/forja-lang/forja-design/main/forge.png" alt="Logo de Forja" width="128">

# Forja (fa)

[![CI](https://img.shields.io/github/actions/workflow/status/forja-lang/forja/ci.yml?style=flat-square&logo=github&label=CI)](https://github.com/forja-lang/forja/actions/workflows/ci.yml)
[![CD](https://img.shields.io/github/actions/workflow/status/forja-lang/forja/cd.yml?style=flat-square&logo=github&label=CD)](https://github.com/forja-lang/forja/actions/workflows/cd.yml)
[![Documentación](https://img.shields.io/badge/docs-forja--lang.github.io-blue?style=flat-square&logo=readthedocs)](https://forja-lang.github.io)

</div>

Forja es un lenguaje de programación educativo moderno con sintaxis y palabras clave completamente en español. Está diseñado para enseñar conceptos de sistemas, teoría de lenguajes de programación y diseño por contrato sin la complejidad sintáctica de Rust. Incluye un compilador nativo (AOT y ensamblador x86-64/ARM64), un compilador al vuelo (JIT *tiered*) para x86-64, un intérprete de bytecode (*ForjaFast*), transpilación a Rust y a WASM, y soporte de interfaces gráficas nativas.

**[Leé la documentación oficial aquí](https://forja-lang.github.io)**

---

## Arquitectura del Compilador e Intérprete

El compilador está partido en **crates por fase**, con dependencias que sólo van hacia abajo
(`tests/frontera_crates.rs` lo verifica). El siguiente diagrama muestra el camino del código
fuente hasta cada entorno de ejecución:

```mermaid
graph LR
    A["Código Fuente (.fa)"] --> B["forja-syntax\ntokens · lexer · parser · AST"]
    B --> C["forja-types\ntipos · préstamos · inferencia"]
    C --> D["forja-hir\nDefId · módulos · desugar"]
    D --> E["forja-ir\nSSA · optimizador · registros"]
    E --> F["forja-bytecode\nopcodes · uops · contratos"]
    F --> G{"Generador de salida"}

    G -->|ForjaFast| H["forja-vm\nintérprete de bytecode"]
    G -->|JIT tiered| I["forja-vm\nJIT x86-64 en caliente"]
    G -->|Ensamblador| K["src/compiler_asm.rs\nx86-64 / ARM64 + gcc"]
    G -->|LLVM IR| L["src/backend_llvm.rs\n.ll + llc"]
    G -->|Transpilador| J["src/transpiler.rs\nRust + rustc"]

    H --> M["forja-runtime + forja-native\nGC · heap · sistema"]
    I --> M
```

> **Entrypoint canónico:** hay una sola ruta de bytecode,
> `compilar_pipeline_completa()` (con imports: `compilar_pipeline_completa_desde()`), que
> delega en el núcleo `compilar_pipeline_nucleo()`; las variantes `…_desde_ir()` y
> `…_desde_ast()` sólo fuerzan el motor de emisión (IR o AST). La transpilación a Rust es
> una ruta independiente que no genera bytecode: `transpilar_a_rust()` y
> `transpilar_a_rust_con_ast()`.
>
> La documentación de arquitectura tiene **una página por crate**
> ([forja-lang.github.io](https://forja-lang.github.io/arquitectura/pipeline/)), no una por
> archivo: el detalle de cada fase vive en el `lib.rs` de su crate.

---

## Sintaxis del Lenguaje por Ejemplo

### 1. Variables, Constantes y Tipos Básicos
```forja
// Las variables son mutables por defecto usando 'variable' o 'var'
variable edad = 18
edad = 19 // Permitido

// Las constantes son inmutables usando 'constante' o 'const'
constante pi = 3.14159
// pi = 3.2 // Error en tiempo de compilación
```

### 2. Control de Flujo (Condicionales y Bucles)
```forja
// Condicional clásico
si (edad >= 18) {
    escribir("Sos mayor de edad")
} sino {
    escribir("Sos menor de edad")
}

// Bucle condicional
variable contador = 0
mientras (contador < 3) {
    escribir("Contador: " + contador)
    contador = contador + 1
}

// Bucle determinado
para (variable i = 0; i < 5; i = i + 1) {
    escribir("Iteración: " + i)
}
```

### 3. Interpolación de Cadenas
Forja interpola expresiones con `${...}`. Las llaves sueltas son caracteres literales:
```forja
variable nombre = "Mundo"
variable edad = 25

// Interpolación con dólar-llave
escribir("Hola ${nombre}, tenés ${edad} años")
escribir("Resultado: ${2 + 3}")

// Las llaves sueltas se imprimen literalmente (útil para JSON/plantillas)
escribir("JSON: {\"clave\": \"valor\"}")
```

### 4. Acceso a Mapas con Sintaxis de Punto
```forja
variable config = {"host": "localhost", "puerto": 8080}

// Acceso con punto (equivale a config["host"])
escribir(config.host)    // → "localhost"
escribir(config.puerto)  // → 8080
```

### 5. Métodos Integrados en Tipos Primitivos
```forja
// Métodos de Texto
variable txt = "hola mundo"
escribir(txt.longitud())       // → 10
escribir(txt.a_mayusculas())   // → "HOLA MUNDO"
escribir(txt.a_minusculas())   // → "hola mundo"
escribir(txt.contiene("mundo"))// → verdadero

// Métodos de Arreglo
variable nums = [1, 2, 3]
nums.empujar(4)
escribir(nums.longitud())      // → 4
```

### 6. Inicialización Struct-Literal
```forja
clase Persona {
    nombre
    edad
}

// Inicialización con llaves (sin constructor manual)
variable p = nuevo Persona { nombre: "Ana", edad: 25 }
escribir(p.nombre) // → "Ana"

// También funciona la forma clásica con constructor
variable q = nuevo Persona("Juan", 30)
```

### 7. Operador Ternario
```forja
variable edad = 20
variable mensaje = edad >= 18 ? "Mayor" : "Menor"
escribir(mensaje) // → "Mayor"
```

### 8. Funciones y Diseño por Contrato (Design by Contract)
Forja implementa el paradigma de diseño por contrato directamente en la firma de las funciones:
```forja
funcion dividir(a: Entero, b: Entero) -> Entero
    requiere b != 0, "No se puede dividir por cero (Precondición)"
    asegura resultado <= a, "El resultado es menor o igual al dividendo (Postcondición)"
{
    retornar a / b
}
```

---

## Consola de Comandos (CLI)

Todos los comandos de la herramienta de consola de Forja aceptan términos y argumentos en español.

### Detección y Configuración Inicial

Para utilizar la herramienta de consola en cualquier directorio, podés configurar un alias o compilar el binario directamente en la carpeta del compilador:

```powershell
# Compilar todas las herramientas en modo optimizado (compilador, depurador, interfaz gráfica y servidor de lenguaje)
PS C:\forja> cargo build --release --features all
```

---

### Referencia de Comandos en Español

| Comando en Consola | Comando Alternativo | Propósito |
| :--- | :--- | :--- |
| **`ejecutar <archivo.fa>`** | `correr` | Compila y ejecuta el script en la máquina virtual *ForjaFast* (por defecto). |
| **`compilar <archivo.fa>`** | `construir` | Genera un ejecutable autónomo nativo. Si usa `importar gui`, usa `forja-rt-gui` (VM + runtime gráfico Xilem). Sin transpilación a Rust. |
| **`interactivo`** | `repl` | Inicia la consola de lectura e interpretación línea por línea. |
| **`probar [archivo.fa]`** | `test` | Corre los módulos de prueba unitaria marcados con `@test`. |
| **`medir [iteraciones] <archivo.fa>`** | `benchmark` | Analiza el rendimiento del código (mediciones en frío y en caliente). |
| **`diagrama <archivo.fa>`** | `grafico` | Genera un diagrama en formato Mermaid (.mmd) con la representación del AST. |
| **`formatear <archivo.fa>`** | `fmt` | Aplica una indentación estándar de 4 espacios al código fuente. |
| **`compilar-asm <archivo.fa>`**| `asm` | Genera código de ensamblador nativo para Windows o Linux. |
| **`transpilar <archivo.fa>`** | `transpilador` | Traduce el código de Forja a un proyecto estructurado en Rust. |
| **`documentar <archivo.fa>`** | `doc` | Lee comentarios documentales (`///`) y exporta páginas de ayuda HTML. |
| **`colorear <archivo.fa>`** | `highlight` | Imprime el código fuente con colores ANSI en la terminal. |
| **`nuevo <nombre>`** | `crear` | Crea un nuevo proyecto con estructura estándar y archivos de configuración. |
| **`iniciar`** | `init` | Inicializa un proyecto de Forja en el directorio actual. |
| **`aprender`** | `learn` | Ejecuta el curso y tutorial interactivo del lenguaje. |
| **`explicar <concepto>`** | `explain` | Busca y describe la sintaxis o palabras clave en el glosario. |
| **`palabras`** | `keywords` | Muestra la lista completa de palabras reservadas del lenguaje. |
| **`ayuda`** | `help` | Imprime el manual general de la consola de comandos. |

---

### Configuración: `forja.yaml` (sin flags)

La CLI **no usa flags** (`--x`). Al arrancar, si el directorio de ejecución no tiene `forja.yaml`, se crea con **todas** las claves y sus valores por defecto. Es un lenguaje en español: los booleanos son `verdadero`/`falso` y las opciones también (`rapido`, `maquina`, `ensamblador`, `nativo`).

```yaml
motor: rapido              # rapido | vm | jit | todas
destino: maquina           # maquina | ensamblador | nativo
estricto: falso            # modo estricto de tipos y ownership
contratos: verdadero       # requiere/asegura/invariantes
pila: falso                # pila de llamadas en errores
matematica_rapida: falso   # aritmética rápida
perfil: ''                 # '' | recoger | usar
recarga: falso             # recarga de módulos (destino: nativo)
permitir_red: ''           # hosts del sandbox de red (coma)
permitir_puertos: ''       # puertos del sandbox de red (coma)
max_archivo_mb: 10         # límite del archivo fuente
arquitectura: ''           # '' | x86_64-windows | x86_64-linux | arm64 | riscv64
consola: falso             # consola visible en el .exe
verificar_formato: falso   # `formatear` sólo verifica
lint_estricto: falso       # `lint`: advertencias = errores
nivel: ''                  # '' | detallado | depuracion | traza
json: falso                # diagnósticos JSON
color: verdadero           # color en la salida
puerto_registry: 8787      # servidor del registry
```

Cambiar el comportamiento = editar ese archivo. Si aparece un flag, la CLI lo avisa y sale con error.

---

### Ejemplos Prácticos de Consola

Aquí se ilustra cómo interactuar con el compilador en un entorno de comandos típico:

#### 1. Ejecutar un Script en la Máquina Virtual
```powershell
# Ejecución estándar (usa la máquina virtual optimizada ForjaFast por defecto)
PS C:\forja> forja ejecutar ejemplos/fundamentos/01_hola.fa

# Ejecutar con el compilador al vuelo (JIT tiered) nativo
# motor: jit  en forja.yaml
PS C:\forja> forja ejecutar ejemplos/fundamentos/01_hola.fa

# (La VM original v1 fue removida en 0.9.0: solo quedan ForjaFast y el JIT.)
```

#### 2. Compilar a un Ejecutable Autónomo Ligero
```powershell
# Compilar script de consola
PS C:\forja> forja compilar ejemplos/fundamentos/05_condicionales.fa condicionales.exe
Ejecutable generado: condicionales.exe

# Ejecutar de forma directa e instantánea sin dependencias
PS C:\forja> .\condicionales.exe
Sos mayor de edad
Buen trabajo!
```

#### 3. Realizar una Medición de Rendimiento (Benchmark)
```powershell
# Comparar el script en los dos motores (ForjaFast y JIT)
# sin número de iteraciones usa 1
PS C:\forja> forja medir 100 ejemplos/algoritmos/40_fibonacci.fa
```

#### 4. Modo Interactivo (Consola de Lectura y Ejecución Directa)
```powershell
PS C:\forja> forja interactivo
Forja v0.10.0 — Escribí 'salir' para terminar
> variable x = 10
> x = x + 5
> escribir(x)
15
> salir
¡Hasta luego!
```

---

## Salidas de Compilación y Motores de Ejecución

Forja genera distintos destinos a partir del mismo programa. Los dos **motores de ejecución** son el intérprete *ForjaFast* y el JIT *tiered*; el resto son backends de traducción:

| Salida / Motor | Tipo | Notas |
| :--- | :--- | :--- |
| **Transpilado a Rust** | Forja → `.rs` + `rustc` | Rendimiento nativo (mismo backend LLVM de Rust) |
| **Ensamblador Nativo** | `.s` x86-64/ARM64 + `gcc -O2` | Multi-arquitectura |
| **Backend LLVM** | LLVM IR + `llc` | En desarrollo |
| **JIT tiered** | Código máquina x86-64 en caliente | Con fallback seguro a la VM |
| **ForjaFast (VM)** | Intérprete de bytecode | Motor por defecto (NaN tagging + GC) |
| **AOT (`forja compilar`)** | Ejecutable con bytecode incrustado | Autónomo, sin dependencias |

---

## Desarrollo Gráfico (GUI)

Forja permite crear complejas aplicaciones de escritorio reactivas e interactivas gracias a la librería de interfaces gráficas (**Xilem**, sobre Masonry/Winit). Las interfaces siguen la guía de estilo de **Material Design 3 (Material You)**, con soporte automático para modo oscuro y claro según el sistema operativo.

Al compilar un programa que usa GUI (`importar gui`), Forja genera un ejecutable autónomo nativo usando el stub `forja-rt-gui` (VM + runtime gráfico), sin necesidad de transpilar a Rust ni tener Cargo instalado.

```powershell
# Ejecutar un script con GUI (modo nativo)
# destino: nativo  en forja.yaml
PS C:\forja> forja ejecutar ejemplos/gui.fa

# Compilar a ejecutable autónomo (autocontenido, sin transpilación)
PS C:\forja> forja compilar ejemplos/gui.fa miapp.exe
```

---

## Compilación Cruzada para Android

El compilador soporta la generación directa de binarios optimizados para dispositivos Android (arquitecturas ARM64, x86_64, ARM32 y x86):

```bash
# Compilar para todos los dispositivos soportados
$ bash scripts/build-android.sh

# Compilar únicamente para arquitectura ARM64 de 64 bits
$ bash scripts/build-android.sh aarch64-linux-android
```

---

## Nativo vs módulos

Forja separa **primitivos nativos** (Rust: sistema operativo, red, crypto, UI,
memoria, rendimiento crítico) de **módulos Forja** (composición pura, API en
español y enseñanza). La regla: la seguridad y el acceso al sistema viven en
nativo; el módulo es la fachada pública y nunca puede saltarse el sandbox.

La frontera nativo/módulo está protegida por [`tests/frontera_crates.rs`](tests/frontera_crates.rs)
(solo dependencias hacia abajo entre crates de `compiler/`). Clasificación
generada (módulo/función → H/M) por `scripts/generar-frontera.py`:
[`frontera-modulos.generated.md`](frontera-modulos.generated.md).
## Estructura del repositorio

Workspace de Cargo con el compilador (`forja`) y sus crates. El repositorio raíz
usa submódulos git para lo que también se consume o publica por separado.

```
compiler/            crates del compilador (todos miembros del workspace)
├── forja/           manifiesto del crate `forja` (el código vive en src/)
├── forja-syntax/    submódulo · tokens, AST, lexer, parser
├── forja-types/     type checker, borrow checker, inferencia
├── forja-errors/    forja-span/   forja-session/   forja-hir/
├── forja-ir/        forja-bytecode/   forja-driver/
├── forja-runtime/   forja-vm/   forja-native/   forja-registry/
└── forja-fmt/       forja-lint/
src/                 código del crate `forja`: API pública, backends, AOT, LSP
├── tools/           forja-cli, forja-repl, forja-lsp, forja-dap
└── bootstrap/       xtask (automatización del repo)
library/             submódulos · runtimes y librería estándar
├── std/ gui/        librería estándar (`library/std`: 47 módulos .fa)
├── rt/ android/     runtimes de ejecución autónoma
└── gui-theme/ gui-core/ gui-rt/ wasm/ wasm-gui/
tests/ benches/ scripts/ packaging/ vscode/ docs/
ejemplos/            submódulo · ejemplos del lenguaje
```

- `library/*`, `compiler/forja-syntax` y `ejemplos` son **submódulos** (repos
  `forja-lang/forja-*`): sus crates los consumen el resto del ecosistema y se
  publican por separado.
- El resto de `compiler/` vive **en el árbol**: sus cambios son atómicos con el
  compilador y no se publican de forma independiente.
- Detalle de clonado, locks y submódulos: [`CONTRIBUTING.md`](CONTRIBUTING.md).

---

## Estado del proyecto

Forja está en **v0.10.0** y en desarrollo activo. El estado verificable de cada
componente —hecho / en progreso / planeado— vive en el
**[roadmap oficial](https://forja-lang.github.io/roadmap/)**.

- **Motores de ejecución (2):** intérprete *ForjaFast* (`compiler/forja-vm/src/vm_fast/`) y JIT *tiered* (`compiler/forja-vm/src/jit_tiered.rs`).
- **Frontend reutilizable:** crates `forja-syntax` y `forja-types` (lexer, parser, AST, tipos).
- **Salidas:** bytecode (VM/JIT), ensamblador x86-64/ARM64, Rust, AOT, WASM, Android y GUI.
- **Ecosistema:** `library/std` (47 módulos), 263 ejemplos, curso de 30 días, LSP + DAP, GC generacional, registro de paquetes local y releases precompilados.
- **En progreso:** IR real (emisión AST→IR), backend LLVM y registry remoto.

---

## Benchmarks

Los benchmarks canónicos viven en `benches/`:

```bash
cargo bench --bench perf-regresion   # puerta de CI (ratios vs baseline)
cargo bench --bench compilar-vm      # criterion: compilación y VM
```

El detalle histórico por versión se retiró junto con el submódulo de benchmarks;
la referencia actual es `benches/README.md`.

---

## Licencia y Uso de Marca

*   **Compilador y Herramientas**: Licenciado bajo la **Licencia Pública General de GNU v3.0 (GPLv3)**.
*   **Tus Programas**: Escribir software en Forja no restringe su distribución. **Tus aplicaciones son de tu propiedad exclusiva** y podés elegir la licencia comercial o de código abierto que prefieras.