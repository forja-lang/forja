use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_ruta(&mut self) {
        self.registrar("_ruta_normalizar", native_ruta_normalizar);
        self.registrar("_ruta_unir", native_ruta_unir);
        self.registrar("_ruta_extension", native_ruta_extension);
        self.registrar("_ruta_sin_extension", native_ruta_sin_extension);
        self.registrar("_ruta_nombre_archivo", native_ruta_nombre_archivo);
        self.registrar("_ruta_dir_padre", native_ruta_dir_padre);
        self.registrar("_ruta_absoluta", native_ruta_absoluta);
        self.registrar("_ruta_es_absoluta", native_ruta_es_absoluta);
        self.registrar("_ruta_separador", native_ruta_separador);
    }
}
pub(crate) fn native_ruta_normalizar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let p = std::path::Path::new(&s);
    let normalized = p.components().collect::<std::path::PathBuf>();
    let out = normalized.to_string_lossy().replace('\\', "/");
    Ok({
        let idx = vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_ruta_unir(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let base = obtener_texto(vm, args[0])?;
    let part = obtener_texto(vm, args[1])?;
    let p = std::path::Path::new(&base).join(&part);
    let out = p.to_string_lossy().replace('\\', "/");
    Ok({
        let idx = vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_ruta_extension(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let ext = std::path::Path::new(&s)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok({
        let idx = vm.alloc_str(Arc::from(ext.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_ruta_sin_extension(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let p = std::path::Path::new(&s);
    let out = p.with_extension("").to_string_lossy().replace('\\', "/");
    Ok({
        let idx = vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_ruta_nombre_archivo(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let name = std::path::Path::new(&s)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok({
        let idx = vm.alloc_str(Arc::from(name.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_ruta_dir_padre(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let parent = std::path::Path::new(&s)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    Ok({
        let idx = vm.alloc_str(Arc::from(parent.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_ruta_absoluta(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let p = std::path::Path::new(&s);
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(p),
            Err(_) => return Ok(ValorFast::nulo()),
        }
    };
    let out = abs.to_string_lossy().replace('\\', "/");
    Ok({
        let idx = vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_ruta_es_absoluta(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(_vm, args[0])?;
    Ok(ValorFast::booleano(std::path::Path::new(&s).is_absolute()))
}

pub(crate) fn native_ruta_separador(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let sep = std::path::MAIN_SEPARATOR.to_string();
    Ok({
        let idx = _vm.alloc_str(Arc::from(sep.as_str()));
        ValorFast::texto(idx)
    })
}
