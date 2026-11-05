//! Configuración de Forja: `forja.yaml`.
//!
//! La CLI no usa flags. Al arrancar, si el directorio actual no tiene
//! `forja.yaml`, se crea con **todas** las claves y sus valores por defecto
//! (ver [`Config::default`]); después se lee ese archivo. Cambiar el
//! comportamiento = editar el YAML.
//!
//! Es un lenguaje en español: los booleanos se escriben `verdadero`/`falso` y
//! las opciones usan palabras del idioma (`rapido`, `maquina`, `ensamblador`,
//! `nativo`, `detallado`, `depuracion`, `traza`). Al leer se aceptan también
//! `true`/`false` y los nombres viejos en inglés, por comodidad.
//!
//! La librería ([`actual`]) arranca con los defaults; la CLI es quien llama a
//! [`recargar`] (o [`fijar`]) al inicio. Así los tests y los embebidos (wasm,
//! LSP) no dependen de un `forja.yaml` que esté en el directorio actual.

use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Nombre del archivo de configuración (en el directorio de ejecución).
pub const NOMBRE_ARCHIVO: &str = "forja.yaml";

/// Booleanos en español: se escriben `verdadero`/`falso` y se aceptan también
/// `true`/`false`, `si`/`no` y `1`/`0`.
mod bool_es {
    use super::*;

    pub fn serialize<S: Serializer>(valor: &bool, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(if *valor { "verdadero" } else { "falso" })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
        struct Visitante;
        impl serde::de::Visitor<'_> for Visitante {
            type Value = bool;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("verdadero o falso")
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<bool, E> {
                Ok(v)
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<bool, E> {
                Ok(v != 0)
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<bool, E> {
                match v.trim().to_lowercase().as_str() {
                    "verdadero" | "si" | "sí" | "true" | "1" | "yes" => Ok(true),
                    "falso" | "no" | "false" | "0" | "off" => Ok(false),
                    otro => Err(E::unknown_variant(otro, &["verdadero", "falso"])),
                }
            }
        }
        d.deserialize_any(Visitante)
    }
}

/// Todas las perillas de la CLI y del pipeline.
///
/// `#[serde(default)]` permite que un YAML parcial complete el resto con los
/// valores por defecto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Motor de ejecución / benchmark: `rapido`, `jit` o `todas`.
    pub motor: String,
    /// Destino de `ejecutar`: `maquina`, `ensamblador` o `nativo`.
    pub destino: String,
    /// Modo estricto de tipos y ownership.
    #[serde(with = "bool_es")]
    pub estricto: bool,
    /// Verificación de `requiere`/`asegura`/invariantes.
    #[serde(with = "bool_es")]
    pub contratos: bool,
    /// Pila de llamadas en errores de runtime.
    #[serde(with = "bool_es")]
    pub pila: bool,
    /// Aritmética flotante optimizada.
    #[serde(with = "bool_es")]
    pub matematica_rapida: bool,
    /// Perfil de ejecución: `""`, `recoger` o `usar`.
    pub perfil: String,
    /// Recarga de módulos en caliente (con `destino: nativo`).
    #[serde(with = "bool_es")]
    pub recarga: bool,
    /// Hosts permitidos en el sandbox de red (separados por coma).
    pub permitir_red: String,
    /// Puertos permitidos en el sandbox de red (separados por coma).
    pub permitir_puertos: String,
    /// Límite de tamaño del archivo fuente, en MB.
    pub max_archivo_mb: u64,
    /// Arquitectura de `asm`: `""` (actual), `x86_64-windows`,
    /// `x86_64-linux`, `arm64`, `riscv64`.
    pub arquitectura: String,
    /// Iteraciones de `medir`.
    pub iteraciones: u64,
    /// Conservar la consola en el `.exe` de `compilar`.
    #[serde(with = "bool_es")]
    pub consola: bool,
    /// `formatear` sólo verifica (no escribe).
    #[serde(with = "bool_es")]
    pub verificar_formato: bool,
    /// `lint` trata las advertencias como errores.
    #[serde(with = "bool_es")]
    pub lint_estricto: bool,
    /// Verbosidad de los diagnósticos: `""`, `detallado`, `depuracion` o `traza`.
    pub nivel: String,
    /// Diagnósticos en JSON.
    #[serde(with = "bool_es")]
    pub json: bool,
    /// Color en la salida.
    #[serde(with = "bool_es")]
    pub color: bool,
    /// Puerto del servidor de registry.
    pub puerto_registry: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            motor: "rapido".to_string(),
            destino: "maquina".to_string(),
            estricto: false,
            contratos: true,
            pila: false,
            matematica_rapida: false,
            perfil: String::new(),
            recarga: false,
            permitir_red: String::new(),
            permitir_puertos: String::new(),
            max_archivo_mb: crate::MAX_ARCHIVO_DEFAULT_MB,
            arquitectura: String::new(),
            iteraciones: 100,
            consola: false,
            verificar_formato: false,
            lint_estricto: false,
            nivel: String::new(),
            json: false,
            color: true,
            puerto_registry: 8787,
        }
    }
}

// ─── Estado en proceso ──────────────────────────────────────────────────────

static ACTUAL: OnceLock<RwLock<Config>> = OnceLock::new();

fn celda() -> &'static RwLock<Config> {
    ACTUAL.get_or_init(|| RwLock::new(Config::default()))
}

/// Configuración vigente (copia).
pub fn actual() -> Config {
    celda().read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Fija la configuración en memoria (la usa la CLI al leer el YAML y los
/// tests que necesitan un modo distinto).
pub fn fijar(cfg: Config) {
    *celda().write().unwrap_or_else(|e| e.into_inner()) = cfg;
}

/// Vuelve a leer `forja.yaml` del directorio actual (o defaults si no está).
pub fn recargar() {
    fijar(leer_disco().unwrap_or_default());
}

/// Ruta del `forja.yaml` del directorio de ejecución.
pub fn ruta() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(NOMBRE_ARCHIVO)
}

/// Crea `forja.yaml` con todas las claves en default si no existe.
///
/// Devuelve la ruta del archivo. Si ya existe no lo toca.
#[cfg(not(target_arch = "wasm32"))]
pub fn asegurar() -> Result<PathBuf, String> {
    let ruta = ruta();
    if !ruta.exists() {
        escribir(&ruta, &Config::default())?;
    }
    Ok(ruta)
}

/// Serializa `cfg` con un encabezado y lo escribe en `ruta`.
#[cfg(not(target_arch = "wasm32"))]
pub fn escribir(ruta: &Path, cfg: &Config) -> Result<(), String> {
    // YAML escrito a mano (no con serde) para poder poner, arriba de cada
    // opción, los valores posibles.
    let mut y = String::new();
    y.push_str(
        "# Configuración de Forja.
",
    );
    y.push_str(
        "# Se crea automáticamente si no existe; editá este archivo para cambiar
",
    );
    y.push_str(
        "# el comportamiento. La CLI no usa flags.
",
    );
    y.push_str(
        "# Los booleanos se escriben: verdadero | falso
",
    );

    macro_rules! opcion {
        ($comentario:expr, $clave:expr, $valor:expr) => {
            y.push_str(&format!(
                "
# {}
{}: {}
",
                $comentario, $clave, $valor
            ));
        };
    }

    opcion!(
        "Valores posibles: rapido | vm | jit | todas",
        "motor",
        escalar(&cfg.motor)
    );
    opcion!(
        "Valores posibles: maquina | ensamblador | nativo",
        "destino",
        escalar(&cfg.destino)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "estricto",
        booleano(cfg.estricto)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "contratos",
        booleano(cfg.contratos)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "pila",
        booleano(cfg.pila)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "matematica_rapida",
        booleano(cfg.matematica_rapida)
    );
    opcion!(
        "Valores posibles: '' (sin perfil) | recoger | usar",
        "perfil",
        escalar(&cfg.perfil)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "recarga",
        booleano(cfg.recarga)
    );
    opcion!(
        "Valores posibles: hosts separados por coma, o '' (sin red)",
        "permitir_red",
        escalar(&cfg.permitir_red)
    );
    opcion!(
        "Valores posibles: puertos separados por coma, o '' (sin restricción)",
        "permitir_puertos",
        escalar(&cfg.permitir_puertos)
    );
    opcion!(
        "Número entero: tamaño máximo del archivo fuente, en MB",
        "max_archivo_mb",
        cfg.max_archivo_mb
    );
    opcion!(
        "Valores posibles: '' (plataforma actual) | x86_64-windows | x86_64-linux | arm64 | riscv64",
        "arquitectura",
        escalar(&cfg.arquitectura)
    );
    opcion!(
        "Número entero: iteraciones del benchmark",
        "iteraciones",
        cfg.iteraciones
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "consola",
        booleano(cfg.consola)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "verificar_formato",
        booleano(cfg.verificar_formato)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "lint_estricto",
        booleano(cfg.lint_estricto)
    );
    opcion!(
        "Valores posibles: '' (normal) | detallado | depuracion | traza",
        "nivel",
        escalar(&cfg.nivel)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "json",
        booleano(cfg.json)
    );
    opcion!(
        "Valores posibles: verdadero | falso",
        "color",
        booleano(cfg.color)
    );
    opcion!(
        "Puerto TCP del servidor del registry",
        "puerto_registry",
        cfg.puerto_registry
    );

    std::fs::write(ruta, y).map_err(|e| format!("no se pudo escribir {}: {e}", ruta.display()))
}

/// `verdadero` / `falso`, como corresponde en un lenguaje en español.
#[cfg(not(target_arch = "wasm32"))]
fn booleano(valor: bool) -> &'static str {
    if valor {
        "verdadero"
    } else {
        "falso"
    }
}

/// Escalar de texto listo para YAML: `''` si está vacío, comillas si hace falta.
#[cfg(not(target_arch = "wasm32"))]
fn escalar(valor: &str) -> String {
    if valor.is_empty() {
        return "''".to_string();
    }
    match serde_yaml::to_string(valor) {
        Ok(s) => s.trim().to_string(),
        Err(_) => format!("{valor:?}"),
    }
}

/// Lee el YAML del directorio actual. `None` si no existe, no se puede leer o
/// no es YAML válido (en ese caso se usan los defaults).
pub fn leer_disco() -> Option<Config> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        leer_de(&ruta())
    }
}

/// Lee un YAML de una ruta concreta (útil para tests y para el runtime AOT).
/// `None` si no existe, no se puede leer o no es YAML válido.
pub fn leer_de(ruta: &Path) -> Option<Config> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ruta;
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let texto = std::fs::read_to_string(ruta).ok()?;
        serde_yaml::from_str(&texto).ok()
    }
}
