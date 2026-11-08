//! Telemetría del recolector de basura de Forja.

/// Generación de un objeto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generation {
    Young,
    Old,
}

/// Métricas de pausas del GC. Las llena la VM en cada colección.
#[derive(Debug, Clone, Default)]
pub struct GcTelemetry {
    /// Número de recolecciones de la VM (mark-sweep real).
    pub colecciones: u64,
    /// Suma de todas las pausas (nanosegundos).
    pub pausa_total_ns: u128,
    /// Pausa máxima observada (nanosegundos).
    pub pausa_max_ns: u128,
    /// Pausa de la última recolección (nanosegundos).
    pub ultima_pausa_ns: u128,
    /// Slots del heap reciclados a las free lists (acumulado).
    pub slots_recolectados: u64,
}

impl GcTelemetry {
    /// Pausa media por colección (ns), o 0 si no hubo colecciones.
    pub fn pausa_media_ns(&self) -> u128 {
        if self.colecciones == 0 {
            0
        } else {
            self.pausa_total_ns / self.colecciones as u128
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pausa_media_sin_colecciones_es_cero() {
        assert_eq!(GcTelemetry::default().pausa_media_ns(), 0);
    }

    #[test]
    fn pausa_media_promedia() {
        let t = GcTelemetry {
            colecciones: 2,
            pausa_total_ns: 10,
            ..Default::default()
        };
        assert_eq!(t.pausa_media_ns(), 5);
    }

    #[test]
    fn generaciones() {
        assert_ne!(Generation::Young, Generation::Old);
    }
}
