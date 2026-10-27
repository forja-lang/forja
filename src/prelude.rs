/// Prelude de Forja: tipos y funciones disponibles sin importar.
///
/// El prelude inyecta en todo programa Forja los tipos algebraicos básicos
/// que el compilador ya reconoce a nivel de sintaxis (constructores `Ok`,
/// `Error`, `Algo`, `Ninguno`, etc.) y a nivel semántico (`Resultado<T, E>`,
/// `Opcion<T>`). Registrar sus variantes aquí permite que el comprobador de
/// exhaustividad de `coincidir` valide patrones sobre estos tipos sin que el
/// usuario tenga que redeclararlos.
///
/// Estas declaraciones se emiten como [`Declaracion::Enum`], que no genera
/// bytecode alguno y es eliminada por el DCE cuando no se usa: los programas
/// que no manejan `Resultado`/`Opcion` conservan exactamente su bytecode.
use crate::ast::*;

/// Genera las declaraciones del prelude (tipos algebraicos básicos).
///
/// Devuelve:
/// - `Resultado<T, E>` = `Ok(T) | Error(E)`
/// - `Opcion<T>` = `Alguno(T) | Ninguno`
pub fn generar_prelude() -> Vec<Declaracion> {
    vec![
        Declaracion::Enum {
            nombre: "Resultado".to_string(),
            variantes: vec![
                Variante {
                    nombre: "Ok".to_string(),
                    tipos: vec![Tipo::Parametro("T".to_string())],
                },
                Variante {
                    nombre: "Error".to_string(),
                    tipos: vec![Tipo::Parametro("E".to_string())],
                },
            ],
            atributos: vec![],
        },
        Declaracion::Enum {
            nombre: "Opcion".to_string(),
            variantes: vec![
                Variante {
                    nombre: "Alguno".to_string(),
                    tipos: vec![Tipo::Parametro("T".to_string())],
                },
                Variante {
                    nombre: "Ninguno".to_string(),
                    tipos: vec![],
                },
            ],
            atributos: vec![],
        },
    ]
}
