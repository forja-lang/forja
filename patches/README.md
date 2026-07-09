# Forja Patches

Forks modificados de dependencias upstream usadas por el compilador **Forja**.

## Parches incluidos

| Crate | Upstream | Motivo del parche |
|-------|----------|-------------------|
| `xilem` | [xilem](https://github.com/linebender/xilem) | Modificaciones para compilación más rápida, API ajustada para el generador de GUI de Forja |
| `masonry_winit` | [masonry](https://github.com/linebender/masonry) | Integración con winit adaptada para el runtime de GUI de Forja |

## Uso

Estos parches se referencian desde el `Cargo.toml` del core de Forja mediante `[patch.crates-io]`:

```toml
[patch.crates-io]
xilem = { git = "https://github.com/forja-lang/patches", package = "xilem" }
masonry_winit = { git = "https://github.com/forja-lang/patches", package = "masonry_winit" }
```

## Mantenimiento

Al actualizar una dependencia upstream:

1. Reemplazar la carpeta del crate con la nueva versión
2. Re-aplicar los cambios específicos de Forja (ver diff con `git log`)
3. Verificar que el core de Forja compile con `cargo build --features gui`

## Repositorios relacionados

- [forja-lang/forja](https://github.com/forja-lang/forja) — Núcleo del lenguaje
