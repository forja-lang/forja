//! Intérprete tree-walking de Forja, compartido por la GUI nativa y la web.
//!
//! Antes vivía dentro de `gui-rt` (y `wasm-gui` tenía otra copia). El almacén de
//! variables se recibe como trait (`AlmacenValores`) para que cada host use el
//! suyo sin duplicar el evaluador.
//!
pub mod valor;

pub use valor::Valor;
pub use valor::Valor as ValorGUI;

/// Lo que el evaluador necesita del almacén de variables del host.
///
/// La GUI nativa lo implementa con su `VariableAlmacen` reactivo y la web con su
/// mapa de `serde_json::Value`; el evaluador no sabe nada de ninguno de los dos.
pub trait AlmacenValores {
    fn obtener(&self, nombre: &str) -> Option<serde_json::Value>;
    fn fijar(&self, nombre: &str, valor: serde_json::Value);
}

// ─── evaluador (movido tal cual desde gui-rt, sin cambios de comportamiento) ───

//
// Reemplaza las funciones parciales ejecutar_callback_forja / evaluar_bloque / evaluar_expresion
// con un intérprete tree-walking que soporta TODO el AST de Forja.
//
// Arquitectura:
//   - Ambito:     ámbito de variables local (HashMap<String, Valor>)
//   - ejecutar_funcion: punto de entrada para ejecutar una función Forja
//   - evaluar_bloque:   evalúa una lista de declaraciones
//   - evaluar_expresion: evalúa una expresión y retorna Valor
//
// Integración:
//   - Escribe variables en VariableStore para que los widgets las vean
//   - Lee variables de VariableStore cuando no están en ámbito local
//   - Soporta llamadas a funciones definidas en el AST y funciones nativas

use forja_syntax::ast::*;
#[cfg(feature = "sqlite")]
use rusqlite::Connection;
use std::collections::HashMap;
use std::ops::{Add, Div, Mul, Sub};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Operadores aritméticos para Valor ────────────────────────

impl Add for Valor {
    type Output = Valor;
    fn add(self, other: Valor) -> Valor {
        match (self, other) {
            (Valor::Entero(a), Valor::Entero(b)) => Valor::Entero(a + b),
            (Valor::Decimal(a), Valor::Decimal(b)) => Valor::Decimal(a + b),
            (Valor::Entero(a), Valor::Decimal(b)) => Valor::Decimal(a as f64 + b),
            (Valor::Decimal(a), Valor::Entero(b)) => Valor::Decimal(a + b as f64),
            (Valor::Texto(a), Valor::Texto(b)) => Valor::Texto(a + &b),
            (Valor::Texto(a), b) => Valor::Texto(a + &b.to_string()),
            (a, Valor::Texto(b)) => Valor::Texto(a.to_string() + &b),
            _ => Valor::Nulo,
        }
    }
}

impl Sub for Valor {
    type Output = Valor;
    fn sub(self, other: Valor) -> Valor {
        match (self, other) {
            (Valor::Entero(a), Valor::Entero(b)) => Valor::Entero(a - b),
            (Valor::Decimal(a), Valor::Decimal(b)) => Valor::Decimal(a - b),
            (Valor::Entero(a), Valor::Decimal(b)) => Valor::Decimal(a as f64 - b),
            (Valor::Decimal(a), Valor::Entero(b)) => Valor::Decimal(a - b as f64),
            _ => Valor::Nulo,
        }
    }
}

impl Mul for Valor {
    type Output = Valor;
    fn mul(self, other: Valor) -> Valor {
        match (self, other) {
            (Valor::Entero(a), Valor::Entero(b)) => Valor::Entero(a * b),
            (Valor::Decimal(a), Valor::Decimal(b)) => Valor::Decimal(a * b),
            (Valor::Entero(a), Valor::Decimal(b)) => Valor::Decimal(a as f64 * b),
            (Valor::Decimal(a), Valor::Entero(b)) => Valor::Decimal(a * b as f64),
            _ => Valor::Nulo,
        }
    }
}

impl Div for Valor {
    type Output = Valor;
    fn div(self, other: Valor) -> Valor {
        match (self, other) {
            (Valor::Entero(a), Valor::Entero(b)) => {
                if b == 0 {
                    Valor::Nulo
                } else {
                    Valor::Entero(a / b)
                }
            }
            (Valor::Decimal(a), Valor::Decimal(b)) => {
                if b == 0.0 {
                    Valor::Nulo
                } else {
                    Valor::Decimal(a / b)
                }
            }
            (Valor::Entero(a), Valor::Decimal(b)) => {
                if b == 0.0 {
                    Valor::Nulo
                } else {
                    Valor::Decimal(a as f64 / b)
                }
            }
            (Valor::Decimal(a), Valor::Entero(b)) => {
                if b == 0 {
                    Valor::Nulo
                } else {
                    Valor::Decimal(a / b as f64)
                }
            }
            _ => Valor::Nulo,
        }
    }
}

impl PartialEq for Valor {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Valor::Entero(a), Valor::Entero(b)) => a == b,
            (Valor::Decimal(a), Valor::Decimal(b)) => (a - b).abs() < f64::EPSILON,
            (Valor::Texto(a), Valor::Texto(b)) => a == b,
            (Valor::Booleano(a), Valor::Booleano(b)) => a == b,
            (Valor::Nulo, Valor::Nulo) => true,
            // Cross-type: convertir a f64 para comparación numérica
            (Valor::Entero(a), Valor::Decimal(b)) => *a as f64 == *b,
            (Valor::Decimal(a), Valor::Entero(b)) => *a == *b as f64,
            _ => false,
        }
    }
}

// ─── Helpers de Valor ────────────────────────────────────────

impl Valor {
    /// Evalúa si el valor es "verdadero" en contexto booleano
    pub fn es_verdadero(&self) -> bool {
        match self {
            Valor::Booleano(b) => *b,
            Valor::Entero(n) => *n != 0,
            Valor::Decimal(n) => *n != 0.0,
            Valor::Texto(t) => !t.is_empty(),
            Valor::Mapa(m) => !m.is_empty(),
            Valor::Arreglo(items) => !items.is_empty(),
            Valor::Nulo => false,
        }
    }

    /// Convierte a serde_json::Value para almacenar en VariableStore
    pub fn to_json_valor(&self) -> serde_json::Value {
        match self {
            Valor::Texto(t) => serde_json::Value::String(t.clone()),
            Valor::Entero(n) => serde_json::Value::Number((*n).into()),
            Valor::Decimal(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Valor::Booleano(b) => serde_json::Value::Bool(*b),
            Valor::Mapa(m) => {
                let mut mapa = serde_json::Map::new();
                for (k, v) in m {
                    mapa.insert(k.clone(), v.to_json_valor());
                }
                serde_json::Value::Object(mapa)
            }
            Valor::Arreglo(items) => {
                serde_json::Value::Array(items.iter().map(|v| v.to_json_valor()).collect())
            }
            Valor::Nulo => serde_json::Value::Null,
        }
    }

    /// Convierte desde serde_json::Value
    pub fn from_serde(val: &serde_json::Value) -> Self {
        match val {
            serde_json::Value::String(s) => Valor::Texto(s.clone()),
            serde_json::Value::Number(n) => n
                .as_i64()
                .map(Valor::Entero)
                .or_else(|| n.as_f64().map(Valor::Decimal))
                .unwrap_or(Valor::Nulo),
            serde_json::Value::Bool(b) => Valor::Booleano(*b),
            serde_json::Value::Array(_arr) => {
                // Representar arrays como JSON textual para compatibilidad
                // con el resto del runtime (que maneja arrays como strings)
                let json_str = serde_json::to_string(val).unwrap_or_else(|_| "[]".to_string());
                Valor::Texto(json_str)
            }
            serde_json::Value::Object(_map) => {
                // Representar mapas como JSON textual para compatibilidad
                // con acceso por clave (mapa["clave"])
                let json_str = serde_json::to_string(val).unwrap_or_else(|_| "{}".to_string());
                Valor::Texto(json_str)
            }
            serde_json::Value::Null => Valor::Nulo,
        }
    }

    /// 5.3: Conversión lazy desde serde_json::Value.
    /// Para objetos, preserva la estructura como Mapa en vez de serializar a JSON string.
    /// Para arrays pequeños (≤10 elems), también preserva como Mapa con claves numéricas.
    pub fn from_serde_lazy(val: &serde_json::Value) -> Self {
        match val {
            serde_json::Value::Object(mapa) => {
                let inner: HashMap<String, Valor> = mapa
                    .iter()
                    .map(|(k, v)| (k.clone(), Valor::from_serde(v)))
                    .collect();
                Valor::Mapa(inner)
            }
            serde_json::Value::Array(arr) => {
                if arr.len() <= 10 {
                    let inner: Vec<Valor> = arr.iter().map(Valor::from_serde).collect();
                    // Usar Mapa con claves numéricas como workaround para arrays pequeños
                    let mapa: HashMap<String, Valor> = inner
                        .into_iter()
                        .enumerate()
                        .map(|(i, v)| (i.to_string(), v))
                        .collect();
                    Valor::Mapa(mapa)
                } else {
                    // Arrays grandes: serializar a JSON string (comportamiento original)
                    let json_str = val.to_string();
                    Valor::Texto(json_str)
                }
            }
            _ => Valor::from_serde(val),
        }
    }

    /// Comparación ordenada para <, <=, >, >=
    pub fn compare(&self, op: &Operador, other: &Valor) -> bool {
        let a = Valor::to_f64(self);
        let b = Valor::to_f64(other);
        match op {
            Operador::Menor => a < b,
            Operador::MenorIgual => a <= b,
            Operador::Mayor => a > b,
            Operador::MayorIgual => a >= b,
            _ => false,
        }
    }

    /// Convierte el valor a String (display)
    pub fn to_display(&self) -> String {
        match self {
            Valor::Texto(s) => s.clone(),
            Valor::Entero(n) => n.to_string(),
            Valor::Decimal(f) => f.to_string(),
            Valor::Booleano(b) => if *b { "verdadero" } else { "falso" }.to_string(),
            Valor::Mapa(m) => {
                let parts: Vec<String> = m
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_display()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            Valor::Arreglo(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.to_display()).collect();
                format!("[{}]", parts.join(", "))
            }
            Valor::Nulo => "nulo".to_string(),
        }
    }

    /// Convierte a lista de valores para iteración (para bucles `para`)
    pub fn a_lista(&self) -> Vec<Valor> {
        match self {
            Valor::Entero(n) => {
                if *n > 0 {
                    (0..*n).map(Valor::Entero).collect()
                } else {
                    vec![]
                }
            }
            Valor::Texto(t) => {
                if let Ok(serde_json::Value::Array(arr)) =
                    serde_json::from_str::<serde_json::Value>(t)
                {
                    arr.iter().map(Valor::from_serde).collect()
                } else {
                    vec![self.clone()]
                }
            }
            _ => vec![self.clone()],
        }
    }
}

// ─── Ámbito de variables ────────────────────────────────────────

/// Ámbito de variables locales para la evaluación de una función
pub struct Ambito {
    variables: HashMap<String, Valor>,
}

impl Default for Ambito {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Ambito {
    pub fn nuevo() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn obtener(&self, nombre: &str) -> Option<&Valor> {
        self.variables.get(nombre)
    }

    pub fn asignar(&mut self, nombre: String, valor: Valor) {
        self.variables.insert(nombre, valor);
    }

    pub fn contiene(&self, nombre: &str) -> bool {
        self.variables.contains_key(nombre)
    }

    /// Obtiene todas las variables del ámbito para su copia global
    pub fn obtener_todas(&self) -> &HashMap<String, Valor> {
        &self.variables
    }
}

// ═════════════════════════════════════════════════════════════════════════
// FUNCIONES NATIVAS (built-ins del compilador Forja)
// ═════════════════════════════════════════════════════════════════════════

/// Registro de funciones nativas: nombre → implementación
/// El segundo argumento `&mut VariableStore` permite a las funciones nativas
/// leer/escribir variables del store (útil para SQLite, callbacks, etc.).
type FuncionNativa = fn(&[Valor], &dyn AlmacenValores) -> Result<Valor, String>;

fn obtener_nativas() -> HashMap<&'static str, FuncionNativa> {
    let mut m: HashMap<&'static str, FuncionNativa> = HashMap::new();
    m.insert("timestamp", nativa_timestamp);
    m.insert("fecha_desde_timestamp", nativa_fecha_desde_timestamp);
    m.insert("_fecha_desde_timestamp", nativa_fecha_desde_timestamp);
    m.insert("dividir", nativa_dividir);
    m.insert("a_numero", nativa_a_numero);
    m.insert("longitud", nativa_longitud);
    m.insert("tipo", nativa_tipo);
    m.insert("a_texto", nativa_a_texto);
    m.insert("escribir", nativa_escribir);
    m.insert("empujar", nativa_empujar);
    // SQLite
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_abrir", sqlite_abrir);
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_cerrar", sqlite_cerrar);
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_ejecutar", sqlite_ejecutar);
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_consultar", sqlite_consultar);
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_ejecutar_params", sqlite_ejecutar_params);
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_consultar_params", sqlite_consultar_params);
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_ultimo_id", sqlite_ultimo_id);
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_tablas", sqlite_tablas);
    #[cfg(feature = "sqlite")]
    m.insert("_sqlite_columnas", sqlite_columnas);
    // Archivo
    m.insert("_archivo_leer", nativa_archivo_leer);
    m.insert("_archivo_escribir", nativa_archivo_escribir);
    // JSON
    m.insert("_json_stringificar", nativa_json_stringificar);
    m
}

/// `timestamp()` — tiempo actual en milisegundos desde Unix epoch
fn nativa_timestamp(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    if !args.is_empty() {
        return Err("timestamp() no requiere argumentos".to_string());
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Error obteniendo timestamp: {}", e))?
        .as_millis() as i64;
    Ok(Valor::Entero(ts))
}

/// `fecha_desde_timestamp(ts)` — formatea timestamp (ms) a "YYYY|MM|DD|HH|mm|ss|n_dia|n_mes"
fn nativa_fecha_desde_timestamp(
    args: &[Valor],
    _almacen: &dyn AlmacenValores,
) -> Result<Valor, String> {
    let ts = match args.first() {
        Some(Valor::Entero(n)) => *n,
        _ => return Err("fecha_desde_timestamp(ts) requiere un entero".to_string()),
    };

    // Si el timestamp está en milisegundos (> 1e12), convertir a segundos
    let ts_secs = if ts > 1_000_000_000_000 {
        ts / 1000
    } else {
        ts
    };

    // Calcular componentes de fecha
    let dias = ts_secs.div_euclid(86400);
    let segundos_del_dia = ts_secs.rem_euclid(86400);
    let hora = (segundos_del_dia / 3600) as u32;
    let minuto = ((segundos_del_dia % 3600) / 60) as u32;
    let segundo = (segundos_del_dia % 60) as u32;

    let (anio, mes, dia) = civil_from_days(dias);
    let dia_semana = day_of_week(anio, mes, dia);
    let nombre_dia = NOMBRES_DIA[dia_semana as usize];
    let nombre_mes = NOMBRES_MES[(mes - 1) as usize];

    let salida = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        anio, mes, dia, hora, minuto, segundo, nombre_dia, nombre_mes
    );
    Ok(Valor::Texto(salida))
}

/// `dividir(texto, separador)` — divide un string y retorna JSON array
fn nativa_dividir(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    if args.len() < 2 {
        return Err("dividir(texto, separador) requiere 2 argumentos".to_string());
    }
    let texto = match &args[0] {
        Valor::Texto(s) => s.clone(),
        Valor::Entero(n) => n.to_string(),
        Valor::Decimal(f) => f.to_string(),
        Valor::Booleano(b) => b.to_string(),
        Valor::Arreglo(_) | Valor::Mapa(_) | Valor::Nulo => String::new(),
        Valor::Mapa(m) => format!("{:?}", m),
        Valor::Nulo => String::new(),
    };
    let separador = match &args[1] {
        Valor::Texto(s) => s.clone(),
        _ => return Err("dividir: separador debe ser texto".to_string()),
    };

    let partes: Vec<serde_json::Value> = if separador.is_empty() {
        texto
            .chars()
            .map(|c| serde_json::Value::String(c.to_string()))
            .collect()
    } else {
        texto
            .split(&separador)
            .map(|s| serde_json::Value::String(s.to_string()))
            .collect()
    };

    Ok(Valor::Arreglo(
        partes.into_iter().map(|p| Valor::from_serde(&p)).collect(),
    ))
}

/// `a_numero(texto)` — convierte texto a entero (i64)
fn nativa_a_numero(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let val = args
        .first()
        .ok_or("a_numero(valor) requiere un argumento".to_string())?;
    match val {
        Valor::Entero(n) => Ok(Valor::Entero(*n)),
        Valor::Decimal(f) => Ok(Valor::Entero(*f as i64)),
        Valor::Booleano(b) => Ok(Valor::Entero(if *b { 1 } else { 0 })),
        Valor::Texto(s) => {
            let trimmed = s.trim();
            // Si el texto es JSON (objeto o array), no se puede convertir a número
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                return Ok(Valor::Entero(0));
            }
            match trimmed.parse::<i64>() {
                Ok(n) => Ok(Valor::Entero(n)),
                Err(_) => match trimmed.parse::<f64>() {
                    Ok(f) => Ok(Valor::Entero(f as i64)),
                    Err(_) => Ok(Valor::Entero(0)),
                },
            }
        }
        Valor::Mapa(_) | Valor::Arreglo(_) => Ok(Valor::Entero(0)),
        Valor::Nulo => Ok(Valor::Entero(0)),
    }
}

/// `longitud(valor)` — longitud de arreglo o de texto
fn nativa_longitud(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let val = args
        .first()
        .ok_or("longitud(valor) requiere 1 argumento".to_string())?;
    match val {
        Valor::Texto(s) => {
            // Intentar parsear como JSON array
            if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str::<serde_json::Value>(s)
            {
                Ok(Valor::Entero(arr.len() as i64))
            } else {
                // Contar caracteres (incluyendo emojis/unicode)
                Ok(Valor::Entero(s.chars().count() as i64))
            }
        }
        Valor::Entero(n) => Ok(Valor::Entero(*n)),
        Valor::Decimal(_) => Ok(Valor::Entero(1)),
        Valor::Booleano(_) => Ok(Valor::Entero(1)),
        Valor::Mapa(m) => Ok(Valor::Entero(m.len() as i64)),
        Valor::Arreglo(items) => Ok(Valor::Entero(items.len() as i64)),
        Valor::Nulo => Ok(Valor::Entero(0)),
    }
}

/// `tipo(valor)` — retorna el nombre del tipo como texto
fn nativa_tipo(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let val = args
        .first()
        .ok_or("tipo(valor) requiere 1 argumento".to_string())?;
    let nombre = match val {
        Valor::Entero(_) => "entero",
        Valor::Decimal(_) => "decimal",
        Valor::Texto(s) => {
            // El runtime Forja representa arrays como JSON textual.
            // Detectar JSON arrays (ej: "[1,2,3]") y retornar "arreglo".
            let trimmed = s.trim_start();
            if trimmed.starts_with('[') {
                "arreglo"
            } else if trimmed.starts_with('{') {
                "mapa"
            } else {
                "texto"
            }
        }
        Valor::Booleano(_) => "booleano",
        Valor::Mapa(_) => "mapa",
        Valor::Arreglo(_) => "arreglo",
        Valor::Nulo => "nulo",
    };
    Ok(Valor::Texto(nombre.to_string()))
}

/// `_archivo_leer(ruta)` — lee texto de un archivo en disco
fn nativa_archivo_leer(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let ruta = match args.first() {
        Some(Valor::Texto(t)) => t.clone(),
        _ => return Err("_archivo_leer(ruta): ruta debe ser texto".to_string()),
    };
    let contenido = std::fs::read_to_string(&ruta)
        .map_err(|e| format!("Error al leer archivo '{}': {}", ruta, e))?;
    Ok(Valor::Texto(contenido))
}

/// `_archivo_escribir(ruta, contenido)` — escribe texto a un archivo en disco
fn nativa_archivo_escribir(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let ruta = match args.first() {
        Some(Valor::Texto(t)) => t.clone(),
        _ => return Err("_archivo_escribir(ruta, contenido): ruta debe ser texto".to_string()),
    };
    let contenido = match args.get(1) {
        Some(Valor::Texto(t)) => t.clone(),
        _ => {
            return Err("_archivo_escribir(ruta, contenido): contenido debe ser texto".to_string())
        }
    };
    std::fs::write(&ruta, &contenido)
        .map_err(|e| format!("Error al escribir archivo '{}': {}", ruta, e))?;
    Ok(Valor::Entero(contenido.len() as i64))
}

/// `_json_stringificar(valor)` — convierte cualquier valor a JSON string
fn nativa_json_stringificar(
    args: &[Valor],
    _almacen: &dyn AlmacenValores,
) -> Result<Valor, String> {
    let val = args
        .first()
        .ok_or("_json_stringificar(valor) requiere 1 argumento".to_string())?;
    let json_valor = val.to_json_valor();
    let json_str = serde_json::to_string(&json_valor)
        .map_err(|e| format!("Error al stringificar a JSON: {}", e))?;
    Ok(Valor::Texto(json_str))
}

/// `a_texto(valor)` — convierte cualquier valor a texto
fn nativa_a_texto(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let val = args
        .first()
        .ok_or("a_texto(valor) requiere 1 argumento".to_string())?;
    Ok(Valor::Texto(val.to_display()))
}

/// `escribir(valor…)` — imprime los argumentos separados por espacio.
///
/// Las funciones Forja llamadas como callbacks se evalúan con este evaluador y
/// no con la VM, así que `escribir` faltaba y todo callback que lo usara fallaba
/// con «Función 'escribir' no encontrada».
fn nativa_escribir(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let texto = args
        .iter()
        .map(|v| v.to_display())
        .collect::<Vec<_>>()
        .join(" ");
    println!("{}", texto);
    Ok(Valor::Nulo)
}

/// `empujar(array, elemento)` — agrega un elemento al final de un array JSON
/// El array se representa como texto conteniendo un JSON array: "[1,2,3]"
/// Retorna el nuevo array serializado como texto.
fn nativa_empujar(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    if args.len() < 2 {
        return Err("empujar(array, elemento) requiere 2 argumentos".to_string());
    }

    let json_str = match &args[0] {
        Valor::Texto(s) => s.clone(),
        _ => return Err("empujar: el primer argumento debe ser un array (texto)".to_string()),
    };

    let mut arr: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("empujar: error parseando array: {}", e))?;

    let arr_ref = match &mut arr {
        serde_json::Value::Array(a) => a,
        _ => return Err("empujar: el primer argumento no es un array JSON".to_string()),
    };

    let valor_json = args[1].to_json_valor();
    arr_ref.push(valor_json);

    let resultado = serde_json::to_string(&arr)
        .map_err(|e| format!("empujar: error serializando array: {}", e))?;
    Ok(Valor::Texto(resultado))
}

// ═════════════════════════════════════════════════════════════════════════
// POOL DE CONEXIONES SQLITE
// ═════════════════════════════════════════════════════════════════════════

/// Heap global de conexiones SQLite (envueltas en Arc<Mutex<>>).
/// Cada conexión se almacena como `Option` para permitir cerrado lógico.
#[cfg(feature = "sqlite")]
static MONTICULO_SQLITE: Mutex<Vec<Option<Arc<Mutex<Connection>>>>> = Mutex::new(Vec::new());

/// Extrae un índice entero de un Valor, aceptando Entero y Decimal.
fn extraer_indice(valor: &Valor, nombre_fn: &str) -> Result<usize, String> {
    match valor {
        Valor::Entero(n) => Ok(*n as usize),
        Valor::Decimal(f) => Ok(*f as usize),
        _ => Err(format!(
            "{}: primer argumento debe ser entero (índice), se recibió: {}",
            nombre_fn,
            valor.to_display()
        )),
    }
}

/// Obtiene una conexión del heap por índice (retorna Arc clonado).
#[cfg(feature = "sqlite")]
fn sqlite_obtener_conn(idx: usize) -> Result<Arc<Mutex<Connection>>, String> {
    let monticulo = MONTICULO_SQLITE
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    if idx >= monticulo.len() {
        return Err("sqlite_error: índice de conexión inválido".to_string());
    }
    match &monticulo[idx] {
        Some(conn) => Ok(Arc::clone(conn)),
        None => Err("sqlite_error: conexión cerrada o inexistente".to_string()),
    }
}

/// Convierte un Value de SQLite a serde_json::Value para serialización.
#[cfg(feature = "sqlite")]
fn sqlite_valor_a_json(val: rusqlite::types::Value) -> serde_json::Value {
    match val {
        rusqlite::types::Value::Null => serde_json::Value::Null,
        rusqlite::types::Value::Integer(n) => serde_json::Value::Number(n.into()),
        rusqlite::types::Value::Real(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        rusqlite::types::Value::Text(s) => serde_json::Value::String(s),
        rusqlite::types::Value::Blob(b) => {
            serde_json::Value::String(String::from_utf8_lossy(&b).to_string())
        }
    }
}

/// Parsea un Valor::Texto como JSON array para extraer parámetros SQL.
#[cfg(feature = "sqlite")]
fn args_a_params(val: &Valor) -> Result<Vec<Box<dyn rusqlite::types::ToSql>>, String> {
    let json_str = match val {
        Valor::Texto(s) => s.clone(),
        Valor::Arreglo(_) => {
            // Ya es un arreglo: se convierte a JSON para reusar el parseo de abajo.
            serde_json::to_string(&val.to_json_valor()).unwrap_or_default()
        }
        Valor::Entero(n) => return Ok(vec![Box::new(*n)]),
        Valor::Decimal(f) => return Ok(vec![Box::new(*f)]),
        Valor::Booleano(b) => return Ok(vec![Box::new(*b as i64)]),
        Valor::Mapa(m) => {
            // Intentar extraer un arreglo de valores del mapa
            // Si el mapa tiene una clave "0", "1", etc., tratarlo como arreglo
            let mut arr = Vec::new();
            let mut i = 0;
            loop {
                let clave = i.to_string();
                if let Some(v) = m.get(&clave) {
                    arr.push(v.to_json_valor());
                    i += 1;
                } else {
                    break;
                }
            }
            if !arr.is_empty() {
                serde_json::to_string(&serde_json::Value::Array(arr)).map_err(|e| {
                    format!(
                        "sqlite_error: no se pudo serializar mapa como arreglo: {}",
                        e
                    )
                })?
            } else {
                // Fallback: serializar como objeto
                let mapa: serde_json::Map<String, serde_json::Value> = m
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json_valor()))
                    .collect();
                serde_json::to_string(&serde_json::Value::Object(mapa))
                    .map_err(|e| format!("sqlite_error: no se pudo serializar mapa: {}", e))?
            }
        }
        Valor::Nulo => {
            return Ok(vec![Box::new(rusqlite::types::Null)]);
        }
    };
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("sqlite_error: no se pudieron parsear los parámetros: {}", e))?;
    let arr = match parsed {
        serde_json::Value::Array(a) => a,
        _ => return Err("sqlite_error: los parámetros deben ser un arreglo JSON".to_string()),
    };
    arr.is_empty();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::with_capacity(arr.len());
    for v in &arr {
        match v {
            serde_json::Value::String(s) => params.push(Box::new(s.clone())),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    params.push(Box::new(i));
                } else if let Some(f) = n.as_f64() {
                    params.push(Box::new(f));
                }
            }
            serde_json::Value::Bool(b) => params.push(Box::new(*b as i64)),
            serde_json::Value::Null => params.push(Box::new(rusqlite::types::Null)),
            _ => return Err("sqlite_error: tipo de parámetro no soportado".to_string()),
        }
    }
    Ok(params)
}

// ═════════════════════════════════════════════════════════════════════════
// FUNCIONES SQLITE NATIVAS
// ═════════════════════════════════════════════════════════════════════════

/// `_sqlite_abrir(ruta)` → índice de conexión (entero)
#[cfg(feature = "sqlite")]
fn sqlite_abrir(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let ruta = match args.first() {
        Some(Valor::Texto(s)) => s.clone(),
        _ => return Err("_sqlite_abrir(ruta) requiere 1 argumento: ruta (texto)".to_string()),
    };
    let conn = Connection::open(&ruta).map_err(|e| format!("sqlite_error_apertura: {}", e))?;
    let mut monticulo = MONTICULO_SQLITE
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    let idx = monticulo.len() as i64;
    monticulo.push(Some(Arc::new(Mutex::new(conn))));
    Ok(Valor::Entero(idx))
}

/// `_sqlite_cerrar(indice_conexion)` → 0 éxito, -1 error
#[cfg(feature = "sqlite")]
fn sqlite_cerrar(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let idx = match args.first() {
        Some(v) => extraer_indice(v, "_sqlite_cerrar")?,
        None => return Err("_sqlite_cerrar(indice) requiere 1 argumento: entero".to_string()),
    };
    let mut monticulo = MONTICULO_SQLITE
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    if idx < monticulo.len() {
        monticulo[idx] = None;
        Ok(Valor::Entero(0))
    } else {
        Err("sqlite_error: índice de conexión inválido".to_string())
    }
}

/// `_sqlite_ejecutar(indice_conexion, sql)` → filas_afectadas
#[cfg(feature = "sqlite")]
fn sqlite_ejecutar(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    if args.len() < 2 {
        return Err("_sqlite_ejecutar(indice, sql) requiere 2 argumentos".to_string());
    }
    let idx = extraer_indice(&args[0], "_sqlite_ejecutar")?;
    let sql = match &args[1] {
        Valor::Texto(s) => s.clone(),
        _ => return Err("_sqlite_ejecutar: segundo argumento debe ser texto (SQL)".to_string()),
    };
    let conn_arc = sqlite_obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    match conn.execute(&sql, []) {
        Ok(filas) => Ok(Valor::Entero(filas as i64)),
        Err(e) => Err(format!("sqlite_error_ejecucion: {}", e)),
    }
}

/// `_sqlite_consultar(indice_conexion, sql)` → JSON array de mapas (como texto)
#[cfg(feature = "sqlite")]
fn sqlite_consultar(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    if args.len() < 2 {
        return Err("_sqlite_consultar(indice, sql) requiere 2 argumentos".to_string());
    }
    let idx = extraer_indice(&args[0], "_sqlite_consultar")?;
    let sql = match &args[1] {
        Valor::Texto(s) => s.clone(),
        _ => return Err("_sqlite_consultar: segundo argumento debe ser texto (SQL)".to_string()),
    };
    let conn_arc = sqlite_obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("sqlite_error_consulta: {}", e))?;
    let col_cantidad = stmt.column_count();
    let col_names: Vec<String> = (0..col_cantidad)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    // Extraer datos sin mantener borrow del closure
    let rows_data: Vec<Vec<(String, rusqlite::types::Value)>> = stmt
        .query_map([], |row| {
            let mut row_data = Vec::with_capacity(col_cantidad);
            for (i, name) in col_names.iter().enumerate().take(col_cantidad) {
                let name = name.clone();
                let val = row
                    .get::<_, rusqlite::types::Value>(i)
                    .unwrap_or(rusqlite::types::Value::Null);
                row_data.push((name, val));
            }
            Ok(row_data)
        })
        .map_err(|e| format!("sqlite_error_consulta: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    // Convertir a JSON
    let json_rows: Vec<serde_json::Value> = rows_data
        .into_iter()
        .map(|row| {
            let mut mapa = serde_json::Map::new();
            for (name, val) in row {
                mapa.insert(name, sqlite_valor_a_json(val));
            }
            serde_json::Value::Object(mapa)
        })
        .collect();

    let json_str = serde_json::to_string(&serde_json::Value::Array(json_rows))
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    Ok(Valor::Texto(json_str))
}

/// `_sqlite_ultimo_id(indice_conexion)` → último rowid insertado
#[cfg(feature = "sqlite")]
fn sqlite_ultimo_id(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let idx = match args.first() {
        Some(v) => extraer_indice(v, "_sqlite_ultimo_id")?,
        None => return Err("_sqlite_ultimo_id(indice) requiere 1 argumento: entero".to_string()),
    };
    let conn_arc = sqlite_obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    Ok(Valor::Entero(conn.last_insert_rowid()))
}

/// `_sqlite_ejecutar_params(indice, sql, arreglo_valores)` → filas_afectadas
#[cfg(feature = "sqlite")]
fn sqlite_ejecutar_params(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    if args.len() < 3 {
        return Err(
            "_sqlite_ejecutar_params(indice, sql, valores) requiere 3 argumentos".to_string(),
        );
    }
    let idx = extraer_indice(&args[0], "_sqlite_ejecutar_params")?;
    let sql = match &args[1] {
        Valor::Texto(s) => s.clone(),
        _ => {
            return Err(
                "_sqlite_ejecutar_params: segundo argumento debe ser texto (SQL)".to_string(),
            )
        }
    };
    let params = args_a_params(&args[2])?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let conn_arc = sqlite_obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    match conn.execute(&sql, params_refs.as_slice()) {
        Ok(filas) => Ok(Valor::Entero(filas as i64)),
        Err(e) => Err(format!("sqlite_error_ejecucion: {}", e)),
    }
}

/// `_sqlite_consultar_params(indice, sql, arreglo_valores)` → JSON array de mapas
#[cfg(feature = "sqlite")]
fn sqlite_consultar_params(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    if args.len() < 3 {
        return Err(
            "_sqlite_consultar_params(indice, sql, valores) requiere 3 argumentos".to_string(),
        );
    }
    let idx = extraer_indice(&args[0], "_sqlite_consultar_params")?;
    let sql = match &args[1] {
        Valor::Texto(s) => s.clone(),
        _ => {
            return Err(
                "_sqlite_consultar_params: segundo argumento debe ser texto (SQL)".to_string(),
            )
        }
    };
    let params = args_a_params(&args[2])?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let conn_arc = sqlite_obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("sqlite_error_consulta: {}", e))?;
    let col_cantidad = stmt.column_count();
    let col_names: Vec<String> = (0..col_cantidad)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let rows_data: Vec<Vec<(String, rusqlite::types::Value)>> = stmt
        .query_map(params_refs.as_slice(), |row| {
            let mut row_data = Vec::with_capacity(col_cantidad);
            for (i, name) in col_names.iter().enumerate().take(col_cantidad) {
                let name = name.clone();
                let val = row
                    .get::<_, rusqlite::types::Value>(i)
                    .unwrap_or(rusqlite::types::Value::Null);
                row_data.push((name, val));
            }
            Ok(row_data)
        })
        .map_err(|e| format!("sqlite_error_consulta: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let json_rows: Vec<serde_json::Value> = rows_data
        .into_iter()
        .map(|row| {
            let mut mapa = serde_json::Map::new();
            for (name, val) in row {
                mapa.insert(name, sqlite_valor_a_json(val));
            }
            serde_json::Value::Object(mapa)
        })
        .collect();

    let json_str = serde_json::to_string(&serde_json::Value::Array(json_rows))
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    Ok(Valor::Texto(json_str))
}

/// `_sqlite_tablas(indice_conexion)` → JSON array de nombres de tablas (como texto)
#[cfg(feature = "sqlite")]
fn sqlite_tablas(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    let idx = match args.first() {
        Some(v) => extraer_indice(v, "_sqlite_tablas")?,
        None => return Err("_sqlite_tablas(indice) requiere 1 argumento: entero".to_string()),
    };
    let conn_arc = sqlite_obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;

    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    ).map_err(|e| format!("sqlite_error_consulta: {}", e))?;

    let nombres: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("sqlite_error_consulta: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let json_arr: Vec<serde_json::Value> =
        nombres.into_iter().map(serde_json::Value::String).collect();
    let json_str = serde_json::to_string(&serde_json::Value::Array(json_arr))
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    Ok(Valor::Texto(json_str))
}

/// `_sqlite_columnas(indice_conexion, tabla)` → JSON array de mapas con info de columnas
#[cfg(feature = "sqlite")]
fn sqlite_columnas(args: &[Valor], _almacen: &dyn AlmacenValores) -> Result<Valor, String> {
    if args.len() < 2 {
        return Err("_sqlite_columnas(indice, tabla) requiere 2 argumentos".to_string());
    }
    let idx = extraer_indice(&args[0], "_sqlite_columnas")?;
    let tabla = match &args[1] {
        Valor::Texto(s) => s.clone(),
        _ => return Err("_sqlite_columnas: segundo argumento debe ser texto (tabla)".to_string()),
    };
    let conn_arc = sqlite_obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;

    let sql = format!("PRAGMA table_info('{}')", tabla.replace('\'', "''"));
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("sqlite_error_consulta: {}", e))?;

    // Extraer datos crudos
    let cols: Vec<(i64, String, String, bool, Option<String>, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| format!("sqlite_error_consulta: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let json_rows: Vec<serde_json::Value> = cols
        .into_iter()
        .map(|(cid, nombre, tipo, not_null, default, pk)| {
            let mut mapa = serde_json::Map::new();
            mapa.insert("cid".to_string(), serde_json::Value::Number(cid.into()));
            mapa.insert("nombre".to_string(), serde_json::Value::String(nombre));
            mapa.insert("tipo".to_string(), serde_json::Value::String(tipo));
            mapa.insert("not_null".to_string(), serde_json::Value::Bool(not_null));
            mapa.insert(
                "default".to_string(),
                match default {
                    Some(d) => serde_json::Value::String(d),
                    None => serde_json::Value::Null,
                },
            );
            mapa.insert("pk".to_string(), serde_json::Value::Number(pk.into()));
            serde_json::Value::Object(mapa)
        })
        .collect();

    let json_str = serde_json::to_string(&serde_json::Value::Array(json_rows))
        .map_err(|e| format!("sqlite_error_interno: {}", e))?;
    Ok(Valor::Texto(json_str))
}

// ═════════════════════════════════════════════════════════════════════════
// Algoritmos de fecha (copiados de native_registry.rs)
// ═════════════════════════════════════════════════════════════════════════

/// Algoritmo: días desde epoch (1970-01-01) hasta una fecha civil (año, mes, día)
/// Basado en el algoritmo de Howard Hinnant (calendario Gregoriano)
#[allow(dead_code)]
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m_shifted = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * m_shifted as i64 + 2) / 5 + day as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Algoritmo inverso: timestamp → componentes de fecha civil
/// Retorna (year, month, day)
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468; // days since 0000-03-01
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month progress [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

/// Día de la semana (0=Domingo, 1=Lunes, ..., 6=Sábado)
fn day_of_week(y: i64, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d as i64) % 7) as u32
}

const NOMBRES_DIA: [&str; 7] = [
    "domingo",
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
];

const NOMBRES_MES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

// ─── Señales de control ────────────────────────
//
// `romper` y `continuar` tienen que atravesar bloques anidados (`si`, `coincidir`)
// hasta el bucle. Se usan dos textos imposibles como señal por el canal de error:
// antes se evaluaban a `Nulo` y se perdían, así que `mientras (verdadero) {
// si (...) { romper } }` **nunca** terminaba (colgaba la ventana).
const SENAL_ROMPER: &str = "\u{0}romper";
const SENAL_CONTINUAR: &str = "\u{0}continuar";

// ─── Evaluación de funciones ─────────────────────────────────────

/// Evalúa una función Forja a partir del AST
pub fn ejecutar_funcion(
    nombre: &str,
    args: &[Valor],
    declaraciones: &[Declaracion],
    almacen: &dyn AlmacenValores,
) -> Result<Valor, String> {
    // 1. Verificar si es una función nativa
    let nativas = obtener_nativas();
    if let Some(nativa_fn) = nativas.get(nombre) {
        return nativa_fn(args, almacen);
    }

    // 2. Buscar la función en las declaraciones del AST
    buscar_funcion(nombre, declaraciones).and_then(|func| {
        // Extraer los campos de la función
        let (parametros, cuerpo) = match func {
            Declaracion::Funcion {
                parametros, cuerpo, ..
            } => (parametros, cuerpo),
            _ => return Err(format!("'{}' no es una función", nombre)),
        };
        let mut ambito = Ambito::nuevo();

        // Asignar parámetros: args explícitos primero, luego cargar desde store
        for (i, param) in parametros.iter().enumerate() {
            if let Some(val) = args.get(i) {
                ambito.asignar(param.nombre.clone(), val.clone());
            } else if let Some(json_val) = almacen.obtener(&param.nombre) {
                ambito.asignar(param.nombre.clone(), Valor::from_serde(&json_val));
            }
        }

        // Fallback: cuando la función se invoca como callback desde widgets GUI
        if args.is_empty() && !parametros.is_empty() {
            if let Some(json_val) = almacen.obtener(nombre) {
                let primer_param = &parametros[0];
                if !ambito.contiene(&primer_param.nombre) {
                    ambito.asignar(primer_param.nombre.clone(), Valor::from_serde(&json_val));
                }
            }
        }

        // Evaluar cuerpo
        evaluar_bloque(cuerpo, &mut ambito, almacen, declaraciones)
    })
}

/// Evalúa una condición contra el estado actual del store.
///
/// Lo usa el extractor de layout para resolver `si/sino` en el cuerpo de `main`
/// con el estado inicial de la app (el layout se extrae una sola vez).
pub fn evaluar_condicion(
    expr: &Expresion,
    almacen: &dyn AlmacenValores,
    declaraciones: &[Declaracion],
) -> Result<bool, String> {
    let mut ambito = Ambito::nuevo();
    let valor = evaluar_expresion(expr, &mut ambito, almacen, declaraciones)?;
    Ok(valor_es_verdadero(&valor))
}

/// Evalúa una expresión y devuelve su valor, con el estado actual.
///
/// Lo usa el extractor de layout para resolver llamadas a **funciones de valor**
/// (por ejemplo `titulo_categoria(pagina)`) en el momento de armar el árbol.
pub fn evaluar_valor(
    expr: &Expresion,
    almacen: &dyn AlmacenValores,
    declaraciones: &[Declaracion],
) -> Result<Valor, String> {
    let mut ambito = Ambito::nuevo();
    evaluar_expresion(expr, &mut ambito, almacen, declaraciones)
}

/// Verdad de un valor en un contexto booleano (condiciones de `si`/`mientras`).
fn valor_es_verdadero(valor: &Valor) -> bool {
    match valor {
        Valor::Booleano(b) => *b,
        Valor::Entero(n) => *n != 0,
        Valor::Decimal(d) => *d != 0.0,
        Valor::Texto(s) => !s.is_empty(),
        Valor::Nulo => false,
        _ => true,
    }
}

/// Busca una declaración de función por nombre.
/// Si no encuentra por nombre exacto, intenta sin el prefijo de módulo
/// (ej: "estadisticas_gastos_por_categoria" → busca "gastos_por_categoria").
// ─── Clases: instancias, `este` y métodos ──────────────────────────

/// Busca la clase declarada con ese nombre.
fn buscar_clase<'a>(nombre: &str, declaraciones: &'a [Declaracion]) -> Option<&'a Declaracion> {
    declaraciones.iter().find(|d| {
        matches!(d, Declaracion::Clase { nombre: n, .. } if n == nombre)
    })
}

/// Busca un método (o el `constructor`) dentro de la clase.
fn buscar_metodo<'a>(
    clase: &str,
    metodo: &str,
    declaraciones: &'a [Declaracion],
) -> Option<&'a forja_syntax::ast::Metodo> {
    match buscar_clase(clase, declaraciones)? {
        Declaracion::Clase { metodos, .. } => metodos.iter().find(|m| m.nombre == metodo),
        _ => None,
    }
}

/// Nombre de la clase de una instancia (`Valor::Mapa` con `__clase`).
fn clase_de(valor: &Valor) -> Option<String> {
    match valor {
        Valor::Mapa(m) => match m.get("__clase") {
            Some(Valor::Texto(nombre)) => Some(nombre.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Ejecuta un método con `este` ligado a la instancia y devuelve la instancia
/// actualizada (para que una mutación dentro del método se vea afuera).
fn ejecutar_metodo(
    metodo: &forja_syntax::ast::Metodo,
    instancia: Valor,
    args: &[Valor],
    almacen: &dyn AlmacenValores,
    declaraciones: &[Declaracion],
) -> Result<(Valor, Valor), String> {
    let mut ambito = Ambito::nuevo();
    let este = instancia.clone();
    ambito.asignar("este".to_string(), este.clone());
    ambito.asignar("self".to_string(), este);
    for (i, parametro) in metodo.parametros.iter().enumerate() {
        let valor = args.get(i).cloned().unwrap_or(Valor::Nulo);
        ambito.asignar(parametro.nombre.clone(), valor);
    }
    let retorno = evaluar_bloque(&metodo.cuerpo, &mut ambito, almacen, declaraciones)?;
    // El AST llama `self` a `este`; se prefiere esa copia porque es la que el
    // método mutó.
    let actualizada = ambito
        .obtener("self")
        .or_else(|| ambito.obtener("este"))
        .cloned()
        .unwrap_or(instancia);
    Ok((actualizada, retorno))
}

fn buscar_funcion<'a>(
    nombre: &str,
    declaraciones: &'a [Declaracion],
) -> Result<&'a Declaracion, String> {
    // Primera pasada: búsqueda exacta
    for d in declaraciones {
        if let Declaracion::Funcion { nombre: ref n, .. } = d {
            if n == nombre {
                return Ok(d);
            }
        }
    }

    // Segunda pasada: si el nombre tiene formato modulo_funcion,
    // buscar solo la parte después del primer _
    if let Some(pos) = nombre.find('_') {
        let sin_prefijo = &nombre[pos + 1..];
        if !sin_prefijo.is_empty() {
            for d in declaraciones {
                if let Declaracion::Funcion { nombre: ref n, .. } = d {
                    if n == sin_prefijo {
                        return Ok(d);
                    }
                }
            }
        }
    }

    Err(format!("Función '{}' no encontrada", nombre))
}

/// Verifica si un nombre corresponde a una función ejecutable real
/// (nativa o declarada en el AST). Los widgets del layout GUI
/// (andamio, barra_superior, navegador, pantalla, columna, boton, ...)
/// no son funciones ejecutables del programa.
fn es_funcion_ejecutable(nombre: &str, declaraciones: &[Declaracion]) -> bool {
    let nativas = obtener_nativas();
    if nativas.contains_key(nombre) {
        return true;
    }
    buscar_funcion(nombre, declaraciones).is_ok()
}

// ─── Evaluación de bloques ──────────────────────────────────────

/// Evalúa un bloque de declaraciones.
/// Retorna Ok(Valor) si encuentra un `retornar`, o el último valor del bloque.
fn evaluar_bloque(
    bloque: &[Declaracion],
    ambito: &mut Ambito,
    almacen: &dyn AlmacenValores,
    declaraciones: &[Declaracion],
) -> Result<Valor, String> {
    let mut ultimo = Valor::Nulo;
    for declaracion in bloque {
        let resultado = evaluar_declaracion(declaracion, ambito, almacen, declaraciones)?;
        // `retornar` directo, o un `retornar` escondido dentro de un bloque
        // (`si`, `mientras`, `para`, `repetir`, `coincidir`, `cuando`): se
        // propaga. Antes sólo se miraba `retornar` al tope, así que
        // `si (x) { retornar 1 }` se perdía y la función seguía de largo.
        if es_retorno(declaracion)
            || (es_compuesta(declaracion) && !matches!(resultado, Valor::Nulo))
        {
            return Ok(resultado);
        }
        ultimo = resultado;
    }
    Ok(ultimo)
}

/// ¿La declaración es un bloque que puede contener un `retornar`?
fn es_compuesta(decl: &Declaracion) -> bool {
    matches!(
        decl,
        Declaracion::Si { .. }
            | Declaracion::Mientras { .. }
            | Declaracion::Para { .. }
            | Declaracion::Repetir { .. }
            | Declaracion::Cuando { .. }
    )
}

/// Indica si una declaración es de tipo retornar
fn es_retorno(decl: &Declaracion) -> bool {
    matches!(decl, Declaracion::Retornar { .. })
}

/// Evalúa una declaración individual.
/// Retorna el valor si es una declaración de retorno, o Valor::Nulo en otro caso.
fn evaluar_declaracion(
    decl: &Declaracion,
    ambito: &mut Ambito,
    almacen: &dyn AlmacenValores,
    declaraciones: &[Declaracion],
) -> Result<Valor, String> {
    match decl {
        Declaracion::Retornar { valor } => {
            if let Some(expr) = valor {
                evaluar_expresion(expr, ambito, almacen, declaraciones)
            } else {
                Ok(Valor::Nulo)
            }
        }

        Declaracion::Si {
            condicion,
            bloque_verdadero,
            bloque_falso,
        } => {
            let cond_val = evaluar_expresion(condicion, ambito, almacen, declaraciones)?;
            if cond_val.es_verdadero() {
                evaluar_bloque(bloque_verdadero, ambito, almacen, declaraciones)
            } else if let Some(sino_bloque) = bloque_falso {
                evaluar_bloque(sino_bloque, ambito, almacen, declaraciones)
            } else {
                Ok(Valor::Nulo)
            }
        }

        Declaracion::Mientras { condicion, bloque } => {
            loop {
                let cond_val = evaluar_expresion(condicion, ambito, almacen, declaraciones)?;
                if !cond_val.es_verdadero() {
                    break;
                }
                match evaluar_bloque(bloque, ambito, almacen, declaraciones) {
                    Ok(Valor::Nulo) => {}
                    Ok(valor) => return Ok(valor),
                    Err(senal) if senal == SENAL_ROMPER => break,
                    Err(senal) if senal == SENAL_CONTINUAR => continue,
                    Err(error) => return Err(error),
                }
            }
            Ok(Valor::Nulo)
        }

        Declaracion::Para {
            inicializacion,
            condicion,
            incremento,
            bloque,
        } => {
            // Estilo C: for (init; cond; inc) { cuerpo }
            if let Some(init) = inicializacion {
                evaluar_declaracion(init, ambito, almacen, declaraciones)?;
            }
            loop {
                if let Some(cond) = condicion {
                    let cond_val = evaluar_expresion(cond, ambito, almacen, declaraciones)?;
                    if !cond_val.es_verdadero() {
                        break;
                    }
                }
                // `continuar` en un `para` salta al incremento, no al inicio.
                match evaluar_bloque(bloque, ambito, almacen, declaraciones) {
                    Ok(Valor::Nulo) => {}
                    Ok(valor) => return Ok(valor),
                    Err(senal) if senal == SENAL_ROMPER => break,
                    Err(senal) if senal == SENAL_CONTINUAR => {}
                    Err(error) => return Err(error),
                }
                if let Some(inc) = incremento {
                    evaluar_declaracion(inc, ambito, almacen, declaraciones)?;
                }
            }
            Ok(Valor::Nulo)
        }

        Declaracion::Repetir { cantidad, bloque } => {
            let veces = evaluar_expresion(cantidad, ambito, almacen, declaraciones)?;
            let n = match veces {
                Valor::Entero(n) if n > 0 => n as usize,
                _ => 0,
            };
            for _ in 0..n {
                match evaluar_bloque(bloque, ambito, almacen, declaraciones) {
                    Ok(Valor::Nulo) => {}
                    Ok(valor) => return Ok(valor),
                    Err(senal) if senal == SENAL_ROMPER => break,
                    Err(senal) if senal == SENAL_CONTINUAR => continue,
                    Err(error) => return Err(error),
                }
            }
            Ok(Valor::Nulo)
        }

        Declaracion::Variable { nombre, valor, .. } => {
            let val = if let Some(expr) = valor {
                evaluar_expresion(expr, ambito, almacen, declaraciones)?
            } else {
                Valor::Nulo
            };
            ambito.asignar(nombre.clone(), val.clone());
            almacen.fijar(nombre, val.to_json_valor());
            Ok(Valor::Nulo)
        }

        Declaracion::Asignacion { nombre, valor, .. } => {
            let val = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            // `este.x = ...` / `obj.campo = ...`: el parser lo entrega como una
            // asignación con el nombre punteado (no hay AST de miembro).
            if let Some((objeto, campo)) = nombre.split_once('.') {
                let actual = ambito
                    .obtener(objeto)
                    .cloned()
                    .or_else(|| almacen.obtener(objeto).map(|j| Valor::from_serde(&j)));
                if let Some(Valor::Mapa(mut mapa)) = actual {
                    mapa.insert(campo.to_string(), val);
                    let actualizado = Valor::Mapa(mapa);
                    ambito.asignar(objeto.to_string(), actualizado.clone());
                    almacen.fijar(objeto, actualizado.to_json_valor());
                    return Ok(Valor::Nulo);
                }
            }
            ambito.asignar(nombre.clone(), val.clone());
            almacen.fijar(nombre, val.to_json_valor());
            Ok(Valor::Nulo)
        }

        Declaracion::AsignacionMiembro {
            objeto,
            miembro,
            valor,
            ..
        } => {
            let val = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            // `este.x = ...` o `obj.campo = ...`: se escribe en el mapa del
            // objeto dentro del ámbito (y en la variable, si es un identificador).
            if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                let obj_val = evaluar_expresion(objeto, ambito, almacen, declaraciones)?;
                if let Valor::Mapa(mut m) = obj_val {
                    m.insert(miembro.clone(), val.clone());
                    let actualizado = Valor::Mapa(m);
                    ambito.asignar(nombre.clone(), actualizado.clone());
                    almacen.fijar(nombre, actualizado.to_json_valor());
                    return Ok(Valor::Nulo);
                }
            }
            let obj_val = evaluar_expresion(objeto, ambito, almacen, declaraciones)?;
            let clave = format!("{}_{}", obj_val.to_display(), miembro);
            almacen.fijar(&clave, val.to_json_valor());
            Ok(Valor::Nulo)
        }

        Declaracion::AsignacionIndex {
            nombre,
            indice,
            valor,
            ..
        } => {
            let idx_val = evaluar_expresion(indice, ambito, almacen, declaraciones)?;
            let val = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            // Actualizar el mapa/objeto en el ámbito local (clave → valor).
            // Los mapas y arrays se representan como Texto(JSON), así que se
            // parsean, se inserta la clave y se vuelven a serializar.
            if let Some(obj) = ambito.obtener(nombre).cloned() {
                match obj {
                    Valor::Mapa(mut m) => {
                        m.insert(idx_val.to_display(), val.clone());
                        ambito.asignar(nombre.to_string(), Valor::Mapa(m));
                    }
                    Valor::Arreglo(mut items) => {
                        if let Valor::Entero(n) = idx_val {
                            if (n as usize) < items.len() {
                                items[n as usize] = val.clone();
                                ambito.asignar(nombre.to_string(), Valor::Arreglo(items));
                                almacen.fijar(nombre, val.to_json_valor());
                            }
                        }
                    }
                    Valor::Texto(s) => {
                        if let Ok(serde_json::Value::Object(mut mapa)) =
                            serde_json::from_str::<serde_json::Value>(&s)
                        {
                            mapa.insert(idx_val.to_display(), val.to_json_valor());
                            if let Ok(nuevo) =
                                serde_json::to_string(&serde_json::Value::Object(mapa))
                            {
                                ambito.asignar(nombre.to_string(), Valor::Texto(nuevo));
                            }
                        } else if let Ok(serde_json::Value::Array(mut arr)) =
                            serde_json::from_str::<serde_json::Value>(&s)
                        {
                            let idx = match &idx_val {
                                Valor::Entero(n) => *n as usize,
                                _ => idx_val.to_display().parse::<usize>().unwrap_or_default(),
                            };
                            if idx < arr.len() {
                                arr[idx] = val.to_json_valor();
                            } else {
                                arr.push(val.to_json_valor());
                            }
                            if let Ok(nuevo) = serde_json::to_string(&serde_json::Value::Array(arr))
                            {
                                ambito.asignar(nombre.to_string(), Valor::Texto(nuevo));
                            }
                        }
                    }
                    _ => {}
                }
            }
            // Mantener compatibilidad con el store (claves con corchetes)
            let clave = format!("{}[{}]", nombre, idx_val.to_display());
            almacen.fijar(&clave, val.to_json_valor());
            Ok(val)
        }

        Declaracion::LlamadaFuncion { nombre, argumentos } => {
            // `empujar(xs, v)` / `remover(xs, i)` como sentencia: mutan la variable.
            if let Some(resultado) =
                nativa_sobre_variable(nombre, argumentos, ambito, almacen, declaraciones)
            {
                return resultado;
            }
            let mut args = Vec::new();
            for arg in argumentos {
                args.push(evaluar_expresion(arg, ambito, almacen, declaraciones)?);
            }
            // Manejar nombres con punto: "objeto.metodo" → método(objeto, args...)
            if let Some(dot_pos) = nombre.find('.') {
                let obj_name = &nombre[..dot_pos];
                let metodo = &nombre[dot_pos + 1..];
                let obj_val = if let Some(val) = ambito.obtener(obj_name) {
                    val.clone()
                } else if let Some(json_val) = almacen.obtener(obj_name) {
                    Valor::from_serde(&json_val)
                } else {
                    return Err(format!("Variable '{}' no encontrada", obj_name));
                };
                // Método de una clase: `este` es la instancia y la mutación
                // que haga el método se escribe de vuelta en la variable.
                if let Some(clase) = clase_de(&obj_val) {
                    if let Some(metodo_fn) = buscar_metodo(&clase, metodo, declaraciones) {
                        let (actualizada, retorno) =
                            ejecutar_metodo(metodo_fn, obj_val, &args, almacen, declaraciones)?;
                        ambito.asignar(obj_name.to_string(), actualizada.clone());
                        almacen.fijar(obj_name, actualizada.to_json_valor());
                        return Ok(retorno);
                    }
                }
                if let Some(clase) = clase_de(&obj_val) {
                    if let Some(metodo_fn) = buscar_metodo(&clase, metodo, declaraciones) {
                        let (actualizada, retorno) =
                            ejecutar_metodo(metodo_fn, obj_val, &args, almacen, declaraciones)?;
                        ambito.asignar(obj_name.to_string(), actualizada.clone());
                        almacen.fijar(obj_name, actualizada.to_json_valor());
                        return Ok(retorno);
                    }
                }
                let mut metodo_args = vec![obj_val];
                metodo_args.extend(args);
                let resultado = ejecutar_funcion(metodo, &metodo_args, declaraciones, almacen)?;
                // Métodos que mutan (ej: params.empujar): actualizar variable original
                if metodo == "empujar" {
                    ambito.asignar(obj_name.to_string(), resultado.clone());
                    almacen.fijar(obj_name, resultado.to_json_valor());
                }
                Ok(resultado)
            } else {
                ejecutar_funcion(nombre, &args, declaraciones, almacen)
            }
        }

        Declaracion::Expresion(expr) => {
            // Retornar el resultado de la expresión directamente.
            // Esto permite que retornos dentro de Coincidir, Ok, etc.
            // se propaguen al bloque contenedor (evaluar_bloque).
            evaluar_expresion(expr, ambito, almacen, declaraciones)
        }

        Declaracion::Cuando {
            condicion, cuerpo, ..
        } => {
            let cond_val = evaluar_expresion(condicion, ambito, almacen, declaraciones)?;
            if cond_val.es_verdadero() {
                evaluar_bloque(cuerpo, ambito, almacen, declaraciones)
            } else {
                Ok(Valor::Nulo)
            }
        }

        Declaracion::AsignacionMultiple {
            variables, valor, ..
        } => {
            let val = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            for var in variables {
                ambito.asignar(var.clone(), val.clone());
                almacen.fijar(var, val.to_json_valor());
            }
            Ok(Valor::Nulo)
        }

        // Declaraciones que se ignoran en runtime
        Declaracion::Funcion { .. }
        | Declaracion::Clase { .. }
        | Declaracion::Importar(_)
        | Declaracion::ImportarExterna(_)
        | Declaracion::Enum { .. }
        | Declaracion::Rasgo { .. }
        | Declaracion::Implementacion { .. }
        | Declaracion::AccesoMiembro { .. } => Ok(Valor::Nulo),

        Declaracion::Romper => Err(SENAL_ROMPER.to_string()),
        Declaracion::Continuar => Err(SENAL_CONTINUAR.to_string()),
    }
}

// ─── Evaluación de expresiones ──────────────────────────────────

/// Evalúa una expresión y retorna su valor como Valor
/// `empujar(arreglo, valor)` y `remover(arreglo, indice)` mutando la variable.
///
/// En la GUI los arreglos son texto JSON: si el nativo los recibe como valor,
/// modifica una copia y el original no cambia (divergencia con la VM). Como el
/// primer argumento es una variable, la mutación se hace acá y se escribe de
/// vuelta en el ámbito y en el almacén.
///
/// Devuelve `None` si la llamada no es de este tipo (se sigue el camino normal).
fn nativa_sobre_variable(
    nombre: &str,
    argumentos: &[Expresion],
    ambito: &mut Ambito,
    almacen: &dyn AlmacenValores,
    declaraciones: &[Declaracion],
) -> Option<Result<Valor, String>> {
    if nombre != "empujar" && nombre != "remover" {
        return None;
    }
    if argumentos.len() != 2 {
        return None;
    }
    let Expresion::Identificador {
        nombre: variable, ..
    } = &argumentos[0]
    else {
        return None;
    };

    let previo = ambito.obtener(variable).cloned().or_else(|| {
        almacen
            .obtener(variable)
            .map(|json| Valor::from_serde(&json))
    })?;
    let mut valores: Vec<Valor> = match &previo {
        Valor::Arreglo(items) => items.clone(),
        Valor::Texto(texto) => match serde_json::from_str::<serde_json::Value>(texto) {
            Ok(serde_json::Value::Array(items)) => items.iter().map(Valor::from_serde).collect(),
            _ => return Some(Err(format!("{nombre}: '{}' no es un arreglo", variable))),
        },
        _ => return Some(Err(format!("{nombre}: '{}' no es un arreglo", variable))),
    };

    let siguiente = match evaluar_expresion(&argumentos[1], ambito, almacen, declaraciones) {
        Ok(valor) => valor,
        Err(error) => return Some(Err(error)),
    };

    if nombre == "empujar" {
        valores.push(siguiente);
    } else {
        let indice = match siguiente {
            Valor::Entero(i) if i >= 0 && (i as usize) < valores.len() => i as usize,
            _ => return Some(Err("remover: índice fuera de rango".to_string())),
        };
        valores.remove(indice);
    }

    let nuevo = Valor::Arreglo(valores);
    ambito.asignar(variable.clone(), nuevo.clone());
    almacen.fijar(variable, nuevo.to_json_valor());
    Some(Ok(Valor::Nulo))
}

fn evaluar_expresion(
    expr: &Expresion,
    ambito: &mut Ambito,
    almacen: &dyn AlmacenValores,
    declaraciones: &[Declaracion],
) -> Result<Valor, String> {
    match expr {
        // ── Literales ──────────────────────────────────────────
        Expresion::LiteralNumero(n) => Ok(Valor::Entero(*n)),
        Expresion::LiteralDecimal(f) => Ok(Valor::Decimal(*f)),
        Expresion::LiteralTexto(s) => Ok(Valor::Texto(s.clone())),
        Expresion::LiteralBooleano(b) => Ok(Valor::Booleano(*b)),
        Expresion::LiteralNulo => Ok(Valor::Nulo),
        // `si` como elemento de un layout: corre la rama elegida y no devuelve valor.
        Expresion::Si {
            condicion,
            bloque_verdadero,
            bloque_falso,
        } => {
            let cond = evaluar_expresion(condicion, ambito, almacen, declaraciones)?;
            let rama = if valor_es_verdadero(&cond) {
                Some(bloque_verdadero)
            } else {
                bloque_falso.as_ref()
            };
            match rama {
                Some(bloque) => evaluar_bloque(bloque, ambito, almacen, declaraciones),
                None => Ok(Valor::Nulo),
            }
        }
        Expresion::LiteralExacto(coeff, scale) => {
            let val = *coeff as f64 / (10f64).powi(*scale as i32);
            Ok(Valor::Decimal(val))
        }

        // ── Identificador ──────────────────────────────────────
        Expresion::Identificador { nombre, .. } => {
            if let Some(val) = ambito.obtener(nombre) {
                Ok(val.clone())
            } else if let Some(json_val) = almacen.obtener(nombre) {
                Ok(Valor::from_serde(&json_val))
            } else {
                Err(format!("Variable '{}' no encontrada", nombre))
            }
        }

        // ── Operaciones binarias ────────────────────────────────
        Expresion::Binaria {
            izquierda,
            operador,
            derecha,
        } => {
            let izq = evaluar_expresion(izquierda, ambito, almacen, declaraciones)?;
            let der = evaluar_expresion(derecha, ambito, almacen, declaraciones)?;
            evaluar_binaria(izq, operador, der)
        }

        // ── Operaciones unarias ────────────────────────────────
        Expresion::Unaria {
            operador,
            expr: inner,
        } => {
            let val = evaluar_expresion(inner, ambito, almacen, declaraciones)?;
            match operador {
                OperadorUnario::Negar => match val {
                    Valor::Entero(n) => Ok(Valor::Entero(-n)),
                    Valor::Decimal(f) => Ok(Valor::Decimal(-f)),
                    _ => Err("No se puede negar un valor no numérico".to_string()),
                },
                OperadorUnario::No => Ok(Valor::Booleano(!val.es_verdadero())),
            }
        }

        // ── Llamada a función ─────────────────────────────────
        Expresion::LlamadaFuncion { nombre, argumentos } => {
            // Mutaciones de arreglo que necesitan la variable, no su copia.
            if let Some(resultado) =
                nativa_sobre_variable(nombre, argumentos, ambito, almacen, declaraciones)
            {
                return resultado;
            }
            let mut args = Vec::new();
            for arg in argumentos {
                args.push(evaluar_expresion(arg, ambito, almacen, declaraciones)?);
            }
            // Si el nombre contiene un punto (ej: "params.empujar"),
            // el compilador Forja genera LlamadaFuncion con nombre "objeto.metodo".
            // Resolver: buscar el objeto en el scope y anteponerlo como primer argumento,
            // luego llamar al método (la parte después del punto).
            if let Some(dot_pos) = nombre.find('.') {
                let obj_name = &nombre[..dot_pos];
                let metodo = &nombre[dot_pos + 1..];
                let obj_val = if let Some(val) = ambito.obtener(obj_name) {
                    val.clone()
                } else if let Some(json_val) = almacen.obtener(obj_name) {
                    Valor::from_serde(&json_val)
                } else {
                    return Err(format!("Variable '{}' no encontrada", obj_name));
                };
                let mut metodo_args = vec![obj_val];
                metodo_args.extend(args);
                let resultado = ejecutar_funcion(metodo, &metodo_args, declaraciones, almacen)?;
                // Métodos que mutan (ej: params.empujar): actualizar variable original
                if metodo == "empujar" {
                    ambito.asignar(obj_name.to_string(), resultado.clone());
                    almacen.fijar(obj_name, resultado.to_json_valor());
                }
                Ok(resultado)
            } else {
                ejecutar_funcion(nombre, &args, declaraciones, almacen)
            }
        }

        // ── Asignación a campo (`este.x = v`, `obj.campo = v`) ──
        Expresion::AsignacionCampo {
            objeto,
            campo,
            valor,
        } => {
            let val = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            let obj_val = evaluar_expresion(objeto, ambito, almacen, declaraciones)?;
            if let Valor::Mapa(mut mapa) = obj_val {
                mapa.insert(campo.clone(), val.clone());
                let actualizado = Valor::Mapa(mapa);
                if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                    ambito.asignar(nombre.clone(), actualizado.clone());
                    almacen.fijar(nombre, actualizado.to_json_valor());
                }
            }
            Ok(val)
        }

        // ── Acceso a miembro ──────────────────────────────────
        Expresion::AccesoMiembro { objeto, miembro } => {
            let obj = evaluar_expresion(objeto, ambito, almacen, declaraciones)?;
            if let Valor::Mapa(m) = &obj {
                if let Some(valor) = m.get(miembro) {
                    return Ok(valor.clone());
                }
            }
            let clave = format!("{}_{}", obj.to_display(), miembro);
            if let Some(json_val) = almacen.obtener(&clave) {
                Ok(Valor::from_serde(&json_val))
            } else {
                Ok(Valor::Nulo)
            }
        }

        // ── Instanciación ─────────────────────────────────────
        Expresion::Instanciacion { clase, argumentos } => {
            let mut instancia = std::collections::HashMap::new();
            instancia.insert("__clase".to_string(), Valor::Texto(clase.clone()));
            let mut valores = Vec::new();
            for arg in argumentos {
                valores.push(evaluar_expresion(arg, ambito, almacen, declaraciones)?);
            }
            // El parser nombra `constructor(...)` como el método "nuevo".
            let ctor = buscar_metodo(clase, "nuevo", declaraciones)
                .or_else(|| buscar_metodo(clase, "constructor", declaraciones))
                .or_else(|| buscar_metodo(clase, "new", declaraciones));
            if let Some(ctor) = ctor {
                let (actualizada, _) = ejecutar_metodo(
                    ctor,
                    Valor::Mapa(instancia),
                    &valores,
                    almacen,
                    declaraciones,
                )?;
                instancia = match actualizada {
                    Valor::Mapa(m) => m,
                    otro => {
                        let mut m = std::collections::HashMap::new();
                        m.insert("__clase".to_string(), Valor::Texto(clase.clone()));
                        let _ = otro;
                        m
                    }
                };
            }
            Ok(Valor::Mapa(instancia))
        }

        // ── Referencia (préstamo) ─────────────────────────────
        Expresion::Referencia { expr: inner, .. } => {
            evaluar_expresion(inner, ambito, almacen, declaraciones)
        }

        // ── Arreglo literal ───────────────────────────────────
        Expresion::Arreglo(elementos) => {
            let mut values = Vec::new();
            for elem in elementos {
                values.push(evaluar_expresion(elem, ambito, almacen, declaraciones)?);
            }
            Ok(Valor::Arreglo(values))
        }

        // ── Mapa literal ──────────────────────────────────────
        Expresion::Mapa(pares) => {
            let mut mapa = serde_json::Map::new();
            for (k, v) in pares {
                let clave_val = evaluar_expresion(k, ambito, almacen, declaraciones)?;
                let val = evaluar_expresion(v, ambito, almacen, declaraciones)?;
                mapa.insert(clave_val.to_display(), val.to_json_valor());
            }
            let json_str = serde_json::to_string(&serde_json::Value::Object(mapa))
                .map_err(|e| format!("Error serializando mapa: {}", e))?;
            Ok(Valor::Texto(json_str))
        }

        // ── Match/Coincidir ───────────────────────────────────
        Expresion::Coincidir {
            expr: inner,
            brazos,
        } => {
            let val = evaluar_expresion(inner, ambito, almacen, declaraciones)?;
            for brazo in brazos {
                if coincidir_patron(&val, &brazo.patron, ambito) {
                    return evaluar_bloque(&brazo.cuerpo, ambito, almacen, declaraciones);
                }
            }
            Err("Ningún brazo de match coincidió".to_string())
        }

        // ── Index (arr[0]) / Mapa["clave"] ───────────────────
        Expresion::Index { objeto, indice } => {
            let obj = evaluar_expresion(objeto, ambito, almacen, declaraciones)?;
            let idx = evaluar_expresion(indice, ambito, almacen, declaraciones)?;
            // Acceso directo a mapa (Valor::Mapa) por clave
            if let Valor::Mapa(m) = &obj {
                let clave = idx.to_display();
                return Ok(m.get(&clave).cloned().unwrap_or(Valor::Nulo));
            }
            // Arreglo con identidad de valor
            if let (Valor::Arreglo(items), Valor::Entero(n)) = (&obj, &idx) {
                return match items.get(*n as usize) {
                    Some(valor) => Ok(valor.clone()),
                    None => Err(format!("Índice {} fuera de rango (len={})", n, items.len())),
                };
            }
            let obj_str = obj.to_display();
            match idx {
                Valor::Entero(n) => {
                    let idx_num = n as usize;
                    if let Ok(serde_json::Value::Array(arr)) =
                        serde_json::from_str::<serde_json::Value>(&obj_str)
                    {
                        if idx_num < arr.len() {
                            Ok(Valor::from_serde(&arr[idx_num]))
                        } else {
                            Err(format!(
                                "Índice {} fuera de rango (len={})",
                                idx_num,
                                arr.len()
                            ))
                        }
                    } else {
                        Err("No se puede indexar un valor que no es un array".to_string())
                    }
                }
                Valor::Texto(clave) => {
                    // Acceso a mapa por clave string (ej: filtros["tipo"])
                    if let Ok(serde_json::Value::Object(mapa)) =
                        serde_json::from_str::<serde_json::Value>(&obj_str)
                    {
                        match mapa.get(&clave) {
                            Some(val) => Ok(Valor::from_serde(val)),
                            None => Ok(Valor::Nulo),
                        }
                    } else if let Ok(serde_json::Value::Array(arr)) =
                        serde_json::from_str::<serde_json::Value>(&obj_str)
                    {
                        // Si key es string numérico, intentar como índice
                        if let Ok(n) = clave.parse::<usize>() {
                            if n < arr.len() {
                                Ok(Valor::from_serde(&arr[n]))
                            } else {
                                Err(format!("Índice {} fuera de rango (len={})", n, arr.len()))
                            }
                        } else {
                            Err(format!("Clave '{}' no encontrada en array", clave))
                        }
                    } else {
                        Err(format!(
                            "No se puede acceder por clave '{}' a un valor no-objeto",
                            clave
                        ))
                    }
                }
                _ => Err("Índice debe ser un entero o una clave de texto".to_string()),
            }
        }

        // ── Closure ───────────────────────────────────────────
        Expresion::Closure { .. } => Err("Closures no soportados en runtime GUI".to_string()),

        // ── Grupo (expresión agrupada) ────────────────────────
        Expresion::Grupo(inner) => evaluar_expresion(inner, ambito, almacen, declaraciones),

        // ── Hilo ligero ───────────────────────────────────────
        Expresion::Hilo { .. } => Err("Hilos ligeros no soportados en runtime GUI".to_string()),

        // ── Canal ─────────────────────────────────────────────
        Expresion::CanalNuevo(_) => Err("Canales no soportados en runtime GUI".to_string()),

        // ── Try (propagación de error) ────────────────────────
        Expresion::Try(inner) => {
            let val = evaluar_expresion(inner, ambito, almacen, declaraciones)?;
            if matches!(val, Valor::Nulo) {
                Err("Error propagado desde expresión?".to_string())
            } else {
                Ok(val)
            }
        }

        // ── Seleccionar ───────────────────────────────────────
        Expresion::Seleccionar { .. } => Err("Seleccionar no soportado en runtime GUI".to_string()),

        // ── Asignación como expresión ─────────────────────────
        Expresion::Asignacion { variable, valor } => {
            let val = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            ambito.asignar(variable.clone(), val.clone());
            almacen.fijar(variable, val.to_json_valor());
            Ok(val)
        }

        // ── Asignación compuesta ─────────────────────────────
        Expresion::AsignacionCompuesta {
            objetivo,
            operador,
            valor,
        } => {
            let viejo = evaluar_expresion(objetivo, ambito, almacen, declaraciones)?;
            let der = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            let nuevo = evaluar_binaria(viejo, operador, der)?;
            match objetivo.as_ref() {
                Expresion::Identificador { nombre, .. } => {
                    ambito.asignar(nombre.clone(), nuevo.clone());
                    almacen.fijar(nombre, nuevo.to_json_valor());
                }
                Expresion::AccesoMiembro { objeto, miembro } => {
                    let obj = evaluar_expresion(objeto, ambito, almacen, declaraciones)?;
                    let clave = format!("{}_{}", obj.to_display(), miembro);
                    almacen.fijar(&clave, nuevo.to_json_valor());
                }
                _ => return Err("Asignación compuesta por índice no soportada en GUI".to_string()),
            }
            Ok(nuevo)
        }

        // ── Asignación a campo como expresión ────────────────
        Expresion::AsignacionCampo {
            objeto,
            campo,
            valor,
        } => {
            let obj = evaluar_expresion(objeto, ambito, almacen, declaraciones)?;
            let val = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            let clave = format!("{}_{}", obj.to_display(), campo);
            almacen.fijar(&clave, val.to_json_valor());
            Ok(val)
        }

        // ── ArraySet (arr[i] = valor como expresión) ─────────
        Expresion::ArregloFijar { array, valor } => {
            let val = evaluar_expresion(valor, ambito, almacen, declaraciones)?;
            // array es un Index(objeto, indice): actualizar el mapa/array en el
            // ámbito (dashboard["saldo_total"] = s). Sin esto, el mapa queda
            // vacío y luego las lecturas devuelven nulo.
            if let Expresion::Index { objeto, indice } = array.as_ref() {
                let obj_nombre = match objeto.as_ref() {
                    Expresion::Identificador { nombre, .. } => Some(nombre.clone()),
                    _ => None,
                };
                if let Some(nombre) = obj_nombre {
                    let idx_val = evaluar_expresion(indice, ambito, almacen, declaraciones)?;
                    if let Some(obj) = ambito.obtener(&nombre).cloned() {
                        match obj {
                            Valor::Mapa(mut m) => {
                                m.insert(idx_val.to_display(), val.clone());
                                ambito.asignar(nombre.clone(), Valor::Mapa(m));
                            }
                            Valor::Arreglo(mut items) => {
                                if let Valor::Entero(n) = idx_val {
                                    if (n as usize) < items.len() {
                                        items[n as usize] = val.clone();
                                        ambito.asignar(nombre.clone(), Valor::Arreglo(items));
                                        almacen.fijar(&nombre, val.to_json_valor());
                                    }
                                }
                            }
                            Valor::Texto(s) => {
                                if let Ok(serde_json::Value::Object(mut mapa)) =
                                    serde_json::from_str::<serde_json::Value>(&s)
                                {
                                    mapa.insert(idx_val.to_display(), val.to_json_valor());
                                    if let Ok(nuevo) =
                                        serde_json::to_string(&serde_json::Value::Object(mapa))
                                    {
                                        ambito.asignar(nombre.clone(), Valor::Texto(nuevo));
                                    }
                                } else if let Ok(serde_json::Value::Array(mut arr)) =
                                    serde_json::from_str::<serde_json::Value>(&s)
                                {
                                    let idx = match &idx_val {
                                        Valor::Entero(n) => *n as usize,
                                        _ => idx_val.to_display().parse::<usize>().unwrap_or(0),
                                    };
                                    if idx < arr.len() {
                                        arr[idx] = val.to_json_valor();
                                    } else {
                                        arr.push(val.to_json_valor());
                                    }
                                    if let Ok(nuevo) =
                                        serde_json::to_string(&serde_json::Value::Array(arr))
                                    {
                                        ambito.asignar(nombre.clone(), Valor::Texto(nuevo));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    // Mantener compatibilidad con el store (claves con corchetes)
                    let clave = format!("{}[{}]", nombre, idx_val.to_display());
                    almacen.fijar(&clave, val.to_json_valor());
                }
            }
            Ok(val)
        }

        // ── Resultado: Ok / Error ─────────────────────────────
        Expresion::Ok(inner) => {
            let val = evaluar_expresion(inner, ambito, almacen, declaraciones)?;
            Ok(val)
        }
        Expresion::Error(inner) => {
            let val = evaluar_expresion(inner, ambito, almacen, declaraciones)?;
            Err(format!("Error: {}", val.to_display()))
        }

        // ── Opción: Algo / Nada / Ninguno ──────────────────────
        Expresion::Algo(inner) => {
            let val = evaluar_expresion(inner, ambito, almacen, declaraciones)?;
            Ok(val)
        }
        Expresion::Nada | Expresion::Ninguno => Ok(Valor::Nulo),

        // ── Design by Contract ────────────────────────────────
        Expresion::Resultado => {
            // El compilador Forja genera Expresion::Resultado para referirse
            // al resultado de la expresión anterior (ej: la variable "resultado"
            // en "variable resultado = fn(); coincidir (resultado) { ... }").
            // Buscar en el ámbito local y luego en el store.
            if let Some(val) = ambito.obtener("resultado") {
                Ok(val.clone())
            } else if let Some(json_val) = almacen.obtener("resultado") {
                Ok(Valor::from_serde(&json_val))
            } else {
                Ok(Valor::Nulo)
            }
        }
        Expresion::Anterior(inner) => evaluar_expresion(inner, ambito, almacen, declaraciones),

        // ── Ternario ──────────────────────────────────────────
        Expresion::Ternario {
            condicion,
            si_verdadero,
            si_falso,
        } => {
            let cond = evaluar_expresion(condicion, ambito, almacen, declaraciones)?;
            let b = match &cond {
                Valor::Booleano(b) => *b,
                Valor::Entero(n) => *n != 0,
                _ => false,
            };
            if b {
                evaluar_expresion(si_verdadero, ambito, almacen, declaraciones)
            } else {
                evaluar_expresion(si_falso, ambito, almacen, declaraciones)
            }
        }

        // ── Llamada a método ──────────────────────────────────
        Expresion::LlamadaMetodo {
            objeto,
            metodo,
            argumentos,
        } => {
            let mut valores = Vec::new();
            let obj_val = evaluar_expresion(objeto, ambito, almacen, declaraciones)?;
            for arg in argumentos {
                valores.push(evaluar_expresion(arg, ambito, almacen, declaraciones)?);
            }

            // `p.metodo(args)`: si `p` es una instancia, se ejecuta el método de
            // su clase con `este` ligado, y la mutación vuelve a la variable.
            if let Some(clase) = clase_de(&obj_val) {
                if let Some(metodo_fn) = buscar_metodo(&clase, metodo, declaraciones) {
                    let (actualizada, retorno) =
                        ejecutar_metodo(metodo_fn, obj_val.clone(), &valores, almacen, declaraciones)?;
                    if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                        ambito.asignar(nombre.clone(), actualizada.clone());
                        almacen.fijar(nombre, actualizada.to_json_valor());
                    }
                    return Ok(retorno);
                }
            }

            let mut args = vec![obj_val];
            args.extend(valores);
            let resultado = ejecutar_funcion(metodo, &args, declaraciones, almacen)?;
            // Métodos que mutan (ej: params.empujar(x)): actualizar variable original
            if metodo == "empujar" {
                if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                    ambito.asignar(nombre.clone(), resultado.clone());
                    almacen.fijar(nombre, resultado.to_json_valor());
                }
            }
            Ok(resultado)
        }
    }
}

// ─── Evaluación binaria ─────────────────────────────────────────

fn evaluar_binaria(izq: Valor, operador: &Operador, der: Valor) -> Result<Valor, String> {
    match operador {
        Operador::Suma => Ok(izq + der),
        Operador::Resta => Ok(izq - der),
        Operador::Multiplicacion => Ok(izq * der),
        Operador::Division => Ok(izq / der),
        Operador::Modulo => match (izq, der) {
            (Valor::Entero(a), Valor::Entero(b)) => {
                if b == 0 {
                    Err("División por cero en módulo".to_string())
                } else {
                    Ok(Valor::Entero(a % b))
                }
            }
            _ => Err("Módulo sólo soportado para enteros".to_string()),
        },
        Operador::IgualIgual => Ok(Valor::Booleano(izq == der)),
        Operador::Diferente => Ok(Valor::Booleano(izq != der)),
        Operador::Menor | Operador::MenorIgual | Operador::Mayor | Operador::MayorIgual => {
            Ok(Valor::Booleano(izq.compare(operador, &der)))
        }
        Operador::Y => Ok(Valor::Booleano(izq.es_verdadero() && der.es_verdadero())),
        Operador::O => Ok(Valor::Booleano(izq.es_verdadero() || der.es_verdadero())),
    }
}

// ─── Pattern matching ───────────────────────────────────────────

/// Verifica si un valor coincide con un patrón
fn coincidir_patron(valor: &Valor, patron: &Patron, ambito: &mut Ambito) -> bool {
    match patron {
        Patron::Ignorar => true,
        Patron::Variable(nombre) => {
            ambito.asignar(nombre.clone(), valor.clone());
            true
        }
        Patron::Literal(expr) => match expr {
            Expresion::LiteralNumero(n) => {
                matches!(valor, Valor::Entero(v) if v == n)
            }
            Expresion::LiteralDecimal(f) => {
                matches!(valor, Valor::Decimal(v) if (v - f).abs() < f64::EPSILON)
            }
            Expresion::LiteralTexto(s) => {
                matches!(valor, Valor::Texto(v) if v == s)
            }
            Expresion::LiteralBooleano(b) => {
                matches!(valor, Valor::Booleano(v) if v == b)
            }
            Expresion::LiteralNulo => matches!(valor, Valor::Nulo),
            _ => false,
        },
        Patron::Constructor(nombre, subpatrones) => match (nombre.as_str(), valor) {
            ("Ok", _) => {
                subpatrones.is_empty() || {
                    subpatrones.len() == 1 && coincidir_patron(valor, &subpatrones[0], ambito)
                }
            }
            ("Error", _) => subpatrones.is_empty(),
            ("Algo", _) => {
                subpatrones.is_empty() || {
                    subpatrones.len() == 1 && coincidir_patron(valor, &subpatrones[0], ambito)
                }
            }
            ("Ninguno", _) => matches!(valor, Valor::Nulo),
            _ => false,
        },
    }
}

// ─── Funciones públicas para integración ───────────────────────

/// Inicializa el estado evaluando:
/// 1. Variables de módulo (declaradas fuera de funciones)
/// 2. La función `main()` para inicializar el estado dinámico
pub fn inicializar_estado(declaraciones: &[Declaracion], almacen: &dyn AlmacenValores) {
    // 1. Procesar variables de módulo (declaraciones fuera de funciones)
    //    Usa el evaluador completo para soportar llamadas a funciones
    //    (ej: conexion = _sqlite_abrir("forjanzas.db")) en lugar del
    //    match limitado a literales que convertía todo a Nulo.
    let mut ambito = Ambito::nuevo();
    for decl in declaraciones {
        match decl {
            Declaracion::Variable { .. } | Declaracion::Asignacion { .. } => {
                // Usar evaluar_declaracion que maneja correctamente
                // todo tipo de expresión, incluyendo llamadas a funciones
                let _ = evaluar_declaracion(decl, &mut ambito, almacen, declaraciones);
            }
            _ => {}
        }
    }

    // 2. Ejecutar declaraciones de main() para inicializar estado dinámico.
    //    Procesa Variable, Asignacion, LlamadaFuncion y Expresion.
    //    Las funciones GUI del layout (columna, fila, boton, etc.) se evalúan
    //    pero devuelven Nulo o error, que se ignora silenciosamente.
    for decl in declaraciones {
        if let Declaracion::Funcion { nombre, cuerpo, .. } = decl {
            if nombre == "main" {
                for d in cuerpo {
                    match d {
                        Declaracion::Variable { .. } | Declaracion::Asignacion { .. } => {
                            match evaluar_declaracion(d, &mut ambito, almacen, declaraciones) {
                                Ok(_) => {}
                                Err(e) => {
                                    eprintln!("[inicializar_estado] main: {} — ignorado", e);
                                }
                            }
                        }
                        Declaracion::LlamadaFuncion { nombre, .. } => {
                            // Solo ejecutar funciones reales (nativas o declaradas en el AST).
                            // Los widgets del layout GUI (andamio, barra_superior, navegador,
                            // etc.) no deben evaluarse aquí: se omiten silenciosamente para
                            // evitar falsos errores tipo "Variable 'buscar_transacciones' no encontrada".
                            if es_funcion_ejecutable(nombre, declaraciones) {
                                let _ = evaluar_declaracion(d, &mut ambito, almacen, declaraciones);
                            }
                        }
                        Declaracion::Expresion(..) => {
                            let _ = evaluar_declaracion(d, &mut ambito, almacen, declaraciones);
                        }
                        _ => {} // Ignorar Funcion, Clase, etc.
                    }
                }

                // 3. Copiar todas las variables del ámbito local al store global.
                //    Esto asegura que las variables definidas en main() (como conexion_bd,
                //    buscar_transacciones, etc.) sean accesibles desde los callbacks
                //    de los widgets, que se ejecutan en un ámbito nuevo.
                for (nombre, valor) in ambito.obtener_todas() {
                    almacen.fijar(nombre, valor.to_json_valor());
                }

                return;
            }
        }
    }
}

#[cfg(test)]
mod pruebas_control {
    use super::*;
    use std::cell::RefCell;

    /// Almacén de prueba: mapa en memoria, sin reactividad.
    #[derive(Default)]
    struct AlmacenPrueba {
        valores: RefCell<std::collections::HashMap<String, serde_json::Value>>,
    }

    impl AlmacenValores for AlmacenPrueba {
        fn obtener(&self, nombre: &str) -> Option<serde_json::Value> {
            self.valores.borrow().get(nombre).cloned()
        }
        fn fijar(&self, nombre: &str, valor: serde_json::Value) {
            self.valores.borrow_mut().insert(nombre.to_string(), valor);
        }
    }

    fn declaraciones_de(fuente: &str) -> Vec<Declaracion> {
        let mut lexer = forja_syntax::lexer::Lexer::nuevo(fuente);
        let tokens = lexer.tokenize().expect("lexer");
        let mut parser = forja_syntax::parser::Parser::nuevo(tokens);
        parser.parsear().expect("parser").declaraciones
    }

    fn ejecutar(fuente: &str, funcion: &str) -> Result<Valor, String> {
        let decls = declaraciones_de(fuente);
        let almacen = AlmacenPrueba::default();
        ejecutar_funcion(funcion, &[], &decls, &almacen)
    }

    #[test]
    fn romper_dentro_de_si_corta_el_bucle() {
        // Antes esto colgaba (romper se evaluaba a Nulo y el bucle seguía).
        let resultado = ejecutar(
            r#"
funcion calcular() -> Texto {
    variable i = 0
    variable suma = 0
    mientras (verdadero) {
        si (i >= 5) {
            romper
        }
        suma = suma + i
        i = i + 1
    }
    retornar a_texto(suma)
}
"#,
            "calcular",
        );
        assert_eq!(
            resultado.expect("no debería fallar"),
            Valor::Texto("10".to_string())
        );
    }

    #[test]
    fn continuar_salta_la_iteracion() {
        let resultado = ejecutar(
            r#"
funcion calcular() -> Texto {
    variable suma = 0
    variable i = 0
    mientras (i < 5) {
        i = i + 1
        si (i == 3) {
            continuar
        }
        suma = suma + i
    }
    retornar a_texto(suma)
}
"#,
            "calcular",
        );
        assert_eq!(
            resultado.expect("no debería fallar"),
            Valor::Texto("12".to_string())
        );
    }

    #[test]
    fn retornar_dentro_de_bucle_devuelve_el_valor() {
        let resultado = ejecutar(
            r#"
funcion calcular() -> Texto {
    variable i = 0
    mientras (verdadero) {
        i = i + 1
        si (i == 4) {
            retornar "cortó en " + a_texto(i)
        }
    }
    retornar "no llegó"
}
"#,
            "calcular",
        );
        assert_eq!(
            resultado.expect("no debería fallar"),
            Valor::Texto("cortó en 4".to_string())
        );
    }
}
