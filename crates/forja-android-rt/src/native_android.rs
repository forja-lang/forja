// Forja Android RT — NativeRegistry específico para Android
//
// Reemplaza funciones nativas de la stdlib de Forja con implementaciones
// compatibles con Android (ContentResolver, AssetManager, etc.)
//
// Las funciones marcadas como "stub" devuelven Error("no soportado en Android")
// hasta que se implementen con JNI.

use forja::vm_fast::{ForjaFast, ValorFast, ErrFast};
use forja::native_registry::NativeRegistry;
use forja::symbol_table::SymId;

/// Registra las funciones nativas Android en un NativeRegistry.
/// Reemplaza ciertas funciones del registro por defecto con versiones
/// compatibles con Android.
pub fn registrar_nativas_android(registry: &mut NativeRegistry) {
    // Archivos (stub hasta implementar ContentResolver)
    registry.registrar("_archivo_leer", stub_no_soportado);
    registry.registrar("_archivo_escribir", stub_no_soportado);
    registry.registrar("_archivo_existe", stub_no_soportado);
    registry.registrar("_archivo_eliminar", stub_no_soportado);
    registry.registrar("_archivo_copiar", stub_no_soportado);
    registry.registrar("_archivo_mover", stub_no_soportado);
    registry.registrar("_archivo_tamano", stub_no_soportado);
    registry.registrar("_archivo_info", stub_no_soportado);
    registry.registrar("_directorio_crear", stub_no_soportado);
    registry.registrar("_directorio_eliminar", stub_no_soportado);
    registry.registrar("_directorio_listar", stub_no_soportado);

    // Sistema (algunas funciones funcionan, otras no)
    registry.registrar("_sistema_comando", stub_no_soportado);
    registry.registrar("_sistema_ejecutar", stub_no_soportado);

    // Web (puede funcionar con permisos de Internet)
    // Las funciones de sockets ya deberían funcionar en Android
    // si se declara <uses-permission android:name="android.permission.INTERNET"/>
}

/// Stub para funciones nativas no soportadas en Android.
fn stub_no_soportado(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    // Crear un valor de error similar a cómo Forja maneja errores
    Err(ErrFast::TipoInv("Función no soportada en Android".to_string()))
}

/// Crea un NativeRegistry con todas las funciones Android.
pub fn crear_registry_android() -> NativeRegistry {
    let mut registry = NativeRegistry::new();
    registrar_nativas_android(&mut registry);
    registry
}
