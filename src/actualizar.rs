//! Instalación portable, actualizaciones y mantenimiento de Forja.
//!
//! Todo lo que necesita hablar con GitHub usa `curl` (viene en Windows 10+,
//! Linux y macOS) y `tar` para los zip. Sin dependencias nuevas.
//!
//! Estado en disco, dentro de `%LOCALAPPDATA%\Forja` (o al lado del .exe en
//! modo portable):
//!   install.json       qué se instaló (versión, carpeta, componentes)
//!   update-check.json  caché del chequeo de versión (24 h)
//!   config.txt         opt-ins: auto_actualizar, telemetria
//!   canal.txt          estable | beta
//!   reportes\          reportes de fallo locales (telemetría opt-in)

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const REPO: &str = "forja-lang/forja";
const SETUP_ASSET: &str = "forja-setup-x64.exe";
const VENTANA_CACHE_SEGS: u64 = 24 * 60 * 60;

// ── Manifiesto de instalación ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Manifiesto {
    pub version: String,
    pub path: String,
    pub features: Vec<String>,
}

impl Manifiesto {
    pub fn tiene(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }

    /// Nombres de los binarios que corresponden a los componentes aceptados.
    pub fn binarios(&self) -> Vec<&'static str> {
        let mut bins = vec!["forja"];
        if self.tiene("LanguageServer") {
            bins.push("forja-lsp");
        }
        if self.tiene("Debugger") {
            bins.push("forja-dap");
        }
        if self.tiene("Runtime") {
            bins.push("forja-rt");
        }
        bins
    }
}

/// Componentes escritos por el usuario (`lsp`, `dap`, `rt`) a ids del MSI.
pub fn normalizar_componentes(texto: &str) -> Vec<String> {
    let mut features = vec!["Core".to_string()];

    for parte in texto.split(|c: char| c == ',' || c == ';' || c == ' ') {
        let p = parte.trim().to_lowercase();
        let id = match p.as_str() {
            "lsp" | "lenguaje" | "languageserver" => "LanguageServer",
            "dap" | "depurador" | "debugger" => "Debugger",
            "rt" | "runtime" | "forja-rt" => "Runtime",
            "core" | "compilador" | "" => continue,
            _ => continue,
        };

        if !features.iter().any(|f| f == id) {
            features.push(id.to_string());
        }
    }

    features
}

// ── Ubicaciones ───────────────────────────────────────────────────────────

/// Ruta real del ejecutable. En Windows la CLI se re-lanza desde una copia en
/// %TEMP% (selfrun), así que el path original viene en FORJA_ORIGINAL_EXE.
pub fn ruta_ejecutable() -> Option<PathBuf> {
    if let Ok(original) = std::env::var("FORJA_ORIGINAL_EXE") {
        let p = PathBuf::from(original);
        if p.exists() {
            return Some(p);
        }
    }

    std::env::current_exe().ok()
}

/// Modo portable: un archivo `forja.portable` al lado del ejecutable.
pub fn modo_portable() -> bool {
    ruta_ejecutable()
        .and_then(|exe| exe.parent().map(|d| d.join("forja.portable").exists()))
        .unwrap_or(false)
}

fn directorio_portable() -> Option<PathBuf> {
    ruta_ejecutable()?.parent().map(|p| p.to_path_buf())
}

fn directorio_datos() -> Option<PathBuf> {
    if modo_portable() {
        return directorio_portable();
    }

    if let Ok(pd) = std::env::var("ProgramData") {
        let p = PathBuf::from(pd).join("Forja");
        if p.join("install.json").exists() {
            return Some(p);
        }
    }

    let base = std::env::var("LOCALAPPDATA")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".local/share"))
        })?;

    Some(base.join("Forja"))
}

pub fn ruta_manifiesto() -> Option<PathBuf> {
    let p = directorio_datos()?.join("install.json");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

pub fn leer_manifiesto() -> Option<Manifiesto> {
    let texto = fs::read_to_string(ruta_manifiesto()?).ok()?;
    // Algunos instaladores escriben UTF-8 con BOM.
    let texto = texto.trim_start_matches('\u{feff}');
    let json: Value = serde_json::from_str(texto).ok()?;

    Some(Manifiesto {
        version: json.get("version")?.as_str()?.to_string(),
        path: json.get("path")?.as_str()?.to_string(),
        features: json
            .get("features")?
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
    })
}

fn guardar_manifiesto(m: &Manifiesto) {
    let Some(dir) = directorio_datos() else {
        return;
    };
    let _ = fs::create_dir_all(&dir);

    let features = m
        .features
        .iter()
        .map(|f| format!("\"{}\"", f))
        .collect::<Vec<_>>()
        .join(",");

    let json = format!(
        "{{\n  \"version\": \"{}\",\n  \"path\": \"{}\",\n  \"features\": [{}]\n}}\n",
        m.version,
        m.path.replace('\\', "\\\\"),
        features
    );

    let _ = fs::write(dir.join("install.json"), json);
}

// ── Configuración opt-in ──────────────────────────────────────────────────

fn leer_config() -> Vec<(String, String)> {
    let Some(dir) = directorio_datos() else {
        return Vec::new();
    };
    let Ok(texto) = fs::read_to_string(dir.join("config.txt")) else {
        return Vec::new();
    };

    texto
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_lowercase()))
        .collect()
}

fn leer_opcion(clave: &str) -> bool {
    leer_config()
        .into_iter()
        .any(|(k, v)| k == clave && matches!(v.as_str(), "si" | "sí" | "true" | "1"))
}

fn guardar_opcion(clave: &str, activo: bool) {
    let Some(dir) = directorio_datos() else {
        return;
    };
    let _ = fs::create_dir_all(&dir);

    let mut opciones = leer_config();
    opciones.retain(|(k, _)| k != clave);
    opciones.push((
        clave.to_string(),
        if activo { "si".into() } else { "no".into() },
    ));

    let texto = opciones
        .iter()
        .map(|(k, v)| format!("{k}={v}\n"))
        .collect::<String>();

    let _ = fs::write(dir.join("config.txt"), texto);
}

pub fn auto_actualizar_activo() -> bool {
    leer_opcion("auto_actualizar")
}

pub fn telemetria_activa() -> bool {
    leer_opcion("telemetria")
}

// ── Canales ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Canal {
    Estable,
    Beta,
}

fn leer_canal() -> Canal {
    let Some(dir) = directorio_datos() else {
        return Canal::Estable;
    };
    match fs::read_to_string(dir.join("canal.txt")) {
        Ok(t) if t.trim().eq_ignore_ascii_case("beta") => Canal::Beta,
        _ => Canal::Estable,
    }
}

fn guardar_canal(canal: Canal) {
    let Some(dir) = directorio_datos() else {
        return;
    };
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(
        dir.join("canal.txt"),
        if canal == Canal::Beta {
            "beta\n"
        } else {
            "estable\n"
        },
    );
}

fn nombre_canal(canal: Canal) -> &'static str {
    if canal == Canal::Beta {
        "beta"
    } else {
        "estable"
    }
}

// ── Red ───────────────────────────────────────────────────────────────────

fn segundos_ahora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn curl(url: &str, tiempo_max: &str) -> Option<Vec<u8>> {
    let salida = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            tiempo_max,
            "-H",
            "Accept: application/vnd.github+json",
            url,
        ])
        .output()
        .ok()?;

    if salida.status.success() {
        Some(salida.stdout)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub zip: Option<String>,
    pub setup: Option<String>,
    pub zip_digest: Option<String>,
    pub setup_digest: Option<String>,
}

fn release_json(canal: Canal) -> Option<Value> {
    let url = if canal == Canal::Beta {
        format!("https://api.github.com/repos/{REPO}/releases?per_page=5")
    } else {
        format!("https://api.github.com/repos/{REPO}/releases/latest")
    };

    let cuerpo = curl(&url, "4")?;
    let json: Value = serde_json::from_slice(&cuerpo).ok()?;

    if canal == Canal::Beta {
        json.as_array()?.first().cloned()
    } else {
        Some(json)
    }
}

fn nuevo_release(canal: Canal) -> Option<Release> {
    let json = release_json(canal)?;
    let version = json
        .get("tag_name")?
        .as_str()?
        .trim_start_matches('v')
        .to_string();

    let assets = json.get("assets")?.as_array()?;
    let buscar = |sufijo: &str| -> Option<(String, Option<String>)> {
        let asset = assets.iter().find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .ends_with(sufijo)
        })?;

        Some((
            asset.get("browser_download_url")?.as_str()?.to_string(),
            asset
                .get("digest")
                .and_then(|d| d.as_str())
                .map(|d| d.trim_start_matches("sha256:").to_string()),
        ))
    };

    let zip = buscar("-windows-x64.zip");
    let setup = buscar(SETUP_ASSET);

    Some(Release {
        version,
        zip: zip.as_ref().map(|z| z.0.clone()),
        zip_digest: zip.and_then(|z| z.1),
        setup: setup.as_ref().map(|s| s.0.clone()),
        setup_digest: setup.and_then(|s| s.1),
    })
}

fn descargar(url: &str, destino: &Path) -> bool {
    Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "300",
            "-o",
            &destino.to_string_lossy(),
            "--",
            url,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// SHA-256 del archivo. Usa las herramientas del sistema para no agregar
/// dependencias de hashing al core.
fn sha256(ruta: &Path) -> Option<String> {
    #[cfg(windows)]
    let (programa, args) = (
        "powershell",
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            format!(
                "(Get-FileHash -Algorithm SHA256 -LiteralPath '{}').Hash.ToLower()",
                ruta.to_string_lossy()
            ),
        ],
    );

    #[cfg(not(windows))]
    let (programa, args) = ("sha256sum", vec![ruta.to_string_lossy().to_string()]);

    let salida = Command::new(programa).args(&args).output().ok()?;
    if !salida.status.success() {
        return None;
    }

    let texto = String::from_utf8_lossy(&salida.stdout);
    let hash = texto.split_whitespace().next()?.to_lowercase();
    Some(hash)
}

fn verificar_digest(ruta: &Path, esperado: &Option<String>) -> Result<(), String> {
    let Some(esperado) = esperado else {
        println!(
            "{}  El release no publica digest: no puedo verificar el archivo.",
            crate::error::color::AMARILLO
        );
        return Ok(());
    };

    match sha256(ruta) {
        Some(real) if real == *esperado => Ok(()),
        Some(real) => Err(format!(
            "El archivo no coincide con el digest publicado.\nesperado: {esperado}\nobtenido: {real}"
        )),
        None => Err("No pude calcular el SHA-256 del archivo descargado.".to_string()),
    }
}

// ── Actualización ─────────────────────────────────────────────────────────

pub fn es_mas_nueva(nueva: &str, actual: &str) -> bool {
    let parsear = |v: &str| -> Vec<u64> {
        v.split(|c: char| c == '.' || c == '-' || c == '+')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };

    let (a, b) = (parsear(nueva), parsear(actual));
    for i in 0..a.len().max(b.len()) {
        let x = *a.get(i).unwrap_or(&0);
        let y = *b.get(i).unwrap_or(&0);
        if x != y {
            return x > y;
        }
    }

    false
}

fn version_disponible(actual: &str) -> Option<String> {
    let canal = leer_canal();

    if let Some(dir) = directorio_datos() {
        let cache = dir.join("update-check.json");
        if let Ok(texto) = fs::read_to_string(&cache) {
            if let Ok(json) = serde_json::from_str::<Value>(&texto) {
                let guardada = json.get("version").and_then(|v| v.as_str());
                let cuando = json.get("checked_at").and_then(|v| v.as_u64()).unwrap_or(0);
                let mismo_canal =
                    json.get("canal").and_then(|v| v.as_str()) == Some(nombre_canal(canal));

                if let Some(v) = guardada {
                    if mismo_canal && segundos_ahora().saturating_sub(cuando) < VENTANA_CACHE_SEGS {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }

    let release = nuevo_release(canal)?;

    if let Some(dir) = directorio_datos() {
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(
            dir.join("update-check.json"),
            format!(
                "{{\"version\":\"{}\",\"checked_at\":{},\"canal\":\"{}\",\"actual\":\"{}\"}}",
                release.version,
                segundos_ahora(),
                nombre_canal(canal),
                actual
            ),
        );
    }

    Some(release.version)
}

pub fn avisar_actualizar() {
    let actual = env!("CARGO_PKG_VERSION");
    let Some(nueva) = version_disponible(actual) else {
        return;
    };

    if es_mas_nueva(&nueva, actual) {
        println!(
            "{}  Hay una versión nueva de Forja ({} → {}, canal {}). Ejecutá: forja actualizar\n",
            crate::error::color::AMARILLO,
            actual,
            nueva,
            nombre_canal(leer_canal()),
        );
    }
}

/// Auto-actualización silenciosa opt-in (`forja autoactualizar activar`).
pub fn auto_si_habilitado() {
    if !auto_actualizar_activo() {
        return;
    }

    let actual = env!("CARGO_PKG_VERSION");
    let Some(nueva) = version_disponible(actual) else {
        return;
    };

    if !es_mas_nueva(&nueva, actual) {
        return;
    }

    println!("Actualización automática: {actual} → {nueva}…");
    let codigo = aplicar_actualizacion();
    if codigo == 0 {
        println!("Forja quedó actualizado a la versión {nueva}.");
    } else {
        eprintln!("La actualización automática falló (código {codigo}). Seguís en {actual}.");
    }
}

fn temporal_directorio() -> Option<PathBuf> {
    std::env::var("TEMP")
        .or_else(|_| std::env::var("TMPDIR"))
        .ok()
        .map(PathBuf::from)
}

/// Reemplaza solo los archivos que cambiaron, respetando los componentes
/// instalados (delta update). Devuelve el código de salida.
fn aplicar_delta(release: &Release, manifiesto: &Manifiesto) -> i32 {
    let Some(url) = release.zip.clone() else {
        return 1;
    };

    let Some(temp) = temporal_directorio() else {
        return 1;
    };

    let zip = temp.join(format!("forja-{}.zip", release.version));
    println!("Bajando paquete de actualización…");

    if !descargar(&url, &zip) {
        eprintln!("No pude descargar {}", url);
        return 1;
    }

    if let Err(e) = verificar_digest(&zip, &release.zip_digest) {
        eprintln!("{e}");
        let _ = fs::remove_file(&zip);
        return 1;
    }

    println!("Verificado (SHA-256). Aplicando cambios…");

    let extraido = temp.join(format!("forja-extraido-{}", release.version));
    let _ = fs::remove_dir_all(&extraido);
    let _ = fs::create_dir_all(&extraido);

    let correcto = Command::new("tar")
        .args([
            "-xf",
            &zip.to_string_lossy(),
            "-C",
            &extraido.to_string_lossy(),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let _ = fs::remove_file(&zip);

    if !correcto {
        eprintln!("No pude descomprimir el paquete (¿falta tar?).");
        return 1;
    }

    // El zip trae una carpeta raíz: forja-<versión>-windows-x64/
    let raiz = match fs::read_dir(&extraido).ok().and_then(|mut d| {
        d.find_map(|e| {
            let p = e.ok()?.path();
            if p.is_dir() {
                Some(p)
            } else {
                None
            }
        })
    }) {
        Some(p) => p,
        None => {
            eprintln!("El paquete no tiene la estructura esperada.");
            return 1;
        }
    };

    let destino = PathBuf::from(&manifiesto.path);
    let mut copiados = 0;
    let mut fallos = 0;

    for bin in manifiesto.binarios() {
        let origen = raiz.join(format!("{bin}.exe"));
        let origen = if origen.exists() {
            origen
        } else {
            raiz.join(bin)
        };

        let objetivo = destino.join(if cfg!(windows) {
            format!("{bin}.exe")
        } else {
            bin.to_string()
        });

        if !origen.exists() || !objetivo.exists() {
            continue; // componente no instalado o nombre inesperado
        }

        match copiar_si_cambia(&origen, &objetivo) {
            Ok(true) => copiados += 1,
            Ok(false) => {}
            Err(_) => fallos += 1,
        }
    }

    // Biblioteca estándar y documentación: forman parte de Core.
    for sub in ["stdlib", "ejemplos"] {
        let origen = raiz.join(sub);
        if origen.is_dir() {
            copiados += copiar_directorio_delta(&origen, &destino.join(sub), &mut fallos);
        }
    }

    for doc in ["README.md", "COMANDOS.md", "INSTRUCCIONES.md", "LICENSE.md"] {
        let origen = raiz.join(doc);
        let objetivo = destino.join(doc);
        if origen.exists() && objetivo.exists() {
            if copiar_si_cambia(&origen, &objetivo).unwrap_or(false) {
                copiados += 1;
            }
        }
    }

    let _ = fs::remove_dir_all(&extraido);

    if fallos > 0 {
        eprintln!("{fallos} archivo(s) no se pudieron reemplazar (¿permisos?).");
    }

    if copiados == 0 && fallos == 0 {
        println!("No había archivos nuevos: ya estabas al día.");
    } else {
        println!("{copiados} archivo(s) actualizados.");
    }

    if fallos > 0 {
        return 1;
    }

    guardar_manifiesto(&Manifiesto {
        version: release.version.clone(),
        ..manifiesto.clone()
    });

    0
}

fn copiar_si_cambia(origen: &Path, destino: &Path) -> std::io::Result<bool> {
    let (a, b) = (fs::metadata(origen)?.len(), fs::metadata(destino)?.len());

    if a == b {
        return Ok(false);
    }

    fs::copy(origen, destino)?;
    Ok(true)
}

fn copiar_directorio_delta(origen: &Path, destino: &Path, fallos: &mut i32) -> i32 {
    let mut copiados = 0;
    let Ok(entradas) = fs::read_dir(origen) else {
        return 0;
    };

    let _ = fs::create_dir_all(destino);

    for entrada in entradas.flatten() {
        let desde = entrada.path();
        let hacia = destino.join(entrada.file_name());

        if desde.is_dir() {
            copiados += copiar_directorio_delta(&desde, &hacia, fallos);
            continue;
        }

        if !hacia.exists() {
            if fs::copy(&desde, &hacia).is_ok() {
                copiados += 1;
            } else {
                *fallos += 1;
            }
            continue;
        }

        match copiar_si_cambia(&desde, &hacia) {
            Ok(true) => copiados += 1,
            Ok(false) => {}
            Err(_) => *fallos += 1,
        }
    }

    copiados
}

fn aplicar_actualizacion() -> i32 {
    let Some(manifiesto) = leer_manifiesto() else {
        eprintln!("No encontré una instalación registrada de Forja (falta install.json).");
        eprintln!("Instalá Forja con el instalador de https://github.com/{REPO}/releases");
        return 1;
    };

    let canal = leer_canal();
    let Some(release) = nuevo_release(canal) else {
        eprintln!("No pude consultar los releases de GitHub (¿sin red?). Probá más tarde.");
        return 1;
    };

    if !es_mas_nueva(&release.version, &manifiesto.version) {
        println!(
            "Ya tenés la última versión del canal {} ({}).",
            nombre_canal(canal),
            manifiesto.version
        );
        return 0;
    }

    println!(
        "Actualizando Forja {} → {} (canal {}).",
        manifiesto.version,
        release.version,
        nombre_canal(canal)
    );
    println!(
        "Carpeta: {}\nComponentes: {}",
        manifiesto.path,
        manifiesto.features.join(", ")
    );

    // 1) Delta por zip: solo toca los archivos que cambiaron.
    let codigo = aplicar_delta(&release, &manifiesto);
    if codigo == 0 {
        return 0;
    }

    // 2) Si el delta no se puede, cae al instalador completo en silencio.
    println!("El modo delta no funcionó; uso el instalador completo…");

    let Some(setup_url) = release.setup.clone() else {
        eprintln!(
            "El release no publica {SETUP_ASSET}. Actualizá a mano desde https://github.com/{REPO}/releases"
        );
        return 1;
    };

    let Some(temp) = temporal_directorio() else {
        return 1;
    };

    let setup = temp.join(format!("forja-setup-{}.exe", release.version));
    if !descargar(&setup_url, &setup) {
        eprintln!("No pude descargar {}", setup_url);
        return 1;
    }

    if let Err(e) = verificar_digest(&setup, &release.setup_digest) {
        eprintln!("{e}");
        let _ = fs::remove_file(&setup);
        return 1;
    }

    let estado = Command::new(&setup).args(["/install", "/quiet"]).status();
    let _ = fs::remove_file(&setup);

    match estado {
        Ok(s) if s.success() => {
            println!("Listo: Forja quedó en la versión {}.", release.version);
            let _ = fs::remove_file(
                directorio_datos()
                    .map(|d| d.join("update-check.json"))
                    .unwrap_or_default(),
            );
            0
        }
        Ok(s) => {
            eprintln!("El instalador terminó con código {}.", s);
            1
        }
        Err(e) => {
            eprintln!("No pude ejecutar el instalador: {e}");
            1
        }
    }
}

// ── Comandos ──────────────────────────────────────────────────────────────

/// `forja actualizar [componentes <lsp,dap,rt>] [beta|estable]`
pub fn actualizar(args: &[String]) -> i32 {
    let mut componentes: Option<Vec<String>> = None;
    let mut canal: Option<Canal> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].to_lowercase().as_str() {
            "componentes" | "components" => {
                let valor = args.get(i + 1).cloned().unwrap_or_default();
                componentes = Some(normalizar_componentes(&valor));
                i += 1;
            }
            "beta" => canal = Some(Canal::Beta),
            "estable" | "stable" => canal = Some(Canal::Estable),
            otro if otro.contains(',') => componentes = Some(normalizar_componentes(otro)),
            _ => {}
        }
        i += 1;
    }

    if let Some(canal) = canal {
        guardar_canal(canal);
        println!("Canal de actualizaciones: {}.", nombre_canal(canal));
    }

    // Cambiar componentes: el MSI no puede cambiar features en caliente, así
    // que se desinstala y se instala de nuevo con el pedido anotado. El pedido
    // va por archivo porque Burn ignora `InstallFolder=`/`Componentes=`.
    if let Some(features) = componentes {
        let Some(manifiesto) = leer_manifiesto() else {
            eprintln!("No hay una instalación registrada; instalá Forja primero.");
            return 1;
        };

        println!(
            "Componentes nuevos: {} (carpeta {})",
            features.join(", "),
            manifiesto.path
        );

        let Some(setup_original) = ruta_instalador() else {
            eprintln!("No encontré el instalador de Forja.");
            eprintln!("Instalá con el forja-setup-x64.exe de los releases y volvé a intentar.");
            return 1;
        };

        // Copia local: la desinstalación borra el ejecutable cacheado del bundle
        // que estamos usando, así que no se puede relanzar desde ahí.
        let Some(temp) = temporal_directorio() else {
            return 1;
        };
        let setup = temp.join("forja-setup-cambio.exe");

        if let Err(e) = fs::copy(&setup_original, &setup) {
            eprintln!("No pude preparar el instalador: {e}");
            return 1;
        }

        escribir_pedido(&manifiesto.path, &features.join(","));

        println!("Desinstalando la versión actual…");
        match Command::new(&setup).args(["/uninstall", "/quiet"]).status() {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("La desinstalación terminó con código {s}. No se cambió nada.");
                let _ = fs::remove_file(ruta_pedido());
                return 1;
            }
            Err(e) => {
                eprintln!("No pude ejecutar el desinstalador: {e}");
                let _ = fs::remove_file(ruta_pedido());
                return 1;
            }
        }

        println!("Instalando con los componentes elegidos…");
        let resultado = Command::new(&setup).args(["/install", "/quiet"]).status();
        let _ = fs::remove_file(&setup);

        return match resultado {
            Ok(s) if s.success() => {
                println!("Listo. Componentes: {}.", features.join(", "));
                0
            }
            _ => {
                eprintln!("El instalador no pudo aplicar el cambio de componentes.");
                1
            }
        };
    }

    aplicar_actualizacion()
}

/// `forja desinstalar`
pub fn desinstalar() -> i32 {
    #[cfg(not(windows))]
    {
        eprintln!("forja desinstalar solo aplica a la instalación de Windows.");
        eprintln!("En este sistema, borrá el directorio de Forja y el PATH que hayas agregado.");
        return 1;
    }

    #[cfg(windows)]
    {
        let clave = match buscar_desinstalador() {
            Some(k) => k,
            None => {
                eprintln!("No encontré Forja en Agregar/Quitar programas.");
                eprintln!("Si lo instalaste en modo portable, alcanza con borrar la carpeta.");
                return 1;
            }
        };

        println!("Desinstalando con: {clave}");
        let programa = clave.split('"').nth(1).unwrap_or(&clave).to_string();

        let estado = Command::new(&programa)
            .args(["/uninstall", "/quiet"])
            .status();

        match estado {
            Ok(s) if s.success() => {
                if let Some(dir) = directorio_datos() {
                    let _ = fs::remove_dir_all(&dir);
                }
                println!("Forja se desinstaló.");
                0
            }
            _ => {
                eprintln!("El desinstalador no terminó bien. Podés usar Agregar/Quitar programas.");
                1
            }
        }
    }
}

#[cfg(windows)]
fn buscar_desinstalador() -> Option<String> {
    for raiz in [
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    ] {
        let Ok(salida) = Command::new("reg")
            .args(["query", raiz, "/s", "/f", "Forja", "/d"])
            .output()
        else {
            continue;
        };

        let texto = String::from_utf8_lossy(&salida.stdout);
        for linea in texto.lines() {
            if linea.contains("UninstallString") && linea.contains("forja-setup") {
                if let Some(valor) = linea.split("REG_SZ").nth(1) {
                    return Some(valor.trim().to_string());
                }
            }
        }
    }

    None
}

/// `forja canal [estable|beta]`
pub fn canal(args: &[String]) -> i32 {
    match args.first().map(|a| a.to_lowercase()) {
        Some(a) if a == "beta" => {
            guardar_canal(Canal::Beta);
            println!("Canal de actualizaciones: beta (incluye versiones de prueba).");
            0
        }
        Some(a) if a == "estable" || a == "stable" => {
            guardar_canal(Canal::Estable);
            println!("Canal de actualizaciones: estable.");
            0
        }
        None => {
            println!("Canal actual: {}.", nombre_canal(leer_canal()));
            0
        }
        _ => {
            eprintln!("Uso: forja canal [estable|beta]");
            1
        }
    }
}

/// `forja autoactualizar [activar|desactivar]`
pub fn autoactualizar(args: &[String]) -> i32 {
    match args.first().map(|a| a.to_lowercase()) {
        Some(a) if a == "activar" || a == "on" || a == "si" || a == "sí" => {
            guardar_opcion("auto_actualizar", true);
            println!("Auto-actualización activada: al ejecutar forja se instala la versión nueva en silencio.");
            0
        }
        Some(a) if a == "desactivar" || a == "off" || a == "no" => {
            guardar_opcion("auto_actualizar", false);
            println!("Auto-actualización desactivada.");
            0
        }
        None | Some(_) => {
            println!(
                "Auto-actualización: {}.",
                if auto_actualizar_activo() {
                    "activada"
                } else {
                    "desactivada"
                }
            );
            println!("Uso: forja autoactualizar [activar|desactivar]");
            0
        }
    }
}

/// `forja telemetria [activar|desactivar]`
///
/// No hay servidor: los reportes de fallo se guardan localmente en
/// `...\Forja\reportes\` y el usuario decide si los comparte.
pub fn telemetria(args: &[String]) -> i32 {
    match args.first().map(|a| a.to_lowercase()) {
        Some(a) if a == "activar" || a == "on" || a == "si" || a == "sí" => {
            guardar_opcion("telemetria", true);
            println!("Telemetría activada: los fallos se guardan como archivos locales en la carpeta de datos.");
            0
        }
        Some(a) if a == "desactivar" || a == "off" || a == "no" => {
            guardar_opcion("telemetria", false);
            println!("Telemetría desactivada.");
            0
        }
        _ => {
            println!(
                "Telemetría: {}. Reportes guardados: {}",
                if telemetria_activa() {
                    "activada"
                } else {
                    "desactivada"
                },
                carpeta_reportes()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "(sin carpeta de datos)".to_string())
            );
            0
        }
    }
}

fn carpeta_reportes() -> Option<PathBuf> {
    Some(directorio_datos()?.join("reportes"))
}

/// Guarda un reporte local de fallo si la telemetría está activa.
/// Nunca envía nada por red.
pub fn registrar_fallo(contexto: &str, detalle: &str) {
    if !telemetria_activa() {
        return;
    }

    let Some(dir) = carpeta_reportes() else {
        return;
    };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    let archivo = dir.join(format!("fallo-{}.txt", segundos_ahora()));

    if let Ok(mut f) = fs::File::create(&archivo) {
        let _ = writeln!(f, "forja {}", env!("CARGO_PKG_VERSION"));
        let _ = writeln!(f, "cuando: {}", segundos_ahora());
        let _ = writeln!(f, "contexto: {contexto}");
        let _ = writeln!(f, "detalle:\n{detalle}");
        eprintln!(
            "Se guardó un reporte de fallo en {} (no se envía automáticamente).",
            archivo.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compara_versiones() {
        assert!(es_mas_nueva("0.10.0", "0.9.7"));
        assert!(es_mas_nueva("1.0.0", "0.99.99"));
        assert!(!es_mas_nueva("0.9.7", "0.9.7"));
        assert!(!es_mas_nueva("0.9.6", "0.9.7"));
        assert!(es_mas_nueva("0.9.7.1", "0.9.7"));
    }

    #[test]
    fn normaliza_componentes() {
        assert_eq!(
            normalizar_componentes("lsp dap"),
            vec!["Core", "LanguageServer", "Debugger"]
        );
        assert_eq!(normalizar_componentes("rt"), vec!["Core", "Runtime"]);
        assert_eq!(normalizar_componentes(""), vec!["Core"]);
        assert_eq!(
            normalizar_componentes("lsp,LSP,otro"),
            vec!["Core", "LanguageServer"]
        );
    }

    #[test]
    fn lee_manifiesto_con_features() {
        let texto = r#"{
  "version": "0.9.7",
  "path": "C:\\Users\\x\\AppData\\Local\\Programs\\Forja",
  "features": ["Core","LanguageServer"]
}"#;
        let json: Value = serde_json::from_str(texto).unwrap();
        let features: Vec<String> = json["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(features, vec!["Core", "LanguageServer"]);
        assert!(json["path"].as_str().unwrap().contains("Programs"));
    }

    #[test]
    fn binarios_respetan_componentes() {
        let m = Manifiesto {
            version: "1.0.0".into(),
            path: "C:\\Forja".into(),
            features: vec!["Core".into(), "Runtime".into()],
        };
        assert_eq!(m.binarios(), vec!["forja", "forja-rt"]);
    }
}

// ── Instalador y pedido de instalación ────────────────────────────────────

/// Ruta del forja-setup-x64.exe que instaló Forja (cache del bundle).
#[cfg(windows)]
fn ruta_instalador() -> Option<PathBuf> {
    let clave = buscar_desinstalador()?;
    let resto = clave.split("\" /uninstall").next().unwrap_or(&clave);
    let ruta = resto.trim().trim_matches('"').trim();

    let p = PathBuf::from(ruta);
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

#[cfg(not(windows))]
fn ruta_instalador() -> Option<PathBuf> {
    None
}

fn ruta_pedido() -> PathBuf {
    directorio_datos()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("setup-request.txt")
}

/// Lo lee el instalador antes de planificar (Burn ignora esos valores por
/// línea de comandos y las variables de entorno no sobreviven al UAC).
fn escribir_pedido(carpeta: &str, componentes: &str) {
    let ruta = ruta_pedido();

    if let Some(dir) = ruta.parent() {
        let _ = fs::create_dir_all(dir);
    }

    let _ = fs::write(
        &ruta,
        format!("InstallFolder={carpeta}\nComponentes={componentes}\n"),
    );
}
