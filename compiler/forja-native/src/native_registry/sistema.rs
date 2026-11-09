use super::*;

impl NativoRegistro {
    pub fn registrar_sistema(&mut self) {
        // ─── Sistema / OS ─────────────────────────────────────────────────────
        self.registrar("_args", nativo_args);
        self.registrar("_salir", nativo_salir);
        self.registrar("_env", nativo_env);
        self.registrar("_ejecutar", nativo_ejecutar);
        self.registrar("_leer_linea", nativo_leer_linea);
        self.registrar("_imprimir_error", nativo_imprimir_error);
        self.registrar("_char", nativo_caracter);
        self.registrar("_codigo_char", nativo_codigo_caracter);
        self.registrar("_numero_a_texto_base", nativo_numero_a_texto_base);
        // Las usa `std/sistema.fa`; faltaban en el registro, así que
        // `sistema.sistema_operativo()` y `directorio_actual()` fallaban con
        // "función no definida".
        self.registrar("_sistema_so", nativo_sistema_so);
        self.registrar(
            "_sistema_directorio_actual",
            nativo_sistema_directorio_actual,
        );
        // `std/sistema.fa` usa estos nombres; el registro tenía los cortos
        // (`_args`, `_env`, `_salir`, `_sistema_tiempo_ms`).
        self.registrar("_sistema_argumentos", nativo_args);
        self.registrar("_sistema_salir", nativo_salir);
        self.registrar("_sistema_variable_entorno", nativo_env);
        self.registrar(
            "_sistema_tiempo_actual_ms",
            crate::native_registry::temporizador::nativo_sistema_tiempo_ms,
        );
    }
}

/// Nombre del sistema operativo (`_sistema_so()`), como lo reporta Rust.
pub fn nativo_sistema_so(
    vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let indice = vm.reservar_texto(Arc::from(std::env::consts::OS));
    Ok(ValorFast::texto(indice))
}

/// Directorio de trabajo actual (`_sistema_directorio_actual()`).
pub fn nativo_sistema_directorio_actual(
    vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let directorio = std::env::current_dir()
        .map_err(|e| ErrFast::TipoInv(format!("no se pudo leer el directorio actual: {e}")))?;
    let indice = vm.reservar_texto(Arc::from(directorio.to_string_lossy().as_ref()));
    Ok(ValorFast::texto(indice))
}
pub fn nativo_args(vm: &mut dyn Anfitrion, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let args: Vec<ValorFast> = std::env::args()
        .map(|a| {
            let indice = vm.reservar_texto(Arc::from(a.as_str()));
            ValorFast::texto(indice)
        })
        .collect();
    let arr_idx = vm.reservar_arr(args);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Termina el proceso con el código dado.
/// _salir(codigo: Entero)
pub fn nativo_salir(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let codigo = if args.is_empty() {
        0i32
    } else {
        obtener_entero(args[0])? as i32
    };
    // Sin volcar la salida acumulada se pierde todo lo impreso antes de salir
    // (por ejemplo el resumen de un test que termina con `_salir(1)`).
    vm.volcar_salida();
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(codigo);
}

/// Obtiene el valor de una variable de entorno.
/// _env(nombre: Texto) → Texto | nulo
pub fn nativo_env(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let nombre = obtener_texto(vm, args[0])?;
    match std::env::var(&nombre) {
        Ok(val) => {
            let indice = vm.reservar_texto(Arc::from(val.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(_) => Ok(ValorFast::nulo()),
    }
}

/// Ejecuta un comando de shell y retorna su salida estándar como texto.
/// _ejecutar(cmd: Texto) → Texto  (o nulo si error)
pub fn nativo_ejecutar(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
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
            let indice = vm.reservar_texto(Arc::from(stdout.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("_ejecutar: {}", e))),
    }
}

/// Lee una línea de la entrada estándar (stdin).
/// _leer_linea() → Texto  (sin el '\n' final)
pub fn nativo_leer_linea(
    vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut linea = String::new();
    match stdin.lock().read_line(&mut linea) {
        Ok(_) => {
            // Eliminar el salto de línea final
            if linea.ends_with('\n') {
                linea.pop();
            }
            if linea.ends_with('\r') {
                linea.pop();
            }
            let indice = vm.reservar_texto(Arc::from(linea.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("_leer_linea: {}", e))),
    }
}

/// Imprime un mensaje en la salida de error estándar (stderr).
/// _imprimir_error(mensaje: Texto)
pub fn nativo_imprimir_error(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        eprintln!();
        return Ok(ValorFast::nulo());
    }
    let mensaje = obtener_texto(vm, args[0])?;
    eprintln!("{}", mensaje);
    Ok(ValorFast::nulo())
}

/// Convierte un código de punto Unicode (entero) en un texto de un carácter.
/// _char(codigo: Entero) → Texto  (o nulo si código inválido)
pub fn nativo_caracter(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let codigo = obtener_entero(args[0])?;
    match u32::try_from(codigo).ok().and_then(char::from_u32) {
        Some(c) => {
            let s: String = c.to_string();
            let indice = vm.reservar_texto(Arc::from(s.as_str()));
            Ok(ValorFast::texto(indice))
        }
        None => Ok(ValorFast::nulo()),
    }
}

/// Retorna el código de punto Unicode del primer carácter de un texto.
/// _codigo_char(s: Texto) → Entero  (o nulo si vacío)
pub fn nativo_codigo_caracter(
    vm: &mut dyn Anfitrion,
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
pub fn nativo_numero_a_texto_base(
    vm: &mut dyn Anfitrion,
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
    let resultado = if n < 0 {
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

    let indice = vm.reservar_texto(Arc::from(resultado.as_str()));
    Ok(ValorFast::texto(indice))
}
