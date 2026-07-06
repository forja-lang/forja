// Forja GUI Runtime (forja-gui-rt)
// Pre-compila xilem para que las apps GUI de Forja compilen en segundos.
// Este crate se compila UNA SOLA VEZ; las apps generadas solo enlazan contra él.

// Re-exportar todo lo que necesita el código generado por Forja
pub use xilem::view::{
    self, button, button_any_pointer, checkbox, flex, grid, image, indexed_stack, label,
    portal, progress_bar, prose, sized_box, slider, spinner, split, text_button, text_input,
    variable_label, virtual_scroll, zstack, Axis, GridParams,
};
pub use xilem::{
    AnyWidgetView, EventLoop, FontWeight, TextAlign, WidgetView, WindowOptions, Xilem,
};
pub use xilem::winit::error::EventLoopError;
pub use xilem::{Affine, Blob, Color, ImageBrush, ImageFormat, palette};
pub use xilem::core::{lens, memoize};
pub use xilem::masonry::properties::types::Length;
