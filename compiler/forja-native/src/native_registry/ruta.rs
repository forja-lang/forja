use super::*;

impl NativoRegistro {
    pub fn registrar_ruta(&mut self) {
        self.registrar("_ruta_normalizar", nativo_ruta_normalizar);
        self.registrar("_ruta_unir", nativo_ruta_unir);
        self.registrar("_ruta_extension", nativo_ruta_extension);
        self.registrar("_ruta_sin_extension", nativo_ruta_sin_extension);
        self.registrar("_ruta_nombre_archivo", nativo_ruta_nombre_archivo);
        self.registrar("_ruta_dir_padre", nativo_ruta_directorio_padre);
        self.registrar("_ruta_absoluta", nativo_ruta_absoluta);
        self.registrar("_ruta_es_absoluta", nativo_ruta_es_absoluta);
        self.registrar("_ruta_separador", nativo_ruta_separador);
    }
}
pub fn nativo_ruta_normalizar(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let p = std::path::Path::new(&s);
    let normalized = p.components().collect::<std::path::PathBuf>();
    let salida = normalized.to_string_lossy().replace('\\', "/");
    Ok({
        let indice = vm.reservar_texto(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn nativo_ruta_unir(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let base = obtener_texto(vm, args[0])?;
    let part = obtener_texto(vm, args[1])?;
    let p = std::path::Path::new(&base).join(&part);
    let salida = p.to_string_lossy().replace('\\', "/");
    Ok({
        let indice = vm.reservar_texto(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn nativo_ruta_extension(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let ext = std::path::Path::new(&s)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok({
        let indice = vm.reservar_texto(Arc::from(ext.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn nativo_ruta_sin_extension(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let p = std::path::Path::new(&s);
    let salida = p.with_extension("").to_string_lossy().replace('\\', "/");
    Ok({
        let indice = vm.reservar_texto(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn nativo_ruta_nombre_archivo(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let nombre = std::path::Path::new(&s)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok({
        let indice = vm.reservar_texto(Arc::from(nombre.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn nativo_ruta_directorio_padre(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    let padre = std::path::Path::new(&s)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    Ok({
        let indice = vm.reservar_texto(Arc::from(padre.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn nativo_ruta_absoluta(
    vm: &mut dyn Anfitrion,
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
    let salida = abs.to_string_lossy().replace('\\', "/");
    Ok({
        let indice = vm.reservar_texto(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn nativo_ruta_es_absoluta(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(_vm, args[0])?;
    Ok(ValorFast::booleano(std::path::Path::new(&s).is_absolute()))
}

pub fn nativo_ruta_separador(
    _vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let sep = std::path::MAIN_SEPARATOR.to_string();
    Ok({
        let indice = _vm.reservar_texto(Arc::from(sep.as_str()));
        ValorFast::texto(indice)
    })
}
