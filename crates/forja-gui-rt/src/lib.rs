// Forja GUI Runtime (forja-gui-rt)
// Pre-compila xilem para que las apps GUI de Forja compilen en segundos.
// Este crate se compila UNA SOLA VEZ; las apps generadas solo enlazan contra él.

// Re-exportar todo lo que necesita el código generado por Forja
pub use xilem::view::{
    self, button, button_any_pointer, checkbox, flex, grid, image, indexed_stack, label,
    portal, progress_bar, prose, sized_box, slider, spinner, split, text_button, text_input,
    variable_label, virtual_scroll, zstack, Axis, GridParams, MainAxisAlignment, CrossAxisAlignment,
};
pub use xilem::{
    AnyWidgetView, EventLoop, FontWeight, TextAlign, WidgetView, WindowOptions, Xilem,
};
pub use xilem::winit::error::EventLoopError;
pub use xilem::{Affine, Blob, Color, ImageBrush, ImageFormat, palette};
pub use xilem::core::{lens, memoize, map_message};
pub use xilem::core::{MessageResult, MessageContext};
pub use xilem::ViewCtx;
pub use xilem::masonry::properties::types::Length;
pub use xilem::style::{Background, Style};

// === Re-exportaciones para WindowSize y resize detection ===
/// LogicalSize: tamaño lógico de ventana (independiente de DPI)
pub use xilem::masonry::dpi::LogicalSize;
/// Window: acceso a la ventana nativa de winit
pub use xilem::winit::window::Window;
/// Pod: wrapper de widgets Masonry como WidgetView
pub use xilem::Pod;
/// Re-exportamos módulos de Masonry necesarios para widgets personalizados
pub use xilem::masonry::widgets;
pub use xilem::masonry::core::Widget;
pub use xilem::masonry::core::{BoxConstraints, NewWidget};
pub use xilem::masonry::core::ChildrenIds;
pub use xilem::masonry::core::{
    WidgetPod, WidgetMut, NoAction, LayoutCtx, RegisterCtx, PaintCtx, AccessCtx,
    PropertiesMut, PropertiesRef, UpdateCtx, WidgetId,
};
pub use xilem::masonry::accesskit::{self, Node, Role};
pub use xilem::masonry::vello::{self, Scene};
pub use xilem::masonry::kurbo::{Point, Size};
pub use xilem::core::{View, ViewMarker, Mut};

// Módulo de tema Material You
pub mod theme;
// Módulo de iconos Material Design
pub mod icons;
// Re-exportar tipos específicos del theme (evitar conflictos de nombres con xilem,
// ej: FontWeight existe tanto en xilem como en theme::typography)
pub use theme::{
    MaterialTheme,    // Tema completo
    ColorScheme,      // Esquema de color con roles
    RgbColor,         // Color RGB (convierte a xilem::Color via From)
    TypeScale,        // Escala tipográfica (15 estilos)
    TextStyle,        // Estilo de texto individual
    ShapeSystem,      // Sistema de formas (radios de borde)
    ShapeFamily,      // Familia de componentes para formas
    ElevationSystem,  // Sistema de elevación (sombras)
};
// Re-exportar tipos principales de iconos para uso directo
pub use icons::{
    MaterialIcon,
    IconStyle,
    icon_widget,
    icon_widget_styled,
};

// Re-exportar tipos principales de animación para uso directo
pub use theme::animation::{
    AnimationEngine,
    AnimatedValue,
    SpringAnimation,
    AnimationPresets,
    Animation,
    interpolate_color,
};
