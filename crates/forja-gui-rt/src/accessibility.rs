// Forja GUI Runtime — Sistema de Accesibilidad (TalkBack/VoiceOver)
//
// Proporciona:
// - Descripciones de widgets para screen readers
// - Roles ARIA/AccessKit para cada tipo de widget
// - Navegación por teclado (Tab/Enter/Escape)
// - Anuncios de accesibilidad (TalkBack-style)
// - Gestión de foco

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use xilem::masonry::accesskit::Role;

// ─── Rol de accesibilidad para cada widget Material ──────────────

/// Traduce un tipo de widget Material a un rol AccessKit
pub fn material_role(tipo: &str) -> Role {
    match tipo {
        "button" | "boton" | "filled_button" | "tonal_button" | "outlined_button"
        | "text_button" | "elevated_button" | "fab" | "icon_button" => Role::Button,
        "label" | "etiqueta" | "titulo" | "text" | "styled_label" | "colored_label"
        | "variable_label" | "title" => Role::Label,
        "text_input" | "campo_texto" | "input" | "textarea" | "campo_perfilado" => Role::TextInput,
        "checkbox" | "casilla" | "check" => Role::CheckBox,
        "slider" | "deslizante" => Role::Slider,
        "switch" | "interruptor" => Role::Switch,
        "progress_bar" | "barra_progreso" | "progress" => Role::ProgressIndicator,
        "spinner" | "cargando" => Role::ProgressIndicator,
        "image" | "imagen" | "avatar" => Role::Image,
        "navigation_bar" | "barra_navegacion" => Role::TabList,
        "navigation_rail" | "riel_navegacion" => Role::TabList,
        "tab" | "pestaña" | "tabs" => Role::Tab,
        "card" | "tarjeta" => Role::GenericContainer,
        "dialog" | "dialogo" | "dialog_alert" => Role::Dialog,
        "menu" | "menú" => Role::Menu,
        "list" | "lista" => Role::List,
        "list_item" | "elemento_lista" => Role::ListItem,
        "separator" | "divider" | "separador" => Role::Splitter,
        "tooltip" | "informacion" | "info" => Role::Tooltip,
        "header" | "encabezado" => Role::Heading,
        "link" | "enlace" => Role::Link,
        _ => Role::GenericContainer,
    }
}

// ─── Descripción accesible para widgets ─────────────────────────

/// Genera una descripción textual accesible para un widget
pub fn descripcion_accesible(tipo: &str, label: &str, valor: &str, estado: &str) -> String {
    let tipo_desc = match tipo {
        "button" | "boton" => "Botón",
        "filled_button" => "Botón relleno",
        "tonal_button" => "Botón tonal",
        "outlined_button" => "Botón perfilado",
        "text_button" => "Botón de texto",
        "elevated_button" => "Botón elevado",
        "fab" => "Botón de acción flotante",
        "icon_button" => "Botón de icono",
        "label" | "etiqueta" => "Etiqueta",
        "titulo" | "title" => "Título",
        "styled_label" => "Texto estilizado",
        "variable_label" => "Etiqueta dinámica",
        "text_input" | "input" | "campo_texto" => "Campo de texto",
        "textarea" | "area_texto" => "Área de texto",
        "campo_perfilado" => "Campo perfilado",
        "checkbox" | "casilla" | "check" => "Casilla de verificación",
        "slider" | "deslizante" => "Deslizante",
        "switch" | "interruptor" => "Interruptor",
        "progress_bar" | "progress" => "Barra de progreso",
        "spinner" => "Indicador de carga",
        "avatar" => "Avatar",
        "image" | "imagen" => "Imagen",
        "card" | "tarjeta" => "Tarjeta",
        "dialog" | "dialogo" => "Diálogo",
        "menu" => "Menú",
        "list" => "Lista",
        "list_item" => "Elemento de lista",
        "separator" => "Separador",
        "tooltip" => "Información adicional",
        "navigation_bar" => "Barra de navegación",
        "tab" => "Pestaña",
        _ => tipo,
    };

    let mut desc = format!("{}", tipo_desc);
    if !label.is_empty() {
        desc.push_str(&format!(": {}", label));
    }
    if !valor.is_empty() {
        desc.push_str(&format!(". Valor: {}", valor));
    }
    if !estado.is_empty() {
        desc.push_str(&format!(". {}", estado));
    }
    desc
}

/// Estados accesibles comunes
pub fn estado_checkbox(checked: bool) -> &'static str {
    if checked { "Seleccionado" } else { "No seleccionado" }
}

pub fn estado_switch(encendido: bool) -> &'static str {
    if encendido { "Activado" } else { "Desactivado" }
}

pub fn estado_seleccion(seleccionado: bool) -> &'static str {
    if seleccionado { "Seleccionado" } else { "No seleccionado" }
}

// ─── Anuncios de accesibilidad (TalkBack-style) ─────────────────

/// Nivel de prioridad del anuncio
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrioridadAnuncio {
    Baja,
    Normal,
    Alta,       // Interrupción
    Urgente,    // Inmediato
}

/// Un anuncio de accesibilidad (equivalente a un mensaje TalkBack)
#[derive(Debug, Clone)]
pub struct Anuncio {
    pub mensaje: String,
    pub prioridad: PrioridadAnuncio,
    pub timestamp: u64,
}

impl Anuncio {
    pub fn nuevo(mensaje: &str) -> Self {
        Anuncio {
            mensaje: mensaje.to_string(),
            prioridad: PrioridadAnuncio::Normal,
            timestamp: Self::now(),
        }
    }

    pub fn urgente(mensaje: &str) -> Self {
        Anuncio {
            mensaje: mensaje.to_string(),
            prioridad: PrioridadAnuncio::Urgente,
            timestamp: Self::now(),
        }
    }

    pub fn alta(mensaje: &str) -> Self {
        Anuncio {
            mensaje: mensaje.to_string(),
            prioridad: PrioridadAnuncio::Alta,
            timestamp: Self::now(),
        }
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

// ─── Gestor de Accesibilidad ────────────────────────────────────

/// Gestor central de accesibilidad para la GUI
///
/// Almacena el estado del screen reader (encendido/apagado),
/// la cola de anuncios, y el foco actual.
#[derive(Debug, Clone)]
pub struct AccessibilityManager {
    /// Indica si el screen reader está activo
    pub enabled: bool,
    /// Anuncios pendientes
    announcements: Arc<std::sync::RwLock<Vec<Anuncio>>>,
    /// ID del widget con foco
    #[allow(dead_code)]
    focused_id: Arc<AtomicU64>,
    /// Último widget anunciado (para evitar repeticiones)
    last_announced: Arc<std::sync::RwLock<String>>,
}

impl AccessibilityManager {
    pub fn new() -> Self {
        AccessibilityManager {
            enabled: true,
            announcements: Arc::new(std::sync::RwLock::new(Vec::new())),
            focused_id: Arc::new(AtomicU64::new(0)),
            last_announced: Arc::new(std::sync::RwLock::new(String::new())),
        }
    }

    /// Encola un anuncio para ser "hablado" por el screen reader
    pub fn announce(&self, mensaje: &str) {
        if !self.enabled {
            return;
        }
        // Verificar que no sea el mismo mensaje que el último
        if let Ok(mut last) = self.last_announced.write() {
            if *last == mensaje {
                return; // Evita repeticiones
            }
            *last = mensaje.to_string();
        }
        let anuncio = Anuncio::nuevo(mensaje);
        if let Ok(mut cola) = self.announcements.write() {
            cola.push(anuncio);
        }
        // También imprimir en consola para depuración
        println!("  ♿ [TalkBack] {}", mensaje);
    }

    /// Anuncia un cambio de foco
    pub fn announce_focus(&self, widget_desc: &str) {
        self.announce(&format!("Enfocado: {}", widget_desc));
    }

    /// Anuncia una acción (clic, activación)
    pub fn announce_action(&self, widget_desc: &str, accion: &str) {
        self.announce(&format!("{}: {}", widget_desc, accion));
    }

    /// Obtiene y limpia los anuncios pendientes
    pub fn drain_announcements(&self) -> Vec<Anuncio> {
        if let Ok(mut cola) = self.announcements.write() {
            cola.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Activa o desactiva el screen reader
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            println!("  ♿ [TalkBack] Activado");
        } else {
            println!("  ♿ [TalkBack] Desactivado");
        }
    }

    /// Alterna el estado del screen reader
    pub fn toggle(&mut self) {
        self.set_enabled(!self.enabled);
    }
}

impl Default for AccessibilityManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Widget Accesible (wrapper) ──────────────────────────────────

/// Datos de accesibilidad para un widget
#[derive(Debug, Clone)]
pub struct A11yData {
    /// Rol AccessKit
    pub role: Role,
    /// Nombre descriptivo del widget
    pub label: String,
    /// Valor actual (para inputs, sliders, etc.)
    pub value: String,
    /// Descripción adicional
    pub description: String,
    /// Si el widget está deshabilitado
    pub disabled: bool,
}

impl A11yData {
    pub fn new(role: Role, label: &str) -> Self {
        A11yData {
            role,
            label: label.to_string(),
            value: String::new(),
            description: String::new(),
            disabled: false,
        }
    }

    pub fn with_value(mut self, value: &str) -> Self {
        self.value = value.to_string();
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Genera un anuncio TalkBack completo
    pub fn to_announcement(&self) -> String {
        let mut msg = self.label.clone();
        if !self.value.is_empty() {
            msg.push_str(&format!(". {}", self.value));
        }
        if !self.description.is_empty() {
            msg.push_str(&format!(". {}", self.description));
        }
        msg
    }
}

// ─── Atajo de teclado para accesibilidad ────────────────────────

/// Atajos de teclado del screen reader
pub mod atajos {
    /// Activar/desactivar screen reader
    pub const TOGGLE_SCREEN_READER: &str = "Ctrl+Shift+A";
    /// Navegar al siguiente widget
    pub const NEXT_WIDGET: &str = "Tab";
    /// Navegar al widget anterior
    pub const PREV_WIDGET: &str = "Shift+Tab";
    /// Activar widget enfocado
    pub const ACTIVATE: &str = "Enter";
    /// Cerrar diálogo/menú
    pub const CLOSE: &str = "Escape";
    /// Leer descripción del widget enfocado
    pub const READ_FOCUSED: &str = "Ctrl+F";
    /// Leer título de la ventana
    pub const READ_TITLE: &str = "Ctrl+T";
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descripcion_accesible() {
        let d = descripcion_accesible("button", "Aceptar", "", "");
        assert!(d.contains("Botón"));
        assert!(d.contains("Aceptar"));
    }

    #[test]
    fn test_descripcion_con_valor() {
        let d = descripcion_accesible("slider", "Volumen", "75%", "");
        assert!(d.contains("Deslizante"));
        assert!(d.contains("Volumen"));
        assert!(d.contains("75%"));
    }

    #[test]
    fn test_anuncio() {
        let mgr = AccessibilityManager::new();
        mgr.announce("Hola mundo");
        let cola = mgr.drain_announcements();
        assert_eq!(cola.len(), 1);
        assert_eq!(cola[0].mensaje, "Hola mundo");
    }

    #[test]
    fn test_anuncio_no_repite() {
        let mgr = AccessibilityManager::new();
        mgr.announce("Prueba");
        mgr.announce("Prueba"); // No debería añadirse
        let cola = mgr.drain_announcements();
        assert_eq!(cola.len(), 1);
    }

    #[test]
    fn test_a11y_data() {
        let data = A11yData::new(Role::Button, "Enviar")
            .with_value("")
            .with_description("Envía el formulario");
        let msg = data.to_announcement();
        assert!(msg.contains("Enviar"));
        assert!(msg.contains("Envía el formulario"));
    }

    #[test]
    fn test_material_role() {
        assert_eq!(material_role("button"), Role::Button);
        assert_eq!(material_role("checkbox"), Role::CheckBox);
        assert_eq!(material_role("slider"), Role::Slider);
        assert_eq!(material_role("text_input"), Role::TextField);
        assert_eq!(material_role("no_existe"), Role::GenericContainer);
    }
}
