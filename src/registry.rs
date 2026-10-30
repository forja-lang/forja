// F3-T1: Registry de paquetes de Forja (alcance pragmático local/offline).
//
// El plan original pedía "levantar registry.forja-lang.dev". Como el servidor
// remoto no es viable en este entorno, este módulo implementa TODA la lógica
// verificable localmente:
//   - Index de paquetes (JSON) con versiones, hashes y firmas.
//   - Empaquetado de un proyecto en un tarball determinista (`.fapkg`).
//   - Firma/verificación de paquetes (SHA-256 + firma HMAC-like con clave).
//   - Versionado semver (parseo, validación y comparación).
//   - `publish` (empaquetar + indexar + firmar) y `yank` (retirar versión).
//
// El cliente (`PackageResolver`) consume este registry local. La capa de red
// (HTTP hacia registry.forja-lang.dev) queda documentada como deuda futura:
// el formato del index y de los tarballs es el mismo que usaría el servidor,
// por lo que conectar un backend HTTP real no requiere cambiar este módulo.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::base64::b64_encode;
use crate::hash::Sha256;

/// Versión semver parseada: `mayor.menor.parche` con pre-release opcional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Semver {
    pub mayor: u64,
    pub menor: u64,
    pub parche: u64,
    /// Pre-release (ej: "alpha.1"). Vacío = versión estable.
    pub pre: String,
}

impl Semver {
    /// Parsea una cadena semver. Acepta `1`, `1.2`, `1.2.3` y `1.2.3-pre`.
    pub fn parse(s: &str) -> Result<Semver, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("versión vacía".to_string());
        }
        // Separar pre-release por el primer '-'
        let (nucleo, pre) = match s.split_once('-') {
            Some((n, p)) => (n, p.to_string()),
            None => (s, String::new()),
        };
        let partes: Vec<&str> = nucleo.split('.').collect();
        if partes.is_empty() || partes.len() > 3 {
            return Err(format!("versión semver inválida: '{}'", s));
        }
        let mut nums = [0u64; 3];
        for (i, p) in partes.iter().enumerate() {
            if p.is_empty() {
                return Err(format!("versión semver inválida: '{}'", s));
            }
            nums[i] = p
                .parse::<u64>()
                .map_err(|_| format!("componente no numérico en '{}': '{}'", s, p))?;
        }
        Ok(Semver {
            mayor: nums[0],
            menor: nums[1],
            parche: nums[2],
            pre,
        })
    }

    /// Valida que una cadena sea semver correcta.
    pub fn es_valida(s: &str) -> bool {
        Semver::parse(s).is_ok()
    }

    /// Clave de ordenamiento: una versión estable es mayor que cualquier
    /// pre-release de la misma terna numérica.
    fn clave_orden(&self) -> (u64, u64, u64, u8, String) {
        let estable = if self.pre.is_empty() { 1 } else { 0 };
        (
            self.mayor,
            self.menor,
            self.parche,
            estable,
            self.pre.clone(),
        )
    }
}

impl std::fmt::Display for Semver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.pre.is_empty() {
            write!(f, "{}.{}.{}", self.mayor, self.menor, self.parche)
        } else {
            write!(
                f,
                "{}.{}.{}-{}",
                self.mayor, self.menor, self.parche, self.pre
            )
        }
    }
}

impl PartialOrd for Semver {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Semver {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.clave_orden().cmp(&other.clave_orden())
    }
}

/// Una versión publicada dentro del index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionPublicada {
    pub version: String,
    /// SHA-256 (hex) del tarball `.fapkg`.
    pub sha256: String,
    /// Firma del hash con la clave del autor (base64 o hex según el esquema).
    pub firma: String,
    /// F12-T4/F14-T3: clave pública Ed25519 del autor (hex). Si está presente,
    /// `firma` es una firma Ed25519 (hex) del `sha256` y la verificación es
    /// asimétrica: el servidor no necesita la clave privada de nadie.
    #[serde(default)]
    pub clave_publica: String,
    /// Autor que publicó.
    #[serde(default)]
    pub autor: String,
    /// Descripción del paquete.
    #[serde(default)]
    pub descripcion: String,
    /// `true` si la versión fue retirada con `yank`.
    #[serde(default)]
    pub yanked: bool,
}

/// Entrada de un paquete en el index: todas sus versiones.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntradaPaquete {
    pub nombre: String,
    /// version -> datos de la versión.
    pub versiones: BTreeMap<String, VersionPublicada>,
}

/// Index completo del registry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    /// nombre -> entrada.
    pub paquetes: BTreeMap<String, EntradaPaquete>,
}

impl Index {
    pub fn nuevo() -> Self {
        Index {
            paquetes: BTreeMap::new(),
        }
    }

    pub fn cargar(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Index::nuevo());
        }
        let contenido = fs::read_to_string(path)
            .map_err(|e| format!("Error leyendo index {}: {}", path.display(), e))?;
        if contenido.trim().is_empty() {
            return Ok(Index::nuevo());
        }
        serde_json::from_str(&contenido)
            .map_err(|e| format!("Error parseando index {}: {}", path.display(), e))
    }

    pub fn guardar(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| format!("Error creando directorio del index: {}", e))?;
        }
        let contenido = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Error serializando index: {}", e))?;
        fs::write(path, contenido)
            .map_err(|e| format!("Error escribiendo index {}: {}", path.display(), e))
    }

    /// Devuelve la versión más alta NO retirada de un paquete.
    pub fn ultima_version(&self, nombre: &str) -> Option<&VersionPublicada> {
        let entrada = self.paquetes.get(nombre)?;
        entrada
            .versiones
            .values()
            .filter(|v| !v.yanked)
            .filter_map(|v| Semver::parse(&v.version).ok().map(|s| (s, v)))
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, v)| v)
    }

    /// Resuelve una versión concreta o `latest`/`*` a la más alta no retirada.
    pub fn resolver_version(&self, nombre: &str, req: &str) -> Option<&VersionPublicada> {
        let entrada = self.paquetes.get(nombre)?;
        if req == "latest" || req == "*" || req.is_empty() {
            return self.ultima_version(nombre);
        }
        entrada.versiones.get(req).filter(|v| !v.yanked)
    }
}

/// Firma un hash SHA-256 con una clave (esquema HMAC-like simplificado).
///
/// No es criptografía de grado producción (no hay gestión de claves ni
/// verificación de identidad), pero detecta manipulación del tarball y
/// demuestra el flujo de firma/verificación end-to-end. La firma real con
/// clave pública/privada queda como deuda documentada.
pub fn firmar_hash(hash_hex: &str, clave: &str) -> String {
    let mut datos = Vec::new();
    datos.extend_from_slice(clave.as_bytes());
    datos.extend_from_slice(b"::forja-pkg::");
    datos.extend_from_slice(hash_hex.as_bytes());
    let digest = Sha256::digest(&datos);
    b64_encode(&digest)
}

/// Firma `hash_hex` con Ed25519 usando la clave privada del autor (32 bytes).
/// Devuelve la firma en hex (64 bytes).
pub fn firmar_hash_ed25519(hash_hex: &str, privada: &[u8; 32]) -> String {
    let firma = crate::ed25519::firmar(privada, hash_hex.as_bytes());
    let mut s = String::with_capacity(128);
    for b in firma.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Verifica una firma Ed25519 (hex) sobre `hash_hex` con la clave pública (hex).
pub fn verificar_firma_ed25519(hash_hex: &str, firma_hex: &str, publica_hex: &str) -> bool {
    let (Some(firma), Some(publica)) = (
        crate::crypto::hex_to_bytes(firma_hex),
        crate::crypto::hex_to_bytes(publica_hex),
    ) else {
        return false;
    };
    if firma.len() != 64 || publica.len() != 32 {
        return false;
    }
    let mut f = [0u8; 64];
    f.copy_from_slice(&firma);
    let mut p = [0u8; 32];
    p.copy_from_slice(&publica);
    crate::ed25519::verificar(&p, hash_hex.as_bytes(), &f)
}

/// Clave **del autor** (Ed25519) para firmar paquetes.
///
/// Se guarda en `~/.forja/keys/author.key` (hex de la semilla de 32 bytes) y se
/// crea la primera vez. `FORJA_AUTHOR_KEY` permite usar otra (hex).
pub fn clave_autor() -> Result<crate::ed25519::ParClaves, String> {
    if let Ok(hex) = std::env::var("FORJA_AUTHOR_KEY") {
        let bytes = crate::crypto::hex_to_bytes(hex.trim())
            .ok_or_else(|| "FORJA_AUTHOR_KEY no es hex válido".to_string())?;
        if bytes.len() != 32 {
            return Err("FORJA_AUTHOR_KEY debe ser una semilla de 32 bytes".to_string());
        }
        let mut semilla = [0u8; 32];
        semilla.copy_from_slice(&bytes);
        return Ok(crate::ed25519::par_desde_semilla(&semilla));
    }

    let ruta = dir_registry()
        .parent()
        .map(|p| p.join("keys"))
        .unwrap_or_else(|| dir_registry())
        .join("author.key");
    if let Ok(texto) = fs::read_to_string(&ruta) {
        let bytes = crate::crypto::hex_to_bytes(texto.trim())
            .ok_or_else(|| format!("clave inválida en {}", ruta.display()))?;
        if bytes.len() != 32 {
            return Err(format!("clave inválida en {}", ruta.display()));
        }
        let mut semilla = [0u8; 32];
        semilla.copy_from_slice(&bytes);
        return Ok(crate::ed25519::par_desde_semilla(&semilla));
    }

    let semilla = crate::ed25519::semilla_aleatoria()
        .map_err(|e| format!("no se pudo generar la clave: {}", e))?;
    if let Some(parent) = ruta.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("no se pudo crear {}: {}", parent.display(), e))?;
    }
    let mut hex = String::with_capacity(64);
    for b in semilla.iter() {
        hex.push_str(&format!("{:02x}", b));
    }
    fs::write(&ruta, hex.as_bytes())
        .map_err(|e| format!("no se pudo guardar la clave en {}: {}", ruta.display(), e))?;
    Ok(crate::ed25519::par_desde_semilla(&semilla))
}

/// Verifica integridad **y** firma de un tarball contra lo que dice el índice.
///
/// Con `clave_publica` presente la firma es asimétrica (Ed25519): un paquete
/// alterado o firmado por otra clave no valida. Sin ella se acepta el formato
/// viejo (clave simétrica local) por compatibilidad.
pub fn verificar_tarball(datos: &[u8], version: &VersionPublicada) -> Result<(), String> {
    let hash = sha256_hex(datos);
    if hash != version.sha256 {
        return Err(format!(
            "Integridad comprometida: hash del tarball no coincide para {}@{}",
            version.version, version.version
        ));
    }
    let firma_ok = if version.clave_publica.is_empty() {
        verificar_firma(&hash, &version.firma, &clave_firma())
    } else {
        verificar_firma_ed25519(&hash, &version.firma, &version.clave_publica)
    };
    if !firma_ok {
        return Err(format!(
            "Firma inválida para {} (posible manipulación o autor no verificado)",
            version.version
        ));
    }
    Ok(())
}

/// Verifica que `firma` corresponda a `hash_hex` con `clave`.
pub fn verificar_firma(hash_hex: &str, firma: &str, clave: &str) -> bool {
    firmar_hash(hash_hex, clave) == firma
}

/// Calcula el SHA-256 (hex) de un bloque de bytes.
pub fn sha256_hex(datos: &[u8]) -> String {
    let digest = Sha256::digest(datos);
    let mut s = String::with_capacity(64);
    for b in digest.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Clave de firma del autor. Se lee de `FORJA_SIGNING_KEY` o, si no existe,
/// se usa una clave por defecto (documentada como insegura para producción).
pub fn clave_firma() -> String {
    std::env::var("FORJA_SIGNING_KEY").unwrap_or_else(|_| "forja-clave-local-dev".to_string())
}

/// Directorio raíz del registry local (`~/.forja/registry`).
pub fn dir_registry() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".forja").join("registry")
}

/// Ruta del index del registry local.
pub fn ruta_index() -> PathBuf {
    dir_registry().join("index.json")
}

/// Ruta del tarball de un paquete/versión dentro del registry.
pub fn ruta_tarball(nombre: &str, version: &str) -> PathBuf {
    dir_registry()
        .join("paquetes")
        .join(nombre)
        .join(format!("{}-{}.fapkg", nombre, version))
}

/// Empaqueta un proyecto en un tarball determinista.
///
/// Formato `.fapkg` (texto, determinista):
///   línea 1: `FORJAPKG1`
///   línea 2: `nombre=<nombre>`
///   línea 3: `version=<version>`
///   luego, por cada archivo `.fa` (ordenado): `=== <ruta relativa> ===\n<contenido>`
///
/// Se usa un formato de texto plano (no comprimido) para mantener el núcleo
/// sin dependencias externas y garantizar determinismo byte a byte.
pub fn empaquetar(project_dir: &Path, nombre: &str, version: &str) -> Result<Vec<u8>, String> {
    let mut archivos: Vec<PathBuf> = Vec::new();
    recolectar_fa(project_dir, project_dir, &mut archivos)?;
    archivos.sort();

    if archivos.is_empty() {
        return Err(format!(
            "No se encontraron archivos .fa en {}",
            project_dir.display()
        ));
    }

    let mut out = String::new();
    out.push_str("FORJAPKG1\n");
    out.push_str(&format!("nombre={}\n", nombre));
    out.push_str(&format!("version={}\n", version));
    for archivo in &archivos {
        let rel = archivo
            .strip_prefix(project_dir)
            .unwrap_or(archivo)
            .to_string_lossy()
            .replace('\\', "/");
        let contenido = fs::read_to_string(archivo)
            .map_err(|e| format!("Error leyendo {}: {}", archivo.display(), e))?;
        out.push_str(&format!("=== {} ===\n", rel));
        out.push_str(&contenido);
        if !contenido.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok(out.into_bytes())
}

/// Recolecta recursivamente archivos `.fa` (ignora `target/`, `.git/`, `.forja/`).
fn recolectar_fa(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("Error leyendo {}: {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Error de entrada: {}", e))?;
        let path = entry.path();
        let nombre = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if nombre == "target" || nombre == ".git" || nombre == ".forja" || nombre == "modulos" {
                continue;
            }
            recolectar_fa(base, &path, out)?;
        } else if path.extension().map(|e| e == "fa").unwrap_or(false) {
            out.push(path);
        }
    }
    Ok(())
}

/// Desempaqueta un tarball `.fapkg` en `destino`.
pub fn desempaquetar(datos: &[u8], destino: &Path) -> Result<(), String> {
    let texto =
        std::str::from_utf8(datos).map_err(|_| "El paquete no es UTF-8 válido".to_string())?;
    let mut lineas = texto.lines();
    let cabecera = lineas.next().unwrap_or("");
    if cabecera != "FORJAPKG1" {
        return Err("Cabecera de paquete inválida (se esperaba FORJAPKG1)".to_string());
    }
    fs::create_dir_all(destino).map_err(|e| format!("Error creando destino: {}", e))?;

    let resto: Vec<&str> = lineas.collect();
    let mut i = 0;
    // Saltar metadatos nombre=/version=
    while i < resto.len() && !resto[i].starts_with("=== ") {
        i += 1;
    }
    while i < resto.len() {
        let cab = resto[i];
        if !cab.starts_with("=== ") || !cab.ends_with(" ===") {
            i += 1;
            continue;
        }
        let rel = &cab[4..cab.len() - 4];
        i += 1;
        let mut contenido = String::new();
        while i < resto.len() && !resto[i].starts_with("=== ") {
            contenido.push_str(resto[i]);
            contenido.push('\n');
            i += 1;
        }
        let destino_archivo = destino.join(rel);
        if let Some(parent) = destino_archivo.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Error creando {}: {}", parent.display(), e))?;
        }
        fs::write(&destino_archivo, contenido)
            .map_err(|e| format!("Error escribiendo {}: {}", destino_archivo.display(), e))?;
    }
    Ok(())
}

/// Publica un paquete: empaqueta, firma, escribe el tarball y actualiza el index.
///
/// Devuelve el hash SHA-256 (hex) del tarball publicado.
pub fn publicar(
    project_dir: &Path,
    nombre: &str,
    version: &str,
    autor: &str,
    descripcion: &str,
) -> Result<String, String> {
    if !Semver::es_valida(version) {
        return Err(format!(
            "Versión '{}' no es semver válido (usa mayor.menor.parche)",
            version
        ));
    }

    let index_path = ruta_index();
    let mut index = Index::cargar(&index_path)?;

    // No permitir republicar una versión ya existente.
    if let Some(entrada) = index.paquetes.get(nombre) {
        if entrada.versiones.contains_key(version) {
            return Err(format!(
                "La versión '{}' de '{}' ya está publicada (usa 'forja yank' para retirarla)",
                version, nombre
            ));
        }
    }

    let datos = empaquetar(project_dir, nombre, version)?;
    let hash = sha256_hex(&datos);
    // F12-T4: firma **asimétrica** con la clave del autor (el servidor sólo
    // necesita la pública para verificar).
    let claves = clave_autor()?;
    let firma = firmar_hash_ed25519(&hash, &claves.privada);
    let clave_publica = claves
        .publica
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    let tarball = ruta_tarball(nombre, version);
    if let Some(parent) = tarball.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Error creando directorio del paquete: {}", e))?;
    }
    fs::write(&tarball, &datos)
        .map_err(|e| format!("Error escribiendo tarball {}: {}", tarball.display(), e))?;

    let entrada = index
        .paquetes
        .entry(nombre.to_string())
        .or_insert_with(|| EntradaPaquete {
            nombre: nombre.to_string(),
            versiones: BTreeMap::new(),
        });
    entrada.versiones.insert(
        version.to_string(),
        VersionPublicada {
            version: version.to_string(),
            sha256: hash.clone(),
            firma,
            clave_publica,
            autor: autor.to_string(),
            descripcion: descripcion.to_string(),
            yanked: false,
        },
    );

    index.guardar(&index_path)?;
    Ok(hash)
}

/// Retira (yank) una versión del index. No borra el tarball.
pub fn yank(nombre: &str, version: &str) -> Result<(), String> {
    let index_path = ruta_index();
    let mut index = Index::cargar(&index_path)?;
    let entrada = index
        .paquetes
        .get_mut(nombre)
        .ok_or_else(|| format!("Paquete '{}' no encontrado en el registry", nombre))?;
    let v = entrada
        .versiones
        .get_mut(version)
        .ok_or_else(|| format!("Versión '{}' de '{}' no encontrada", version, nombre))?;
    if v.yanked {
        return Err(format!(
            "La versión '{}' de '{}' ya estaba retirada",
            version, nombre
        ));
    }
    v.yanked = true;
    index.guardar(&index_path)?;
    Ok(())
}

/// Instala un paquete desde el registry local en `destino`.
///
/// Verifica la firma antes de desempaquetar. Devuelve la versión instalada.
pub fn instalar_desde_registry(nombre: &str, req: &str, destino: &Path) -> Result<String, String> {
    let index = Index::cargar(&ruta_index())?;
    let version = index
        .resolver_version(nombre, req)
        .ok_or_else(|| {
            format!(
                "Paquete '{}' (req '{}') no encontrado en el registry local {}",
                nombre,
                req,
                ruta_index().display()
            )
        })?
        .clone();

    let tarball = ruta_tarball(nombre, &version.version);
    let datos = fs::read(&tarball)
        .map_err(|e| format!("Error leyendo tarball {}: {}", tarball.display(), e))?;

    verificar_tarball(&datos, &version)?;

    desempaquetar(&datos, destino)?;
    Ok(version.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parsea_y_valida() {
        assert!(Semver::es_valida("1.2.3"));
        assert!(Semver::es_valida("0.1.0"));
        assert!(Semver::es_valida("1.0.0-alpha.1"));
        assert!(Semver::es_valida("2"));
        assert!(!Semver::es_valida(""));
        assert!(!Semver::es_valida("a.b.c"));
        assert!(!Semver::es_valida("1.2.3.4"));
    }

    #[test]
    fn semver_ordena_correctamente() {
        let a = Semver::parse("1.2.3").unwrap();
        let b = Semver::parse("1.2.4").unwrap();
        let c = Semver::parse("1.3.0").unwrap();
        let d = Semver::parse("2.0.0").unwrap();
        assert!(a < b);
        assert!(b < c);
        assert!(c < d);
    }

    #[test]
    fn semver_estable_mayor_que_prerelease() {
        let estable = Semver::parse("1.0.0").unwrap();
        let pre = Semver::parse("1.0.0-alpha").unwrap();
        assert!(estable > pre);
    }

    #[test]
    fn firma_verifica_correctamente() {
        let hash = sha256_hex(b"contenido de prueba");
        let firma = firmar_hash(&hash, "clave-secreta");
        assert!(verificar_firma(&hash, &firma, "clave-secreta"));
        assert!(!verificar_firma(&hash, &firma, "otra-clave"));
        assert!(!verificar_firma("otro-hash", &firma, "clave-secreta"));
    }

    #[test]
    fn sha256_hex_es_estable() {
        // Vector conocido: SHA-256("abc")
        let h = sha256_hex(b"abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn empaquetar_y_desempaquetar_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("forja_pkg_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("main.fa"), "escribir(\"hola\")\n").unwrap();
        fs::write(tmp.join("lib.fa"), "funcion f() { retornar 1 }\n").unwrap();

        let datos = empaquetar(&tmp, "demo", "1.0.0").unwrap();
        let texto = String::from_utf8(datos.clone()).unwrap();
        assert!(texto.starts_with("FORJAPKG1\n"));
        assert!(texto.contains("nombre=demo"));
        assert!(texto.contains("=== main.fa ==="));
        assert!(texto.contains("=== lib.fa ==="));

        let destino = tmp.join("extraido");
        desempaquetar(&datos, &destino).unwrap();
        let main = fs::read_to_string(destino.join("main.fa")).unwrap();
        assert!(main.contains("hola"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn index_resuelve_ultima_version_no_yanked() {
        let mut index = Index::nuevo();
        let mut entrada = EntradaPaquete {
            nombre: "p".to_string(),
            versiones: BTreeMap::new(),
        };
        entrada.versiones.insert(
            "1.0.0".to_string(),
            VersionPublicada {
                version: "1.0.0".to_string(),
                sha256: "x".to_string(),
                firma: "y".to_string(),
                clave_publica: String::new(),
                autor: String::new(),
                descripcion: String::new(),
                yanked: false,
            },
        );
        entrada.versiones.insert(
            "1.1.0".to_string(),
            VersionPublicada {
                version: "1.1.0".to_string(),
                sha256: "x".to_string(),
                firma: "y".to_string(),
                clave_publica: String::new(),
                autor: String::new(),
                descripcion: String::new(),
                yanked: true,
            },
        );
        index.paquetes.insert("p".to_string(), entrada);

        // 1.1.0 está yanked, debe resolver 1.0.0.
        let v = index.ultima_version("p").unwrap();
        assert_eq!(v.version, "1.0.0");
        assert_eq!(
            index.resolver_version("p", "latest").unwrap().version,
            "1.0.0"
        );
        assert_eq!(
            index.resolver_version("p", "1.0.0").unwrap().version,
            "1.0.0"
        );
        assert!(index.resolver_version("p", "1.1.0").is_none());
    }
}
