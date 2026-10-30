use super::*;

impl NativeRegistry {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn registrar_hot_reload(&mut self) {
        // ─── Hot Reload Builtins ──────────────────────────────────────────────
        self.registrar("_recargar_modulo", native_recargar_modulo);
        self.registrar("_version_modulo", native_version_modulo);
        self.registrar("_recargar_todo", native_recargar_todo);
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn native_recargar_modulo(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::texto(
            vm.alloc_str(Arc::from("error: se requiere nombre del módulo")),
        ));
    }

    let nombre_modulo = crate::native_registry::obtener_texto(vm, args[0])?;
    let module_id = IdSimbolo(vm.tabla_simbolos.internar(&nombre_modulo).0);

    // Verificar que el módulo está registrado
    if !vm.module_registry.contains_key(&module_id) {
        return Ok(ValorFast::texto(vm.alloc_str(Arc::from(format!(
            "error: módulo '{}' no está cargado",
            nombre_modulo
        )))));
    }

    // Verificar si el módulo cambió en disco
    if !vm.module_resolver.modulo_cambio(&nombre_modulo) {
        return Ok(ValorFast::texto(vm.alloc_str(Arc::from("ok: sin cambios"))));
    }

    // Recargar: obtener nuevo AST del módulo
    let programa = match vm.module_resolver.recargar(module_id) {
        Ok(p) => p,
        Err(e) => {
            let msg = e
                .iter()
                .map(|err| format!("{}", err))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(ValorFast::texto(
                vm.alloc_str(Arc::from(format!("error: {}", msg))),
            ));
        }
    };

    // Generar nuevo bytecode para el módulo
    let mut gen = BytecodeGenerator::new();
    let module_bc = match gen.generar_para_modulo(&programa, module_id) {
        Ok(mbc) => mbc,
        Err(e) => {
            let msg = e
                .iter()
                .map(|err| format!("{}", err))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(ValorFast::texto(
                vm.alloc_str(Arc::from(format!("error: {}", msg))),
            ));
        }
    };

    // Hot-swap: reemplazar bytecode en caliente
    match vm.hot_swap_module(module_id, &module_bc) {
        Ok(()) => Ok(ValorFast::texto(vm.alloc_str(Arc::from(format!(
                "ok: recargado (v{})",
                vm.module_registry
                    .get(&module_id)
                    .map(|i| i.version)
                    .unwrap_or(0)
            ))))),
        Err(e) => Ok(ValorFast::texto(
            vm.alloc_str(Arc::from(format!("error: {}", e))),
        )),
    }
}

/// Retorna la versión actual de un módulo.
/// args[0]: nombre del módulo (Texto)
/// Retorna: entero con la versión, o -1 si no está registrado.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn native_version_modulo(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::entero(-1i64));
    }

    let nombre_modulo = crate::native_registry::obtener_texto(vm, args[0])?;
    let module_id = IdSimbolo(vm.tabla_simbolos.internar(&nombre_modulo).0);

    let version = vm
        .module_registry
        .get(&module_id)
        .map(|info| info.version as i64)
        .unwrap_or(-1i64);

    Ok(ValorFast::entero(version))
}

/// Recarga todos los módulos que hayan cambiado en disco.
/// Retorna: texto con lista de módulos recargados, o "ok: sin cambios".
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn native_recargar_todo(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let _ = args; // sin argumentos

    let cambiados = vm.module_resolver.modulos_cambiados();
    if cambiados.is_empty() {
        return Ok(ValorFast::texto(vm.alloc_str(Arc::from("ok: sin cambios"))));
    }

    let mut recargados: Vec<String> = Vec::new();
    for (module_id, ruta) in &cambiados {
        // Recargar AST del módulo
        let programa = match vm.module_resolver.recargar(*module_id) {
            Ok(p) => p,
            Err(e) => {
                let msg = e
                    .iter()
                    .map(|err| format!("{}", err))
                    .collect::<Vec<_>>()
                    .join(", ");
                recargados.push(format!("{}: error: {}", ruta, msg));
                continue;
            }
        };

        // Generar nuevo bytecode
        let mut gen = BytecodeGenerator::new();
        let module_bc = match gen.generar_para_modulo(&programa, *module_id) {
            Ok(mbc) => mbc,
            Err(e) => {
                let msg = e
                    .iter()
                    .map(|err| format!("{}", err))
                    .collect::<Vec<_>>()
                    .join(", ");
                recargados.push(format!("{}: error: {}", ruta, msg));
                continue;
            }
        };

        // Hot-swap
        match vm.hot_swap_module(*module_id, &module_bc) {
            Ok(()) => {
                let version = vm
                    .module_registry
                    .get(module_id)
                    .map(|i| i.version)
                    .unwrap_or(0);
                recargados.push(format!("{}: v{}", ruta, version));
            }
            Err(e) => {
                recargados.push(format!("{}: error: {}", ruta, e));
            }
        }
    }

    let resultado = recargados.join(", ");
    Ok(ValorFast::texto(vm.alloc_str(Arc::from(resultado))))
}
