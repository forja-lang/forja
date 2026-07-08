// Typography — sistema tipográfico Material You (TypeScale)
//
// Define los 15 estilos tipográficos de Material Design 3 con sus
// propiedades: tamaño, altura de línea, tracking y peso.

/// Peso de fuente tipográfica
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontWeight {
    Thin,       // 100
    Light,      // 300
    Regular,    // 400
    Medium,     // 500
    Bold,       // 700
}

impl FontWeight {
    /// Convierte al peso numérico
    pub fn value(&self) -> u16 {
        match self {
            FontWeight::Thin => 100,
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::Bold => 700,
        }
    }

    /// Convierte al tipo FontWeight de Xilem
    pub fn to_xilem_weight(&self) -> xilem::FontWeight {
        match self {
            FontWeight::Thin => xilem::FontWeight::THIN,
            FontWeight::Light => xilem::FontWeight::LIGHT,
            FontWeight::Regular => xilem::FontWeight::NORMAL,
            FontWeight::Medium => xilem::FontWeight::MEDIUM,
            FontWeight::Bold => xilem::FontWeight::BOLD,
        }
    }
}

/// Estilo de texto con todas las propiedades tipográficas
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Tamaño de fuente en sp (pixels escalados)
    pub font_size: f64,
    /// Altura de línea en sp
    pub line_height: f64,
    /// Tracking (letter-spacing) en sp
    pub tracking: f64,
    /// Peso de la fuente
    pub weight: FontWeight,
}

impl TextStyle {
    /// Crea un nuevo estilo de texto
    pub const fn new(font_size: f64, line_height: f64, tracking: f64, weight: FontWeight) -> Self {
        TextStyle { font_size, line_height, tracking, weight }
    }
}

/// Escala tipográfica completa de Material You (15 estilos)
#[derive(Clone, Debug, PartialEq)]
pub struct TypeScale {
    // Display
    pub display_large: TextStyle,
    pub display_medium: TextStyle,
    pub display_small: TextStyle,
    // Headline
    pub headline_large: TextStyle,
    pub headline_medium: TextStyle,
    pub headline_small: TextStyle,
    // Title
    pub title_large: TextStyle,
    pub title_medium: TextStyle,
    pub title_small: TextStyle,
    // Body
    pub body_large: TextStyle,
    pub body_medium: TextStyle,
    pub body_small: TextStyle,
    // Label
    pub label_large: TextStyle,
    pub label_medium: TextStyle,
    pub label_small: TextStyle,
}

impl TypeScale {
    /// Escala tipográfica por defecto de Material You
    pub fn default() -> Self {
        TypeScale {
            // Display
            display_large: TextStyle::new(57.0, 64.0, -0.25, FontWeight::Regular),
            display_medium: TextStyle::new(45.0, 52.0, 0.0, FontWeight::Regular),
            display_small: TextStyle::new(36.0, 44.0, 0.0, FontWeight::Regular),
            // Headline
            headline_large: TextStyle::new(32.0, 40.0, 0.0, FontWeight::Regular),
            headline_medium: TextStyle::new(28.0, 36.0, 0.0, FontWeight::Regular),
            headline_small: TextStyle::new(24.0, 32.0, 0.0, FontWeight::Regular),
            // Title
            title_large: TextStyle::new(22.0, 28.0, 0.0, FontWeight::Regular),
            title_medium: TextStyle::new(16.0, 24.0, 0.15, FontWeight::Medium),
            title_small: TextStyle::new(14.0, 20.0, 0.1, FontWeight::Medium),
            // Body
            body_large: TextStyle::new(16.0, 24.0, 0.5, FontWeight::Regular),
            body_medium: TextStyle::new(14.0, 20.0, 0.25, FontWeight::Regular),
            body_small: TextStyle::new(12.0, 16.0, 0.4, FontWeight::Regular),
            // Label
            label_large: TextStyle::new(14.0, 20.0, 0.1, FontWeight::Medium),
            label_medium: TextStyle::new(12.0, 16.0, 0.5, FontWeight::Medium),
            label_small: TextStyle::new(11.0, 16.0, 0.5, FontWeight::Medium),
        }
    }

    /// Busca un estilo por nombre ("display_large", "body_medium", etc.)
    pub fn apply(&self, style_name: &str) -> &TextStyle {
        match style_name {
            "display_large" => &self.display_large,
            "display_medium" => &self.display_medium,
            "display_small" => &self.display_small,
            "headline_large" => &self.headline_large,
            "headline_medium" => &self.headline_medium,
            "headline_small" => &self.headline_small,
            "title_large" => &self.title_large,
            "title_medium" => &self.title_medium,
            "title_small" => &self.title_small,
            "body_large" => &self.body_large,
            "body_medium" => &self.body_medium,
            "body_small" => &self.body_small,
            "label_large" => &self.label_large,
            "label_medium" => &self.label_medium,
            "label_small" => &self.label_small,
            _ => &self.body_medium,
        }
    }

    /// Lista todos los nombres de estilos disponibles
    pub fn style_names() -> &'static [&'static str] {
        &[
            "display_large", "display_medium", "display_small",
            "headline_large", "headline_medium", "headline_small",
            "title_large", "title_medium", "title_small",
            "body_large", "body_medium", "body_small",
            "label_large", "label_medium", "label_small",
        ]
    }
}

impl std::fmt::Display for TypeScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypeScale(display_large={}sp, body_medium={}sp)",
            self.display_large.font_size, self.body_medium.font_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_scale_default() {
        let ts = TypeScale::default();
        assert!((ts.display_large.font_size - 57.0).abs() < 0.01);
        assert!((ts.body_medium.font_size - 14.0).abs() < 0.01);
        assert!((ts.label_small.font_size - 11.0).abs() < 0.01);
    }

    #[test]
    fn test_apply() {
        let ts = TypeScale::default();
        let style = ts.apply("display_large");
        assert!((style.font_size - 57.0).abs() < 0.01);
        let style = ts.apply("inexistente");
        assert!((style.font_size - 14.0).abs() < 0.01); // default body_medium
    }

    #[test]
    fn test_style_names() {
        let names = TypeScale::style_names();
        assert_eq!(names.len(), 15);
        assert!(names.contains(&"display_large"));
        assert!(names.contains(&"label_small"));
    }

    #[test]
    fn test_font_weight_values() {
        assert_eq!(FontWeight::Thin.value(), 100);
        assert_eq!(FontWeight::Regular.value(), 400);
        assert_eq!(FontWeight::Bold.value(), 700);
    }
}
