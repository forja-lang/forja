// Forja Android RT — Punto de entrada JNI
#![allow(unused_imports)]

mod error;
mod jni_bridge;
mod native_android;

use std::sync::Mutex;
use std::time::Instant;

use jni::JNIEnv;
use jni::objects::{JClass, JString, JObject, GlobalRef};
use jni::sys::{jlong, jstring, jbyteArray, jobject};
use jni::JavaVM;

use forja::vm_fast::{ForjaFast, ValorFast};
use forja::bytecode;

use crate::error::{ForjaAndroidError, lanzar_excepcion, panic_a_excepcion};
use crate::jni_bridge::{valor_a_java, resultado_a_java};

const ANDROID_RT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_MAX_INST: usize = 10_000_000;

/// JavaVM guardado en JNI_OnLoad para poder attach hilos y llamar callbacks.
static JAVA_VM: std::sync::OnceLock<JavaVM> = std::sync::OnceLock::new();

struct ForjaSessionInner {
    vm: ForjaFast,
    _created_at: Instant,
    output_callback: Option<GlobalRef>,
    input_callback: Option<GlobalRef>,
    _bytecode_cache: std::collections::HashMap<u64, Vec<bytecode::Opcode>>,
}

impl ForjaSessionInner {
    /// Invoca el callback de output (Consumer<String>) si está registrado.
    fn call_output_callback(&self, line: &str) {
        let Some(cb) = &self.output_callback else { return };
        let Some(jvm) = JAVA_VM.get() else { return };
        if let Ok(mut env) = jvm.attach_current_thread() {
            if let Ok(jstr) = env.new_string(line) {
                let _ = env.call_method(
                    cb.as_obj(),
                    "accept",
                    "(Ljava/lang/Object;)V",
                    &[jni::objects::JValue::Object(&jstr.into())],
                );
            }
        }
    }

    /// Invoca el callback de input (Supplier<String>) si está registrado.
    fn call_input_callback(&self) -> Option<String> {
        let Some(cb) = &self.input_callback else { return None };
        let Some(jvm) = JAVA_VM.get() else { return None };
        if let Ok(mut env) = jvm.attach_current_thread() {
            let result = env.call_method(
                cb.as_obj(),
                "get",
                "()Ljava/lang/Object;",
                &[],
            );
            if let Ok(val) = result {
                if let Ok(jstr) = val.l() {
                    if let Ok(s) = env.get_string(&JString::from(jstr)) {
                        return Some(s.into());
                    }
                }
            }
        }
        None
    }
}

static SESSIONS: std::sync::OnceLock<Mutex<Vec<Option<Mutex<ForjaSessionInner>>>>> =
    std::sync::OnceLock::new();

fn sessions() -> &'static Mutex<Vec<Option<Mutex<ForjaSessionInner>>>> {
    SESSIONS.get_or_init(|| Mutex::new(Vec::with_capacity(64)))
}

fn crear_session_inner(max_inst: usize) -> Result<jlong, ForjaAndroidError> {
    let mut vm = ForjaFast::new();
    vm.set_max_inst(max_inst);
    // Reemplazar native_registry con la versión Android
    vm.native_registry = native_android::crear_registry_android();

    let session = ForjaSessionInner {
        vm,
        _created_at: Instant::now(),
        output_callback: None,
        input_callback: None,
        _bytecode_cache: std::collections::HashMap::new(),
    };

    let mut guard = sessions().lock().map_err(|e| ForjaAndroidError::Internal {
        mensaje: format!("Error lockeando sessions: {}", e),
    })?;

    for (i, slot) in guard.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(Mutex::new(session));
            return Ok(i as jlong);
        }
    }
    let idx = guard.len();
    guard.push(Some(Mutex::new(session)));
    Ok(idx as jlong)
}

/// Ejecuta un closure con acceso a la sesión identificada por handle.
/// Maneja el lockeo del sessions global y del Mutex individual.
fn con_sesion<R>(
    handle: jlong,
    f: impl FnOnce(&mut ForjaSessionInner) -> Result<R, ForjaAndroidError>,
) -> Result<R, ForjaAndroidError> {
    let idx = handle as usize;
    let guard = sessions().lock().map_err(|e| ForjaAndroidError::Internal {
        mensaje: format!("Error lockeando sessions: {}", e),
    })?;
    let session_mutex = guard
        .get(idx)
        .and_then(|s| s.as_ref())
        .ok_or_else(|| ForjaAndroidError::Internal {
            mensaje: format!("Handle de sesión inválido: {}", handle),
        })?;
    let mut session = session_mutex.lock().map_err(|e| ForjaAndroidError::Internal {
        mensaje: format!("Error lockeando sesión: {}", e),
    })?;
    f(&mut *session)
}

fn destruir_session_inner(handle: jlong) -> Result<(), ForjaAndroidError> {
    let idx = handle as usize;
    let mut guard = sessions().lock().map_err(|e| ForjaAndroidError::Internal {
        mensaje: format!("Error lockeando sessions: {}", e),
    })?;
    if idx < guard.len() {
        guard[idx] = None;
        Ok(())
    } else {
        Err(ForjaAndroidError::Internal {
            mensaje: format!("Handle de sesión inválido: {}", handle),
        })
    }
}

// ─── Macros JNI ───────────────────────────────────────────────────

macro_rules! jni_panic_boundary {
    ($env:expr, $body:expr) => {{
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body));
        match result {
            Ok(val) => val,
            Err(panic) => {
                panic_a_excepcion(&mut $env, panic);
                Default::default()
            }
        }
    }};
}

// ─── Funciones JNI ────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeVersion<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    jni_panic_boundary!(env, {
        let v = format!("forja-android-rt v{}", ANDROID_RT_VERSION);
        env.new_string(&v).ok().map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
    })
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeCrearSession<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    max_inst: jlong,
) -> jlong {
    jni_panic_boundary!(env, {
        let max = if max_inst <= 0 { DEFAULT_MAX_INST } else { max_inst as usize };
        match crear_session_inner(max) {
            Ok(h) => h,
            Err(e) => { let _ = lanzar_excepcion(&mut env, e); -1 }
        }
    })
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeDestruirSession<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_ptr: jlong,
) {
    if let Err(e) = destruir_session_inner(session_ptr) {
        let _ = lanzar_excepcion(&mut env, e);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeResetSession<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_ptr: jlong,
) {
    let result = con_sesion(session_ptr, |session| {
        session.vm = ForjaFast::new();
        session.vm.set_max_inst(DEFAULT_MAX_INST);
        Ok(())
    });
    if let Err(e) = result {
        let _ = lanzar_excepcion(&mut env, e);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeEjecutar<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_ptr: jlong,
    source: JString<'local>,
    _ruta_base: JString<'local>,
) -> jobject {
    jni_panic_boundary!(env, {
        let result = con_sesion(session_ptr, |session| {
            let source_str: String = env.get_string(&source)?.into();
            let inicio = Instant::now();

            let bc = forja::compilar_pipeline(&source_str)
                .map_err(|e| ForjaAndroidError::from(e))?;
            let antes = session.vm.ejecutadas;

            session.vm.cargar_bytecode(bc);
            session.vm.ejecutar().map_err(|e| ForjaAndroidError::from(e))?;

            let duracion = inicio.elapsed().as_nanos() as u64;
            let ejec = session.vm.ejecutadas - antes;
            let output = session.vm.obtener_output().to_vec();
            session.vm.output.clear();

            resultado_a_java(&mut env, output, ejec as usize, duracion)
                .map_err(|e| ForjaAndroidError::Jni(e.to_string()))
        });

        match result {
            Ok(obj) => obj.into_raw(),
            Err(e) => { let _ = lanzar_excepcion(&mut env, e); std::ptr::null_mut() }
        }
    })
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeCompilarABytecode<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _session_ptr: jlong,
    source: JString<'local>,
) -> jbyteArray {
    jni_panic_boundary!(env, {
        let result = (|| -> Result<jbyteArray, ForjaAndroidError> {
            let src: String = env.get_string(&source)?.into();
            let bc = forja::compilar_pipeline(&src)
                .map_err(|e| ForjaAndroidError::from(e))?;
            let data = bytecode::serializar_bytecode(&bc);
            let arr = env.byte_array_from_slice(&data)?;
            Ok(arr.as_raw())
        })();
        match result {
            Ok(arr) => arr,
            Err(e) => { let _ = lanzar_excepcion(&mut env, e); std::ptr::null_mut() }
        }
    })
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeEjecutarBytecode<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_ptr: jlong,
    bytecode_arr: jbyteArray,
) -> jobject {
    jni_panic_boundary!(env, {
        let result = con_sesion(session_ptr, |session| {
            let bc_arr = unsafe { jni::objects::JPrimitiveArray::from_raw(bytecode_arr) };
            let bytes = env.convert_byte_array(&bc_arr)?;
            let opcodes = bytecode::deserializar_bytecode(&bytes)
                .ok_or_else(|| ForjaAndroidError::Internal {
                    mensaje: "Bytecode inválido o CRC incorrecto".to_string(),
                })?;

            let antes = session.vm.ejecutadas;
            let inicio = Instant::now();
            session.vm.cargar_bytecode(opcodes);
            session.vm.ejecutar().map_err(|e| ForjaAndroidError::from(e))?;

            let duracion = inicio.elapsed().as_nanos() as u64;
            let ejec = session.vm.ejecutadas - antes;
            let output = session.vm.obtener_output().to_vec();
            session.vm.output.clear();

            // Invocar callback de output si está registrado
            if session.output_callback.is_some() {
                for line in &output {
                    session.call_output_callback(line);
                }
            }

            resultado_a_java(&mut env, output, ejec as usize, duracion)
                .map_err(|e| ForjaAndroidError::Jni(e.to_string()))
        });

        match result {
            Ok(obj) => obj.into_raw(),
            Err(e) => { let _ = lanzar_excepcion(&mut env, e); std::ptr::null_mut() }
        }
    })
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeEvaluar<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_ptr: jlong,
    expresion: JString<'local>,
) -> jobject {
    jni_panic_boundary!(env, {
        let result = con_sesion(session_ptr, |session| {
            let expr: String = env.get_string(&expresion)?.into();
            let wrapped = format!("funcion __expr__() {{ retornar {} }} __expr__()", expr);

            let bc = forja::compilar_pipeline(&wrapped)
                .map_err(|e| ForjaAndroidError::from(e))?;
            session.vm.cargar_bytecode(bc);
            session.vm.ejecutar().map_err(|e| ForjaAndroidError::from(e))?;

            let valor = session.vm.stack.last().copied().unwrap_or(ValorFast::nulo());
            let output = session.vm.obtener_output().to_vec();
            session.vm.output.clear();

            if output.is_empty() {
                valor_a_java(&mut env, &session.vm, valor)
                    .map_err(|e| ForjaAndroidError::Jni(e.to_string()))
            } else {
                env.new_string(&output.join("\n"))
                    .map(|s| s.into())
                    .map_err(|e| ForjaAndroidError::Jni(e.to_string()))
            }
        });

        match result {
            Ok(obj) => obj.into_raw(),
            Err(e) => { let _ = lanzar_excepcion(&mut env, e); std::ptr::null_mut() }
        }
    })
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeSetOutputCallback<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_ptr: jlong,
    callback: JObject<'local>,
) {
    let result = con_sesion(session_ptr, |session| {
        session.output_callback = if callback.is_null() {
            None
        } else {
            Some(env.new_global_ref(callback)
                .map_err(|e| ForjaAndroidError::Jni(e.to_string()))?)
        };
        Ok(())
    });
    if let Err(e) = result { let _ = lanzar_excepcion(&mut env, e); }
}

#[no_mangle]
pub extern "C" fn Java_com_forja_Runtime_nativeSetInputCallback<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_ptr: jlong,
    callback: JObject<'local>,
) {
    let result = con_sesion(session_ptr, |session| {
        session.input_callback = if callback.is_null() {
            None
        } else {
            Some(env.new_global_ref(callback)
                .map_err(|e| ForjaAndroidError::Jni(e.to_string()))?)
        };
        Ok(())
    });
    if let Err(e) = result { let _ = lanzar_excepcion(&mut env, e); }
}

// ─── JNI OnLoad / OnUnload ──────────────────────────────────────

#[no_mangle]
pub extern "C" fn JNI_OnLoad(
    vm: jni::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    let _ = JAVA_VM.set(vm);
    jni::sys::JNI_VERSION_1_6
}

#[no_mangle]
pub extern "C" fn JNI_OnUnload(
    _vm: jni::JavaVM,
    _reserved: *mut std::ffi::c_void,
) {
    if let Ok(mut guard) = sessions().lock() {
        guard.clear();
    }
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crear_y_destruir_sesion() {
        let handle = crear_session_inner(1000).unwrap();
        assert!(handle >= 0);
        destruir_session_inner(handle).unwrap();
    }

    #[test]
    fn test_sesion_invalida() {
        let result = destruir_session_inner(999);
        assert!(result.is_err());
    }
}
