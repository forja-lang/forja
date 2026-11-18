use super::*;

impl NativoRegistro {
    pub fn registrar_proceso(&mut self) {
        self.registrar("_proceso_ejecutar", nativo_proceso_ejecutar);
        self.registrar("_proceso_pid_actual", nativo_proceso_pid_actual);
        self.registrar("_proceso_esta_ejecutando", nativo_proceso_esta_ejecutando);
    }
}

/// `_proceso_pid_actual()` → Entero con el PID de este proceso.
pub fn nativo_proceso_pid_actual(
    _vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    Ok(ValorFast::entero(std::process::id() as i64))
}

/// `_proceso_esta_ejecutando(pid)` → Booleano.
///
/// En Unix alcanza con `kill(pid, 0)`; en Windows se abre el proceso con
/// `PROCESS_QUERY_LIMITED_INFORMATION` y se consulta si sigue activo.
pub fn nativo_proceso_esta_ejecutando(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let pid = obtener_entero(args.first().copied().unwrap_or(ValorFast::nulo()))?;
    if pid <= 0 {
        return Ok(ValorFast::booleano(false));
    }
    Ok(ValorFast::booleano(pid_vivo(pid as u32)))
}

#[cfg(unix)]
fn pid_vivo(pid: u32) -> bool {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    // señal 0 = sólo comprobar existencia/permiso
    unsafe { kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn pid_vivo(pid: u32) -> bool {
    // 0x1000 = PROCESS_QUERY_LIMITED_INFORMATION (Vista+)
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(acceso: u32, heredar: i32, pid: u32) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(proceso: *mut std::ffi::c_void, codigo: *mut u32) -> i32;
        fn CloseHandle(objeto: *mut std::ffi::c_void) -> i32;
    }
    // STILL_ACTIVE
    const ACTIVO: u32 = 259;
    unsafe {
        let manejador = OpenProcess(0x1000, 0, pid);
        if manejador.is_null() {
            return false;
        }
        let mut codigo: u32 = 0;
        let ok = GetExitCodeProcess(manejador, &mut codigo) != 0;
        CloseHandle(manejador);
        ok && codigo == ACTIVO
    }
}

#[cfg(not(any(unix, windows)))]
fn pid_vivo(_pid: u32) -> bool {
    false
}
pub fn nativo_proceso_ejecutar(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let cmd = obtener_texto(vm, args[0])?;

    // Verificar sandbox de procesos
    if let Err(e) = vm.sandbox_proc().verificar_comando(&cmd) {
        return Err(ErrFast::TipoInv(format!("Sandbox: {}", e)));
    }

    let stdin_entrada = if args.len() > 1 {
        obtener_texto(vm, args[1])?
    } else {
        String::new()
    };
    let directorio = if args.len() > 2 {
        obtener_texto(vm, args[2])?
    } else {
        String::new()
    };

    let mut command = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg(&cmd);
        c
    } else {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(&cmd);
        c
    };

    if !directorio.is_empty() {
        command.current_dir(&directorio);
    }

    if !stdin_entrada.is_empty() {
        command.stdin(std::process::Stdio::piped());
    }

    let salida = match command.output() {
        Ok(o) => o,
        Err(e) => return Err(ErrFast::TipoInv(format!("_proceso_ejecutar: {}", e))),
    };

    let stdout = String::from_utf8_lossy(&salida.stdout).to_string();
    let stderr = String::from_utf8_lossy(&salida.stderr).to_string();
    let codigo = salida.status.code().unwrap_or(-1) as i64;

    // SalidaProceso { stdout, stderr, codigo_salida } con campos accesibles.
    let idx_stdout = vm.reservar_texto(Arc::from(stdout.as_str()));
    let idx_stderr = vm.reservar_texto(Arc::from(stderr.as_str()));
    Ok(super::objeto_con_campos(
        vm,
        "SalidaProceso",
        &["stdout", "stderr", "codigo_salida"],
        vec![
            ValorFast::texto(idx_stdout),
            ValorFast::texto(idx_stderr),
            ValorFast::entero(codigo),
        ],
    ))
}
