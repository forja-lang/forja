use crate::vm_fast::ForjaFast;
use forja_bytecode::{tabla_simbolos::IdSimbolo, GeneradorBytecode};
use forja_native::native_registry::*;
use forja_runtime::{Anfitrion, ErrFast, ValorFast};
use std::sync::Arc;

fn vm_concreta(vm: &mut dyn Anfitrion) -> Result<&mut ForjaFast, ErrFast> {
    vm.as_any_mut()
        .downcast_mut::<ForjaFast>()
        .ok_or_else(|| ErrFast::TipoInv("hot reload requiere la VM ForjaFast".into()))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn registrar_hot_reload(reg: &mut NativoRegistro) {
    reg.registrar("_recargar_modulo", nativo_recargar_modulo);
    reg.registrar("_version_modulo", nativo_version_modulo);
    reg.registrar("_recargar_todo", nativo_recargar_todo);
}
#[cfg(not(target_arch = "wasm32"))]
pub fn nativo_recargar_modulo(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let vm = vm_concreta(vm)?;
    if args.is_empty() {
        return Ok(ValorFast::texto(vm.reservar_texto(Arc::from(
            "error: se requiere nombre del módulo",
        ))));
    }

    let nombre_modulo = crate::native_registry::obtener_texto(vm, args[0])?;
    let modulo_id = IdSimbolo(vm.tabla_simbolos().internar(&nombre_modulo).0);

    // Verificar que el módulo está registrado
    if !vm.registro_modulos.contains_key(&modulo_id) {
        return Ok(ValorFast::texto(vm.reservar_texto(Arc::from(format!(
            "error: módulo '{}' no está cargado",
            nombre_modulo
        )))));
    }

    // Verificar si el módulo cambió en disco
    if !vm.resolvedor_modulos.modulo_cambio(&nombre_modulo) {
        return Ok(ValorFast::texto(
            vm.reservar_texto(Arc::from("ok: sin cambios")),
        ));
    }

    // Recargar: obtener nuevo AST del módulo
    let programa = match vm.resolvedor_modulos.recargar(modulo_id) {
        Ok(p) => p,
        Err(e) => {
            let mensaje = e
                .iter()
                .map(|err| format!("{}", err))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(ValorFast::texto(
                vm.reservar_texto(Arc::from(format!("error: {}", mensaje))),
            ));
        }
    };

    // Generar nuevo bytecode para el módulo
    let mut gen = GeneradorBytecode::nuevo();
    let modulo_bytecode = match gen.generar_para_modulo(&programa, modulo_id) {
        Ok(mbc) => mbc,
        Err(e) => {
            let mensaje = e
                .iter()
                .map(|err| format!("{}", err))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(ValorFast::texto(
                vm.reservar_texto(Arc::from(format!("error: {}", mensaje))),
            ));
        }
    };

    // Hot-swap: reemplazar bytecode en caliente
    match vm.hot_swap_modulo(modulo_id, &modulo_bytecode) {
        Ok(()) => Ok(ValorFast::texto(vm.reservar_texto(Arc::from(format!(
                "ok: recargado (v{})",
                vm.registro_modulos
                    .get(&modulo_id)
                    .map(|i| i.version)
                    .unwrap_or(0)
            ))))),
        Err(e) => Ok(ValorFast::texto(
            vm.reservar_texto(Arc::from(format!("error: {}", e))),
        )),
    }
}

/// Retorna la versión actual de un módulo.
/// args[0]: nombre del módulo (Texto)
/// Retorna: entero con la versión, o -1 si no está registrado.
#[cfg(not(target_arch = "wasm32"))]
pub fn nativo_version_modulo(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let vm = vm_concreta(vm)?;
    if args.is_empty() {
        return Ok(ValorFast::entero(-1i64));
    }

    let nombre_modulo = crate::native_registry::obtener_texto(vm, args[0])?;
    let modulo_id = IdSimbolo(vm.tabla_simbolos().internar(&nombre_modulo).0);

    let version = vm
        .registro_modulos
        .get(&modulo_id)
        .map(|informacion| informacion.version as i64)
        .unwrap_or(-1i64);

    Ok(ValorFast::entero(version))
}

/// Recarga todos los módulos que hayan cambiado en disco.
/// Retorna: texto con lista de módulos recargados, o "ok: sin cambios".
#[cfg(not(target_arch = "wasm32"))]
pub fn nativo_recargar_todo(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let vm = vm_concreta(vm)?;
    let _ = args; // sin argumentos

    let cambiados = vm.resolvedor_modulos.modulos_cambiados();
    if cambiados.is_empty() {
        return Ok(ValorFast::texto(
            vm.reservar_texto(Arc::from("ok: sin cambios")),
        ));
    }

    let mut recargados: Vec<String> = Vec::new();
    for (modulo_id, ruta) in &cambiados {
        // Recargar AST del módulo
        let programa = match vm.resolvedor_modulos.recargar(*modulo_id) {
            Ok(p) => p,
            Err(e) => {
                let mensaje = e
                    .iter()
                    .map(|err| format!("{}", err))
                    .collect::<Vec<_>>()
                    .join(", ");
                recargados.push(format!("{}: error: {}", ruta, mensaje));
                continue;
            }
        };

        // Generar nuevo bytecode
        let mut gen = GeneradorBytecode::nuevo();
        let modulo_bytecode = match gen.generar_para_modulo(&programa, *modulo_id) {
            Ok(mbc) => mbc,
            Err(e) => {
                let mensaje = e
                    .iter()
                    .map(|err| format!("{}", err))
                    .collect::<Vec<_>>()
                    .join(", ");
                recargados.push(format!("{}: error: {}", ruta, mensaje));
                continue;
            }
        };

        // Hot-swap
        match vm.hot_swap_modulo(*modulo_id, &modulo_bytecode) {
            Ok(()) => {
                let version = vm
                    .registro_modulos
                    .get(modulo_id)
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
    Ok(ValorFast::texto(vm.reservar_texto(Arc::from(resultado))))
}
