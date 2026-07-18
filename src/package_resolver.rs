use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct PackageResolver {
    /// Directorio del proyecto actual
    project_dir: PathBuf,
    /// Directorio global de paquetes (~/.forja/paquetes)
    global_cache: PathBuf,
    /// Paquetes instalados: nombre -> version
    installed: HashMap<String, String>,
}

impl PackageResolver {
    pub fn new(project_dir: &Path) -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let global_cache = Path::new(&home).join(".forja").join("paquetes");

        PackageResolver {
            project_dir: project_dir.to_path_buf(),
            global_cache,
            installed: HashMap::new(),
        }
    }

    /// Resuelve la ruta de un módulo importado
    pub fn resolver_modulo(&self, ruta: &str) -> Option<PathBuf> {
        let path = Path::new(ruta);

        // 1. Ruta absoluta o relativa directa
        if path.is_absolute() {
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }

        // 2. Relativa al directorio del proyecto
        let local = self.project_dir.join(path);
        if local.with_extension("fa").is_file() {
            return Some(local.with_extension("fa"));
        }
        let local_entrypoint = local.join(format!("{}.fa", path.display()));
        if local_entrypoint.is_file() {
            return Some(local_entrypoint);
        }
        if local.is_file() {
            return Some(local);
        }

        // 3. En stdlib
        let mut base_dir = self.project_dir.clone();
        loop {
            let stdlib = base_dir.join("stdlib").join(path);
            if stdlib.with_extension("fa").is_file() {
                return Some(stdlib.with_extension("fa"));
            }
            let dir_entrypoint = stdlib.join(format!("{}.fa", path.display()));
            if dir_entrypoint.is_file() {
                return Some(dir_entrypoint);
            }
            if stdlib.is_file() {
                return Some(stdlib);
            }
            if !base_dir.pop() {
                break;
            }
        }

        // 4. En paquetes globales
        let pkg = self.global_cache.join(path);
        if pkg.with_extension("fa").is_file() {
            return Some(pkg.with_extension("fa"));
        }
        let pkg_entrypoint = pkg.join(format!("{}.fa", path.display()));
        if pkg_entrypoint.is_file() {
            return Some(pkg_entrypoint);
        }
        if pkg.is_file() {
            return Some(pkg);
        }

        None
    }

    /// Instala una dependencia desde el registro
    pub fn instalar_dependencia(&mut self, nombre: &str, version: &str) -> Result<(), String> {
        let pkg_dir = self.global_cache.join(nombre).join(version);
        if pkg_dir.exists() {
            self.installed
                .insert(nombre.to_string(), version.to_string());
            return Ok(());
        }

        fs::create_dir_all(&pkg_dir).map_err(|e| format!("Error creando directorio: {}", e))?;

        // Buscar paquete builtin en stdlib (ej: stdlib/gui/gui.fa)
        let mut base_dir = self.project_dir.clone();
        let mut builtin_src = None;
        loop {
            let candidate = base_dir
                .join("stdlib")
                .join(nombre)
                .join(format!("{}.fa", nombre));
            if candidate.exists() {
                builtin_src = Some(candidate);
                break;
            }
            if !base_dir.pop() {
                break;
            }
        }

        if let Some(src) = builtin_src {
            let dest = pkg_dir.join(format!("{}.fa", nombre));
            fs::copy(&src, &dest)
                .map_err(|e| format!("Error copiando paquete builtin '{}': {}", nombre, e))?;
            self.installed
                .insert(nombre.to_string(), version.to_string());
            return Ok(());
        }

        // Si no es builtin, simular descarga — en producción descargaría del registry
        self.installed
            .insert(nombre.to_string(), version.to_string());
        Ok(())
    }
}
