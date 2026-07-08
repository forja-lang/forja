// Material You Theme — sistema de tema completo para Forja GUI
//
// Este módulo implementa el sistema de theming Material Design 3 (Material You)
// con colores dinámicos (Monet), tipografía, formas, elevación, estados, motion y animaciones.

pub mod animation;
pub mod color;
pub mod dynamic_color;
pub mod elevation;
pub mod motion;
pub mod palette;
pub mod scheme;
pub mod shape;
pub mod state;
pub mod system_theme;
pub mod typography;

pub use animation::*;
pub use color::*;
pub use dynamic_color::*;
pub use elevation::*;
pub use motion::*;
pub use palette::*;
pub use scheme::*;
pub use shape::*;
pub use state::*;
pub use system_theme::*;
pub use typography::*;


/// Tema Material You completo con todos los subsistemas
#[derive(Clone, Debug)]
pub struct MaterialTheme {
    /// Esquema de color (roles de color light/dark)
    pub scheme: ColorScheme,
    /// Escala tipográfica (15 estilos)
    pub typography: TypeScale,
    /// Sistema de formas (radios de borde)
    pub shapes: ShapeSystem,
    /// Sistema de elevación (sombras)
    pub elevation: ElevationSystem,
    /// Sistema de movimiento (animaciones)
    pub motion: MotionSystem,
    /// Indica si es modo oscuro
    pub is_dark: bool,
    /// Color semilla usado para generar el tema
    pub seed_color: String,
}

impl MaterialTheme {
    /// Crea un tema desde un color semilla (hex o nombre)
    ///
    /// # Argumentos
    /// - `seed_color`: color semilla en formato hex (#RRGGBB) o nombre en español
    /// - `is_dark`: `true` para modo oscuro, `false` para modo claro
    ///
    /// # Ejemplo
    /// ```ignore
    /// let tema = MaterialTheme::from_seed("#FF5722", false);
    /// let tema_oscuro = MaterialTheme::from_seed("rojo", true);
    /// ```
    pub fn from_seed(seed_color: &str, is_dark: bool) -> Self {
        // Parsear el color semilla
        let seed: RgbColor = if let Some(color) = color::nombre_a_color(seed_color) {
            color
        } else {
            RgbColor::from_hex(seed_color).unwrap_or(RgbColor(103, 80, 164))
        };

        // Generar paletas desde el seed usando algoritmo Monet
        let palettes = Palettes::from_seed(&seed);

        // Crear esquema de color
        let scheme = ColorScheme::from_palettes(&palettes, is_dark);

        MaterialTheme {
            scheme,
            typography: TypeScale::default(),
            shapes: ShapeSystem::default(),
            elevation: ElevationSystem::default(),
            motion: MotionSystem::default(),
            is_dark,
            seed_color: seed.to_hex(),
        }
    }

    /// Crea un tema claro por defecto (seed: #6750A4 — púrpura)
    pub fn light() -> Self {
        MaterialTheme::from_seed(color::SEED_DEFAULT, false)
    }

    /// Crea un tema oscuro por defecto (seed: #6750A4 — púrpura)
    pub fn dark() -> Self {
        MaterialTheme::from_seed(color::SEED_DEFAULT, true)
    }

    /// Crea un tema dinámico que detecta automáticamente claro/oscuro
    ///
    /// NOTA: La detección automática del modo del sistema requiere
    /// acceso a `winit::window::Theme`. Si no está disponible,
    /// devuelve modo claro.
    pub fn dynamic(seed_color: &str) -> Self {
        // Por defecto usamos modo claro
        // En el futuro, aquí se integrará detección del theme del sistema
        MaterialTheme::from_seed(seed_color, false)
    }

    /// Crea un tema que sigue la preferencia del sistema operativo
    ///
    /// Detecta automáticamente si el sistema está en modo oscuro
    /// (Windows: registro, Linux: gsettings, macOS: defaults)
    /// y aplica el tema correspondiente.
    ///
    /// Si no se puede detectar, fallback a modo claro.
    pub fn system(seed_color: &str) -> Self {
        Self::from_seed(seed_color, is_system_dark())
    }

    /// Cambia entre modo claro y oscuro
    pub fn toggle_dark_mode(&self) -> Self {
        MaterialTheme::from_seed(&self.seed_color, !self.is_dark)
    }

    /// Crea un tema con un nuevo color semilla, manteniendo el modo
    pub fn with_seed(&self, seed_color: &str) -> Self {
        MaterialTheme::from_seed(seed_color, self.is_dark)
    }
}

impl Default for MaterialTheme {
    fn default() -> Self {
        MaterialTheme::light()
    }
}

impl std::fmt::Display for MaterialTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MaterialTheme(seed={}, dark={})", self.seed_color, self.is_dark)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_light() {
        let theme = MaterialTheme::light();
        assert!(!theme.is_dark);
        assert_eq!(theme.seed_color, "#6750A4");
        assert_eq!(theme.scheme.primary, RgbColor(103, 80, 164));
    }

    #[test]
    fn test_theme_dark() {
        let theme = MaterialTheme::dark();
        assert!(theme.is_dark);
        // El primary dark es tone 80 de la paleta primaria simplificada
        // Puede diferir ligeramente del valor oficial de Material You
        // debido a la implementación HCT simplificada
        assert_ne!(theme.scheme.primary, RgbColor(0, 0, 0));
        assert!(theme.scheme.primary.0 > 200); // Debe ser claro
        assert!(theme.scheme.primary.2 > 200); // Debe tener componente azul
    }

    #[test]
    fn test_theme_from_seed_hex() {
        let theme = MaterialTheme::from_seed("#FF5722", false);
        assert_eq!(theme.seed_color.to_uppercase(), "#FF5722");
    }

    #[test]
    fn test_theme_from_seed_name() {
        let theme = MaterialTheme::from_seed("rojo", false);
        assert_eq!(theme.seed_color, "#F44336".to_string());
    }

    #[test]
    fn test_theme_toggle() {
        let light = MaterialTheme::light();
        let dark = light.toggle_dark_mode();
        assert!(dark.is_dark);
        assert_eq!(dark.seed_color, light.seed_color);
        let back_to_light = dark.toggle_dark_mode();
        assert!(!back_to_light.is_dark);
    }

    #[test]
    fn test_theme_default() {
        let theme = MaterialTheme::default();
        assert!(!theme.is_dark);
        assert_eq!(theme.seed_color, "#6750A4");
    }

    #[test]
    fn test_all_subsystems_present() {
        let theme = MaterialTheme::light();
        // Verificar que todos los subsistemas están accesibles
        let _ = theme.typography.body_medium;
        let _ = theme.shapes.small;
        let _ = theme.elevation.level1;
        let _ = theme.motion.durations.duration_300;
    }

    #[test]
    fn test_with_seed() {
        let original = MaterialTheme::light();
        let nuevo = original.with_seed("#4CAF50");
        assert_eq!(nuevo.seed_color, "#4CAF50");
        assert!(!nuevo.is_dark);
        assert_ne!(nuevo.scheme.primary, original.scheme.primary);
    }
}
