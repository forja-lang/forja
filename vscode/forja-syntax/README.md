# Forja Syntax — Extensión de VS Code

Soporte de sintaxis para el lenguaje de programación **Forja** (archivos `.fa`).

## Características

- ✅ Resaltado de sintaxis completo para todas las keywords de Forja
- ✅ Paleta de colores coherente con el CLI (`forja highlight`) y la documentación web
- ✅ Auto-cierre de brackets `{}`, `()`, `[]`
- ✅ Auto-cierre de comillas `""`
- ✅ Indentación automática después de `{`
- ✅ Comentarios de línea con `//`

## Paleta de colores

| Token | Color | CSS/ANSI | Elementos |
|-------|-------|----------|-----------|
| `keyword` | `#f59e0b` 🔶 | `var(--fuego-light)` | `variable`, `si`, `funcion`, `clase`, `coincidir`, etc. |
| `function` | `#10b981` 🟢 | `var(--verde)` | `escribir()`, `leer()` |
| `type` | `#06b6d4` 🔷 | `var(--cian)` | `Entero`, `Decimal`, `Texto`, `Booleano` |
| `string` | `#fbbf24` ⭐ | gold | Texto entre comillas dobles |
| `comment` | `#5b6a84` 🌫️ | `var(--text-muted)` | Líneas con `//` |
| `operator` | `#ec4899` 🩷 | `var(--rosa)` | `+`, `-`, `*`, `/`, `==`, `&&`, `\|\|` |
| `number` | `#818cf8` 💜 | `var(--acero-light)` | Literales numéricos |
| `boolean` | `#f59e0b` 🔶 | `var(--fuego-light)` | `verdadero`, `falso` |

## Instalación

### Desde VSIX (recomendado)

1. En VS Code: `Ctrl+Shift+P` → "Extensions: Install from VSIX..."
2. Seleccioná `vscode/forja-syntax/forja-syntax-0.2.0.vsix`
3. Abrí un archivo `.fa` — la sintaxis se colorea automáticamente

### Desde terminal

```bash
code --install-extension vscode/forja-syntax/forja-syntax-0.2.0.vsix
```

### Manual

1. Copiá la carpeta `forja-syntax` a `~/.vscode/extensions/`
2. Reiniciá VS Code
3. Abrí un archivo `.fa` — la sintaxis se colorea automáticamente

## Archivos incluidos

| Archivo | Propósito |
|---------|-----------|
| `package.json` | Manifiesto de la extensión: nombre, versión, idioma asociado |
| `language-configuration.json` | Configuración de comentarios, brackets, auto-cierre, indentación |
| `syntaxes/forja.tmLanguage.json` | Gramática TextMate completa con todas las reglas de resaltado |

## Keywords soportadas

```
variable, var, constante, const, mut, si, sino, mientras, para, repetir,
funcion, retornar, clase, constructor, nuevo, este, prestado, importar,
coincidir, caso, tipo, verdadero, falso, nulo, arreglo, mapa
```

## Desarrollo

Para modificar la extensión:

1. Editá los archivos en `syntaxes/` o `language-configuration.json`
2. Recargá VS Code: `Ctrl+Shift+P` → "Developer: Reload Window"
3. Opcional: regenerá el VSIX con `vsce package`

## Coherencia de paleta

La paleta de colores de esta extensión es idéntica a la usada en:

- El comando CLI `forja highlight`
- La documentación web del proyecto
- El playground interactivo WASM

Esto asegura una experiencia visual consistente en todos los entornos donde se usa Forja.
