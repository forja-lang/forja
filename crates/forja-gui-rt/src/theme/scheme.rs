// Scheme — esquema de color Material You (ColorScheme)
//
// Define los 13+ color roles de Material Design 3, tanto para modo
// claro como oscuro, generados desde las paletas tonales.

use crate::theme::color::RgbColor;
use crate::theme::palette::Palettes;

/// Esquema de color completo de Material You con 13+ color roles
#[derive(Clone, Debug, PartialEq)]
pub struct ColorScheme {
    // Primario
    pub primary: RgbColor,
    pub on_primary: RgbColor,
    pub primary_container: RgbColor,
    pub on_primary_container: RgbColor,

    // Secundario
    pub secondary: RgbColor,
    pub on_secondary: RgbColor,
    pub secondary_container: RgbColor,
    pub on_secondary_container: RgbColor,

    // Terciario
    pub tertiary: RgbColor,
    pub on_tertiary: RgbColor,
    pub tertiary_container: RgbColor,
    pub on_tertiary_container: RgbColor,

    // Error
    pub error: RgbColor,
    pub on_error: RgbColor,
    pub error_container: RgbColor,
    pub on_error_container: RgbColor,

    // Superficie
    pub surface: RgbColor,
    pub on_surface: RgbColor,
    pub surface_variant: RgbColor,
    pub on_surface_variant: RgbColor,

    // Fondo
    pub background: RgbColor,
    pub on_background: RgbColor,

    // Outline
    pub outline: RgbColor,
    pub outline_variant: RgbColor,

    // Inverse
    pub inverse_surface: RgbColor,
    pub inverse_on_surface: RgbColor,
    pub inverse_primary: RgbColor,
}

impl ColorScheme {
    /// Crea un esquema de color desde las paletas tonales
    ///
    /// Mapea los tonos de cada paleta a los color roles según modo claro/oscuro:
    ///
    /// Light mode:
    /// - primary: tone 40, on_primary: tone 100, primary_container: tone 90, on_primary_container: tone 10
    /// - surface: tone 98, background: tone 98
    /// - etc.
    ///
    /// Dark mode:
    /// - primary: tone 80, on_primary: tone 20, primary_container: tone 30, on_primary_container: tone 90
    /// - surface: tone 6, background: tone 6
    pub fn from_palettes(palettes: &Palettes, is_dark: bool) -> Self {
        if is_dark {
            ColorScheme {
                // Primario
                primary: palettes.primary.tone(80),
                on_primary: palettes.primary.tone(20),
                primary_container: palettes.primary.tone(30),
                on_primary_container: palettes.primary.tone(90),
                // Secundario
                secondary: palettes.secondary.tone(80),
                on_secondary: palettes.secondary.tone(20),
                secondary_container: palettes.secondary.tone(30),
                on_secondary_container: palettes.secondary.tone(90),
                // Terciario
                tertiary: palettes.tertiary.tone(80),
                on_tertiary: palettes.tertiary.tone(20),
                tertiary_container: palettes.tertiary.tone(30),
                on_tertiary_container: palettes.tertiary.tone(90),
                // Error (usamos paleta fija para error: rojo)
                error: RgbColor(242, 184, 181),
                on_error: RgbColor(89, 26, 19),
                error_container: RgbColor(140, 29, 24),
                on_error_container: RgbColor(249, 222, 220),
                // Superficie
                surface: palettes.neutral.tone(6),
                on_surface: palettes.neutral.tone(90),
                surface_variant: palettes.neutral_variant.tone(30),
                on_surface_variant: palettes.neutral_variant.tone(80),
                // Fondo
                background: palettes.neutral.tone(6),
                on_background: palettes.neutral.tone(90),
                // Outline
                outline: palettes.neutral_variant.tone(60),
                outline_variant: palettes.neutral_variant.tone(30),
                // Inverse
                inverse_surface: palettes.neutral.tone(90),
                inverse_on_surface: palettes.neutral.tone(20),
                inverse_primary: palettes.primary.tone(40),
            }
        } else {
            ColorScheme {
                // Primario
                primary: palettes.primary.tone(40),
                on_primary: palettes.primary.tone(100),
                primary_container: palettes.primary.tone(90),
                on_primary_container: palettes.primary.tone(10),
                // Secundario
                secondary: palettes.secondary.tone(40),
                on_secondary: palettes.secondary.tone(100),
                secondary_container: palettes.secondary.tone(90),
                on_secondary_container: palettes.secondary.tone(10),
                // Terciario
                tertiary: palettes.tertiary.tone(40),
                on_tertiary: palettes.tertiary.tone(100),
                tertiary_container: palettes.tertiary.tone(90),
                on_tertiary_container: palettes.tertiary.tone(10),
                // Error (fijo)
                error: RgbColor(179, 38, 30),
                on_error: RgbColor(255, 255, 255),
                error_container: RgbColor(249, 222, 220),
                on_error_container: RgbColor(65, 14, 11),
                // Superficie
                surface: palettes.neutral.tone(98),
                on_surface: palettes.neutral.tone(10),
                surface_variant: palettes.neutral_variant.tone(90),
                on_surface_variant: palettes.neutral_variant.tone(30),
                // Fondo
                background: palettes.neutral.tone(98),
                on_background: palettes.neutral.tone(10),
                // Outline
                outline: palettes.neutral_variant.tone(50),
                outline_variant: palettes.neutral_variant.tone(80),
                // Inverse
                inverse_surface: palettes.neutral.tone(20),
                inverse_on_surface: palettes.neutral.tone(95),
                inverse_primary: palettes.primary.tone(80),
            }
        }
    }

    /// Esquema claro por defecto Material You (seed: #6750A4)
    pub fn light() -> Self {
        ColorScheme {
            primary: RgbColor(103, 80, 164),
            on_primary: RgbColor(255, 255, 255),
            primary_container: RgbColor(234, 221, 255),
            on_primary_container: RgbColor(33, 0, 93),
            secondary: RgbColor(98, 91, 113),
            on_secondary: RgbColor(255, 255, 255),
            secondary_container: RgbColor(232, 222, 248),
            on_secondary_container: RgbColor(29, 25, 43),
            tertiary: RgbColor(125, 82, 96),
            on_tertiary: RgbColor(255, 255, 255),
            tertiary_container: RgbColor(255, 216, 228),
            on_tertiary_container: RgbColor(49, 17, 29),
            error: RgbColor(179, 38, 30),
            on_error: RgbColor(255, 255, 255),
            error_container: RgbColor(249, 222, 220),
            on_error_container: RgbColor(65, 14, 11),
            surface: RgbColor(255, 251, 254),
            on_surface: RgbColor(28, 27, 31),
            surface_variant: RgbColor(231, 224, 236),
            on_surface_variant: RgbColor(73, 69, 79),
            background: RgbColor(255, 251, 254),
            on_background: RgbColor(28, 27, 31),
            outline: RgbColor(121, 116, 126),
            outline_variant: RgbColor(196, 199, 197),
            inverse_surface: RgbColor(49, 48, 51),
            inverse_on_surface: RgbColor(244, 239, 244),
            inverse_primary: RgbColor(208, 188, 255),
        }
    }

    /// Esquema oscuro por defecto Material You (seed: #6750A4)
    pub fn dark() -> Self {
        ColorScheme {
            primary: RgbColor(208, 188, 255),
            on_primary: RgbColor(55, 30, 115),
            primary_container: RgbColor(79, 55, 139),
            on_primary_container: RgbColor(234, 221, 255),
            secondary: RgbColor(204, 194, 220),
            on_secondary: RgbColor(50, 45, 65),
            secondary_container: RgbColor(74, 68, 88),
            on_secondary_container: RgbColor(232, 222, 248),
            tertiary: RgbColor(239, 184, 200),
            on_tertiary: RgbColor(73, 38, 50),
            tertiary_container: RgbColor(99, 60, 73),
            on_tertiary_container: RgbColor(255, 216, 228),
            error: RgbColor(242, 184, 181),
            on_error: RgbColor(89, 26, 19),
            error_container: RgbColor(140, 29, 24),
            on_error_container: RgbColor(249, 222, 220),
            surface: RgbColor(28, 27, 31),
            on_surface: RgbColor(230, 225, 229),
            surface_variant: RgbColor(73, 69, 79),
            on_surface_variant: RgbColor(202, 196, 208),
            background: RgbColor(28, 27, 31),
            on_background: RgbColor(230, 225, 229),
            outline: RgbColor(147, 143, 153),
            outline_variant: RgbColor(73, 69, 79),
            inverse_surface: RgbColor(230, 225, 229),
            inverse_on_surface: RgbColor(49, 48, 51),
            inverse_primary: RgbColor(103, 80, 164),
        }
    }

    /// Calcula el color de superficie con elevación
    ///
    /// En dark mode, la superficie se aclara con el color primario según nivel.
    /// En light mode, la superficie no cambia (solo sombra).
    pub fn surface_at_elevation(&self, level: u8, is_dark: bool) -> RgbColor {
        if !is_dark {
            return self.surface;
        }
        // En dark: overlay de primary al 5-14% según nivel
        let opacity = match level {
            0 => 0.00,
            1 => 0.05,
            2 => 0.08,
            3 => 0.11,
            4 => 0.12,
            5 => 0.14,
            _ => 0.14_f64.min(0.02 * level as f64),
        };
        self.surface.blend(&self.primary, opacity)
    }
}

impl std::fmt::Display for ColorScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ColorScheme(primary={})", self.primary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_scheme_default() {
        let scheme = ColorScheme::light();
        assert_eq!(scheme.primary, RgbColor(103, 80, 164)); // #6750A4
        assert_eq!(scheme.on_primary, RgbColor(255, 255, 255));
        assert_eq!(scheme.surface, RgbColor(255, 251, 254));
        assert_eq!(scheme.background, RgbColor(255, 251, 254));
    }

    #[test]
    fn test_dark_scheme_default() {
        let scheme = ColorScheme::dark();
        assert_eq!(scheme.primary, RgbColor(208, 188, 255)); // #D0BCFF
        assert_eq!(scheme.on_primary, RgbColor(55, 30, 115));
        assert_eq!(scheme.surface, RgbColor(28, 27, 31));
    }

    #[test]
    fn test_scheme_from_palettes_light() {
        let seed = RgbColor::from_hex("#6750A4").unwrap();
        let palettes = Palettes::from_seed(&seed);
        let scheme = ColorScheme::from_palettes(&palettes, false);
        // El primary debe ser un tono 40 de la paleta primaria
        let expected_primary = palettes.primary.tone(40);
        assert_eq!(scheme.primary, expected_primary);
    }

    #[test]
    fn test_scheme_from_palettes_dark() {
        let seed = RgbColor::from_hex("#6750A4").unwrap();
        let palettes = Palettes::from_seed(&seed);
        let scheme = ColorScheme::from_palettes(&palettes, true);
        // El primary debe ser un tono 80 de la paleta primaria
        let expected_primary = palettes.primary.tone(80);
        assert_eq!(scheme.primary, expected_primary);
    }

    #[test]
    fn test_surface_at_elevation_light() {
        let scheme = ColorScheme::light();
        let surface = scheme.surface_at_elevation(3, false);
        assert_eq!(surface, scheme.surface); // Sin cambio en light
    }

    #[test]
    fn test_surface_at_elevation_dark() {
        let scheme = ColorScheme::dark();
        let base = scheme.surface;
        let elevated = scheme.surface_at_elevation(3, true);
        // Con elevación, la superficie debe aclararse (mezclarse con primary)
        assert_ne!(base, elevated, "La superficie con elevación debe diferir de la base en dark mode");
    }
}
