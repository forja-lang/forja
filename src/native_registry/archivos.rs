use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_archivos(&mut self) {
        // ─── Archivos ────────────────────────────────────────────────────
        self.registrar("_archivo_leer", native_archivo_leer);
        self.registrar("_archivo_escribir", native_archivo_escribir);
        self.registrar("_archivo_existe", native_archivo_existe);
        self.registrar("_archivo_eliminar", native_archivo_eliminar);
        self.registrar("_archivo_copiar", native_archivo_copiar);
        self.registrar("_archivo_mover", native_archivo_mover);
        self.registrar("_archivo_tamano", native_archivo_tamano);
        self.registrar("_archivo_info", native_archivo_info);
        // ─── Archivos Binarios ───────────────────────────────────────────
        self.registrar("_archivo_leer_binario", native_archivo_leer_binario);
        self.registrar("_archivo_escribir_binario", native_archivo_escribir_binario);

        // ─── Directorios ─────────────────────────────────────────────────
        self.registrar("_directorio_crear", native_directorio_crear);
        self.registrar("_directorio_eliminar", native_directorio_eliminar);
        self.registrar("_directorio_listar", native_directorio_listar);
    }
}
pub(crate) fn native_archivo_leer(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_leer requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    // Verificar sandbox de filesystem (lectura)
    if let Err(e) = vm.sandbox_fs.verificar_lectura(&ruta) {
        return Err(ErrFast::TipoInv(format!("Sandbox: {}", e)));
    }
    match std::fs::read_to_string(&ruta) {
        Ok(contenido) => {
            let idx = vm.alloc_str(Arc::from(contenido.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_archivo_escribir(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_archivo_escribir requiere 2 argumentos: ruta (texto), contenido (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let contenido = obtener_texto(vm, args[1])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    // Verificar sandbox de filesystem (escritura)
    if let Err(e) = vm.sandbox_fs.verificar_escritura(&ruta) {
        return Err(ErrFast::TipoInv(format!("Sandbox: {}", e)));
    }
    match std::fs::write(&ruta, contenido.as_bytes()) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_archivo_existe(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_existe requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    Ok(ValorFast::booleano(std::path::Path::new(&ruta).exists()))
}

pub(crate) fn native_archivo_eliminar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_eliminar requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::remove_file(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_archivo_copiar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_archivo_copiar requiere 2 argumentos: origen (texto), destino (texto)".into(),
        ));
    }
    let origen = obtener_texto(vm, args[0])?;
    let destino = obtener_texto(vm, args[1])?;
    if origen.trim().is_empty() || destino.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: las rutas no pueden estar vacías".into(),
        ));
    }
    match std::fs::copy(&origen, &destino) {
        Ok(_) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_archivo_mover(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_archivo_mover requiere 2 argumentos: origen (texto), destino (texto)".into(),
        ));
    }
    let origen = obtener_texto(vm, args[0])?;
    let destino = obtener_texto(vm, args[1])?;
    if origen.trim().is_empty() || destino.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: las rutas no pueden estar vacías".into(),
        ));
    }
    match std::fs::rename(&origen, &destino) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_archivo_tamano(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_tamano requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::metadata(&ruta) {
        Ok(meta) => {
            let tamano = meta.len() as i64;
            Ok(ValorFast::entero(tamano))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_directorio_crear(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_directorio_crear requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::create_dir_all(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_directorio_eliminar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_directorio_eliminar requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::remove_dir_all(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_directorio_listar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_directorio_listar requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::read_dir(&ruta) {
        Ok(entradas) => {
            let mut nombres = Vec::new();
            for entrada in entradas.flatten() {
                if let Some(nombre) = entrada.file_name().to_str() {
                    nombres.push(nombre.to_string());
                }
            }
            let resultado = nombres.join("\n");
            let idx = vm.alloc_str(Arc::from(resultado.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

pub(crate) fn native_archivo_info(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_info requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::metadata(&ruta) {
        Ok(meta) => {
            let modificado = meta
                .modified()
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_else(|_| "0".to_string())
                })
                .unwrap_or_else(|_| "0".to_string());
            let info = format!(
                "tamano:{};es_directorio:{};es_archivo:{};permisos:{};modificado:{}",
                meta.len(),
                meta.is_dir(),
                meta.is_file(),
                if meta.permissions().readonly() {
                    "solo_lectura"
                } else {
                    "lectura_escritura"
                },
                modificado
            );
            let idx = vm.alloc_str(Arc::from(info.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

// ─── Archivos Binarios ─────────────────────────────────────────────

/// Lee un archivo como bytes y retorna su contenido como hexadecimal
/// args[0]: ruta (Texto)
/// Retorna: contenido del archivo en hexadecimal (Texto)
pub(crate) fn native_archivo_leer_binario(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_leer_binario requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::read(&ruta) {
        Ok(bytes) => {
            let hex_str = bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

/// Escribe bytes (desde hexadecimal) a un archivo
/// args[0]: ruta (Texto)
/// args[1]: contenido_hex (Texto) - datos en hexadecimal
/// Retorna: 0 si éxito, -1 si error
pub(crate) fn native_archivo_escribir_binario(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_archivo_escribir_binario requiere 2 argumentos: ruta (texto), contenido_hex (texto)"
                .into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let hex_str = obtener_texto(vm, args[1])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    // Decodificar hex a bytes
    let bytes: Vec<u8> = (0..hex_str.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 1 < hex_str.len() {
                u8::from_str_radix(&hex_str[i..i + 2], 16).ok()
            } else {
                None
            }
        })
        .collect();
    match std::fs::write(&ruta, &bytes) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}
