# Tests UI (diagnóstico congelado)

Cada `*.fa` de este árbol debe fallar en compilación y su salida se compara
contra el `.stderr` hermano, sin colores y con saltos de línea normalizados.

- Correr: `cargo test --test ui`
- Regenerar los `.stderr` después de un cambio intencional:
  `FORJA_UPDATE=1 cargo test --test ui`

La idea es la de `tests/ui` de Rust: un cambio de mensaje se ve en el diff, no
se descubre en producción.
