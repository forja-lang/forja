use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_sistema(&mut self) {
        // ─── Sistema / OS ─────────────────────────────────────────────────────
        self.registrar("_args", native_args);
        self.registrar("_salir", native_salir);
        self.registrar("_env", native_env);
        self.registrar("_ejecutar", native_ejecutar);
        self.registrar("_leer_linea", native_leer_linea);
        self.registrar("_imprimir_error", native_imprimir_error);
        self.registrar("_char", native_char);
        self.registrar("_codigo_char", native_codigo_char);
        self.registrar("_numero_a_texto_base", native_numero_a_texto_base);
        // Las usa `std/sistema.fa`; faltaban en el registro, así que
        // `sistema.sistema_operativo()` y `directorio_actual()` fallaban con
        // "función no definida".
        self.registrar("_sistema_so", native_sistema_so);
        self.registrar("_sistema_directorio_actual", native_sistema_directorio_actual);
        // `std/sistema.fa` usa estos nombres; el registro tenía los cortos
        // (`_args`, `_env`, `_salir`, `_sistema_tiempo_ms`).
        self.registrar("_sistema_argumentos", native_args);
        self.registrar("_sistema_salir", native_salir);
        self.registrar("_sistema_variable_entorno", native_env);
        self.registrar(
            "_sistema_tiempo_actual_ms",
            crate::native_registry::temporizador::native_sistema_tiempo_ms,
        );
    }
}

/// Nombre del sistema operativo (`_sistema_so()`), como lo reporta Rust.
pub(crate) fn native_sistema_so(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let idx = vm.alloc_str(Arc::from(std::env::consts::OS));
    Ok(ValorFast::texto(idx))
}

/// Directorio de trabajo actual (`_sistema_directorio_actual()`).
pub(crate) fn native_sistema_directorio_actual(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let dir = std::env::current_dir().map_err(|e| {
        ErrFast::TipoInv(format!("no se pudo leer el directorio actual: {e}"))
    })?;
    let idx = vm.alloc_str(Arc::from(dir.to_string_lossy().as_ref()));
    Ok(ValorFast::texto(idx))
}
pub(crate) fn native_args(vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let args: Vec<ValorFast> = std::env::args()
        .map(|a| {
            let idx = vm.alloc_str(Arc::from(a.as_str()));
            ValorFast::texto(idx)
        })
        .collect();
    let arr_idx = vm.alloc_arr(args);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Termina el proceso con el código dado.
/// _salir(codigo: Entero)
pub(crate) fn native_salir(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let code = if args.is_empty() {
        0i32
    } else {
        obtener_entero(args[0])? as i32
    };
    std::process::exit(code);
}

/// Obtiene el valor de una variable de entorno.
/// _env(nombre: Texto) → Texto | nulo
pub(crate) fn native_env(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let nombre = obtener_texto(vm, args[0])?;
    match std::env::var(&nombre) {
        Ok(val) => {
            let idx = vm.alloc_str(Arc::from(val.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(_) => Ok(ValorFast::nulo()),
    }
}

/// Ejecuta un comando de shell y retorna su salida estándar como texto.
/// _ejecutar(cmd: Texto) → Texto  (o nulo si error)
pub(crate) fn native_ejecutar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_ejecutar requiere un argumento: comando (texto)".into(),
        ));
    }
    let cmd = obtener_texto(vm, args[0])?;

    #[cfg(target_os = "windows")]
    let resultado = std::process::Command::new("cmd")
        .args(["/C", &cmd])
        .output();

    #[cfg(not(target_os = "windows"))]
    let resultado = std::process::Command::new("sh").args(["-c", &cmd]).output();

    match resultado {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let idx = vm.alloc_str(Arc::from(stdout.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("_ejecutar: {}", e))),
    }
}

/// Lee una línea de la entrada estándar (stdin).
/// _leer_linea() → Texto  (sin el '\n' final)
pub(crate) fn native_leer_linea(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut line = String::new();
    match stdin.lock().read_line(&mut line) {
        Ok(_) => {
            // Eliminar el salto de línea final
            if line.ends_with('\n') {
                line.pop();
            }
            if line.ends_with('\r') {
                line.pop();
            }
            let idx = vm.alloc_str(Arc::from(line.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("_leer_linea: {}", e))),
    }
}

/// Imprime un mensaje en la salida de error estándar (stderr).
/// _imprimir_error(mensaje: Texto)
pub(crate) fn native_imprimir_error(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        eprintln!();
        return Ok(ValorFast::nulo());
    }
    let msg = obtener_texto(vm, args[0])?;
    eprintln!("{}", msg);
    Ok(ValorFast::nulo())
}

/// Convierte un código de punto Unicode (entero) en un texto de un carácter.
/// _char(codigo: Entero) → Texto  (o nulo si código inválido)
pub(crate) fn native_char(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let code = obtener_entero(args[0])?;
    match u32::try_from(code).ok().and_then(char::from_u32) {
        Some(c) => {
            let s: String = c.to_string();
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        None => Ok(ValorFast::nulo()),
    }
}

/// Retorna el código de punto Unicode del primer carácter de un texto.
/// _codigo_char(s: Texto) → Entero  (o nulo si vacío)
pub(crate) fn native_codigo_char(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let s = obtener_texto(vm, args[0])?;
    match s.chars().next() {
        Some(c) => Ok(ValorFast::entero(c as i64)),
        None => Ok(ValorFast::nulo()),
    }
}

/// Convierte un entero a texto en la base dada (2-36).
/// _numero_a_texto_base(n: Entero, base: Entero) → Texto
pub(crate) fn native_numero_a_texto_base(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_numero_a_texto_base requiere 2 argumentos: numero, base".into(),
        ));
    }
    let n = obtener_entero(args[0])?;
    let base = obtener_entero(args[1])?;

    if !(2..=36).contains(&base) {
        return Err(ErrFast::TipoInv(format!(
            "_numero_a_texto_base: base {} fuera de rango (2-36)",
            base
        )));
    }

    let base = base as u32;
    let result = if n < 0 {
        let mut digits = Vec::new();
        let mut val = (n as i128).unsigned_abs();
        if val == 0 {
            digits.push(b'0');
        }
        while val > 0 {
            let d = (val % base as u128) as u32;
            digits.push(if d < 10 {
                b'0' + d as u8
            } else {
                b'a' + (d - 10) as u8
            });
            val /= base as u128;
        }
        digits.reverse();
        format!("-{}", String::from_utf8(digits).unwrap_or_default())
    } else {
        let mut digits = Vec::new();
        let mut val = n as u64;
        if val == 0 {
            digits.push(b'0');
        }
        while val > 0 {
            let d = (val % base as u64) as u32;
            digits.push(if d < 10 {
                b'0' + d as u8
            } else {
                b'a' + (d - 10) as u8
            });
            val /= base as u64;
        }
        digits.reverse();
        String::from_utf8(digits).unwrap_or_default()
    };

    let idx = vm.alloc_str(Arc::from(result.as_str()));
    Ok(ValorFast::texto(idx))
}
