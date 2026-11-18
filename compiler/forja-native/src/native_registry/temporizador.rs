use super::*;

impl NativoRegistro {
    pub fn registrar_temporizador(&mut self) {
        self.registrar("_temporizador_nuevo", nativo_temporizador_nuevo);
        self.registrar("_temporizador_dormir", nativo_temporizador_dormir);
        self.registrar("_sistema_tiempo_ms", nativo_sistema_tiempo_ms);
    }
}
pub fn nativo_sistema_tiempo_ms(
    _vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    Ok(ValorFast::entero(ms))
}

pub fn nativo_temporizador_nuevo(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let ms = obtener_entero(args[0])?;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(super::objeto_con_campos(
        vm,
        "Timer",
        &["id", "duracion_ms", "activo"],
        vec![
            ValorFast::entero(id as i64),
            ValorFast::entero(ms),
            ValorFast::booleano(true),
        ],
    ))
}

pub fn nativo_temporizador_dormir(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let ms = obtener_entero(args[0])? as u64;
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(ValorFast::nulo())
}
