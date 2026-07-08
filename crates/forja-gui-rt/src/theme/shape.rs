// Shape — sistema de formas Material You
//
// Define los radios de borde para componentes siguiendo
// Material Design 3.

/// Sistema de formas con radios predefinidos
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeSystem {
    /// Sin redondeo (0dp)
    pub none: f64,
    /// Muy pequeño (4dp)
    pub extra_small: f64,
    /// Pequeño (8dp)
    pub small: f64,
    /// Mediano (12dp)
    pub medium: f64,
    /// Grande (16dp)
    pub large: f64,
    /// Extra grande (28dp)
    pub extra_large: f64,
    /// Completo (-1.0 representa 50% para formas circulares)
    pub full: f64,
}

impl ShapeSystem {
    /// Valores por defecto de Material You
    pub fn default() -> Self {
        ShapeSystem {
            none: 0.0,
            extra_small: 4.0,
            small: 8.0,
            medium: 12.0,
            large: 16.0,
            extra_large: 28.0,
            full: -1.0, // -1.0 representa 50%
        }
    }

    /// Obtiene el radio para una familia de componentes
    pub fn for_family(&self, family: ShapeFamily) -> f64 {
        match family {
            ShapeFamily::Surface => self.small,
            ShapeFamily::Container => self.medium,
            ShapeFamily::Button => 20.0,
            ShapeFamily::Navigation => self.none,
            ShapeFamily::Badge => self.small,
            ShapeFamily::Fab => self.large,
            ShapeFamily::TextField => self.extra_small,
        }
    }
}

/// Familia de componentes para determinar el redondeo
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeFamily {
    /// Tarjetas y superficies (small - 8dp)
    Surface,
    /// Contenedores (medium - 12dp)
    Container,
    /// Botones (20dp)
    Button,
    /// Barras de navegación (none - 0dp)
    Navigation,
    /// Badges (small - 8dp)
    Badge,
    /// FAB (large - 16dp)
    Fab,
    /// Campos de texto (extra_small - 4dp)
    TextField,
}

impl ShapeFamily {
    /// Nombre descriptivo de la familia
    pub fn name(&self) -> &'static str {
        match self {
            ShapeFamily::Surface => "Surface",
            ShapeFamily::Container => "Container",
            ShapeFamily::Button => "Button",
            ShapeFamily::Navigation => "Navigation",
            ShapeFamily::Badge => "Badge",
            ShapeFamily::Fab => "FAB",
            ShapeFamily::TextField => "TextField",
        }
    }
}

/// Crea un CornerRadius de Xilem/Masonry a partir de un radio uniforme
pub fn corner_radius(radius: f64) -> xilem::masonry::properties::CornerRadius {
    xilem::masonry::properties::CornerRadius::all(radius)
}

/// Crea un CornerRadius asimétrico (usa el promedio de las 4 esquinas como radio uniforme)
///
/// NOTA: Xilem 0.4 / Masonry 0.4 solo soporta CornerRadius uniforme.
/// Los valores individuales están disponibles para uso futuro.
pub fn corner_radius_asymmetric(
    _top_left: f64,
    _top_right: f64,
    _bottom_right: f64,
    _bottom_left: f64,
) -> xilem::masonry::properties::CornerRadius {
    let avg = (_top_left + _top_right + _bottom_right + _bottom_left) / 4.0;
    xilem::masonry::properties::CornerRadius::all(avg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_defaults() {
        let shapes = ShapeSystem::default();
        assert!((shapes.none - 0.0).abs() < 0.01);
        assert!((shapes.extra_small - 4.0).abs() < 0.01);
        assert!((shapes.small - 8.0).abs() < 0.01);
        assert!((shapes.medium - 12.0).abs() < 0.01);
        assert!((shapes.large - 16.0).abs() < 0.01);
        assert!((shapes.extra_large - 28.0).abs() < 0.01);
        assert!((shapes.full - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn test_for_family() {
        let shapes = ShapeSystem::default();
        assert!((shapes.for_family(ShapeFamily::Surface) - 8.0).abs() < 0.01);
        assert!((shapes.for_family(ShapeFamily::Button) - 20.0).abs() < 0.01);
        assert!((shapes.for_family(ShapeFamily::Fab) - 16.0).abs() < 0.01);
        assert!((shapes.for_family(ShapeFamily::TextField) - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_corner_radius() {
        let radius = corner_radius(8.0);
        assert!((radius.radius - 8.0).abs() < 0.01);
    }
}
