// Elevation — sistema de elevación Material You
//
// Define los niveles de elevación (sombras) y cómo afectan
// al color de superficie en modo oscuro.

use crate::theme::color::RgbColor;

/// Sistema de elevación con 6 niveles (0-5)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevationSystem {
    /// 0dp — sin elevación
    pub level0: f64,
    /// 1dp — elevación muy baja
    pub level1: f64,
    /// 3dp — elevación baja
    pub level2: f64,
    /// 6dp — elevación media
    pub level3: f64,
    /// 8dp — elevación alta
    pub level4: f64,
    /// 12dp — elevación máxima
    pub level5: f64,
}

impl ElevationSystem {
    /// Valores por defecto de Material You
    pub fn default() -> Self {
        ElevationSystem {
            level0: 0.0,
            level1: 1.0,
            level2: 3.0,
            level3: 6.0,
            level4: 8.0,
            level5: 12.0,
        }
    }

    /// Obtiene el valor en dp para un nivel (0-5)
    pub fn get(&self, level: u8) -> f64 {
        match level {
            0 => self.level0,
            1 => self.level1,
            2 => self.level2,
            3 => self.level3,
            4 => self.level4,
            5 => self.level5,
            _ => {
                // Para niveles > 5, extrapolamos
                if level > 5 {
                    self.level5 + (level as f64 - 5.0) * 4.0
                } else {
                    self.level0
                }
            }
        }
    }

    /// Calcula la sombra para un nivel de elevación
    ///
    /// Devuelve un struct Shadow con la configuración de la sombra.
    /// Xilem no tiene sombras nativas, pero esta configuración puede
    /// usarse para renderizar sombras con primitivas gráficas.
    pub fn shadow_for_level(&self, level: u8) -> Shadow {
        let elevation = self.get(level);
        // A mayor elevación, mayor blur y offset
        let offset_y = elevation * 0.5;
        let blur_radius = elevation * 0.75;
        let spread = elevation * 0.1;

        // Opacidad de la sombra: aumenta con elevación
        let alpha = ((elevation / 12.0) * 0.3 + 0.1).min(0.4);
        let alpha_byte = (alpha * 255.0).round() as u8;

        Shadow {
            offset_x: 0.0,
            offset_y,
            blur_radius,
            spread,
            color: (0, 0, 0, alpha_byte),
        }
    }
}

/// Configuración de sombra para un nivel de elevación
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// Desplazamiento horizontal en dp
    pub offset_x: f64,
    /// Desplazamiento vertical en dp
    pub offset_y: f64,
    /// Radio de desenfoque en dp
    pub blur_radius: f64,
    /// Expansión de la sombra en dp
    pub spread: f64,
    /// Color RGBA de la sombra
    pub color: (u8, u8, u8, u8),
}

impl Shadow {
    /// Crea una sombra vacía (sin sombra)
    pub fn none() -> Self {
        Shadow {
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
            spread: 0.0,
            color: (0, 0, 0, 0),
        }
    }

    /// Convierte a tupla de parámetros para renderizado
    pub fn to_params(&self) -> (f64, f64, f64, f64, (u8, u8, u8, u8)) {
        (self.offset_x, self.offset_y, self.blur_radius, self.spread, self.color)
    }
}

/// Calcula el color de superficie con elevación
///
/// En modo oscuro (is_dark = true), la superficie se aclara
/// superponiendo el color primario con opacidad según el nivel.
/// En modo claro, la superficie no cambia.
///
/// # Parámetros
/// - `surface`: color de superficie base
/// - `primary`: color primario para el overlay en dark
/// - `level`: nivel de elevación (0-5)
/// - `is_dark`: si es modo oscuro
pub fn surface_at_elevation(
    surface: RgbColor,
    primary: RgbColor,
    level: u8,
    is_dark: bool,
) -> RgbColor {
    if !is_dark {
        return surface;
    }

    // En dark: overlay de primary al 5-14% según nivel
    let opacity = match level {
        0 => 0.00,
        1 => 0.05,
        2 => 0.08,
        3 => 0.11,
        4 => 0.12,
        5 => 0.14,
        _ => (level as f64 * 0.03).min(0.25),
    };
    surface.blend(&primary, opacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elevation_defaults() {
        let elev = ElevationSystem::default();
        assert!((elev.level0 - 0.0).abs() < 0.01);
        assert!((elev.level3 - 6.0).abs() < 0.01);
        assert!((elev.level5 - 12.0).abs() < 0.01);
    }

    #[test]
    fn test_get() {
        let elev = ElevationSystem::default();
        assert!((elev.get(0) - 0.0).abs() < 0.01);
        assert!((elev.get(3) - 6.0).abs() < 0.01);
        assert!((elev.get(5) - 12.0).abs() < 0.01);
    }

    #[test]
    fn test_shadow_for_level() {
        let elev = ElevationSystem::default();
        let shadow = elev.shadow_for_level(3);
        assert!(shadow.offset_y > 0.0);
        assert!(shadow.blur_radius > 0.0);
        assert!(shadow.color.3 > 0); // alpha > 0
        // Mayor elevación = mayor blur
        let shadow_low = elev.shadow_for_level(0);
        let shadow_high = elev.shadow_for_level(5);
        assert!(shadow_high.blur_radius >= shadow_low.blur_radius);
    }

    #[test]
    fn test_shadow_none() {
        let shadow = Shadow::none();
        assert_eq!(shadow, Shadow {
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
            spread: 0.0,
            color: (0, 0, 0, 0),
        });
    }

    #[test]
    fn test_surface_at_elevation_light() {
        let surface = RgbColor(255, 251, 254);
        let primary = RgbColor(103, 80, 164);
        let result = surface_at_elevation(surface, primary, 3, false);
        assert_eq!(result, surface); // Sin cambio en light
    }

    #[test]
    fn test_surface_at_elevation_dark() {
        let surface = RgbColor(28, 27, 31);
        let primary = RgbColor(208, 188, 255);
        let result = surface_at_elevation(surface, primary, 3, true);
        assert_ne!(result, surface); // Debe cambiar en dark
    }
}
