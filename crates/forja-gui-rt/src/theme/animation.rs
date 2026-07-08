// Animation — motor de animaciones con easing curves Material You
//
// Proporciona un sistema completo para interpolar valores (f64, color)
// usando las curvas de easing definidas en motion.rs. Incluye:
// - AnimationEngine: motor frame-by-frame que gestiona animaciones activas
// - AnimatedValue: interpolación desde→hasta con curva easing
// - SpringAnimation: simulación física de resorte
// - AnimationPresets: configuraciones predefinidas para componentes Material You
// - interpolate_color: interpola entre dos colores RGB

use crate::theme::color::RgbColor;
use crate::theme::motion::EasingCurve;
use crate::theme::motion::Easings;
use std::time::Instant;

/// Motor de animaciones con frame-by-frame timing
///
/// Gestiona el ciclo de vida de todas las animaciones activas:
/// 1. Llama a [`begin_frame`] al inicio de cada frame de render
/// 2. Actualiza todas las animaciones con el delta time
/// 3. Elimina automáticamente las animaciones completadas
///
/// # Ejemplo
/// ```ignore
/// let mut engine = AnimationEngine::new();
/// engine.add_animation(Box::new(
///     AnimatedValue::new(0.0, 100.0, 500.0)
///         .with_curve(Easings::default().emphasized)
/// ));
///
/// // En cada frame:
/// engine.begin_frame();
/// ```
pub struct AnimationEngine {
    /// Instante del último frame renderizado
    pub last_frame: Instant,
    /// Tiempo transcurrido desde el último frame (en milisegundos)
    pub delta_ms: f64,
    /// Lista de animaciones activas
    pub animations: Vec<Box<dyn Animation>>,
}

impl AnimationEngine {
    /// Crea un nuevo motor de animaciones
    pub fn new() -> Self {
        Self {
            last_frame: Instant::now(),
            delta_ms: 0.0,
            animations: Vec::new(),
        }
    }

    /// Llama al inicio de cada frame de render
    ///
    /// Calcula el delta time desde el último frame, actualiza todas las
    /// animaciones activas y elimina aquellas que ya completaron su ciclo.
    pub fn begin_frame(&mut self) {
        let now = Instant::now();
        self.delta_ms = now.duration_since(self.last_frame).as_secs_f64() * 1000.0;
        self.last_frame = now;

        // Actualizar todas las animaciones activas
        for anim in &mut self.animations {
            anim.update(self.delta_ms);
        }
        // Eliminar animaciones completadas
        self.animations.retain(|a| !a.is_finished());
    }

    /// Agrega una nueva animación al motor
    pub fn add_animation(&mut self, anim: Box<dyn Animation>) {
        self.animations.push(anim);
    }

    /// Reinicia el timer interno (útil al pausar/reanudar)
    pub fn reset_timer(&mut self) {
        self.last_frame = Instant::now();
    }
}

impl Default for AnimationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait para una animación individual
///
/// Cualquier tipo que implemente este trait puede ser agregado al
/// [`AnimationEngine`] para ser actualizado automáticamente en cada frame.
pub trait Animation: Send {
    /// Actualiza el estado interno con el delta time en milisegundos
    fn update(&mut self, delta_ms: f64);
    /// Indica si la animación ha finalizado
    fn is_finished(&self) -> bool;
    /// Valor actual interpolado de la animación
    fn current_value(&self) -> f64;
}

// ──────────────────────────────────────────────
//  AnimatedValue
// ──────────────────────────────────────────────

/// Valor que se anima interpolando entre `desde` → `hasta` con una curva easing
///
/// Es la animación más común: toma un valor inicial, un valor final, una
/// duración y una curva de easing, e interpola suavemente entre ambos.
///
/// # Modos
/// - **Normal**: anima una vez de `desde` a `hasta` y se detiene
/// - **Loop**: reinicia la animación al llegar al final (para spinners)
/// - **Yoyo**: invierte la dirección al llegar al final (va y vuelve)
///
/// # Ejemplo
/// ```ignore
/// let mut anim = AnimatedValue::new(0.0, 100.0, 300.0)
///     .with_curve(Easings::default().emphasized);
/// anim.update(16.0); // avanza 16ms (~60fps)
/// println!("{}", anim.valor_actual);
/// ```
pub struct AnimatedValue {
    /// Valor inicial de la interpolación
    pub desde: f64,
    /// Valor final de la interpolación
    pub hasta: f64,
    /// Duración total de la animación en milisegundos
    pub duracion_ms: f64,
    /// Progreso actual normalizado (0.0 a 1.0)
    pub progreso: f64,
    /// Indica si la animación está en ejecución
    pub jugando: bool,
    /// Curva de easing que transforma el progreso lineal
    pub curva: EasingCurve,
    /// Valor actual interpolado (listo para ser leído en cada frame)
    pub valor_actual: f64,
    /// Si es `true`, la animación se repite infinitamente
    pub loop_: bool,
    /// Si es `true`, invierte dirección al llegar al final (va y vuelve)
    pub yoyo: bool,
}

impl AnimatedValue {
    /// Crea una nueva animación entre `desde` y `hasta` con la duración dada
    ///
    /// # Argumentos
    /// - `desde`: valor inicial
    /// - `hasta`: valor final
    /// - `duracion_ms`: duración en milisegundos
    ///
    /// Usa la curva estándar (standard) de Material You por defecto.
    pub fn new(desde: f64, hasta: f64, duracion_ms: f64) -> Self {
        Self {
            desde,
            hasta,
            duracion_ms,
            progreso: 0.0,
            jugando: true,
            curva: EasingCurve(0.2, 0.0, 0.0, 1.0), // standard
            valor_actual: desde,
            loop_: false,
            yoyo: false,
        }
    }

    /// Establece la curva de easing para la animación
    pub fn with_curve(mut self, curva: EasingCurve) -> Self {
        self.curva = curva;
        self
    }

    /// Activa el modo loop infinito (para spinners, barras de carga)
    pub fn with_loop(mut self) -> Self {
        self.loop_ = true;
        self
    }

    /// Activa el modo yoyo: va y vuelve entre `desde` y `hasta`
    pub fn with_yoyo(mut self) -> Self {
        self.yoyo = true;
        self
    }

    /// Inicia o reanuda la animación
    pub fn play(&mut self) {
        self.jugando = true;
    }

    /// Pausa la animación (mantiene la posición actual)
    pub fn pause(&mut self) {
        self.jugando = false;
    }

    /// Reinicia la animación al valor inicial
    pub fn reset(&mut self) {
        self.progreso = 0.0;
        self.valor_actual = self.desde;
    }

    /// Obtiene el valor actual interpolado aplicando la curva de easing
    ///
    /// Transforma el progreso lineal usando la curva cubic-bezier
    /// y luego interpola linealmente entre `desde` y `hasta`.
    pub fn valor_interpolado(&self) -> f64 {
        let t = self.curva.apply(self.progreso.clamp(0.0, 1.0));
        self.desde + (self.hasta - self.desde) * t
    }

    /// Actualiza la animación con el delta time en milisegundos
    ///
    /// Avanza el progreso según el tiempo transcurrido y actualiza
    /// el valor interpolado. Si la animación completa un ciclo y
    /// tiene modo loop o yoyo, se reinicia apropiadamente.
    pub fn update(&mut self, delta_ms: f64) {
        if !self.jugando || self.duracion_ms <= 0.0 {
            return;
        }

        self.progreso += delta_ms / self.duracion_ms;

        if self.progreso >= 1.0 {
            if self.loop_ {
                if self.yoyo {
                    // Invertir dirección: intercambiar desde ↔ hasta
                    std::mem::swap(&mut self.desde, &mut self.hasta);
                }
                self.progreso = 0.0;
            } else {
                self.progreso = 1.0;
                self.jugando = false;
            }
        }

        self.valor_actual = self.valor_interpolado();
    }
}

impl Animation for AnimatedValue {
    fn update(&mut self, delta_ms: f64) {
        self.update(delta_ms);
    }

    fn is_finished(&self) -> bool {
        !self.jugando && self.progreso >= 1.0
    }

    fn current_value(&self) -> f64 {
        self.valor_actual
    }
}

// ──────────────────────────────────────────────
//  SpringAnimation
// ──────────────────────────────────────────────

/// Animación con física de resorte (spring)
///
/// Simula un sistema masa-resorte-amortiguador para lograr movimientos
/// naturales con overshoot y settling. Ideal para animaciones orgánicas
/// que necesitan sensación física real.
///
/// # Parámetros
/// - **masa**: masa del objeto (1.0 por defecto). Mayor masa = más lentitud.
/// - **rigidez**: constante del resorte (100-300, típico 180). Mayor rigidez = más velocidad.
/// - **amortiguacion**: coeficiente de amortiguación (10-30, típico 18). Mayor amortiguación = menos rebote.
///
/// # Ejemplo
/// ```ignore
/// let mut spring = SpringAnimation::new(100.0);
/// spring.posicion = 0.0;
/// loop {
///     spring.update(16.0);
///     if spring.is_settled() { break; }
///     println!("Posición: {}", spring.posicion);
/// }
/// ```
pub struct SpringAnimation {
    /// Masa del objeto (1.0 por defecto)
    pub masa: f64,
    /// Constante del resorte (100-300, 180 típico)
    pub rigidez: f64,
    /// Coeficiente de amortiguación (10-30, 18 típico)
    pub amortiguacion: f64,
    /// Velocidad actual del movimiento
    pub velocidad: f64,
    /// Posición actual del objeto
    pub posicion: f64,
    /// Posición objetivo (hacia donde se dirige el resorte)
    pub objetivo: f64,
    /// Tolerancia para considerar que se ha estabilizado (0.01)
    pub tolerancia: f64,
}

impl SpringAnimation {
    /// Crea una nueva animación spring hacia el objetivo dado
    ///
    /// Usa valores típicos de Material You:
    /// - masa: 1.0
    /// - rigidez: 180.0
    /// - amortiguacion: 18.0
    pub fn new(objetivo: f64) -> Self {
        Self {
            masa: 1.0,
            rigidez: 180.0,
            amortiguacion: 18.0,
            velocidad: 0.0,
            posicion: 0.0,
            objetivo,
            tolerancia: 0.01,
        }
    }

    /// Configura la masa del sistema
    pub fn with_masa(mut self, masa: f64) -> Self {
        self.masa = masa;
        self
    }

    /// Configura la rigidez del resorte
    pub fn with_rigidez(mut self, rigidez: f64) -> Self {
        self.rigidez = rigidez;
        self
    }

    /// Configura la amortiguación del sistema
    pub fn with_amortiguacion(mut self, amortiguacion: f64) -> Self {
        self.amortiguacion = amortiguacion;
        self
    }

    /// Cambia el objetivo del resorte (crea una nueva tensión)
    pub fn set_objetivo(&mut self, objetivo: f64) {
        self.objetivo = objetivo;
    }

    /// Actualiza la simulación física con el delta time en milisegundos
    ///
    /// Calcula la fuerza del resorte (ley de Hooke), aplica
    /// amortiguación y resuelve la aceleración para obtener
    /// nueva velocidad y posición.
    pub fn update(&mut self, delta_ms: f64) {
        let dt = delta_ms / 1000.0; // convertir a segundos
        let fuerza = -self.rigidez * (self.posicion - self.objetivo);
        let amort = -self.amortiguacion * self.velocidad;
        let aceleracion = (fuerza + amort) / self.masa;
        self.velocidad += aceleracion * dt;
        self.posicion += self.velocidad * dt;
    }

    /// Indica si el resorte se ha estabilizado (dentro de la tolerancia)
    ///
    /// Se considera estabilizado cuando la posición está cerca del
    /// objetivo Y la velocidad es casi cero.
    pub fn is_settled(&self) -> bool {
        (self.posicion - self.objetivo).abs() < self.tolerancia
            && self.velocidad.abs() < self.tolerancia
    }
}

// ──────────────────────────────────────────────
//  AnimationPresets — Material You presets
// ──────────────────────────────────────────────

/// Presets de animación para componentes Material You
///
/// Proporciona configuraciones predefinidas de [`AnimatedValue`]
/// para los componentes comunes de Material Design 3, con las
/// curvas de easing y duraciones recomendadas.
///
/// # Ejemplo
/// ```ignore
/// let ripple = AnimationPresets::button_ripple();
/// let hover = AnimationPresets::button_hover();
/// let spin = AnimationPresets::spinner_loop();
/// ```
pub struct AnimationPresets;

impl AnimationPresets {
    /// Botón: efecto ripple (150ms, curva emphasized)
    ///
    /// Usado para la onda expansiva al hacer clic en un botón.
    pub fn button_ripple() -> AnimatedValue {
        AnimatedValue::new(0.0, 1.0, 150.0)
            .with_curve(Easings::default().emphasized)
    }

    /// Botón: hover (100ms, curva standard)
    ///
    /// Usado para el cambio de elevación/color al pasar el mouse.
    pub fn button_hover() -> AnimatedValue {
        AnimatedValue::new(0.0, 1.0, 100.0)
            .with_curve(Easings::default().standard)
    }

    /// Card: elevación (200ms, curva standard)
    ///
    /// Usado para la sombra de elevación al hacer hover en una tarjeta.
    pub fn card_elevate() -> AnimatedValue {
        AnimatedValue::new(0.0, 1.0, 200.0)
            .with_curve(Easings::default().standard)
    }

    /// Transición de página (300ms, curva emphasized)
    ///
    /// Usado para animaciones entre pantallas o vistas completas.
    pub fn page_transition() -> AnimatedValue {
        AnimatedValue::new(0.0, 1.0, 300.0)
            .with_curve(Easings::default().emphasized)
    }

    /// Container transform (350ms, curva emphasized)
    ///
    /// Usado para la transición de container transform de Material You,
    /// donde un elemento se expande/transforma en otro.
    pub fn container_transform() -> AnimatedValue {
        AnimatedValue::new(0.0, 1.0, 350.0)
            .with_curve(Easings::default().emphasized)
    }

    /// Spinner: loop infinito (800ms por ciclo, curva standard)
    ///
    /// Anima un valor de 0° a 360° en bucle infinito para indicadores
    /// de carga rotatorios.
    pub fn spinner_loop() -> AnimatedValue {
        AnimatedValue::new(0.0, 360.0, 800.0)
            .with_curve(Easings::default().standard)
            .with_loop()
    }

    /// Expressive: fade (450ms, curva expressive con overshoot)
    ///
    /// Fundido de entrada/salida con estilo expresivo, adecuado para
    /// animaciones decorativas o de bienvenida.
    pub fn expressive_fade() -> AnimatedValue {
        AnimatedValue::new(0.0, 1.0, 450.0)
            .with_curve(Easings::default().expressive)
    }

    /// Expressive: morph (500ms, curva expressive con overshoot)
    ///
    /// Transición de morphing entre formas con estilo expresivo,
    /// usando overshoot para un efecto más dramático.
    pub fn expressive_morph() -> AnimatedValue {
        AnimatedValue::new(0.0, 1.0, 500.0)
            .with_curve(Easings::default().expressive)
    }
}

// ──────────────────────────────────────────────
//  interpolate_color
// ──────────────────────────────────────────────

/// Interpola linealmente entre dos colores RGB usando un factor `t` (0.0 a 1.0)
///
/// # Argumentos
/// - `a`: color inicial (t = 0.0)
/// - `b`: color final (t = 1.0)
/// - `t`: factor de interpolación (0.0 = solo `a`, 1.0 = solo `b`)
///
/// # Devuelve
/// Un nuevo [`RgbColor`] interpolado entre `a` y `b`.
///
/// # Ejemplo
/// ```
/// use forja_gui_rt::theme::color::RgbColor;
/// use forja_gui_rt::theme::animation::interpolate_color;
///
/// let negro = RgbColor(0, 0, 0);
/// let blanco = RgbColor(255, 255, 255);
/// let gris = interpolate_color(negro, blanco, 0.5);
/// assert_eq!(gris, RgbColor(127, 127, 127));
/// ```
pub fn interpolate_color(a: RgbColor, b: RgbColor, t: f64) -> RgbColor {
    let t = t.clamp(0.0, 1.0);
    RgbColor(
        (a.0 as f64 + (b.0 as f64 - a.0 as f64) * t) as u8,
        (a.1 as f64 + (b.1 as f64 - a.1 as f64) * t) as u8,
        (a.2 as f64 + (b.2 as f64 - a.2 as f64) * t) as u8,
    )
}

// También re-exportamos blend_colors de color.rs como alias
// para mantener compatibilidad semántica
pub use crate::theme::color::blend_colors;

// ──────────────────────────────────────────────
//  Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animated_value_interpolation() {
        let mut anim = AnimatedValue::new(0.0, 100.0, 1000.0);
        assert_eq!(anim.valor_actual, 0.0);
        anim.update(500.0); // 50% del tiempo
        assert!(
            anim.valor_actual > 0.0 && anim.valor_actual < 100.0,
            "valor_actual = {}",
            anim.valor_actual
        );
        anim.update(500.0); // 100%
        assert!(
            (anim.valor_actual - 100.0).abs() < 0.01,
            "valor_actual = {}",
            anim.valor_actual
        );
    }

    #[test]
    fn test_animated_valor_interpolado() {
        let anim = AnimatedValue::new(0.0, 100.0, 1000.0);
        // Al inicio, t=0 → valor = desde
        assert!((anim.valor_interpolado() - 0.0).abs() < 0.01);

        // Con progreso=1.0, debería dar hasta (con la curva aplicada)
        let mut anim2 = AnimatedValue::new(0.0, 100.0, 1000.0);
        anim2.progreso = 1.0;
        assert!(
            (anim2.valor_interpolado() - 100.0).abs() < 0.01,
            "valor = {}",
            anim2.valor_interpolado()
        );
    }

    #[test]
    fn test_animated_value_pause() {
        let mut anim = AnimatedValue::new(0.0, 100.0, 1000.0);
        anim.update(300.0);
        let valor_al_pausar = anim.valor_actual;
        anim.pause();
        anim.update(500.0); // no debería avanzar
        assert!(
            (anim.valor_actual - valor_al_pausar).abs() < 0.01,
            "valor_actual = {}, esperado = {}",
            anim.valor_actual,
            valor_al_pausar
        );
    }

    #[test]
    fn test_animated_value_reset() {
        let mut anim = AnimatedValue::new(0.0, 100.0, 1000.0);
        anim.update(500.0);
        anim.reset();
        assert!((anim.valor_actual - 0.0).abs() < 0.01);
        assert!((anim.progreso - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_animated_value_loop() {
        let mut anim = AnimatedValue::new(0.0, 100.0, 500.0).with_loop();
        anim.update(500.0); // completa un ciclo
        assert!(anim.jugando, "debería seguir jugando en modo loop");
        assert!(
            anim.progreso < 0.01,
            "progreso debería reiniciarse: {}",
            anim.progreso
        );
    }

    #[test]
    fn test_animated_value_yoyo() {
        let mut anim = AnimatedValue::new(0.0, 100.0, 500.0).with_loop().with_yoyo();
        let primer_desde = anim.desde;
        let primer_hasta = anim.hasta;
        anim.update(500.0); // completa un ciclo, invierte desde↔hasta
        assert_eq!(anim.desde, primer_hasta, "desde debería intercambiarse");
        assert_eq!(anim.hasta, primer_desde, "hasta debería intercambiarse");
    }

    #[test]
    fn test_spring_animation() {
        let mut spring = SpringAnimation::new(100.0);
        spring.posicion = 0.0;
        for _ in 0..1000 {
            spring.update(16.0);
        } // ~60fps por ~1 segundo
        assert!(
            spring.is_settled(),
            "spring no se estabilizó: pos={}, vel={}",
            spring.posicion,
            spring.velocidad
        );
        assert!(
            (spring.posicion - 100.0).abs() < 1.0,
            "posición final={}",
            spring.posicion
        );
    }

    #[test]
    fn test_spring_parameters() {
        let mut spring = SpringAnimation::new(50.0)
            .with_masa(2.0)
            .with_rigidez(300.0)
            .with_amortiguacion(30.0);
        spring.posicion = 0.0;
        // Más rígido y amortiguado debería estabilizarse rápido
        for _ in 0..500 {
            spring.update(16.0);
        }
        assert!(spring.is_settled());
        assert!((spring.posicion - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_spring_set_objetivo() {
        let mut spring = SpringAnimation::new(100.0);
        spring.posicion = 0.0;
        spring.set_objetivo(200.0);
        assert_eq!(spring.objetivo, 200.0);
        for _ in 0..1000 {
            spring.update(16.0);
        }
        assert!(spring.is_settled());
        assert!(
            (spring.posicion - 200.0).abs() < 1.0,
            "pos={}",
            spring.posicion
        );
    }

    #[test]
    fn test_interpolate_color() {
        let a = RgbColor(0, 0, 0);
        let b = RgbColor(255, 255, 255);
        let mid = interpolate_color(a, b, 0.5);
        assert_eq!(mid, RgbColor(127, 127, 127));
    }

    #[test]
    fn test_interpolate_color_extremos() {
        let a = RgbColor(10, 20, 30);
        let b = RgbColor(200, 210, 220);

        // t = 0.0 debe dar el color inicial
        assert_eq!(interpolate_color(a, b, 0.0), a);
        // t = 1.0 debe dar el color final
        assert_eq!(interpolate_color(a, b, 1.0), b);
    }

    #[test]
    fn test_interpolate_color_clamp() {
        let a = RgbColor(0, 0, 0);
        let b = RgbColor(255, 255, 255);
        // t fuera de rango debe clampearse
        assert_eq!(interpolate_color(a, b, -0.5), a);
        assert_eq!(interpolate_color(a, b, 1.5), b);
    }

    #[test]
    fn test_animation_engine() {
        let mut engine = AnimationEngine::new();
        let anim = AnimatedValue::new(0.0, 100.0, 200.0);
        engine.add_animation(Box::new(anim));
        assert_eq!(engine.animations.len(), 1);

        // Avanzar frames
        engine.begin_frame(); // primer frame, delta ~0
        engine.begin_frame(); // segundo frame
        assert_eq!(engine.animations.len(), 1); // aún no termina

        // Forzar finalización simulando que pasó suficiente tiempo
        // (no podemos controlar Instant, pero podemos verificar la lógica)
    }

    #[test]
    fn test_animation_trait() {
        let anim = AnimatedValue::new(0.0, 100.0, 1000.0);
        let boxed: Box<dyn Animation> = Box::new(anim);
        assert!(!boxed.is_finished());
        assert!((boxed.current_value() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_animation_presets_no_panic() {
        // Verificar que todos los presets se crean sin pánico
        let _ = AnimationPresets::button_ripple();
        let _ = AnimationPresets::button_hover();
        let _ = AnimationPresets::card_elevate();
        let _ = AnimationPresets::page_transition();
        let _ = AnimationPresets::container_transform();
        let _ = AnimationPresets::spinner_loop();
        let _ = AnimationPresets::expressive_fade();
        let _ = AnimationPresets::expressive_morph();
    }

    #[test]
    fn test_animated_value_finishes() {
        let mut anim = AnimatedValue::new(0.0, 100.0, 100.0);
        assert!(!anim.is_finished());
        anim.update(100.0); // completa
        assert!(anim.is_finished());
    }

    #[test]
    fn test_animated_value_zero_duration() {
        let mut anim = AnimatedValue::new(0.0, 100.0, 0.0);
        anim.update(1000.0); // no debería cambiar nada
        assert!((anim.valor_actual - 0.0).abs() < 0.01);
    }
}
