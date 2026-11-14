//! Modelo de valores del intérprete: el que comparten la GUI nativa y la web.
//!
//! Es el `Valor` de `gui-rt` promovido a crate propio. Los arreglos siguen
//! viajando como texto JSON (deuda conocida: ver plans/B16-UNIFICAR-EVALUADOR.md).

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Valor {
    Texto(String),
    Entero(i64),
    Decimal(f64),
    Booleano(bool),
    Nulo,
    /// Mapa con claves String para representar valores estructurados (Ok, Error, Algo, etc.)
    Mapa(HashMap<String, Valor>),
}

impl std::fmt::Display for Valor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Valor::Texto(s) => f.write_str(s),
            Valor::Entero(n) => write!(f, "{}", n),
            Valor::Decimal(d) => write!(f, "{}", d),
            Valor::Booleano(true) => f.write_str("verdadero"),
            Valor::Booleano(false) => f.write_str("falso"),
            Valor::Nulo => f.write_str("nulo"),
            Valor::Mapa(m) => write!(f, "Mapa({:?})", m),
        }
    }
}

impl Valor {
    pub fn to_f64(&self) -> f64 {
        match self {
            Valor::Entero(n) => *n as f64,
            Valor::Decimal(f) => *f,
            Valor::Texto(s) => s.parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Valor::Booleano(b) => *b,
            Valor::Texto(s) => s == "verdadero" || s == "true",
            Valor::Entero(n) => *n != 0,
            _ => false,
        }
    }
}

impl From<&str> for Valor {
    fn from(s: &str) -> Self {
        Valor::Texto(s.to_string())
    }
}

impl From<Valor> for serde_json::Value {
    fn from(val: Valor) -> Self {
        match val {
            Valor::Texto(s) => serde_json::Value::String(s),
            Valor::Entero(n) => serde_json::Value::Number(n.into()),
            Valor::Decimal(f) => serde_json::Value::String(f.to_string()),
            Valor::Booleano(b) => serde_json::Value::Bool(b),
            Valor::Nulo => serde_json::Value::Null,
            Valor::Mapa(m) => {
                let mut mapa = serde_json::Map::new();
                for (k, v) in m {
                    mapa.insert(k, serde_json::Value::from(v));
                }
                serde_json::Value::Object(mapa)
            }
        }
    }
}

impl From<serde_json::Value> for Valor {
    fn from(val: serde_json::Value) -> Self {
        match val {
            serde_json::Value::String(s) => Valor::Texto(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Valor::Entero(i)
                } else if let Some(f) = n.as_f64() {
                    Valor::Decimal(f)
                } else {
                    Valor::Nulo
                }
            }
            serde_json::Value::Bool(b) => Valor::Booleano(b),
            serde_json::Value::Object(obj) => {
                let mut mapa = HashMap::new();
                for (k, v) in obj {
                    mapa.insert(k, Valor::from(v));
                }
                Valor::Mapa(mapa)
            }
            serde_json::Value::Array(arr) => {
                // Convertir array a JSON string para mantener compatibilidad
                if let Some(first) = arr.first() {
                    Valor::from(first.clone())
                } else {
                    Valor::Nulo
                }
            }
            serde_json::Value::Null => Valor::Nulo,
        }
    }
}
