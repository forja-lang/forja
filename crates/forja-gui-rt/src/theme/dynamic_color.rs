// Dynamic Color — espacio de color HCT (Hue, Chroma, Tone) simplificado
//
// Implementación simplificada del algoritmo HCT basado en CAM16,
// que permite generar colores armónicos dinámicamente (algoritmo Monet).
//
// El espacio HCT separa:
// - Hue (tono): 0-360 grados
// - Chroma (saturación): 0-150+  
// - Tone (luminosidad): 0-100 (negro a blanco)

use crate::theme::color::RgbColor;

/// Espacio de color HCT (Hue, Chroma, Tone) basado en CAM16
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hct {
    pub hue: f64,
    pub chroma: f64,
    pub tone: f64,
}

impl Hct {
    /// Crea un HCT desde un RgbColor usando transformación simplificada
    pub fn from_rgb(rgb: &RgbColor) -> Self {
        // 1. sRGB -> lineal
        let r = srgb_to_linear(rgb.r_f64());
        let g = srgb_to_linear(rgb.g_f64());
        let b = srgb_to_linear(rgb.b_f64());

        // 2. lineal -> XYZ (D65)
        let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
        let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
        let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;

        // 3. XYZ -> Lab (D65)
        let xn = 0.95047;
        let yn = 1.00000;
        let zn = 1.08883;

        let fx = lab_f(x / xn);
        let fy = lab_f(y / yn);
        let fz = lab_f(z / zn);

        let l = 116.0 * fy - 16.0;
        let a = 500.0 * (fx - fy);
        let b_lab = 200.0 * (fy - fz);

        // 4. Lab -> HCT
        let hue = (b_lab.atan2(a) * 180.0 / std::f64::consts::PI).rem_euclid(360.0);
        let chroma = (a * a + b_lab * b_lab).sqrt();
        let tone = l.clamp(0.0, 100.0);

        Hct { hue, chroma, tone }
    }

    /// Convierte HCT a RgbColor
    pub fn to_rgb(&self) -> RgbColor {
        let hue_rad = self.hue * std::f64::consts::PI / 180.0;
        let chroma = self.chroma.max(0.0);
        let tone = self.tone.clamp(0.0, 100.0);

        // Lab -> Lab
        let l = tone;
        let a = chroma * hue_rad.cos();
        let b_lab = chroma * hue_rad.sin();

        // Lab -> XYZ (D65)
        let xn = 0.95047;
        let yn = 1.00000;
        let zn = 1.08883;

        let fy = (l + 16.0) / 116.0;
        let fx = a / 500.0 + fy;
        let fz = fy - b_lab / 200.0;

        let x = lab_f_inv(fx) * xn;
        let y = lab_f_inv(fy) * yn;
        let z = lab_f_inv(fz) * zn;

        // XYZ -> lineal -> sRGB
        let rl =  3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
        let gl = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
        let bl =  0.0556434 * x - 0.2040259 * y + 1.0572252 * z;

        let r = linear_to_srgb(rl);
        let g = linear_to_srgb(gl);
        let b = linear_to_srgb(bl);

        RgbColor(
            (r * 255.0).round().clamp(0.0, 255.0) as u8,
            (g * 255.0).round().clamp(0.0, 255.0) as u8,
            (b * 255.0).round().clamp(0.0, 255.0) as u8,
        )
    }

    /// Crea un HCT desde valores directos de hue, chroma y tone
    pub fn new(hue: f64, chroma: f64, tone: f64) -> Self {
        Hct {
            hue: hue.rem_euclid(360.0),
            chroma: chroma.max(0.0),
            tone: tone.clamp(0.0, 100.0),
        }
    }

    /// Obtiene un nuevo HCT con el tono especificado, manteniendo hue y chroma
    pub fn with_tone(&self, tone: f64) -> Self {
        Hct {
            hue: self.hue,
            chroma: self.chroma,
            tone: tone.clamp(0.0, 100.0),
        }
    }

    /// Crea desde un string hexadecimal
    pub fn from_hex(hex: &str) -> Self {
        let rgb = RgbColor::from_hex(hex).unwrap_or(RgbColor(0, 0, 0));
        Hct::from_rgb(&rgb)
    }

    /// Convierte a string hexadecimal
    pub fn to_hex(&self) -> String {
        self.to_rgb().to_hex()
    }

    /// Calcula la diferencia perceptible entre dos colores HCT
    pub fn distance(&self, other: &Hct) -> f64 {
        let dh = (self.hue - other.hue).abs().min(360.0 - (self.hue - other.hue).abs()) / 360.0;
        let dc = (self.chroma - other.chroma).abs() / 150.0;
        let dt = (self.tone - other.tone).abs() / 100.0;
        (dh * dh + dc * dc + dt * dt).sqrt()
    }
}

impl std::fmt::Display for Hct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hct({:.1}°, {:.1}, {:.1})", self.hue, self.chroma, self.tone)
    }
}

// --- Funciones auxiliares de conversión de color ---

/// Convierte sRGB (gamma) a lineal
fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convierte lineal a sRGB (gamma)
fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Función f para Lab (CIE)
fn lab_f(t: f64) -> f64 {
    let delta: f64 = 6.0 / 29.0;
    if t > delta.powi(3) {
        t.powf(1.0 / 3.0)
    } else {
        t / (3.0 * delta * delta) + 4.0 / 29.0
    }
}

/// Inversa de lab_f
fn lab_f_inv(t: f64) -> f64 {
    let delta: f64 = 6.0 / 29.0;
    if t > delta {
        t.powi(3)
    } else {
        3.0 * delta * delta * (t - 4.0 / 29.0)
    }
}

// --- Algoritmos de armonización ---

/// Armoniza un color de diseño con un color fuente
pub fn harmonize(design_color: &RgbColor, source_color: &RgbColor) -> RgbColor {
    let design_hct = Hct::from_rgb(design_color);
    let source_hct = Hct::from_rgb(source_color);

    // Mezcla los hues manteniendo el chroma y tone del design
    let h_diff = (design_hct.hue - source_hct.hue).abs().min(360.0 - (design_hct.hue - source_hct.hue).abs());
    
    let harmonized_hue = if h_diff > 60.0 {
        // Desplazar el hue hacia el source
        let direction = if (design_hct.hue - source_hct.hue + 360.0).rem_euclid(360.0) > 180.0 {
            -1.0
        } else {
            1.0
        };
        (design_hct.hue + direction * 30.0).rem_euclid(360.0)
    } else {
        design_hct.hue
    };

    Hct::new(harmonized_hue, design_hct.chroma, design_hct.tone).to_rgb()
}

/// Aumenta el chroma para efecto expressive (1.3x por defecto)
pub fn chroma_boost(hct: &Hct, factor: f64) -> Hct {
    Hct::new(hct.hue, (hct.chroma * factor).min(150.0), hct.tone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hct_from_rgb() {
        let rgb = RgbColor(103, 80, 164); // #6750A4 (púrpura)
        let hct = Hct::from_rgb(&rgb);
        // El hue simplificado puede variar; verificamos que esté en rango violeta/púrpura
        assert!(hct.hue > 270.0 && hct.hue < 340.0, "hue = {} (esperado ~270-340)", hct.hue);
        assert!(hct.tone > 30.0 && hct.tone < 60.0, "tone = {}", hct.tone);
    }

    #[test]
    fn test_hct_roundtrip() {
        let original = RgbColor(103, 80, 164);
        let hct = Hct::from_rgb(&original);
        let recovered = hct.to_rgb();
        // Pequeña tolerancia por conversiones
        assert!(original.0.abs_diff(recovered.0) <= 2);
        assert!(original.1.abs_diff(recovered.1) <= 2);
        assert!(original.2.abs_diff(recovered.2) <= 2);
    }

    #[test]
    fn test_with_tone() {
        let hct = Hct::new(270.0, 36.0, 50.0);
        let lighter = hct.with_tone(80.0);
        assert!((lighter.tone - 80.0).abs() < 0.01);
        assert!((lighter.hue - 270.0).abs() < 0.01);
    }

    #[test]
    fn test_hct_black_white() {
        let black = Hct::from_rgb(&RgbColor(0, 0, 0));
        assert!(black.tone < 1.0);
        let white = Hct::from_rgb(&RgbColor(255, 255, 255));
        assert!(white.tone > 99.0);
    }
}
