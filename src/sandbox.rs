// Forja — Sandbox de red
// Control de acceso a operaciones de red (TCP/UDP)
//
// Por defecto (SandboxRed::new()), el sandbox está en modo "air-gapped":
// ninguna conexión de red está permitida.
//
// Uso:
//   --allow-net localhost,127.0.0.1  → solo esos hosts
//   --allow-net "*"                  → todos los hosts
//   --allow-port 80,443              → solo esos puertos
/// Control de acceso a red para programas Forja.
///
/// # Air-gapped por defecto
/// `hosts_permitidos = None` → no se permite ninguna conexión de red.
///
/// # Hosts permitidos
/// `hosts_permitidos = Some(vec![])` → hosts permitidos (vacío = ninguno).
/// Si contiene `"*"`, todos los hosts están permitidos.
///
/// # Puertos permitidos
/// `puertos_permitidos = None` → sin restricción de puertos (si hay red).
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
    /// Crea un sandbox con toda la red permitida (modo desarrollo).
    pub fn new() -> Self {
        SandboxRed {
            hosts_permitidos: Some(vec!["*".to_string()]), // Toda la red permitida
            puertos_permitidos: None,
        }
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
                    "Host no permitido: '{}'. Hosts permitidos: [{}]. Usa --allow-net para agregarlo.",
                    host, hosts_str
                ));
            }
        } else {
            return Err(
                "Red deshabilitada (air-gapped). Usa --allow-net para habilitar conexiones de red."
                    .into(),
            );
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
                    "Puerto no permitido: {}. Puertos permitidos: [{}]. Usa --allow-port para agregarlo.",
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
    fn test_air_gapped_por_defecto() {
        let s = SandboxRed::new();
        assert!(s.hosts_permitidos.is_none());
        // Cualquier conexión debe fallar
        assert!(s.verificar_conexion("localhost", 80).is_err());
        assert!(s.verificar_conexion("127.0.0.1", 8080).is_err());
        assert!(s.verificar_conexion("google.com", 443).is_err());
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
        assert!(s.hosts_permitidos.is_none());
        assert!(s.puertos_permitidos.is_none());
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
        assert!(s.verificar_conexion("cualquier.host.com", 8080).is_ok());
        assert!(s.verificar_conexion("cualquier.host.com", 9999).is_err());
    }
}
