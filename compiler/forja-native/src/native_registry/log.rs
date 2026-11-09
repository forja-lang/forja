use super::*;

impl NativeRegistry {
    pub fn registrar_log(&mut self) {
        self.registrar("_log_escribir", native_log_escribir);
        self.registrar("_log_configurar", native_log_configurar);
    }
}

use std::fs::OpenOptions;
static LOG_FILE: OnceLock<Mutex<String>> = OnceLock::new();
static LOG_LEVEL: OnceLock<Mutex<String>> = OnceLock::new();
pub fn log_nivel_min() -> i32 {
    let nivel = LOG_LEVEL
        .get_or_init(|| Mutex::new("INFO".to_string()))
        .lock()
        .unwrap()
        .clone();
    match nivel.as_str() {
        "DEBUG" => 0,
        "INFO" => 1,
        "ADVERTENCIA" => 2,
        "ERROR" => 3,
        "CRITICAL" => 4,
        _ => 1,
    }
}

pub fn nivel_num(nivel: &str) -> i32 {
    match nivel {
        "DEBUG" => 0,
        "INFO" => 1,
        "ADVERTENCIA" => 2,
        "ERROR" => 3,
        "CRITICAL" => 4,
        _ => 1,
    }
}

pub fn native_log_escribir(_vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let nivel = obtener_texto(_vm, args[0])?;
    let mensaje = obtener_texto(_vm, args[1])?;
    if nivel_num(&nivel) < log_nivel_min() {
        return Ok(ValorFast::nulo());
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let linea = format!("[{}] [{}] {}\n", timestamp, nivel, mensaje);
    let ruta = LOG_FILE
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .unwrap()
        .clone();
    if ruta.is_empty() {
        eprint!("{}", linea);
    } else if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&ruta) {
        let _ = f.write_all(linea.as_bytes());
    }
    Ok(ValorFast::nulo())
}

pub fn native_log_configurar(_vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let nivel = obtener_texto(_vm, args[0])?;
    let archivo = obtener_texto(_vm, args[1])?;
    *LOG_LEVEL
        .get_or_init(|| Mutex::new("INFO".to_string()))
        .lock()
        .unwrap() = nivel;
    *LOG_FILE
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .unwrap() = archivo;
    Ok(ValorFast::nulo())
}
