# Reorganización en Repositorios Separados

Este documento describe cómo dividir el monorepo actual de Forja en repositorios independientes dentro de una organización de GitHub (ej. `forja-lang`).

## Estado Actual

El monorepo contiene todo en un solo repositorio:

```
forja/
├── src/                    # Compilador, VM, JIT, LSP, GUI binary
├── crates/
│   ├── forja-gui-rt/       # Precompilación de xilem para GUI
│   └── forja-wasm/         # Compilador compilado a WASM
├── stdlib/                 # Standard library (.fa)
├── docs/                   # Sitio de documentación (Astro)
├── examples/               # 256 ejemplos .fa
├── tests/                  # Tests de integración
├── benchmarks/             # Benchmarks
├── patches/                # Forks de dependencias (xilem, masonry_winit)
├── vscode/forja-syntax/    # Extensión VS Code
├── design/                 # Assets gráficos (logo, icono)
├── scripts/                # Scripts de build
├── plans/                  # Documentos de diseño/roadmap
└── .github/workflows/      # CI/CD
```

## Separación Propuesta

### 1. `forja-lang/forja` — Núcleo del lenguaje

**Contenido:** Todo lo necesario para compilar y ejecutar Forja.

```
forja/
├── src/                    # Compilador, VMs, JIT, LSP
├── crates/
│   ├── forja-gui-rt/       # GUI runtime (dependencia interna)
│   └── forja-wasm/         # Target WASM
├── stdlib/                 # Librería estándar
├── examples/               # Ejemplos (o podrían ir a su propio repo)
├── tests/                  # Tests de integración
├── benchmarks/             # Benchmarks
├── patches/                # Parches a dependencias
├── Cargo.toml              # Workspace raíz
├── build.rs
└── .github/workflows/      # CI del core
```

**Dependencias:** Ninguna externa (las patches son locales al repo).
**CI:** Build, test, release en todas las plataformas.
**Candidato a separar después:** `examples/` y `benchmarks/` si crecen demasiado.

### 2. `forja-lang/docs` — Documentación

**Contenido:** Sitio de documentación en Astro.

```
docs/
├── astro.config.mjs
├── package.json
├── src/
│   ├── pages/              # Páginas del sitio
│   ├── components/         # Componentes Astro
│   ├── layouts/            # Layouts
│   └── wasm/               # Playground WASM
├── public/
├── dist/
├── docs/
│   ├── gui/                # Docs de la GUI
│   └── stdlib/             # Docs de la stdlib
└── .github/workflows/      # Deploy a GH Pages
```

**Dependencias:** El WASM se copia desde el release del repo `forja`.
**CI:** Build del sitio + deploy a GitHub Pages.
**Acción:** El playground WASM necesita obtener `forja-wasm` desde el CI del repo core (ej. vía artifact descargado o submodulo).

### 3. `forja-lang/vscode` — Extensión VS Code

**Contenido:** Extensión de syntax highlighting + LSP client.

```
vscode/
├── src/extension.ts
├── syntaxes/forja.tmLanguage.json
├── language-configuration.json
├── package.json
├── tsconfig.json
├── out/
└── .github/workflows/      # Build + publish al marketplace
```

**Dependencias:** `forja-lsp` (el binario LSP se distribuye aparte o se referencia).
**CI:** Empaquetado `.vsix` + publicación en VS Code Marketplace.
**Acción:** Publicar el binario `forja-lsp` como release en el repo `forja` y descargarlo en el CI de la extensión.

### 4. (Opcional) `forja-lang/examples` — Ejemplos

Si los 256+ ejemplos crecen y tienen su propio ritmo de actualización.

```
examples/
├── 01_hola.fa
├── 02_variables.fa
├── ...
└── README.md
```

**Dependencias:** Ninguna.
**CI:** Tests de ejemplo (ejecutar `forja test` contra cada ejemplo).

### 5. (Opcional) `forja-lang/patches` — Parches upstream

Si quieres mantener los forks de xilem/masonry_winit como un proyecto separado.

```
patches/
├── xilem/
├── masonry_winit/
└── README.md
```

**CI:** Build de prueba de cada crate.
**Acción:** Referenciar via `[patch.crates-io]` usando git = `https://github.com/forja-lang/patches`.

---

## Plan de Migración

### Paso 1: Preparar splits con `git filter-repo`

```bash
# Split del core
git filter-repo --path src/ --path crates/ --path stdlib/ \
  --path Cargo.toml --path build.rs --path .github/ \
  --path LICENSE.md --path NOTICE --path README.md \
  --target forja

# Split de docs
git filter-repo --path docs/ --path .github/ \
  --target docs

# Split de vscode
git filter-repo --path vscode/forja-syntax/ \
  --target vscode
```

### Paso 2: Crear repos en la org

```bash
gh repo create forja-lang/forja --private
gh repo create forja-lang/docs --private
gh repo create forja-lang/vscode --private
gh repo create forja-lang/examples --private
gh repo create forja-lang/patches --private
```

### Paso 3: Pushear cada split

```bash
cd forja && git remote add origin gh:forja-lang/forja && git push -u origin main
cd docs && git remote add origin gh:forja-lang/docs && git push -u origin main
cd vscode && git remote add origin gh:forja-lang/vscode && git push -u origin main
# Opcionales:
cd examples && git remote add origin gh:forja-lang/examples && git push -u origin main
cd patches && git remote add origin gh:forja-lang/patches && git push -u origin main
```

### Paso 4: Ajustar CI/CD

- **forja core:** El CI actual funciona igual.
- **docs:** Agregar workflow que despliegue a GitHub Pages. El playground WASM debe descargar `forja-wasm` desde el CI del core (ej. via `actions/download-artifact` cruzando repos).
- **vscode:** Build + publish a marketplace. El binario `forja-lsp` debe descargarse desde los releases del core.

### Paso 5: Ajustar dependencias cruzadas

El `Cargo.toml` actual del workspace referencia `crates/forja-gui-rt` y `crates/forja-wasm` por path local. Al separar, el `forja-wasm` depende del core via:

```toml
forja = { git = "https://github.com/forja-lang/forja" }
```

Las `patches` pueden seguir siendo locales o apuntar a los forks separados:

```toml
[patch.crates-io]
xilem = { git = "https://github.com/forja-lang/patches", package = "xilem" }
```

## Resumen de Repos

| Repositorio | Contenido | Prioridad |
|---|---|---|
| `forja-lang/forja` | Core, stdlib, VMs, JIT, CLI, LSP, GUI | Alta (obligatorio) |
| `forja-lang/docs` | Sitio de documentación Astro | Alta |
| `forja-lang/vscode` | Extensión VS Code | Media |
| `forja-lang/examples` | Ejemplos del lenguaje | Baja (opcional) |
| `forja-lang/patches` | Forks de xilem/masonry_winit | Baja (opcional) |

## Consideraciones

- **Historial git:** Usar `git filter-repo` preserva el historial completo de los archivos migrados a cada repo.
- **Issues/PRs:** Al separar, centralizar issues en el repo core con labels para identificar el área (docs, vscode, etc.), o usar issues por repo.
- **Compatibilidad:** El core debe mantener la API pública (`lib.rs`) estable para que `forja-wasm` y `forja-gui-rt` sigan compilando.
- **README:** Cada repo debe tener su propio README específico y apuntar a los otros repos de la org.
- **Release tagging:** Coordinar versiones entre repos (ej. tag `v0.7.0` en core y docs apunta al mismo release).
