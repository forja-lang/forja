use super::*;

/// Estado global para el generador aleatorio xorshift32
static _ESTADO_ALEATORIO: AtomicI32 = AtomicI32::new(123456789);

impl NativeRegistry {
    pub(crate) fn registrar_aleatorio(&mut self) {
        // ─── Aleatorio ─────────────────────────────────────────────────────
        self.registrar("_aleatorio_semilla", native_aleatorio_semilla);
        self.registrar("_aleatorio_entero", native_aleatorio_entero);
    }
}
pub(crate) fn native_aleatorio_semilla(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_aleatorio_semilla requiere 1 argumento: valor (entero)".into(),
        ));
    }
    let valor = obtener_entero(args[0])?;
    _ESTADO_ALEATORIO.store(valor as i32, Ordering::SeqCst);
    Ok(ValorFast::nulo())
}

/// Genera un entero aleatorio en [0, max) usando xorshift32
pub(crate) fn native_aleatorio_entero(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_aleatorio_entero requiere 1 argumento: max (entero)".into(),
        ));
    }
    let max = obtener_entero(args[0])?;
    if max <= 0 {
        return Err(ErrFast::TipoInv(
            "_aleatorio_entero: max debe ser > 0".into(),
        ));
    }

    // xorshift32
    let mut estado = _ESTADO_ALEATORIO.load(Ordering::SeqCst);
    estado ^= estado << 13;
    estado ^= estado >> 17;
    estado ^= estado << 5;
    _ESTADO_ALEATORIO.store(estado, Ordering::SeqCst);

    // Valor absoluto y módulo para asegurar rango positivo
    let valor = if estado < 0 { -estado } else { estado };
    Ok(ValorFast::entero((valor % max as i32) as i64))
}

/// Helper para mapear std::io::Error a códigos de error estandarizados
pub(crate) fn codigo_error_archivo(error: &std::io::Error) -> &'static str {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => "archivo_no_encontrado",
        ErrorKind::PermissionDenied => "permiso_denegado",
        ErrorKind::InvalidInput => "ruta_invalida",
        ErrorKind::IsADirectory => "es_directorio",
        ErrorKind::DirectoryNotEmpty => "directorio_no_vacio",
        _ => "error_interno",
    }
}
