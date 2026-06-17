# Forja Syntax — Extensión de VS Code

Soporte de sintaxis para el lenguaje de programación **Forja** (`.fa`).

## Paleta de colores

| Token | Color | CSS/ANSI |
|-------|-------|----------|
| `keyword` | `#f59e0b` 🔶 | `var(--fuego-light)` |
| `function` | `#10b981` 🟢 | `var(--verde)` |
| `type` | `#06b6d4` 🔷 | `var(--cian)` |
| `string` | `#fbbf24` ⭐ | gold |
| `comment` | `#5b6a84` 🌫️ | `var(--text-muted)` |
| `operator` | `#ec4899` 🩷 | `var(--rosa)` |
| `number` | `#818cf8` 💜 | `var(--acero-light)` |

## Instalación

1. Copiá la carpeta `forja-syntax` a `~/.vscode/extensions/`
2. Reiniciá VS Code
3. Abrí un archivo `.fa` — la sintaxis se colorea automáticamente

## Archivos incluidos

- `package.json` — Manifiesto de la extensión
- `language-configuration.json` — Configuración de comentarios, brackets, indentación
- `syntaxes/forja.tmLanguage.json` — Gramática TextMate completa
