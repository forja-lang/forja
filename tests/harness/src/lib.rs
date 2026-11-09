//! Helpers compartidos por los tests de integración de Forja.

use std::path::{Path, PathBuf};

pub use forja::bytecode::Opcode;

/// Compila un fuente completo y devuelve su bytecode.
pub fn compilar(fuente: &str) -> Result<Vec<Opcode>, String> {
    forja::compilar_pipeline_completa(fuente)
        .map(|(bc, _)| bc)
        .map_err(|e| e.to_string())
}

/// Compila y ejecuta un fuente en la VM canónica.
pub fn ejecutar(fuente: &str) -> Result<Vec<String>, String> {
    let (bc, contratos) = forja::compilar_pipeline_completa(fuente).map_err(|e| e.to_string())?;
    let mut vm = forja::vm_fast::ForjaFast::new();
    vm.contratos = contratos;
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bc);
    vm.ejecutar().map_err(|e| e.to_string())?;
    Ok(vm.obtener_output().to_vec())
}

/// Exige que la compilación falle con un fragmento del diagnóstico.
pub fn esperar_error(fuente: &str, fragmento: &str) {
    let err = compilar(fuente).expect_err("el fuente debía fallar");
    assert!(
        err.contains(fragmento),
        "se esperaba `{fragmento}` en el diagnóstico:\n{err}"
    );
}

/// Exige una salida exacta (una línea por elemento).
pub fn esperar_salida(fuente: &str, esperado: &[&str]) {
    let salida = ejecutar(fuente).expect("el fuente debía ejecutarse");
    assert_eq!(salida, esperado);
}

/// Quita secuencias ANSI y normaliza fin de línea.
pub fn limpiar_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut caracteres = s.chars().peekable();
    while let Some(c) = caracteres.next() {
        if c == '\x1b' && caracteres.peek() == Some(&'[') {
            caracteres.next();
            for c in caracteres.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out.replace("\r\n", "\n")
}

/// Crea un directorio temporal limpio para un test.
pub fn proyecto_temporal(nombre: &str) -> PathBuf {
    let directorio =
        std::env::temp_dir().join(format!("forja_test_{}_{}", nombre, std::process::id()));
    let _ = std::fs::remove_dir_all(&directorio);
    std::fs::create_dir_all(&directorio).expect("crear directorio temporal");
    directorio
}

/// Escribe un archivo dentro de un proyecto temporal.
pub fn escribir(directorio: &Path, nombre: &str, contenido: &str) -> PathBuf {
    let ruta = directorio.join(nombre);
    std::fs::write(&ruta, contenido).expect("escribir archivo");
    ruta
}

#[macro_export]
macro_rules! caso {
    ($nombre:ident, $fuente:expr, $esperado:expr) => {
        #[test]
        fn $nombre() {
            $crate::esperar_salida($fuente, &$esperado);
        }
    };
}

#[macro_export]
macro_rules! falla {
    ($nombre:ident, $fuente:expr, $fragmento:expr) => {
        #[test]
        fn $nombre() {
            $crate::esperar_error($fuente, $fragmento);
        }
    };
}

#[macro_export]
macro_rules! parsea {
    ($nombre:ident, $fuente:expr) => {
        #[test]
        fn $nombre() {
            $crate::compilar($fuente).expect("el fuente debía compilar");
        }
    };
}

/// Raíz del repositorio Forja vista desde el crate de tests.
pub fn raiz_forja() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("raíz del repo")
}

/// Todos los `.fa` de `ejemplos/`, recursivo y ordenado.
pub fn ejemplos_fa() -> Vec<PathBuf> {
    fn recorrer(directorio: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entradas) = std::fs::read_dir(directorio) else {
            return;
        };
        for e in entradas.flatten() {
            let p = e.path();
            if p.is_dir() {
                recorrer(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("fa") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    recorrer(&raiz_forja().join("ejemplos"), &mut out);
    out.sort();
    out
}

/// Busca un ejemplo por nombre de archivo, en cualquier subcarpeta.
pub fn ruta_ejemplo(nombre: &str) -> PathBuf {
    ejemplos_fa()
        .into_iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(nombre))
        .unwrap_or_else(|| panic!("no existe el ejemplo `{nombre}`"))
}

/// Compila y ejecuta sin verificar contratos (variante rápida).
pub fn ejecutar_vm(fuente: &str) -> Result<Vec<String>, String> {
    let bc = compilar(fuente)?;
    let mut vm = forja::vm_fast::ForjaFast::new();
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bc);
    vm.ejecutar().map_err(|e| e.to_string())?;
    Ok(vm.obtener_output().to_vec())
}

/// Compila y ejecuta resolviendo imports contra `raiz`.
pub fn ejecutar_desde(fuente: &str, raiz: &Path) -> Result<Vec<String>, String> {
    forja::ejecutar_desde(fuente, raiz)
}
