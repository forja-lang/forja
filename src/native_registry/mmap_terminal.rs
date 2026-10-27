use super::*;

pub(crate) fn native_mmap_abrir(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_mmap_abrir requiere 4 args: ruta, offset, longitud, modo".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let offset = obtener_entero(args[1])? as u64;
    let len = obtener_entero(args[2])? as usize;
    let modo = obtener_texto(vm, args[3])?;
    let writable = modo == "rw";
    match crate::mmap::mmap_abrir(&ruta, offset, len, writable) {
        Ok(h) => Ok(ValorFast::entero(h)),
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_mmap_cerrar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_mmap_cerrar requiere 1 arg: handle".into(),
        ));
    }
    let handle = obtener_entero(args[0])?;
    match crate::mmap::mmap_cerrar(handle) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_mmap_leer(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_mmap_leer requiere 3 args: handle, offset, longitud".into(),
        ));
    }
    let handle = obtener_entero(args[0])?;
    let offset = obtener_entero(args[1])? as usize;
    let len = obtener_entero(args[2])? as usize;
    match crate::mmap::mmap_leer(handle, offset, len) {
        Ok(bytes) => {
            let s = String::from_utf8_lossy(&bytes).to_string();
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_mmap_escribir(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_mmap_escribir requiere 3 args: handle, offset, datos".into(),
        ));
    }
    let handle = obtener_entero(args[0])?;
    let offset = obtener_entero(args[1])? as usize;
    let datos = obtener_texto(vm, args[2])?;
    match crate::mmap::mmap_escribir(handle, offset, datos.as_bytes()) {
        Ok(n) => Ok(ValorFast::entero(n as i64)),
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_mmap_sincronizar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_mmap_sincronizar requiere 1 arg: handle".into(),
        ));
    }
    let handle = obtener_entero(args[0])?;
    match crate::mmap::mmap_sincronizar(handle) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_mmap_tamano(
    vm: &mut ForjaFast,
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

pub(crate) fn native_mmap_page_size(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    Ok(ValorFast::entero(crate::mmap::page_size() as i64))
}

// ─── Terminal ────────────────────────────────────────────────────────────

pub(crate) fn native_terminal_tamano(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    match crate::terminal::tamano_terminal() {
        Ok((cols, rows)) => {
            let s = format!("{},{}", cols, rows);
            let ret = ValorFast::texto(_vm.alloc_str(Arc::from(s.as_str())));
            Ok(ret)
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_terminal_raw(
    _vm: &mut ForjaFast,
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
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_terminal_leer_tecla(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    match crate::terminal::leer_tecla() {
        Ok(s) => {
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}
