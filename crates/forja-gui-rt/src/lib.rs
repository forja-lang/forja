// Forja GUI Runtime (forja-gui-rt)
// Pre-compila xilem para que las apps GUI de Forja compilen en segundos.
// Este crate se compila UNA SOLA VEZ; las apps generadas solo enlazan contra él.

// Re-exportar todo lo que necesita el código generado por Forja
pub use xilem::view::{self, Axis, flex, label, text_button, text_input, progress_bar, sized_box};
pub use xilem::{WidgetView, Xilem, WindowOptions, EventLoop, AnyWidgetView};
pub use xilem::winit::error::EventLoopError;
