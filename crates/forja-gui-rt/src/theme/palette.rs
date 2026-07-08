// Palette — paletas tonales de Material You (TonalPalette)
//
// Genera paletas de 13 tonos desde un color semilla, siguiendo el
// algoritmo simplificado de Material You (Monet).

use crate::theme::color::RgbColor;
use crate::theme::dynamic_color::Hct;

/// Los 13 tonos estándar de Material You
pub const TONES: [u8; 13] = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 95, 99, 100];

/// Paleta tonal: 13 tonos de un mismo matiz
#[derive(Clone, Debug)]
pub struct TonalPalette {
    pub hue: f64,
    pub chroma: f64,
    pub tones: [RgbColor; 13],
}

impl TonalPalette {
    /// Crea una paleta tonal desde valores HCT
    pub fn from_hct(hue: f64, chroma: f64) -> Self {
        let mut tones = [RgbColor(0, 0, 0); 13];
        for (i, tone_val) in TONES.iter().enumerate() {
            let hct = Hct::new(hue, chroma, *tone_val as f64);
            tones[i] = hct.to_rgb();
        }
        TonalPalette { hue, chroma, tones }
    }

    /// Obtiene el color para un tono específico (0-100)
    pub fn tone(&self, tone: u8) -> RgbColor {
        // Busca el tono más cercano
        let mut best_idx = 0;
        let mut best_dist = i16::MAX;
        for (i, t) in TONES.iter().enumerate() {
            let dist = ((*t as i16) - (tone as i16)).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }
        self.tones[best_idx]
    }

    /// Obtiene todos los colores de la paleta
    pub fn all_tones(&self) -> &[RgbColor; 13] {
        &self.tones
    }
}

impl std::fmt::Display for TonalPalette {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TonalPalette(hue={:.1}°, chroma={:.1})", self.hue, self.chroma)
    }
}

/// Conjunto de las 5 paletas tonales de Material You
#[derive(Clone, Debug)]
pub struct Palettes {
    pub primary: TonalPalette,
    pub secondary: TonalPalette,
    pub tertiary: TonalPalette,
    pub neutral: TonalPalette,
    pub neutral_variant: TonalPalette,
}

impl Palettes {
    /// Genera las 5 paletas desde un color semilla
    ///
    /// Algoritmo simplificado de Material You (Monet):
    /// - Primary: hue del seed, chroma = max(36, seed_chroma)
    /// - Secondary: hue = seed_hue + 60, chroma = 16
    /// - Tertiary: hue = seed_hue + 180, chroma = 24
    /// - Neutral: hue = seed_hue, chroma = 4
    /// - Neutral Variant: hue = seed_hue, chroma = 8
    pub fn from_seed(seed: &RgbColor) -> Self {
        let hct = Hct::from_rgb(seed);

        // Extraer valores del seed
        let seed_hue = hct.hue;
        let seed_chroma = hct.chroma;

        // Primary: mantiene el hue del seed, chroma mínimo 36
        let primary_chroma = seed_chroma.max(36.0);
        let primary = TonalPalette::from_hct(seed_hue, primary_chroma);

        // Secondary: hue desplazado +60, chroma fijo 16
        let secondary_hue = (seed_hue + 60.0).rem_euclid(360.0);
        let secondary = TonalPalette::from_hct(secondary_hue, 16.0);

        // Tertiary: hue complementario (+180), chroma 24
        let tertiary_hue = (seed_hue + 180.0).rem_euclid(360.0);
        let tertiary = TonalPalette::from_hct(tertiary_hue, 24.0);

        // Neutral: mismo hue, chroma muy bajo
        let neutral = TonalPalette::from_hct(seed_hue, 4.0);

        // Neutral Variant: mismo hue, chroma bajo
        let neutral_variant = TonalPalette::from_hct(seed_hue, 8.0);

        Palettes {
            primary,
            secondary,
            tertiary,
            neutral,
            neutral_variant,
        }
    }

    /// Genera paletas desde un string hex
    pub fn from_seed_hex(hex: &str) -> Self {
        let seed = RgbColor::from_hex(hex).unwrap_or(RgbColor(103, 80, 164));
        Palettes::from_seed(&seed)
    }
}

impl std::fmt::Display for Palettes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Palettes(primary={}, secondary={}, tertiary={})",
            self.primary, self.secondary, self.tertiary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tonal_palette_has_13_tones() {
        let palette = TonalPalette::from_hct(270.0, 36.0);
        assert_eq!(palette.tones.len(), 13);
    }

    #[test]
    fn test_tonal_palette_tone_values() {
        let palette = TonalPalette::from_hct(270.0, 36.0);
        // Tone 100 debe ser muy claro (casi blanco)
        let white_tone = palette.tone(100);
        // El brillo percibido debe ser alto
        let brightness = (white_tone.0 as u16 + white_tone.1 as u16 + white_tone.2 as u16) / 3;
        assert!(brightness > 200, "brightness = {} para tone 100", brightness);

        // Tone 0 debe ser muy oscuro (casi negro)
        let black_tone = palette.tone(0);
        let brightness = (black_tone.0 as u16 + black_tone.1 as u16 + black_tone.2 as u16) / 3;
        assert!(brightness < 30, "brightness = {} para tone 0", brightness);
    }

    #[test]
    fn test_palettes_from_seed() {
        let seed = RgbColor(103, 80, 164); // #6750A4
        let palettes = Palettes::from_seed(&seed);

        // Primary debe tener chroma >= 36
        assert!(palettes.primary.chroma >= 36.0);

        // Secondary debe tener hue diferente al primary
        assert!((palettes.secondary.hue - palettes.primary.hue).abs() > 30.0);

        // Neutral debe tener chroma bajo
        assert!(palettes.neutral.chroma < 10.0);
    }

    #[test]
    fn test_palettes_roundtrip() {
        let seed = RgbColor::from_hex("#FF5722").unwrap();
        let palettes = Palettes::from_seed(&seed);

        // Verificar que primary tiene tono 40 razonable
        let primary_40 = palettes.primary.tone(40);
        assert!(primary_40.0 > 0 || primary_40.1 > 0 || primary_40.2 > 0);
    }
}
