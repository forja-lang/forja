use super::*;

pub fn nativo_mmap_abrir(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_mmap_abrir requiere 4 args: ruta, offset, longitud, modo".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let desplazamiento = obtener_entero(args[1])? as u64;
    let longitud = obtener_entero(args[2])? as usize;
    let modo = obtener_texto(vm, args[3])?;
    let writable = modo == "rw";
    match crate::mmap::mmap_abrir(&ruta, desplazamiento, longitud, writable) {
        Ok(h) => Ok(ValorFast::entero(h)),
        Err(e) => Err(ErrFast::TipoInv(e)),
    }
}

pub fn nativo_mmap_cerrar(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_mmap_cerrar requiere 1 arg: handle".into(),
        ));
    }
    let manejador = obtener_entero(args[0])?;
    match crate::mmap::mmap_cerrar(manejador) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(e)),
    }
}

pub fn nativo_mmap_leer(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_mmap_leer requiere 3 args: handle, offset, longitud".into(),
        ));
    }
    let manejador = obtener_entero(args[0])?;
    let desplazamiento = obtener_entero(args[1])? as usize;
    let longitud = obtener_entero(args[2])? as usize;
    match crate::mmap::mmap_leer(manejador, desplazamiento, longitud) {
        Ok(bytes) => {
            let s = String::from_utf8_lossy(&bytes).to_string();
            let indice = vm.reservar_texto(Arc::from(s.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e)),
    }
}

pub fn nativo_mmap_escribir(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_mmap_escribir requiere 3 args: handle, offset, datos".into(),
        ));
    }
    let manejador = obtener_entero(args[0])?;
    let desplazamiento = obtener_entero(args[1])? as usize;
    let datos = obtener_texto(vm, args[2])?;
    match crate::mmap::mmap_escribir(manejador, desplazamiento, datos.as_bytes()) {
        Ok(n) => Ok(ValorFast::entero(n as i64)),
        Err(e) => Err(ErrFast::TipoInv(e)),
    }
}

pub fn nativo_mmap_sincronizar(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_mmap_sincronizar requiere 1 arg: handle".into(),
        ));
    }
    let manejador = obtener_entero(args[0])?;
    match crate::mmap::mmap_sincronizar(manejador) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(e)),
    }
}

pub fn nativo_mmap_tamano(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_mmap_tamano requiere 1 arg: ruta".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    match std::fs::metadata(&ruta) {
        Ok(meta) => Ok(ValorFast::entero(meta.len() as i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("Error: {}", e))),
    }
}

pub fn nativo_mmap_page_tamano(
    _vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    Ok(ValorFast::entero(crate::mmap::page_tamano() as i64))
}

// ─── Terminal ────────────────────────────────────────────────────────────

pub fn nativo_terminal_tamano(
    _vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    match crate::terminal::tamano_terminal() {
        Ok((cols, rows)) => {
            let s = format!("{},{}", cols, rows);
            let ret = ValorFast::texto(_vm.reservar_texto(Arc::from(s.as_str())));
            Ok(ret)
        }
        Err(e) => Err(ErrFast::TipoInv(e)),
    }
}

pub fn nativo_terminal_raw(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_terminal_raw requiere 1 arg: 1=raw, 0=normal".into(),
        ));
    }
    let activar = obtener_entero(args[0])? != 0;
    match crate::terminal::raw_mode(activar) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(e)),
    }
}

pub fn nativo_terminal_leer_tecla(
    vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    match crate::terminal::leer_tecla() {
        Ok(s) => {
            let indice = vm.reservar_texto(Arc::from(s.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e)),
    }
}
