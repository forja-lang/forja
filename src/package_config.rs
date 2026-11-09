use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ForjaConfig {
    pub nombre: String,
    pub version: String,
    #[serde(default)]
    pub forja: String,
    #[serde(default)]
    pub autor: String,
    #[serde(default)]
    pub descripcion: String,
    #[serde(default = "predeterminado_entrada")]
    pub entrada: String,
    #[serde(default)]
    pub modulos: Vec<String>,
    #[serde(default)]
    pub dependencias: HashMap<String, String>,
    #[serde(default)]
    #[serde(rename = "dev-dependencias")]
    pub dev_dependencias: HashMap<String, String>,
}

fn predeterminado_entrada() -> String {
    "main.fa".to_string()
}

impl ForjaConfig {
    pub fn load(ruta: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(ruta)
            .map_err(|e| format!("Error leyendo {}: {}", ruta.display(), e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Error parseando {}: {}", ruta.display(), e))
    }

    pub fn guardar(&self, ruta: &Path) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Error serializando config: {}", e))?;
        fs::write(ruta, content).map_err(|e| format!("Error escribiendo {}: {}", ruta.display(), e))
    }

    pub fn nuevo(nombre: &str, version: &str) -> Self {
        ForjaConfig {
            nombre: nombre.to_string(),
            version: version.to_string(),
            forja: "0.1.0".to_string(),
            autor: String::new(),
            descripcion: String::new(),
            entrada: predeterminado_entrada(),
            modulos: vec!["std".to_string()],
            dependencias: HashMap::new(),
            dev_dependencias: HashMap::new(),
        }
    }
}
