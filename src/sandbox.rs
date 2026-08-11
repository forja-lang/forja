// Forja — Sandbox (red, archivos, procesos)
// Control de acceso a recursos del sistema.
//
// Por defecto, TODO está permitido. Los sandboxes solo se usan para RESTRINGIR.
//
// Uso (solo para restringir):
//   --bloquear-red google.com        → bloquear ese host
//   --bloquear-archivos /etc         → bloquear ese directorio
//   --bloquear-procesos rm           → bloquear ese comando
/// Control de acceso a red para programas Forja.
///
/// # Todo permitido por defecto
/// `SandboxRed::new()` permite todos los hosts y puertos.
/// Use `SandboxRed::restringir(...)` para restringir.
///
/// # Hosts permitidos
/// `hosts_permitidos = Some(vec![])` → hosts permitidos (vacío = ninguno).
/// Si contiene `"*"`, todos los hosts están permitidos.
///
/// # Puertos permitidos
/// `puertos_permitidos = None` → sin restricción de puertos.
/// `puertos_permitidos = Some(vec![])` → ningún puerto permitido.
/// `puertos_permitidos = Some(vec![80, 443])` → solo esos puertos.

#[derive(Debug, Clone)]
pub struct SandboxRed {
    /// None = modo air-gapped (sin red).
    /// Some(lista) = hosts permitidos. "*" significa todos.
    pub hosts_permitidos: Option<Vec<String>>,
    /// None = sin restricción de puertos.
    /// Some(lista) = puertos específicos permitidos.
    pub puertos_permitidos: Option<Vec<u16>>,
}

impl SandboxRed {
    /// Crea un sandbox que permite todos los hosts y puertos (comportamiento por defecto).
    pub fn new() -> Self {
        SandboxRed::todo_permitido()
    }

    /// Crea un sandbox que permite todos los hosts y puertos (comportamiento legacy).
    pub fn todo_permitido() -> Self {
        SandboxRed {
            hosts_permitidos: Some(vec!["*".to_string()]),
            puertos_permitidos: None,
        }
    }

    /// Verifica si una conexión al `host:puerto` está permitida.
    ///
    /// # Errores
    /// - Si el sandbox está en modo air-gapped (`hosts_permitidos = None`).
    /// - Si el host no está en la lista de hosts permitidos.
    /// - Si el puerto no está en la lista de puertos permitidos.
    pub fn verificar_conexion(&self, host: &str, puerto: u16) -> Result<(), String> {
        // Verificar hosts
        if let Some(hosts) = &self.hosts_permitidos {
            // Si la lista contiene "*", todos los hosts están permitidos
            if hosts.iter().any(|h| h == "*") {
                // Host permitido, verificar puerto
            } else if !hosts.iter().any(|h| h == host) {
                let hosts_str = hosts.join(", ");
                return Err(format!(
                    "Host no permitido: '{}'. Hosts permitidos: [{}]. Usa --bloquear-red para restringir.",
                    host, hosts_str
                ));
            }
        } else {
            return Err("Red habilitada (sin restricciones).".into());
        }

        // Verificar puertos
        if let Some(puertos) = &self.puertos_permitidos {
            if puertos.is_empty() {
                return Err(format!(
                    "Puerto no permitido: {}. No hay puertos habilitados. Usa --allow-port para permitir puertos.",
                    puerto
                ));
            }
            if !puertos.contains(&puerto) {
                let puertos_str: Vec<String> = puertos.iter().map(|p| p.to_string()).collect();
                return Err(format!(
                    "Puerto no permitido: {}. Puertos permitidos: [{}].",
                    puerto,
                    puertos_str.join(", ")
                ));
            }
        }

        Ok(())
    }
}

impl Default for SandboxRed {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_permitido_por_defecto() {
        let s = SandboxRed::new();
        // Por defecto todo está permitido
        assert!(s.verificar_conexion("localhost", 80).is_ok());
        assert!(s.verificar_conexion("127.0.0.1", 8080).is_ok());
        assert!(s.verificar_conexion("google.com", 443).is_ok());
    }

    #[test]
    fn test_todo_permitido() {
        let s = SandboxRed::todo_permitido();
        assert!(s.verificar_conexion("google.com", 443).is_ok());
        assert!(s.verificar_conexion("localhost", 80).is_ok());
        assert!(s.verificar_conexion("127.0.0.1", 8080).is_ok());
    }

    #[test]
    fn test_hosts_especificos() {
        let s = SandboxRed {
            hosts_permitidos: Some(vec!["localhost".into(), "127.0.0.1".into()]),
            puertos_permitidos: None,
        };
        assert!(s.verificar_conexion("localhost", 80).is_ok());
        assert!(s.verificar_conexion("127.0.0.1", 8080).is_ok());
        assert!(s.verificar_conexion("google.com", 443).is_err());
        assert!(s.verificar_conexion("192.168.1.1", 80).is_err());
    }

    #[test]
    fn test_puertos_especificos() {
        let s = SandboxRed {
            hosts_permitidos: Some(vec!["*".into()]),
            puertos_permitidos: Some(vec![80, 443]),
        };
        assert!(s.verificar_conexion("google.com", 80).is_ok());
        assert!(s.verificar_conexion("google.com", 443).is_ok());
        assert!(s.verificar_conexion("google.com", 8080).is_err());
        assert!(s.verificar_conexion("localhost", 22).is_err());
    }

    #[test]
    fn test_host_y_puerto_combinados() {
        let s = SandboxRed {
            hosts_permitidos: Some(vec!["localhost".into()]),
            puertos_permitidos: Some(vec![3000]),
        };
        assert!(s.verificar_conexion("localhost", 3000).is_ok());
        assert!(s.verificar_conexion("localhost", 80).is_err()); // puerto no permitido
        assert!(s.verificar_conexion("google.com", 3000).is_err()); // host no permitido
    }

    #[test]
    fn test_puertos_vacio_con_hosts() {
        // hosts permitidos pero lista de puertos vacía → nada permitido
        let s = SandboxRed {
            hosts_permitidos: Some(vec!["*".into()]),
            puertos_permitidos: Some(vec![]),
        };
        assert!(s.verificar_conexion("localhost", 80).is_err());
        assert!(s.verificar_conexion("google.com", 443).is_err());
    }

    #[test]
    fn test_default_trait() {
        let s: SandboxRed = Default::default();
        // Default = todo permitido
        assert!(s.verificar_conexion("localhost", 80).is_ok());
    }

    #[test]
    fn test_conexion_con_host_permitido_y_sin_restriccion_puertos() {
        let s = SandboxRed {
            hosts_permitidos: Some(vec!["localhost".into()]),
            puertos_permitidos: None, // sin restricción de puertos
        };
        assert!(s.verificar_conexion("localhost", 80).is_ok());
        assert!(s.verificar_conexion("localhost", 9999).is_ok());
        assert!(s.verificar_conexion("other", 80).is_err());
    }

    #[test]
    fn test_wildcard_host_con_puertos_restringidos() {
        let s = SandboxRed {
            hosts_permitidos: Some(vec!["*".into()]),
            puertos_permitidos: Some(vec![80, 443, 8080]),
        };
        assert!(s.verificar_conexion("cualquier.host.com", 80).is_ok());
        assert!(s.verificar_conexion("cualquier.host.com", 443).is_ok());
    }
}

// ══════════════════════════════════════════════════════════════════════
// Sandbox de Filesystem
// ══════════════════════════════════════════════════════════════════════

/// Control de acceso a archivos para programas Forja.
///
/// # Todo permitido por defecto
/// `SandboxFilesystem::new()` permite acceso a todos los directorios.
/// Use `SandboxFilesystem::restringir(...)` para restringir.
///
/// # Directorios permitidos
/// `directorios_permitidos = Some(vec![])` → lista vacía = ninguno.
/// `directorios_permitidos = Some(vec!["*".into()])` → todos los directorios.
#[derive(Debug, Clone)]
pub struct SandboxFilesystem {
    /// None = sin acceso a archivos (modo restrictivo).
    /// Some(lista) = directorios permitidos. "*" = todos.
    pub directorios_permitidos: Option<Vec<String>>,
    /// Si es true, solo se permite lectura (no escritura).
    pub solo_lectura: bool,
}

impl SandboxFilesystem {
    /// Crea un sandbox que permite acceso a todos los directorios (comportamiento por defecto).
    pub fn new() -> Self {
        SandboxFilesystem::todo_permitido()
    }

    /// Crea un sandbox que permite acceso a todos los directorios.
    pub fn todo_permitido() -> Self {
        SandboxFilesystem {
            directorios_permitidos: Some(vec!["*".to_string()]),
            solo_lectura: false,
        }
    }

    /// Crea un sandbox de solo lectura que permite todos los directorios.
    pub fn solo_lectura() -> Self {
        SandboxFilesystem {
            directorios_permitidos: Some(vec!["*".to_string()]),
            solo_lectura: true,
        }
    }

    /// Verifica si una ruta de lectura está permitida.
    pub fn verificar_lectura(&self, ruta: &str) -> Result<(), String> {
        self.verificar_acceso(ruta, false)
    }

    /// Verifica si una ruta de escritura está permitida.
    pub fn verificar_escritura(&self, ruta: &str) -> Result<(), String> {
        self.verificar_acceso(ruta, true)
    }

    fn verificar_acceso(&self, ruta: &str, escritura: bool) -> Result<(), String> {
        if let Some(directorios) = &self.directorios_permitidos {
            if directorios.iter().any(|d| d == "*") {
                // Todos los directorios permitidos
                if escritura && self.solo_lectura {
                    return Err(
                        "Escritura denegada: sandbox en modo solo-lectura. Usa --allow-write para habilitar escritura.".into()
                    );
                }
                return Ok(());
            }

            // Normalizar la ruta: resolver .. y .
            let ruta_normalizada = Self::normalizar_ruta(ruta);

            for dir in directorios {
                let dir_normalizado = Self::normalizar_ruta(dir);
                if ruta_normalizada.starts_with(&dir_normalizado) {
                    if escritura && self.solo_lectura {
                        return Err(
                            "Escritura denegada: sandbox en modo solo-lectura. Usa --allow-write para habilitar escritura.".into()
                        );
                    }
                    return Ok(());
                }
            }

            let dirs_str = directorios.join(", ");
            if escritura {
                return Err(format!(
                    "Escritura denegada en '{}'. Directorios permitidos: [{}].",
                    ruta, dirs_str
                ));
            } else {
                return Err(format!(
                    "Lectura denegada en '{}'. Directorios permitidos: [{}].",
                    ruta, dirs_str
                ));
            }
        } else {
            return Err("Acceso a archivos habilitado (sin restricciones).".into());
        }
    }

    /// Normaliza una ruta: resuelve `.` y `..`
    fn normalizar_ruta(ruta: &str) -> String {
        let normalizada = ruta.replace("\\", "/");
        let partes: Vec<&str> = normalizada.split('/').collect();
        let mut resultado: Vec<&str> = Vec::new();
        for parte in partes {
            match parte {
                "" | "." => continue,
                ".." => {
                    resultado.pop();
                }
                p => resultado.push(p),
            }
        }
        if resultado.is_empty() {
            "/".to_string()
        } else {
            let mut r = resultado.join("/");
            // Preservar prefijo de Windows (C:/)
            if ruta.len() >= 2 && ruta.as_bytes()[1] == b':' {
                r = format!("{}:{}", ruta.as_bytes()[0] as char, r);
            }
            // Preservar /
            if ruta.starts_with('/') {
                r = format!("/{}", r);
            }
            r
        }
    }
}

impl Default for SandboxFilesystem {
    fn default() -> Self {
        Self::new()
    }
}

// ══════════════════════════════════════════════════════════════════════
// Sandbox de Procesos
// ══════════════════════════════════════════════════════════════════════

/// Control de ejecución de procesos para programas Forja.
///
/// # Todo permitido por defecto
/// `SandboxProceso::new()` permite ejecutar cualquier comando.
/// Use `SandboxProceso::restringir(...)` para restringir.
#[derive(Debug, Clone)]
pub struct SandboxProceso {
    /// None = sin ejecución de procesos (modo restrictivo).
    /// Some(lista) = comandos/binarios permitidos. "*" = todos.
    pub comandos_permitidos: Option<Vec<String>>,
    /// Límite máximo de procesos simultáneos.
    pub max_procesos: usize,
}

impl SandboxProceso {
    /// Crea un sandbox que permite ejecutar cualquier comando (comportamiento por defecto).
    pub fn new() -> Self {
        SandboxProceso::todo_permitido()
    }

    /// Crea un sandbox que permite ejecutar cualquier comando.
    pub fn todo_permitido() -> Self {
        SandboxProceso {
            comandos_permitidos: Some(vec!["*".to_string()]),
            max_procesos: 100,
        }
    }

    /// Verifica si un comando puede ser ejecutado.
    pub fn verificar_comando(&self, comando: &str) -> Result<(), String> {
        if let Some(comandos) = &self.comandos_permitidos {
            if comandos.iter().any(|c| c == "*") {
                return Ok(());
            }

            // Extraer el nombre del binario del comando
            let binario = comando.split_whitespace().next().unwrap_or("");
            // También verificar con el nombre sin ruta
            let nombre_corto = binario.rsplit('/').next().unwrap_or(binario);
            let nombre_corto = nombre_corto.rsplit('\\').next().unwrap_or(nombre_corto);

            if comandos.iter().any(|c| c == binario || c == nombre_corto) {
                return Ok(());
            }

            let cmds_str = comandos.join(", ");
            return Err(format!(
                "Comando no permitido: '{}'. Comandos permitidos: [{}].",
                nombre_corto, cmds_str
            ));
        } else {
            return Err("Ejecución de procesos habilitada (sin restricciones).".into());
        }
    }
}

impl Default for SandboxProceso {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests_sandbox_fs {
    use super::*;

    #[test]
    fn test_fs_todo_permitido_por_defecto() {
        let s = SandboxFilesystem::new();
        assert!(s.verificar_lectura("/tmp/test.txt").is_ok());
        assert!(s.verificar_escritura("/tmp/test.txt").is_ok());
    }

    #[test]
    fn test_fs_todo_permitido() {
        let s = SandboxFilesystem::todo_permitido();
        assert!(s.verificar_lectura("/tmp/test.txt").is_ok());
        assert!(s.verificar_escritura("/tmp/test.txt").is_ok());
    }

    #[test]
    fn test_fs_solo_lectura() {
        let s = SandboxFilesystem::solo_lectura();
        assert!(s.verificar_lectura("/tmp/test.txt").is_ok());
        assert!(s.verificar_escritura("/tmp/test.txt").is_err());
    }

    #[test]
    fn test_fs_directorios_especificos() {
        let s = SandboxFilesystem {
            directorios_permitidos: Some(vec!["/tmp".into(), "/home/user".into()]),
            solo_lectura: false,
        };
        assert!(s.verificar_lectura("/tmp/test.txt").is_ok());
        assert!(s.verificar_escritura("/home/user/doc.txt").is_ok());
        assert!(s.verificar_lectura("/etc/passwd").is_err());
        assert!(s.verificar_escritura("/root/secret").is_err());
    }

    #[test]
    fn test_fs_path_traversal() {
        let s = SandboxFilesystem {
            directorios_permitidos: Some(vec!["/tmp".into()]),
            solo_lectura: false,
        };
        // Path traversal con .. no debería escapar
        assert!(s.verificar_lectura("/tmp/../etc/passwd").is_err());
    }
}

#[cfg(test)]
mod tests_sandbox_proc {
    use super::*;

    #[test]
    fn test_proc_todo_permitido_por_defecto() {
        let s = SandboxProceso::new();
        assert!(s.verificar_comando("ls").is_ok());
        assert!(s.verificar_comando("cmd /c dir").is_ok());
    }

    #[test]
    fn test_proc_todo_permitido() {
        let s = SandboxProceso::todo_permitido();
        assert!(s.verificar_comando("ls -la").is_ok());
        assert!(s.verificar_comando("python script.py").is_ok());
    }

    #[test]
    fn test_proc_comandos_especificos() {
        let s = SandboxProceso {
            comandos_permitidos: Some(vec!["ls".into(), "cat".into()]),
            max_procesos: 10,
        };
        assert!(s.verificar_comando("ls -la").is_ok());
        assert!(s.verificar_comando("cat file.txt").is_ok());
        assert!(s.verificar_comando("rm -rf /").is_err());
        assert!(s.verificar_comando("python script.py").is_err());
    }

    #[test]
    fn test_proc_ruta_completa() {
        let s = SandboxProceso {
            comandos_permitidos: Some(vec!["/usr/bin/ls".into()]),
            max_procesos: 10,
        };
        assert!(s.verificar_comando("/usr/bin/ls -la").is_ok());
        assert!(s.verificar_comando("ls").is_err()); // sin ruta no coincide
    }
}
