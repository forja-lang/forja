// forja-theme — Sistema de tema Material You (Material 3) para Forja GUI.
//
// Este crate es autocontenido: NO depende de `xilem` ni de `forja`.
// Provee colores dinámicos (HCT), esquemas de color, tipografía, formas,
// elevación, movimiento y animación. `forja-gui-rt` lo re-exporta para
// preservar la API pública histórica (`forja_gui_rt::theme::*`).
#![forbid(unsafe_code)]

pub mod theme;

pub use theme::*;
