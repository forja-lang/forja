# forja-gui-core

Núcleo reutilizable de **Forja GUI**: señales reactivas (`signals`) y logging de
estado/callbacks (`debug`). No depende de `xilem`, `masonry` ni del compilador
`forja`, por lo que puede publicarse o reutilizarse por separado.

`forja-gui-rt` depende de este crate y re-exporta `signals`.
