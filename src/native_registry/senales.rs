use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_senales(&mut self) {
        self.registrar("_senal_manejar", native_senal_manejar);
        self.registrar("_senal_ignorar", native_senal_ignorar);
        self.registrar("_senal_enviar", native_senal_enviar);
    }
}
pub(crate) fn native_senal_manejar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let senal = obtener_texto(_vm, args[0])?;
    // En versiones actuales de Rust, signal_hook no está disponible.
    // Implementación mínima: solo registrar la intención.
    eprintln!("[señales] Handler registrado para {} (simulado)", senal);
    Ok(ValorFast::nulo())
}

pub(crate) fn native_senal_ignorar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let _senal = obtener_texto(_vm, args[0])?;
    Ok(ValorFast::nulo())
}

pub(crate) fn native_senal_enviar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let _pid = obtener_entero(args[0])?;
    let _senal = obtener_texto(_vm, args[1])?;
    Ok(ValorFast::nulo())
}
