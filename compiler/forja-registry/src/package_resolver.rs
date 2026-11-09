use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct PackageResolver {
    /// Directorio del proyecto actual
    directorio_proyecto: PathBuf,
    /// Directorio global de paquetes (~/.forja/paquetes)
    global_cache: PathBuf,
    /// Paquetes instalados: nombre -> version
    installed: HashMap<String, String>,
}

impl PackageResolver {
    pub fn new(directorio_proyecto: &Path) -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let global_cache = Path::new(&home).join(".forja").join("paquetes");

        PackageResolver {
            directorio_proyecto: directorio_proyecto.to_path_buf(),
            global_cache,
            installed: HashMap::new(),
        }
    }

    /// Resuelve la ruta de un módulo importado.
    ///
    /// Prioridad:
    ///   1. Stdlib embebida (compilada dentro del binario)
    ///   2. Ruta absoluta o relativa directa
    ///   3. Relativa al directorio del proyecto
    ///   4. En stdlib/ del proyecto y sus ancestros
    ///   5. En stdlib/ junto al ejecutable de Forja
    ///   6. Cache global de paquetes
    pub fn resolver_modulo(&self, ruta: &str) -> Option<PathBuf> {
        // 0. Stdlib embebida — SIEMPRE primero
        if crate::stdlib_embedded::existe(ruta) {
            return Some(PathBuf::from(format!("embedded://{}", ruta)));
        }

        let ruta_paquete = Path::new(ruta);

        // 1. Ruta absoluta o relativa directa
        if ruta_paquete.is_absolute() && ruta_paquete.exists() {
            return Some(ruta_paquete.to_path_buf());
        }

        // 2. Relativa al directorio del proyecto
        let ruta_local = self.directorio_proyecto.join(ruta_paquete);
        if ruta_local.with_extension("fa").is_file() {
            return Some(ruta_local.with_extension("fa"));
        }
        let entrada_local = ruta_local.join(format!("{}.fa", ruta_paquete.display()));
        if entrada_local.is_file() {
            return Some(entrada_local);
        }
        if ruta_local.is_file() {
            return Some(ruta_local);
        }

        // 3. En la biblioteca estándar del proyecto y sus ancestros.
        //    Se aceptan `library/` (estructura nueva) y `stdlib/` (proyectos
        //    viejos) para no romper código existente.
        let mut directorio_base = self.directorio_proyecto.clone();
        let mut found_stdlib = None;
        loop {
            if let Some(p) = buscar_en_stdlib(&directorio_base, ruta_paquete) {
                found_stdlib = Some(p);
                break;
            }
            if !directorio_base.pop() {
                break;
            }
        }

        // Fallback: Buscar stdlib relativo al ejecutable de Forja (usando FORJA_ORIGINAL_EXE si existe debido al shadow copy en Windows)
        if found_stdlib.is_none() {
            let ruta_exe = std::env::var("FORJA_ORIGINAL_EXE")
                .map(PathBuf::from)
                .or_else(|_| std::env::current_exe());
            if let Ok(exe_path) = ruta_exe {
                if let Some(exe_dir) = exe_path.parent() {
                    let mut directorio_base = exe_dir.to_path_buf();
                    loop {
                        if let Some(p) = buscar_en_stdlib(&directorio_base, ruta_paquete) {
                            found_stdlib = Some(p);
                            break;
                        }
                        if !directorio_base.pop() {
                            break;
                        }
                    }
                }
            }
        }

        if let Some(p) = found_stdlib {
            return Some(p);
        }

        // 4. En paquetes globales
        let pkg = self.global_cache.join(ruta_paquete);
        if pkg.with_extension("fa").is_file() {
            return Some(pkg.with_extension("fa"));
        }
        let pkg_entrypoint = pkg.join(format!("{}.fa", ruta_paquete.display()));
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
        let directorio_paquete = self.global_cache.join(nombre).join(version);
        if directorio_paquete.exists() {
            self.installed
                .insert(nombre.to_string(), version.to_string());
            return Ok(());
        }

        fs::create_dir_all(&directorio_paquete)
            .map_err(|e| format!("Error creando directorio: {}", e))?;

        // Buscar paquete builtin en stdlib (ej: stdlib/gui/gui.fa)
        let mut directorio_base = self.directorio_proyecto.clone();
        let mut builtin_src = None;
        loop {
            let candidate = ["library", "stdlib"].iter().find_map(|dir| {
                let p = directorio_base
                    .join(dir)
                    .join(nombre)
                    .join(format!("{}.fa", nombre));
                p.is_file().then_some(p)
            });
            if let Some(candidate) = candidate {
                builtin_src = Some(candidate);
                break;
            }
            if !directorio_base.pop() {
                break;
            }
        }

        // Fallback para builtin: Buscar stdlib relativo al ejecutable de Forja (considerando shadow copy en Windows)
        if builtin_src.is_none() {
            let ruta_exe = std::env::var("FORJA_ORIGINAL_EXE")
                .map(PathBuf::from)
                .or_else(|_| std::env::current_exe());
            if let Ok(exe_path) = ruta_exe {
                if let Some(exe_dir) = exe_path.parent() {
                    let mut directorio_base = exe_dir.to_path_buf();
                    loop {
                        let candidate = ["library", "stdlib"].iter().find_map(|dir| {
                            let p = directorio_base
                                .join(dir)
                                .join(nombre)
                                .join(format!("{}.fa", nombre));
                            p.is_file().then_some(p)
                        });
                        if let Some(candidate) = candidate {
                            builtin_src = Some(candidate);
                            break;
                        }
                        if !directorio_base.pop() {
                            break;
                        }
                    }
                }
            }
        }

        if let Some(src) = builtin_src {
            let dest = directorio_paquete.join(format!("{}.fa", nombre));
            fs::copy(&src, &dest)
                .map_err(|e| format!("Error copiando paquete builtin '{}': {}", nombre, e))?;
            self.installed
                .insert(nombre.to_string(), version.to_string());
            return Ok(());
        }

        // No es builtin: intentar consumir del registry ruta_local.
        // El registry ruta_local vive en ~/.forja/registry y es el mismo formato
        // que usaría el servidor remoto registry.forja-lang.dev (deuda futura).
        match crate::registry::instalar_desde_registry(nombre, version, &directorio_paquete) {
            Ok(version_instalada) => {
                self.installed.insert(nombre.to_string(), version_instalada);
                Ok(())
            }
            Err(e_registry) => {
                // si hay registry remoto configurado, intentar allí antes
                // de fallar. El paquete se verifica por SHA-256 y se descomprime
                // en el directorio de paquetes del proyecto.
                #[cfg(not(target_arch = "wasm32"))]
                if crate::registry_remote::configurado() {
                    match crate::registry_remote::instalar_desde_remoto(
                        nombre,
                        version,
                        &directorio_paquete,
                    ) {
                        Ok(version_instalada) => {
                            self.installed.insert(nombre.to_string(), version_instalada);
                            return Ok(());
                        }
                        Err(e_remoto) => {
                            return Err(format!(
                                "Paquete '{}' versión '{}' no encontrado. Registry ruta_local: {}. \
                                 Registry remoto ({}): {}",
                                nombre,
                                version,
                                e_registry,
                                crate::registry_remote::url_base().unwrap_or_default(),
                                e_remoto
                            ));
                        }
                    }
                }
                // Ni builtin ni en el registry ruta_local: falla explícito en vez de
                // fingir que el paquete quedó instalado.
                Err(format!(
                    "Paquete '{}' versión '{}' no encontrado. No es builtin y no \
                     está en el registry ruta_local ({}). Detalle: {}. Publicá el \
                     paquete con 'forja publish' o instalalo manualmente en {}",
                    nombre,
                    version,
                    crate::registry::ruta_index().display(),
                    e_registry,
                    directorio_paquete.display()
                ))
            }
        }
    }
}

fn buscar_en_stdlib(base: &Path, ruta_paquete: &Path) -> Option<PathBuf> {
    for dir in ["library", "stdlib"] {
        let stdlib = base.join(dir).join(ruta_paquete);
        if stdlib.with_extension("fa").is_file() {
            return Some(stdlib.with_extension("fa"));
        }
        let entrada_directorio = stdlib.join(format!("{}.fa", ruta_paquete.display()));
        if entrada_directorio.is_file() {
            return Some(entrada_directorio);
        }
        if stdlib.is_file() {
            return Some(stdlib);
        }
    }
    None
}
