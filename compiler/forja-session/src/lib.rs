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
    /// Iteraciones por defecto de `medir`.
    #[serde(default = "iteraciones_default")]
    pub iteraciones: usize,
    /// Memoización automática de funciones puras en AOT/LLVM.
    #[serde(with = "bool_es")]
    pub memoizacion: bool,
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
    /// Puerto del servidor de registro.
    pub puerto_registry: u16,
}

fn iteraciones_default() -> usize {
    100
}

impl Default for Config {
    fn default() -> Self {
        Config {
            motor: "rapido".to_string(),
            destino: "maquina".to_string(),
            iteraciones: 100,
            memoizacion: true,
            estricto: false,
            contratos: true,
            pila: false,
            matematica_rapida: false,
            perfil: String::new(),
            recarga: false,
            permitir_red: String::new(),
            permitir_puertos: String::new(),
            max_archivo_mb: 10,
            arquitectura: String::new(),
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
        "Motor de ejecución: rapido = VM canónica; jit = tiered; todas = medir todas. Valores: rapido | vm | jit | todas",
        "motor",
        escalar(&cfg.motor)
    );
    opcion!(
        "Cómo ejecutar: maquina = VM; ensamblador = ASM nativo; nativo = GUI/AOT. Valores: maquina | ensamblador | nativo",
        "destino",
        escalar(&cfg.destino)
    );
    opcion!(
        "Iteraciones que usa `forja medir` cuando no se indica una cantidad.",
        "iteraciones",
        cfg.iteraciones
    );
    opcion!(
        "Memoiza funciones puras recursivas en LLVM/AOT. Acelera mucho; desactivala para medir el cálculo real. Valores: verdadero | falso",
        "memoizacion",
        booleano(cfg.memoizacion)
    );
    opcion!(
        "Modo estricto de tipos y ownership: convierte advertencias en errores. Valores: verdadero | falso",
        "estricto",
        booleano(cfg.estricto)
    );
    opcion!(
        "Verifica `requiere`/`asegura`/invariantes en runtime. Valores: verdadero | falso",
        "contratos",
        booleano(cfg.contratos)
    );
    opcion!(
        "Incluye la pila de llamadas en errores de runtime. Valores: verdadero | falso",
        "pila",
        booleano(cfg.pila)
    );
    opcion!(
        "Atajos aritméticos inseguros: división por cero => inf y sin chequeos. Sólo benchmarks. Valores: verdadero | falso",
        "matematica_rapida",
        booleano(cfg.matematica_rapida)
    );
    opcion!(
        "Perfilado PGO: recoger = medir; usar = aplicar un perfil guardado; vacío = desactivado. Valores: '' | recoger | usar",
        "perfil",
        escalar(&cfg.perfil)
    );
    opcion!(
        "Recarga módulos en caliente (hot reload) para destino nativo/GUI. Valores: verdadero | falso",
        "recarga",
        booleano(cfg.recarga)
    );
    opcion!(
        "Hosts permitidos por el sandbox de red; vacío = sin red. Separalos por coma.",
        "permitir_red",
        escalar(&cfg.permitir_red)
    );
    opcion!(
        "Puertos permitidos por el sandbox de red; vacío = sin restricción. Separalos por coma.",
        "permitir_puertos",
        escalar(&cfg.permitir_puertos)
    );
    opcion!(
        "Tamaño máximo del archivo fuente que la CLI acepta leer, en MB.",
        "max_archivo_mb",
        cfg.max_archivo_mb
    );
    opcion!(
        "Arquitectura objetivo para ASM; vacío = plataforma actual. Valores: '' | x86_64-windows | x86_64-linux | arm64 | riscv64",
        "arquitectura",
        escalar(&cfg.arquitectura)
    );
    opcion!(
        "Muestra la ventana de consola en ejecutables GUI. Valores: verdadero | falso",
        "consola",
        booleano(cfg.consola)
    );
    opcion!(
        "`forja formatear` sólo verifica, no reescribe archivos. Valores: verdadero | falso",
        "verificar_formato",
        booleano(cfg.verificar_formato)
    );
    opcion!(
        "El linter trata las advertencias como errores. Valores: verdadero | falso",
        "lint_estricto",
        booleano(cfg.lint_estricto)
    );
    opcion!(
        "Nivel de detalle de los diagnósticos. Valores: '' | detallado | depuracion | traza",
        "nivel",
        escalar(&cfg.nivel)
    );
    opcion!(
        "Emite los diagnósticos en JSON. Valores: verdadero | falso",
        "json",
        booleano(cfg.json)
    );
    opcion!(
        "Usa colores ANSI en la salida. Valores: verdadero | falso",
        "color",
        booleano(cfg.color)
    );
    opcion!(
        "Puerto TCP del servidor local del registry de paquetes.",
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

/// Sesión de compilación: opciones + estado que debe viajar junta.
///
/// Es el primer paso para dejar de usar `config::actual()` global. Todavía no
/// reemplaza al pipeline; permite crear dos compilaciones con opciones distintas
/// en el mismo proceso (tests, LSP, wasm).
#[derive(Debug, Clone)]
pub struct Sesion {
    pub config: Config,
    pub mapa_fuente: forja_span::FuenteMapa,
    pub diagnosticos: Vec<forja_errors::Diagnostico>,
}

impl Sesion {
    pub fn nueva(config: Config) -> Self {
        Sesion {
            config,
            mapa_fuente: forja_span::FuenteMapa::nuevo(),
            diagnosticos: Vec::new(),
        }
    }

    pub fn por_defecto() -> Self {
        Sesion {
            config: Config::default(),
            mapa_fuente: forja_span::FuenteMapa::nuevo(),
            diagnosticos: Vec::new(),
        }
    }

    pub fn agregar_fuente(
        &mut self,
        nombre: impl Into<std::path::PathBuf>,
        fuente: impl Into<String>,
    ) -> forja_span::ArchivoId {
        self.mapa_fuente.agregar(nombre, fuente)
    }

    pub fn emitir(&mut self, diagnostico: forja_errors::Diagnostico) {
        self.diagnosticos.push(diagnostico);
    }

    pub fn estricto(&self) -> bool {
        self.config.estricto
    }

    pub fn contratos(&self) -> bool {
        self.config.contratos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sesion_registra_fuente_y_diagnostico() {
        let mut s = Sesion::por_defecto();
        let f = s.agregar_fuente(
            "a.fa", "uno
dos",
        );
        let span = forja_span::Span::nuevo(f, 4, 7);
        s.emitir(forja_errors::Diagnostico::error("E1", "mal").con_span(span));
        assert_eq!(s.mapa_fuente.ubicacion(span).unwrap().linea, 2);
        assert_eq!(s.diagnosticos.len(), 1);
        assert_eq!(s.diagnosticos[0].principal(), Some(span));
    }

    #[test]
    fn dos_sesiones_no_comparten_opciones() {
        let a = Sesion::nueva(Config {
            estricto: true,
            ..Config::default()
        });
        let b = Sesion::por_defecto();
        assert!(a.estricto());
        assert!(!b.estricto());
    }

    #[test]
    fn yaml_parcial_usa_defaults() {
        let cfg: Config = serde_yaml::from_str(
            "estricto: verdadero
",
        )
        .unwrap();
        assert!(cfg.estricto);
        assert!(cfg.contratos);
    }
}
