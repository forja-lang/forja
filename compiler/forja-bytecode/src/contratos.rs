//! Tipos de contrato compilados.

/// Tipo de contrato de *Design by Contract*.
#[derive(Debug, Clone, PartialEq)]
pub enum TipoContrato {
    /// `requiere` — precondición: debe cumplirse al entrar.
    Precondicion,
    /// `asegura` — postcondición: debe cumplirse al salir.
    Postcondicion,
    /// `siempre` — invariante: debe cumplirse antes y después.
    Invariante,
}

/// Contrato compilado a bytecode (condición expresada como micro-códigos).
#[derive(Debug, Clone)]
pub struct ContratoBytecode {
    /// Condición del contrato, ya bajada a micro-operaciones.
    pub condicion: Vec<crate::uops::Uop>,
    /// Mensaje que se muestra al usuario si el contrato falla.
    pub mensaje: String,
    /// Tipo de contrato al que pertenece.
    pub tipo: TipoContrato,
}
