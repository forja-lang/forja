use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_temporizador(&mut self) {
        self.registrar("_temporizador_nuevo", native_temporizador_nuevo);
        self.registrar("_temporizador_dormir", native_temporizador_dormir);
        self.registrar("_sistema_tiempo_ms", native_sistema_tiempo_ms);
    }
}
pub(crate) fn native_sistema_tiempo_ms(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    Ok(ValorFast::entero(ms))
}

pub(crate) fn native_temporizador_nuevo(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let ms = obtener_entero(args[0])?;
    let sym = vm.tabla_simbolos.internar("Timer");
    let shape_id = vm.registro_formas.obtener_o_crear(sym);
    let mut obj = crate::vm_fast::ObjVal::new(sym, shape_id);
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    obj.campos_vec = vec![
        ValorFast::entero(id as i64),
        ValorFast::entero(ms),
        ValorFast::booleano(true),
    ];
    let idx = vm.alloc_obj(obj);
    Ok(ValorFast::objeto(idx))
}

pub(crate) fn native_temporizador_dormir(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let ms = obtener_entero(args[0])? as u64;
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(ValorFast::nulo())
}
