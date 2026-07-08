// State — capas de estado Material You (StateLayer)
//
// Define las opacidades y overlays para los estados interactivos
// de los componentes (hover, focus, pressed, dragged, disabled).

use crate::theme::color::RgbColor;

/// Capa de estado que aplica overlays semitransparentes
/// sobre colores base según el estado interactivo.
#[derive(Clone, Debug, PartialEq)]
pub struct StateLayer {
    /// Opacidad para hover: 0.08 (8%)
    pub hover_opacity: f64,
    /// Opacidad para focus: 0.12 (12%)
    pub focus_opacity: f64,
    /// Opacidad para pressed: 0.12 (12%)
    pub pressed_opacity: f64,
    /// Opacidad para dragged: 0.16 (16%)
    pub dragged_opacity: f64,
    /// Opacidad para disabled: 0.38 (38%)
    pub disabled_opacity: f64,
    /// Color del overlay (por defecto: on-surface)
    pub overlay_color: RgbColor,
}

impl StateLayer {
    /// Capa de estado por defecto (overlay color = on-surface oscuro)
    pub fn default() -> Self {
        StateLayer {
            hover_opacity: 0.08,
            focus_opacity: 0.12,
            pressed_opacity: 0.12,
            dragged_opacity: 0.16,
            disabled_opacity: 0.38,
            overlay_color: RgbColor(28, 27, 31), // on-surface
        }
    }

    /// Capa de estado para superficies claras (overlay color = on-surface claro)
    pub fn on_surface_light() -> Self {
        StateLayer {
            hover_opacity: 0.08,
            focus_opacity: 0.12,
            pressed_opacity: 0.12,
            dragged_opacity: 0.16,
            disabled_opacity: 0.38,
            overlay_color: RgbColor(230, 225, 229), // on-surface dark
        }
    }

    /// Aplica el overlay de estado sobre un color base
    ///
    /// Mezcla el `overlay_color` con el `base_color` usando la
    /// opacidad especificada. Útil para calcular colores de estado.
    pub fn apply(&self, base_color: RgbColor, opacity: f64) -> RgbColor {
        let opacity = opacity.clamp(0.0, 1.0);
        base_color.blend(&self.overlay_color, opacity)
    }

    /// Color para estado hover
    pub fn hover_color(&self, base: RgbColor) -> RgbColor {
        self.apply(base, self.hover_opacity)
    }

    /// Color para estado focus
    pub fn focus_color(&self, base: RgbColor) -> RgbColor {
        self.apply(base, self.focus_opacity)
    }

    /// Color para estado pressed
    pub fn pressed_color(&self, base: RgbColor) -> RgbColor {
        self.apply(base, self.pressed_opacity)
    }

    /// Color para estado disabled
    pub fn disabled_color(&self, base: RgbColor) -> RgbColor {
        self.apply(base, self.disabled_opacity)
    }

    /// Color para estado dragged
    pub fn dragged_color(&self, base: RgbColor) -> RgbColor {
        self.apply(base, self.dragged_opacity)
    }

    /// Calcula si un componente debe considerarse visible con opacidad reducida
    pub fn disabled_alpha(&self) -> f64 {
        1.0 - self.disabled_opacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_layer_default() {
        let state = StateLayer::default();
        assert!((state.hover_opacity - 0.08).abs() < 0.001);
        assert!((state.focus_opacity - 0.12).abs() < 0.001);
        assert!((state.pressed_opacity - 0.12).abs() < 0.001);
        assert!((state.dragged_opacity - 0.16).abs() < 0.001);
        assert!((state.disabled_opacity - 0.38).abs() < 0.001);
    }

    #[test]
    fn test_apply() {
        let state = StateLayer::default();
        let base = RgbColor(255, 255, 255);

        // Aplicar con opacidad 0 debe devolver el color base
        let result = state.apply(base, 0.0);
        assert_eq!(result, base);

        // Aplicar con opacidad 1 debe devolver el overlay color
        let result = state.apply(base, 1.0);
        assert_eq!(result, state.overlay_color);
    }

    #[test]
    fn test_hover_color() {
        let state = StateLayer::default();
        let base = RgbColor(255, 255, 255);
        let hover = state.hover_color(base);
        // El hover debe estar entre el base y el overlay
        assert_ne!(hover, base);
        assert_ne!(hover, state.overlay_color);
    }

    #[test]
    fn test_disabled_alpha() {
        let state = StateLayer::default();
        let alpha = state.disabled_alpha();
        assert!((alpha - 0.62).abs() < 0.001);
    }
}
