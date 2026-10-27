use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_sandbox(&mut self) {
        // Restringir host de red
        self.registrar("_sandbox_restringir_host", native_sandbox_restringir_host);
        // Restringir puerto de red
        self.registrar(
            "_sandbox_restringir_puerto",
            native_sandbox_restringir_puerto,
        );
        // Restringir directorio de archivos
        self.registrar(
            "_sandbox_restringir_directorio",
            native_sandbox_restringir_directorio,
        );
        // Restringir comando de procesos
        self.registrar(
            "_sandbox_restringir_comando",
            native_sandbox_restringir_comando,
        );
        // Consultar estado del sandbox
        self.registrar("_sandbox_estado", native_sandbox_estado);
    }
}
pub(crate) fn native_sandbox_restringir_host(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let host = obtener_texto(vm, args[0])?;
    match &mut vm.sandbox.hosts_permitidos {
        Some(hosts) => {
            // Si contiene "*", empezar con lista vacía
            if hosts.iter().any(|h| h == "*") {
                hosts.clear();
            }
            hosts.push(host.clone());
            // Si el host ya estaba en la lista, no duplicar
            hosts.sort();
            hosts.dedup();
        }
        None => {
            vm.sandbox.hosts_permitidos = Some(vec![host.clone()]);
        }
    }
    Ok(ValorFast::nulo())
}

/// Restringe un puerto de red. Uso: _sandbox_restringir_puerto(80)
pub(crate) fn native_sandbox_restringir_puerto(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let puerto = obtener_entero(args[0])? as u16;
    match &mut vm.sandbox.puertos_permitidos {
        Some(puertos) => {
            puertos.push(puerto);
            puertos.sort();
            puertos.dedup();
        }
        None => {
            vm.sandbox.puertos_permitidos = Some(vec![puerto]);
        }
    }
    Ok(ValorFast::nulo())
}

/// Restringe un directorio de archivos. Uso: _sandbox_restringir_directorio("/etc")
pub(crate) fn native_sandbox_restringir_directorio(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let dir = obtener_texto(vm, args[0])?;
    match &mut vm.sandbox_fs.directorios_permitidos {
        Some(directorios) => {
            if directorios.iter().any(|d| d == "*") {
                directorios.clear();
            }
            directorios.push(dir.clone());
            directorios.sort();
            directorios.dedup();
        }
        None => {
            vm.sandbox_fs.directorios_permitidos = Some(vec![dir.clone()]);
        }
    }
    Ok(ValorFast::nulo())
}

/// Restringe un comando de procesos. Uso: _sandbox_restringir_comando("rm")
pub(crate) fn native_sandbox_restringir_comando(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let cmd = obtener_texto(vm, args[0])?;
    match &mut vm.sandbox_proc.comandos_permitidos {
        Some(comandos) => {
            if comandos.iter().any(|c| c == "*") {
                comandos.clear();
            }
            comandos.push(cmd.clone());
            comandos.sort();
            comandos.dedup();
        }
        None => {
            vm.sandbox_proc.comandos_permitidos = Some(vec![cmd.clone()]);
        }
    }
    Ok(ValorFast::nulo())
}

/// Retorna el estado actual del sandbox como mapa. Uso: _sandbox_estado()
pub(crate) fn native_sandbox_estado(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let mut map = std::collections::HashMap::new();

    // Estado de red
    let red = match &vm.sandbox.hosts_permitidos {
        Some(hosts) if hosts.iter().any(|h| h == "*") => "abierto",
        Some(_) => "restringido",
        None => "restringido",
    };
    map.insert(
        "red".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from(red))),
    );

    // Estado de archivos
    let archivos = match &vm.sandbox_fs.directorios_permitidos {
        Some(dirs) if dirs.iter().any(|d| d == "*") => "abierto",
        Some(_) => "restringido",
        None => "restringido",
    };
    map.insert(
        "archivos".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from(archivos))),
    );

    // Estado de procesos
    let procesos = match &vm.sandbox_proc.comandos_permitidos {
        Some(cmds) if cmds.iter().any(|c| c == "*") => "abierto",
        Some(_) => "restringido",
        None => "restringido",
    };
    map.insert(
        "procesos".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from(procesos))),
    );

    let midx = vm.alloc_map(map);
    Ok(ValorFast::mapa(midx))
}
