// Forja Android RT — Error Handling
// Mapea errores del compilador/VM de Forja a excepciones Java via JNI.

use jni::JNIEnv;
use jni::errors::Error as JniError;

/// Clases de excepción Java en el package com.forja.
mod exc_class {
    pub const FORJA_ERROR: &str = "com/forja/ForjaException";
    pub const COMPILE_ERROR: &str = "com/forja/ForjaCompileError";
    pub const RUNTIME_ERROR: &str = "com/forja/ForjaRuntimeError";
    pub const CONTRACT_ERROR: &str = "com/forja/ForjaContractError";
    pub const TIMEOUT_ERROR: &str = "com/forja/ForjaTimeoutError";
    pub const INTERNAL_ERROR: &str = "com/forja/ForjaInternalError";
}

/// Tipos de error agrupados para mapeo a excepción Java.
#[derive(Debug)]
pub enum ForjaAndroidError {
    /// Error en tiempo de compilación (sintaxis, tipos, semántica).
    Compile {
        mensaje: String,
        linea: usize,
        columna: usize,
        sugerencia: String,
    },
    /// Error en tiempo de ejecución de la VM.
    Runtime {
        mensaje: String,
        codigo: RuntimeErrorCode,
    },
    /// Error de contrato (pre/post condición).
    Contract {
        mensaje: String,
        linea: usize,
    },
    /// Límite de ejecución excedido.
    Timeout {
        mensaje: String,
        instrucciones: usize,
    },
    /// Error interno de Rust (panic, bug) o JNI.
    Internal {
        mensaje: String,
    },
    /// Error JNI (problema de comunicación con Java).
    Jni(String),
}

/// Códigos específicos de error runtime.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeErrorCode {
    StackUnderflow,
    VariableNoDeclarada,
    TipoIncompatible,
    DivisionPorCero,
    FuncionNoDefinida,
    IndiceFueraRango,
    LimiteInstrucciones,
    ErrorPropagado,
    Desconocido,
}

impl std::fmt::Display for ForjaAndroidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForjaAndroidError::Compile { mensaje, linea, columna, sugerencia } => {
                write!(f, "Error de compilación en línea {}:{}: {}. Sugerencia: {}", linea, columna, mensaje, sugerencia)
            }
            ForjaAndroidError::Runtime { mensaje, codigo } => {
                write!(f, "[{:?}] {}", codigo, mensaje)
            }
            ForjaAndroidError::Contract { mensaje, linea } => {
                write!(f, "Error de contrato en línea {}: {}", linea, mensaje)
            }
            ForjaAndroidError::Timeout { mensaje, instrucciones } => {
                write!(f, "Timeout: {} ({} instrucciones)", mensaje, instrucciones)
            }
            ForjaAndroidError::Internal { mensaje } => {
                write!(f, "Error interno: {}", mensaje)
            }
            ForjaAndroidError::Jni(msg) => {
                write!(f, "Error JNI: {}", msg)
            }
        }
    }
}

impl std::error::Error for ForjaAndroidError {}

// ─── Conversión desde JNI Error ───────────────────────────────

impl From<JniError> for ForjaAndroidError {
    fn from(err: JniError) -> Self {
        ForjaAndroidError::Jni(err.to_string())
    }
}

// ─── Conversión desde errores de Forja ─────────────────────────────

impl From<Vec<forja::error::ErrorForja>> for ForjaAndroidError {
    fn from(errors: Vec<forja::error::ErrorForja>) -> Self {
        if errors.is_empty() {
            return ForjaAndroidError::Internal {
                mensaje: "Error de compilación vacío".to_string(),
            };
        }
        // Tomar el primer error (el más relevante)
        let err = &errors[0];
        ForjaAndroidError::Compile {
            mensaje: err.mensaje.clone(),
            linea: err.linea,
            columna: err.columna,
            sugerencia: err.sugerencia.clone(),
        }
    }
}

impl From<String> for ForjaAndroidError {
    fn from(msg: String) -> Self {
        // Intentar clasificar el mensaje de error de la pipeline
        let lower = msg.to_lowercase();
        if lower.contains("div/0") || lower.contains("división por cero") {
            ForjaAndroidError::Runtime {
                mensaje: msg,
                codigo: RuntimeErrorCode::DivisionPorCero,
            }
        } else if lower.contains("límite") || lower.contains("max_inst") {
            ForjaAndroidError::Timeout {
                mensaje: msg,
                instrucciones: 0,
            }
        } else if lower.contains("tipo") || lower.contains("incompatible") {
            ForjaAndroidError::Runtime {
                mensaje: msg,
                codigo: RuntimeErrorCode::TipoIncompatible,
            }
        } else if lower.contains("no declarada") || lower.contains("no definida") {
            ForjaAndroidError::Runtime {
                mensaje: msg,
                codigo: RuntimeErrorCode::VariableNoDeclarada,
            }
        } else if lower.contains("stack") || lower.contains("pila") {
            ForjaAndroidError::Runtime {
                mensaje: msg,
                codigo: RuntimeErrorCode::StackUnderflow,
            }
        } else if lower.contains("índice") || lower.contains("indice") || lower.contains("idx") {
            ForjaAndroidError::Runtime {
                mensaje: msg,
                codigo: RuntimeErrorCode::IndiceFueraRango,
            }
        } else if lower.contains("propagado") || lower.contains("?") {
            ForjaAndroidError::Runtime {
                mensaje: msg,
                codigo: RuntimeErrorCode::ErrorPropagado,
            }
        } else if lower.contains("función") || lower.contains("funcion") || lower.contains("fn ") {
            ForjaAndroidError::Runtime {
                mensaje: msg,
                codigo: RuntimeErrorCode::FuncionNoDefinida,
            }
        } else {
            ForjaAndroidError::Runtime {
                mensaje: msg,
                codigo: RuntimeErrorCode::Desconocido,
            }
        }
    }
}

impl From<forja::vm_fast::ErrFast> for ForjaAndroidError {
    fn from(err: forja::vm_fast::ErrFast) -> Self {
        match err {
            forja::vm_fast::ErrFast::DivCero => ForjaAndroidError::Runtime {
                mensaje: "División por cero".to_string(),
                codigo: RuntimeErrorCode::DivisionPorCero,
            },
            forja::vm_fast::ErrFast::Limite => ForjaAndroidError::Timeout {
                mensaje: "Límite de instrucciones excedido".to_string(),
                instrucciones: 0,
            },
            forja::vm_fast::ErrFast::StackUnder(m) => ForjaAndroidError::Runtime {
                mensaje: format!("Stack underflow: {}", m),
                codigo: RuntimeErrorCode::StackUnderflow,
            },
            forja::vm_fast::ErrFast::VarNoDecl(v) => ForjaAndroidError::Runtime {
                mensaje: format!("Variable '{}' no declarada", v),
                codigo: RuntimeErrorCode::VariableNoDeclarada,
            },
            forja::vm_fast::ErrFast::TipoInv(m) => ForjaAndroidError::Runtime {
                mensaje: format!("Tipo incompatible: {}", m),
                codigo: RuntimeErrorCode::TipoIncompatible,
            },
            forja::vm_fast::ErrFast::FnNoDef(f) => ForjaAndroidError::Runtime {
                mensaje: format!("Función '{}' no definida", f),
                codigo: RuntimeErrorCode::FuncionNoDefinida,
            },
            forja::vm_fast::ErrFast::IdxOut(m) => ForjaAndroidError::Runtime {
                mensaje: format!("Índice fuera de rango: {}", m),
                codigo: RuntimeErrorCode::IndiceFueraRango,
            },
            forja::vm_fast::ErrFast::ErrorPropagado(_) => ForjaAndroidError::Runtime {
                mensaje: "Error propagado con el operador ?".to_string(),
                codigo: RuntimeErrorCode::ErrorPropagado,
            },
        }
    }
}

// ─── Lanzar excepción Java desde JNI ─────────────────────────────

/// Lanza una excepción Java correspondiente al error Forja.
pub fn lanzar_excepcion(env: &mut JNIEnv, error: ForjaAndroidError) -> Result<(), JniError> {
    let mensaje = error.to_string();
    let class = match &error {
        ForjaAndroidError::Compile { .. } => exc_class::COMPILE_ERROR,
        ForjaAndroidError::Runtime { .. } => exc_class::RUNTIME_ERROR,
        ForjaAndroidError::Contract { .. } => exc_class::CONTRACT_ERROR,
        ForjaAndroidError::Timeout { .. } => exc_class::TIMEOUT_ERROR,
        ForjaAndroidError::Internal { .. } => exc_class::INTERNAL_ERROR,
        ForjaAndroidError::Jni(_) => exc_class::INTERNAL_ERROR,
    };

    env.throw_new(class, mensaje)?;
    Ok(())
}

/// Convierte un panic catch_unwind a una excepción Java.
pub fn panic_a_excepcion(env: &mut JNIEnv, panic: Box<dyn std::any::Any + Send>) {
    let msg = if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown Forja internal error (panic)".to_string()
    };
    let _ = env.throw_new(exc_class::INTERNAL_ERROR, msg);
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_compile_display() {
        let err = ForjaAndroidError::Compile {
            mensaje: "Expected ';'".to_string(),
            linea: 5,
            columna: 10,
            sugerencia: "Agregá un punto y coma".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("5:10"));
        assert!(s.contains("Expected ';'"));
    }

    #[test]
    fn test_error_runtime_display() {
        let err = ForjaAndroidError::Runtime {
            mensaje: "División por cero".to_string(),
            codigo: RuntimeErrorCode::DivisionPorCero,
        };
        let s = err.to_string();
        assert!(s.contains("DivisionPorCero"));
    }

    #[test]
    fn test_from_jni_error() {
        // JniError::JavaException changed in jni 0.21; use a generic wrapper
        let jni_err = JniError::MethodNotFound {
            name: "test".to_string(),
            sig: "()V".to_string(),
        };
        let err = ForjaAndroidError::from(jni_err);
        match err {
            ForjaAndroidError::Jni(_) => {} // ok
            _ => panic!("Expected Jni variant"),
        }
    }

    #[test]
    fn test_from_string_division() {
        let err = ForjaAndroidError::from("Div/0 en línea 42".to_string());
        match err {
            ForjaAndroidError::Runtime { codigo, .. } => {
                assert_eq!(codigo, RuntimeErrorCode::DivisionPorCero);
            }
            _ => panic!("Expected Runtime error"),
        }
    }

    #[test]
    fn test_from_string_timeout() {
        let err = ForjaAndroidError::from("Límite de ejecución alcanzado".to_string());
        match err {
            ForjaAndroidError::Timeout { .. } => {} // ok
            _ => panic!("Expected Timeout error"),
        }
    }

    #[test]
    fn test_from_errfast_divzero() {
        let err = ForjaAndroidError::from(forja::vm_fast::ErrFast::DivCero);
        match err {
            ForjaAndroidError::Runtime { codigo, .. } => {
                assert_eq!(codigo, RuntimeErrorCode::DivisionPorCero);
            }
            _ => panic!("Expected Runtime error"),
        }
    }

    #[test]
    fn test_from_errfast_limit() {
        let err = ForjaAndroidError::from(forja::vm_fast::ErrFast::Limite);
        match err {
            ForjaAndroidError::Timeout { .. } => {} // ok
            _ => panic!("Expected Timeout error"),
        }
    }
}
