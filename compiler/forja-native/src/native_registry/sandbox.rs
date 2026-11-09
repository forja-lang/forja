use super::*;

impl NativoRegistro {
    pub fn registrar_sandbox(&mut self) {
        // Restringir anfitrion de red
        self.registrar(
            "_sandbox_restringir_host",
            nativo_sandbox_restringir_anfitrion,
        );
        // Restringir puerto de red
        self.registrar(
            "_sandbox_restringir_puerto",
            nativo_sandbox_restringir_puerto,
        );
        // Restringir directorio de archivos
        self.registrar(
            "_sandbox_restringir_directorio",
            nativo_sandbox_restringir_directorio,
        );
        // Restringir comando de procesos
        self.registrar(
            "_sandbox_restringir_comando",
            nativo_sandbox_restringir_comando,
        );
        // Consultar estado del sandbox
        self.registrar("_sandbox_estado", nativo_sandbox_estado);
    }
}
pub fn nativo_sandbox_restringir_anfitrion(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let anfitrion = obtener_texto(vm, args[0])?;
    match &mut vm.sandbox().hosts_permitidos {
        Some(hosts) => {
            // Si contiene "*", empezar con lista vacía
            if hosts.iter().any(|h| h == "*") {
                hosts.clear();
            }
            hosts.push(anfitrion.clone());
            // Si el anfitrion ya estaba en la lista, no duplicar
            hosts.sort();
            hosts.dedup();
        }
        None => {
            vm.sandbox().hosts_permitidos = Some(vec![anfitrion.clone()]);
        }
    }
    Ok(ValorFast::nulo())
}

/// Restringe un puerto de red. Uso: _sandbox_restringir_puerto(80)
pub fn nativo_sandbox_restringir_puerto(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let puerto = obtener_entero(args[0])? as u16;
    match &mut vm.sandbox().puertos_permitidos {
        Some(puertos) => {
            puertos.push(puerto);
            puertos.sort();
            puertos.dedup();
        }
        None => {
            vm.sandbox().puertos_permitidos = Some(vec![puerto]);
        }
    }
    Ok(ValorFast::nulo())
}

/// Restringe un directorio de archivos. Uso: _sandbox_restringir_directorio("/etc")
pub fn nativo_sandbox_restringir_directorio(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let directorio = obtener_texto(vm, args[0])?;
    match &mut vm.sandbox_fs().directorios_permitidos {
        Some(directorios) => {
            if directorios.iter().any(|d| d == "*") {
                directorios.clear();
            }
            directorios.push(directorio.clone());
            directorios.sort();
            directorios.dedup();
        }
        None => {
            vm.sandbox_fs().directorios_permitidos = Some(vec![directorio.clone()]);
        }
    }
    Ok(ValorFast::nulo())
}

/// Restringe un comando de procesos. Uso: _sandbox_restringir_comando("rm")
pub fn nativo_sandbox_restringir_comando(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let cmd = obtener_texto(vm, args[0])?;
    match &mut vm.sandbox_proc().comandos_permitidos {
        Some(comandos) => {
            if comandos.iter().any(|c| c == "*") {
                comandos.clear();
            }
            comandos.push(cmd.clone());
            comandos.sort();
            comandos.dedup();
        }
        None => {
            vm.sandbox_proc().comandos_permitidos = Some(vec![cmd.clone()]);
        }
    }
    Ok(ValorFast::nulo())
}

/// Retorna el estado actual del sandbox como mapa. Uso: _sandbox_estado()
pub fn nativo_sandbox_estado(
    vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let mut mapa = std::collections::HashMap::new();

    // Estado de red
    let red = match &vm.sandbox().hosts_permitidos {
        Some(hosts) if hosts.iter().any(|h| h == "*") => "abierto",
        Some(_) => "restringido",
        None => "restringido",
    };
    mapa.insert(
        "red".to_string(),
        ValorFast::texto(vm.reservar_texto(Arc::from(red))),
    );

    // Estado de archivos
    let archivos = match &vm.sandbox_fs().directorios_permitidos {
        Some(dirs) if dirs.iter().any(|d| d == "*") => "abierto",
        Some(_) => "restringido",
        None => "restringido",
    };
    mapa.insert(
        "archivos".to_string(),
        ValorFast::texto(vm.reservar_texto(Arc::from(archivos))),
    );

    // Estado de procesos
    let procesos = match &vm.sandbox_proc().comandos_permitidos {
        Some(cmds) if cmds.iter().any(|c| c == "*") => "abierto",
        Some(_) => "restringido",
        None => "restringido",
    };
    mapa.insert(
        "procesos".to_string(),
        ValorFast::texto(vm.reservar_texto(Arc::from(procesos))),
    );

    let midx = vm.reservar_mapa(mapa);
    Ok(ValorFast::mapa(midx))
}
