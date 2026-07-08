// Motion — sistema de movimiento Material You
//
// Define duraciones y curvas de easing para animaciones
// siguiendo Material Design 3.

/// Curva de easing como cubic-bezier (x1, y1, x2, y2)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EasingCurve(pub f64, pub f64, pub f64, pub f64);

impl EasingCurve {
    /// Aplica la curva de easing a un valor t (0-1)
    ///
    /// Usa interpolación cúbica de Bézier para transformar
    /// un progreso lineal en uno con aceleración/desaceleración.
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);

        // Aproximación usando curva cúbica de Bézier
        // Para simplificar, usamos una aproximación polinómica
        let (x1, y1, x2, y2) = (self.0, self.1, self.2, self.3);

        // Newton-Raphson para encontrar t dado x
        let mut guess = t;
        for _ in 0..8 {
            let x = cubic_bezier_x(x1, x2, guess);
            let dx = cubic_bezier_dx(x1, x2, guess);
            if dx.abs() < 1e-10 {
                break;
            }
            guess = (guess - (x - t) / dx).clamp(0.0, 1.0);
        }

        // Evaluar y en el t encontrado
        cubic_bezier_y(y1, y2, guess)
    }

    /// Crea una curva de easing desde valores
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        EasingCurve(x1.clamp(0.0, 1.0), y1, x2.clamp(0.0, 1.0), y2)
    }
}

/// Componente x de la curva cúbica de Bézier
fn cubic_bezier_x(x1: f64, x2: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * t * x1 + 3.0 * mt * t2 * x2 + t3
}

/// Derivada del componente x
fn cubic_bezier_dx(x1: f64, x2: f64, t: f64) -> f64 {
    let t2 = t * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * x1 + 6.0 * mt * t * (x2 - x1) + 3.0 * t2 * (1.0 - x2)
}

/// Componente y de la curva cúbica de Bézier
fn cubic_bezier_y(y1: f64, y2: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * t * y1 + 3.0 * mt * t2 * y2 + t3
}

// --- Curvas de easing predefinidas de Material You ---

/// Standard: (0.2, 0.0, 0.0, 1.0) — para animaciones generales
pub const EASE_STANDARD: EasingCurve = EasingCurve(0.2, 0.0, 0.0, 1.0);

/// Emphasized: (0.2, 0.0, 0.0, 1.0) — para animaciones destacadas
pub const EASE_EMPHASIZED: EasingCurve = EasingCurve(0.2, 0.0, 0.0, 1.0);

/// Decelerate: (0.0, 0.0, 0.0, 1.0) — para entradas
pub const EASE_DECELERATE: EasingCurve = EasingCurve(0.0, 0.0, 0.0, 1.0);

/// Accelerate: (0.3, 0.0, 1.0, 1.0) — para salidas
pub const EASE_ACCELERATE: EasingCurve = EasingCurve(0.3, 0.0, 1.0, 1.0);

/// Expressive: (0.34, 1.56, 0.64, 1.0) — con overshoot
pub const EASE_EXPRESSIVE: EasingCurve = EasingCurve(0.34, 1.56, 0.64, 1.0);

/// Duración estándar para animaciones (en milisegundos)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Durations {
    /// 50ms — micro-interacciones
    pub duration_50: f64,
    /// 100ms — hover, ripple
    pub duration_100: f64,
    /// 200ms — estándar corto
    pub duration_200: f64,
    /// 250ms — decelerado
    pub duration_250: f64,
    /// 300ms — estándar
    pub duration_300: f64,
    /// 350ms — container transform
    pub duration_350: f64,
    /// 400ms — transición media
    pub duration_400: f64,
    /// 450ms — emphasized
    pub duration_450: f64,
    /// 500ms — transición larga
    pub duration_500: f64,
}

impl Durations {
    /// Duraciones por defecto de Material You
    pub fn default() -> Self {
        Durations {
            duration_50: 50.0,
            duration_100: 100.0,
            duration_200: 200.0,
            duration_250: 250.0,
            duration_300: 300.0,
            duration_350: 350.0,
            duration_400: 400.0,
            duration_450: 450.0,
            duration_500: 500.0,
        }
    }

    /// Obtiene una duración por nombre
    pub fn get(&self, name: &str) -> f64 {
        match name {
            "50" | "micro" => self.duration_50,
            "100" | "hover" | "ripple" => self.duration_100,
            "200" | "short" => self.duration_200,
            "250" | "decelerate" => self.duration_250,
            "300" | "standard" => self.duration_300,
            "350" | "container" => self.duration_350,
            "400" | "medium" => self.duration_400,
            "450" | "emphasized" => self.duration_450,
            "500" | "long" => self.duration_500,
            _ => self.duration_300,
        }
    }
}

/// Conjunto de curvas de easing de Material You
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Easings {
    /// Standard: entrada/salida suave
    pub standard: EasingCurve,
    /// Emphasized: más dramático, con duración 1.5x
    pub emphasized: EasingCurve,
    /// Decelerate: solo desaceleración (para entradas)
    pub decelerate: EasingCurve,
    /// Accelerate: solo aceleración (para salidas)
    pub accelerate: EasingCurve,
    /// Expressive: con overshoot (Material You Expressive)
    pub expressive: EasingCurve,
}

impl Easings {
    /// Curvas de easing por defecto de Material You
    pub fn default() -> Self {
        Easings {
            standard: EASE_STANDARD,
            emphasized: EASE_EMPHASIZED,
            decelerate: EASE_DECELERATE,
            accelerate: EASE_ACCELERATE,
            expressive: EASE_EXPRESSIVE,
        }
    }

    /// Obtiene una curva de easing por nombre
    pub fn get(&self, name: &str) -> &EasingCurve {
        match name {
            "standard" => &self.standard,
            "emphasized" => &self.emphasized,
            "decelerate" => &self.decelerate,
            "accelerate" => &self.accelerate,
            "expressive" => &self.expressive,
            _ => &self.standard,
        }
    }
}

/// Sistema completo de movimiento Material You
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSystem {
    /// Duraciones predefinidas
    pub durations: Durations,
    /// Curvas de easing predefinidas
    pub easings: Easings,
}

impl MotionSystem {
    /// Sistema de movimiento por defecto
    pub fn default() -> Self {
        MotionSystem {
            durations: Durations::default(),
            easings: Easings::default(),
        }
    }

    /// Sistema de movimiento con easing expresivo (overshoot)
    pub fn expressive() -> Self {
        MotionSystem {
            durations: Durations::default(),
            easings: Easings {
                standard: EASE_EXPRESSIVE,
                emphasized: EASE_EXPRESSIVE,
                decelerate: EASE_DECELERATE,
                accelerate: EASE_ACCELERATE,
                expressive: EASE_EXPRESSIVE,
            },
        }
    }

    /// Obtiene la duración para un nombre de transición
    pub fn duration_for(&self, name: &str) -> f64 {
        self.durations.get(name)
    }

    /// Obtiene la curva de easing para un nombre
    pub fn ease_for(&self, name: &str) -> &EasingCurve {
        self.easings.get(name)
    }
}

/// Tipo de transición entre vistas
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransitionType {
    /// Fundido
    Fade,
    /// Eje compartido X (deslizar horizontal)
    SharedAxisX,
    /// Eje compartido Y (deslizar vertical)
    SharedAxisY,
    /// Eje compartido Z (escala)
    SharedAxisZ,
    /// Fundido a través (fade through)
    FadeThrough,
    /// Transformación de contenedor
    ContainerTransform,
}

impl TransitionType {
    /// Obtiene las configuraciones recomendadas para cada tipo
    pub fn duration(&self) -> f64 {
        match self {
            TransitionType::Fade => 200.0,
            TransitionType::SharedAxisX => 300.0,
            TransitionType::SharedAxisY => 300.0,
            TransitionType::SharedAxisZ => 300.0,
            TransitionType::FadeThrough => 350.0,
            TransitionType::ContainerTransform => 350.0,
        }
    }

    /// Easing recomendado para cada tipo
    pub fn easing(&self) -> EasingCurve {
        match self {
            TransitionType::Fade => EASE_STANDARD,
            TransitionType::SharedAxisX => EASE_STANDARD,
            TransitionType::SharedAxisY => EASE_STANDARD,
            TransitionType::SharedAxisZ => EASE_STANDARD,
            TransitionType::FadeThrough => EASE_EMPHASIZED,
            TransitionType::ContainerTransform => EASE_EMPHASIZED,
        }
    }

    /// Nombre descriptivo
    pub fn name(&self) -> &'static str {
        match self {
            TransitionType::Fade => "Fade",
            TransitionType::SharedAxisX => "SharedAxisX",
            TransitionType::SharedAxisY => "SharedAxisY",
            TransitionType::SharedAxisZ => "SharedAxisZ",
            TransitionType::FadeThrough => "FadeThrough",
            TransitionType::ContainerTransform => "ContainerTransform",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motion_default() {
        let motion = MotionSystem::default();
        assert!((motion.durations.duration_300 - 300.0).abs() < 0.01);
        assert!((motion.durations.duration_100 - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_easing_apply() {
        // Standard: t debería estar entre 0 y 1
        let result = EASE_STANDARD.apply(0.5);
        assert!(result >= 0.0 && result <= 1.0, "result = {}", result);

        // Decelerate: t=1.0 debe dar 1.0
        let result = EASE_DECELERATE.apply(1.0);
        assert!((result - 1.0).abs() < 0.01);

        // t=0 debe dar 0
        let result = EASE_STANDARD.apply(0.0);
        assert!((result - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_duration_get() {
        let durations = Durations::default();
        assert!((durations.get("300") - 300.0).abs() < 0.01);
        assert!((durations.get("standard") - 300.0).abs() < 0.01);
        assert!((durations.get("hover") - 100.0).abs() < 0.01);
        // Nombre desconocido devuelve 300
        assert!((durations.get("nonexistent") - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_easing_get() {
        let easings = Easings::default();
        assert_eq!(*easings.get("standard"), EASE_STANDARD);
        assert_eq!(*easings.get("expressive"), EASE_EXPRESSIVE);
        assert_eq!(*easings.get("unknown"), EASE_STANDARD); // default
    }

    #[test]
    fn test_transition_type() {
        assert!((TransitionType::Fade.duration() - 200.0).abs() < 0.01);
        assert!((TransitionType::ContainerTransform.duration() - 350.0).abs() < 0.01);
        assert_eq!(TransitionType::Fade.name(), "Fade");
    }

    #[test]
    fn test_motion_expressive() {
        let motion = MotionSystem::expressive();
        assert_eq!(motion.easings.standard, EASE_EXPRESSIVE);
        assert_eq!(motion.easings.emphasized, EASE_EXPRESSIVE);
    }
}
