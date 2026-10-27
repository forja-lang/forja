# forja-theme

Sistema de tema **Material You (Material 3)** para Forja GUI, extraído como crate
independiente y autocontenido.

## Características

- **Colores dinámicos (HCT)**: genera paletas tonales a partir de un color semilla.
- **Esquema de color completo**: roles Material 3 (primary, secondary, tertiary,
  surface, error, etc.) en modo claro y oscuro.
- **Tipografía**: escala tipográfica Material 3 (15 estilos).
- **Formas**: sistema de radios de borde por familia de componentes.
- **Elevación**: sombras Material 3.
- **Movimiento y animación**: curvas de easing, `AnimationEngine`, presets y
  animaciones de resorte.

## Sin dependencias pesadas

Este crate **no depende de `xilem` ni de `forja`**. Solo usa la biblioteca estándar
de Rust, por lo que puede usarse en cualquier proyecto Rust que quiera el sistema
de tema Material 3.

## Uso

```rust
use forja_theme::MaterialTheme;

let theme = MaterialTheme::from_seed("#6750A4", false);
println!("{}", theme.scheme.primary);
```

## Licencia

MIT — ver [LICENSE.md](../../LICENSE.md).
