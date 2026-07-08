// Color — representación RGB y constantes de color Material You
// Define colores en formato RGB con conversiones a hex, HCT y blending.

use crate::theme::dynamic_color::Hct;

/// Color RGB con componentes de 8 bits (0-255)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl RgbColor {
    /// Crea un nuevo RgbColor
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        RgbColor(r, g, b)
    }

    /// Convierte a formato hexadecimal #RRGGBB
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }

    /// Parse un color desde string hexadecimal (#RRGGBB o RRGGBB)
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(RgbColor(r, g, b))
    }

    /// Convierte a espacio de color HCT
    pub fn to_hct(&self) -> Hct {
        Hct::from_rgb(self)
    }

    /// Crea un RgbColor desde valores HCT
    pub fn from_hct(hue: f64, chroma: f64, tone: f64) -> Self {
        Hct::new(hue, chroma, tone).to_rgb()
    }

    /// Componente rojo como f64 (0.0 - 1.0)
    pub fn r_f64(&self) -> f64 {
        self.0 as f64 / 255.0
    }

    /// Componente verde como f64 (0.0 - 1.0)
    pub fn g_f64(&self) -> f64 {
        self.1 as f64 / 255.0
    }

    /// Componente azul como f64 (0.0 - 1.0)
    pub fn b_f64(&self) -> f64 {
        self.2 as f64 / 255.0
    }

    /// Mezcla dos colores con proporción t (0.0 = solo self, 1.0 = solo other)
    pub fn blend(&self, other: &RgbColor, t: f64) -> RgbColor {
        let t = t.clamp(0.0, 1.0);
        let r = (self.0 as f64 * (1.0 - t) + other.0 as f64 * t).round() as u8;
        let g = (self.1 as f64 * (1.0 - t) + other.1 as f64 * t).round() as u8;
        let b = (self.2 as f64 * (1.0 - t) + other.2 as f64 * t).round() as u8;
        RgbColor(r, g, b)
    }

    /// Convierte a tupla (R, G, B)
    pub fn to_tuple(&self) -> (u8, u8, u8) {
        (self.0, self.1, self.2)
    }

    /// Crea un Color (xilem) con el nivel de transparencia especificado
    /// alpha: 0.0 (transparente) a 1.0 (opaco)
    pub fn with_alpha(&self, alpha: f64) -> xilem::Color {
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        xilem::Color::from_rgba8(self.0, self.1, self.2, a)
    }
}

impl std::fmt::Display for RgbColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// --- Conversiones ---

impl From<RgbColor> for xilem::Color {
    fn from(c: RgbColor) -> Self {
        xilem::Color::from_rgb8(c.0, c.1, c.2)
    }
}

impl From<&str> for RgbColor {
    fn from(s: &str) -> Self {
        // Primero intenta como nombre de color
        if let Some(color) = nombre_a_color(s) {
            return color;
        }
        // Si no, intenta como hex
        RgbColor::from_hex(s).unwrap_or(RgbColor(0, 0, 0))
    }
}

impl From<(u8, u8, u8)> for RgbColor {
    fn from(tuple: (u8, u8, u8)) -> Self {
        RgbColor(tuple.0, tuple.1, tuple.2)
    }
}

// --- Constantes de colores Material predefinidos ---

/// Color semilla por defecto (púrpura Material You)
pub const SEED_DEFAULT: &str = "#6750A4";

pub const ROJO: RgbColor = RgbColor(244, 67, 54);
pub const AZUL: RgbColor = RgbColor(33, 150, 243);
pub const VERDE: RgbColor = RgbColor(76, 175, 80);
pub const BLANCO: RgbColor = RgbColor(255, 255, 255);
pub const NEGRO: RgbColor = RgbColor(0, 0, 0);
pub const GRIS: RgbColor = RgbColor(158, 158, 158);
pub const NARANJA: RgbColor = RgbColor(255, 152, 0);
pub const MORADO: RgbColor = RgbColor(156, 39, 176);
pub const AMARILLO: RgbColor = RgbColor(255, 235, 59);
pub const CIAN: RgbColor = RgbColor(0, 188, 212);
pub const ROSA: RgbColor = RgbColor(233, 30, 99);
pub const AZUL_MARINO: RgbColor = RgbColor(25, 25, 112);
pub const PLATEADO: RgbColor = RgbColor(192, 192, 192);
pub const MARRON: RgbColor = RgbColor(121, 85, 72);

/// Convierte un nombre de color en español a RgbColor
pub fn nombre_a_color(nombre: &str) -> Option<RgbColor> {
    match nombre.to_lowercase().trim() {
        "rojo" => Some(ROJO),
        "azul" => Some(AZUL),
        "verde" => Some(VERDE),
        "blanco" => Some(BLANCO),
        "negro" => Some(NEGRO),
        "gris" => Some(GRIS),
        "naranja" | "anaranjado" => Some(NARANJA),
        "morado" | "purpura" | "púrpura" | "violeta" => Some(MORADO),
        "amarillo" => Some(AMARILLO),
        "cian" | "ciano" | "turquesa" => Some(CIAN),
        "rosa" | "rosado" => Some(ROSA),
        "azul marino" | "azul_marino" | "marino" => Some(AZUL_MARINO),
        "plateado" | "plata" => Some(PLATEADO),
        "marron" | "marrón" | "cafe" | "café" => Some(MARRON),
        _ => None,
    }
}

/// Mezcla dos colores con proporción t (0.0 = a, 1.0 = b)
pub fn blend_colors(a: RgbColor, b: RgbColor, t: f64) -> RgbColor {
    a.blend(&b, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_conversion() {
        let c = RgbColor::from_hex("#FF5733").unwrap();
        assert_eq!(c.0, 255);
        assert_eq!(c.1, 87);
        assert_eq!(c.2, 51);
        assert_eq!(c.to_hex(), "#FF5733");
    }

    #[test]
    fn test_hex_without_hash() {
        let c = RgbColor::from_hex("AABBCC").unwrap();
        assert_eq!(c.to_hex(), "#AABBCC");
    }

    #[test]
    fn test_blend() {
        let a = RgbColor(255, 0, 0);
        let b = RgbColor(0, 255, 0);
        let blended = blend_colors(a, b, 0.5);
        assert_eq!(blended.0, 128);
        assert_eq!(blended.1, 128);
    }

    #[test]
    fn test_nombre_a_color() {
        assert_eq!(nombre_a_color("rojo"), Some(ROJO));
        assert_eq!(nombre_a_color("Rojo"), Some(ROJO));
        assert_eq!(nombre_a_color("verde"), Some(VERDE));
        assert_eq!(nombre_a_color("inexistente"), None);
    }

    #[test]
    fn test_from_str() {
        let c: RgbColor = "#FF0000".into();
        assert_eq!(c, RgbColor(255, 0, 0));
        let c: RgbColor = "rojo".into();
        assert_eq!(c, ROJO);
        // Hex sin hash
        let c: RgbColor = "00FF00".into();
        assert_eq!(c, RgbColor(0, 255, 0));
    }

    #[test]
    fn test_display() {
        let c = RgbColor(103, 80, 164);
        assert_eq!(format!("{}", c), "#6750A4");
    }
}
