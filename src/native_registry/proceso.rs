use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_proceso(&mut self) {
        self.registrar("_proceso_ejecutar", native_proceso_ejecutar);
    }
}
pub(crate) fn native_proceso_ejecutar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let cmd = obtener_texto(vm, args[0])?;

    // Verificar sandbox de procesos
    if let Err(e) = vm.sandbox_proc.verificar_comando(&cmd) {
        return Err(ErrFast::TipoInv(format!("Sandbox: {}", e)));
    }

    let stdin_input = if args.len() > 1 {
        obtener_texto(vm, args[1])?
    } else {
        String::new()
    };
    let dir = if args.len() > 2 {
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

    if !dir.is_empty() {
        command.current_dir(&dir);
    }

    if !stdin_input.is_empty() {
        command.stdin(std::process::Stdio::piped());
    }

    let output = match command.output() {
        Ok(o) => o,
        Err(e) => return Err(ErrFast::TipoInv(format!("_proceso_ejecutar: {}", e))),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1) as i64;

    // Build result object: SalidaProceso { stdout, stderr, codigo_salida }
    let sym = vm.tabla_simbolos.internar("SalidaProceso");
    let shape_id = vm.registro_formas.obtener_o_crear(sym);
    let mut obj = crate::vm_fast::ObjVal::new(sym, shape_id);
    obj.campos_vec = vec![
        {
            let idx = vm.alloc_str(Arc::from(stdout.as_str()));
            ValorFast::texto(idx)
        },
        {
            let idx = vm.alloc_str(Arc::from(stderr.as_str()));
            ValorFast::texto(idx)
        },
        ValorFast::entero(code),
    ];
    let idx = vm.alloc_obj(obj);
    Ok(ValorFast::objeto(idx))
}
