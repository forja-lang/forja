//! Logging de estado y callbacks del runtime GUI (F6-T2).

/// Retorna true si el modo depuración de GUI está activo.
pub fn is_debug_enabled() -> bool {
    true
}

/// Imprime un mensaje formateado con prefijo si el modo debug está activo.
pub fn debug_log(category: &str, message: &str) {
    if is_debug_enabled() {
        log::debug!("[Forja GUI::{}] {}", category, message);
        eprintln!("🔍 [Forja GUI::{}] {}", category, message);
    }
}

/// Registra el cambio de valor de una variable reactiva.
pub fn log_state_change(
    var_name: &str,
    old_val: Option<&serde_json::Value>,
    new_val: &serde_json::Value,
) {
    if is_debug_enabled() {
        match old_val {
            Some(old) => {
                log::info!(
                    "[GUI State] Variable '{}' cambió: {} ──→ {}",
                    var_name,
                    old,
                    new_val
                );
                eprintln!(
                    "⚡ [GUI State] Variable '{}' cambió: {} ──→ {}",
                    var_name, old, new_val
                );
            }
            None => {
                log::info!(
                    "[GUI State] Variable '{}' inicializada: {}",
                    var_name,
                    new_val
                );
                eprintln!(
                    "⚡ [GUI State] Variable '{}' inicializada: {}",
                    var_name, new_val
                );
            }
        }
    }
}

/// Registra el inicio de ejecución de un callback.
pub fn log_callback_start(nombre_fn: &str) {
    if is_debug_enabled() {
        log::info!("[GUI Callback] Ejecutando callback '{}'...", nombre_fn);
        eprintln!("▶️ [GUI Callback] Ejecutando callback '{}'...", nombre_fn);
    }
}

/// Registra el éxito de un callback.
pub fn log_callback_success(nombre_fn: &str, resultado: &str) {
    if is_debug_enabled() {
        log::info!(
            "[GUI Callback] Callback '{}' completado con éxito: {}",
            nombre_fn,
            resultado
        );
        eprintln!(
            "✅ [GUI Callback] Callback '{}' completado con éxito: {}",
            nombre_fn, resultado
        );
    }
}

/// Registra un error durante la ejecución de un callback o evaluador.
pub fn log_callback_error(nombre_fn: &str, error: &str) {
    log::error!("[GUI Error] Error en callback '{}': {}", nombre_fn, error);
    eprintln!(
        "❌ [GUI Error] Error en callback '{}': {}",
        nombre_fn, error
    );
}

/// Registra una advertencia de conversión de layout.
pub fn log_layout_warning(widget_name: &str, reason: &str) {
    log::warn!("[GUI Layout Warning] Widget '{}': {}", widget_name, reason);
    eprintln!(
        "⚠️ [GUI Layout Warning] Widget '{}': {}",
        widget_name, reason
    );
}
