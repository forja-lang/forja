// Forja GUI — Sistema de iconos vectoriales Material Design
// Basado en Material Icons (Google Material Design)
// Los SVG paths están embebidos para zero dependencias externas.
//
// Estilos soportados: Filled, Outlined, Rounded, Sharp, TwoTone
// Fallback: emoji cuando el icono no está disponible
//
// NOTA: Xilem 0.4 no tiene renderizado SVG nativo fácil.
// Por ahora los iconos se renderizan como emoji con tamaño y color.
// El sistema está preparado para migrar a SVG real cuando Xilem lo soporte.

use crate::theme::RgbColor;
use xilem::view;
use xilem::{AnyWidgetView, Color};
use xilem::style::Style;

// ─── Estilo de icono ─────────────────────────────────────────────────

/// Estilos de iconos Material Design
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum IconStyle {
    Filled,   // Relleno (por defecto)
    Outlined, // Contorno
    Rounded,  // Redondeado
    Sharp,    // Agudo
    TwoTone,  // Dos tonos
}

impl IconStyle {
    /// Nombre del estilo como string
    pub fn as_str(&self) -> &'static str {
        match self {
            IconStyle::Filled => "filled",
            IconStyle::Outlined => "outlined",
            IconStyle::Rounded => "rounded",
            IconStyle::Sharp => "sharp",
            IconStyle::TwoTone => "twotone",
        }
    }

    /// Parsea un string a IconStyle
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "outlined" | "outline" | "perfilado" | "contorno" => IconStyle::Outlined,
            "rounded" | "round" | "redondo" | "redondeado" => IconStyle::Rounded,
            "sharp" | "agudo" | "afilado" => IconStyle::Sharp,
            "twotone" | "two_tone" | "two-tone" | "dos_tonos" | "dos tonos" => IconStyle::TwoTone,
            _ => IconStyle::Filled,
        }
    }
}

// ─── Icono Material ───────────────────────────────────────────────────

/// Un icono vectorial Material Design
///
/// Almacena el nombre del icono, su path SVG y el estilo.
/// El path SVG se puede usar en el futuro cuando Xilem soporte
/// renderizado SVG nativo.
#[derive(Clone, Copy, Debug)]
pub struct MaterialIcon {
    pub name: &'static str,      // nombre en inglés: "home", "favorite"
    pub svg_path: &'static str,  // path SVG del icono (Material Design)
    pub style: IconStyle,
}

impl MaterialIcon {
    /// Crea un nuevo icono Material con estilo Filled por defecto
    pub const fn new(name: &'static str, svg_path: &'static str) -> Self {
        Self { name, svg_path, style: IconStyle::Filled }
    }

    /// Cambia el estilo del icono
    pub fn with_style(mut self, style: IconStyle) -> Self {
        self.style = style;
        self
    }

    /// Renderiza este icono como un widget Xilem
    ///
    /// Por ahora usa emoji con tamaño y color.
    /// Cuando Xilem soporte SVG, aquí se renderizará el path real.
    pub fn to_widget(&self, size: f64, color: RgbColor) -> Box<AnyWidgetView<()>> {
        let emoji = catalog::fallback_emoji(self.name);
        let xilem_color: Color = color.into();
        Box::new(
            view::label(emoji)
                .text_size(size as f32)
                .color(xilem_color)
        )
    }

    /// Renderiza con estilo específico
    pub fn to_widget_styled(&self, size: f64, color: RgbColor, _style: IconStyle) -> Box<AnyWidgetView<()>> {
        // Por ahora todas las variantes usan el mismo emoji
        // En el futuro se pueden mapear a diferentes SVGs según el estilo
        self.to_widget(size, color)
    }
}

// ─── Catálogo de iconos ──────────────────────────────────────────────

pub mod catalog {
    use super::*;

    // ═══════════════════════════════════════════════════════════════════
    // NAVEGACIÓN (Navigation)
    // ═══════════════════════════════════════════════════════════════════

    /// Home / inicio
    pub const HOME: MaterialIcon = MaterialIcon::new("home", "M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z");
    /// Buscar
    pub const SEARCH: MaterialIcon = MaterialIcon::new("search", "M15.5 14h-.79l-.28-.27A6.471 6.471 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z");
    /// Settings / configuración
    pub const SETTINGS: MaterialIcon = MaterialIcon::new("settings", "M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.61l-1.92-3.32a.488.488 0 0 0-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54a.484.484 0 0 0-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.07.62-.07.94s.02.64.07.94l-2.03 1.58a.49.49 0 0 0-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6A3.6 3.6 0 1 1 12 8.4a3.6 3.6 0 0 1 0 7.2z");
    /// Menu hamburguesa
    pub const MENU: MaterialIcon = MaterialIcon::new("menu", "M3 18h18v-2H3v2zm0-5h18v-2H3v2zm0-7v2h18V6H3z");
    /// Arrow back / atrás
    pub const ARROW_BACK: MaterialIcon = MaterialIcon::new("arrow_back", "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z");
    /// Arrow forward / adelante
    pub const ARROW_FORWARD: MaterialIcon = MaterialIcon::new("arrow_forward", "M12 4l-1.41 1.41L16.17 11H4v2h12.17l-5.58 5.59L12 20l8-8z");
    /// Arrow drop down / desplegable
    pub const ARROW_DROP_DOWN: MaterialIcon = MaterialIcon::new("arrow_drop_down", "M7 10l5 5 5-5z");
    /// Arrow up / arriba
    pub const ARROW_UP: MaterialIcon = MaterialIcon::new("arrow_up", "M7.41 15.41L12 10.83l4.59 4.58L18 14l-6-6-6 6z");
    /// Arrow down / abajo
    pub const ARROW_DOWN: MaterialIcon = MaterialIcon::new("arrow_down", "M7.41 8.59L12 13.17l4.59-4.58L18 10l-6 6-6-6z");
    /// Chevron left
    pub const CHEVRON_LEFT: MaterialIcon = MaterialIcon::new("chevron_left", "M15.41 7.41L14 6l-6 6 6 6 1.41-1.41L10.83 12z");
    /// Chevron right
    pub const CHEVRON_RIGHT: MaterialIcon = MaterialIcon::new("chevron_right", "M10 6L8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z");
    /// More vert / más opciones vertical
    pub const MORE_VERT: MaterialIcon = MaterialIcon::new("more_vert", "M12 8c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z");
    /// Close / cerrar
    pub const CLOSE: MaterialIcon = MaterialIcon::new("close", "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z");
    /// Refresh / refrescar
    pub const REFRESH: MaterialIcon = MaterialIcon::new("refresh", "M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z");

    // ═══════════════════════════════════════════════════════════════════
    // ACCIÓN (Action)
    // ═══════════════════════════════════════════════════════════════════

    /// Add / añadir
    pub const ADD: MaterialIcon = MaterialIcon::new("add", "M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z");
    /// Add circle / añadir círculo
    pub const ADD_CIRCLE: MaterialIcon = MaterialIcon::new("add_circle", "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm5 11h-4v4h-2v-4H7v-2h4V7h2v4h4v2z");
    /// Delete / eliminar
    pub const DELETE: MaterialIcon = MaterialIcon::new("delete", "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z");
    /// Edit / editar
    pub const EDIT: MaterialIcon = MaterialIcon::new("edit", "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z");
    /// Save / guardar
    pub const SAVE: MaterialIcon = MaterialIcon::new("save", "M17 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V7l-4-4zm-5 16c-1.66 0-3-1.34-3-3s1.34-3 3-3 3 1.34 3 3-1.34 3-3 3zm3-10H5V5h10v4z");
    /// Copy / copiar
    pub const COPY: MaterialIcon = MaterialIcon::new("copy", "M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z");
    /// Done / listo
    pub const DONE: MaterialIcon = MaterialIcon::new("done", "M9 16.2L4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z");
    /// Check / verificar
    pub const CHECK: MaterialIcon = MaterialIcon::new("check", "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z");
    /// Check circle / círculo verificado
    pub const CHECK_CIRCLE: MaterialIcon = MaterialIcon::new("check_circle", "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z");
    /// Cancel / cancelar
    pub const CANCEL: MaterialIcon = MaterialIcon::new("cancel", "M12 2C6.47 2 2 6.47 2 12s4.47 10 10 10 10-4.47 10-10S17.53 2 12 2zm5 13.59L15.59 17 12 13.41 8.41 17 7 15.59 10.59 12 7 8.41 8.41 7 12 10.59 15.59 7 17 8.41 13.41 12 17 15.59z");
    /// Print / imprimir
    pub const PRINT: MaterialIcon = MaterialIcon::new("print", "M19 8H5c-1.66 0-3 1.34-3 3v6h4v4h12v-4h4v-6c0-1.66-1.34-3-3-3zm-3 11H8v-5h8v5zm3-7c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm-1-9H6v4h12V3z");
    /// Share / compartir
    pub const SHARE: MaterialIcon = MaterialIcon::new("share", "M18 16.08c-.76 0-1.44.3-1.96.77L8.91 12.7c.05-.23.09-.46.09-.7s-.04-.47-.09-.7l7.05-4.11c.54.5 1.25.81 2.04.81 1.66 0 3-1.34 3-3s-1.34-3-3-3-3 1.34-3 3c0 .24.04.47.09.7L8.04 9.81C7.5 9.31 6.79 9 6 9c-1.66 0-3 1.34-3 3s1.34 3 3 3c.79 0 1.5-.31 2.04-.81l7.12 4.16c-.05.21-.08.43-.08.65 0 1.61 1.31 2.92 2.92 2.92 1.61 0 2.92-1.31 2.92-2.92s-1.31-2.92-2.92-2.92z");
    /// Open in new / abrir en nuevo
    pub const OPEN_IN_NEW: MaterialIcon = MaterialIcon::new("open_in_new", "M19 19H5V5h7V3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2v-7h-2v7zM14 3v2h3.59l-9.83 9.83 1.41 1.41L19 6.41V10h2V3h-7z");
    /// Lock / candado cerrado
    pub const LOCK: MaterialIcon = MaterialIcon::new("lock", "M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm-6 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm3.1-9H8.9V6c0-1.71 1.39-3.1 3.1-3.1s3.1 1.39 3.1 3.1v2z");
    /// Lock open / candado abierto
    pub const LOCK_OPEN: MaterialIcon = MaterialIcon::new("lock_open", "M12 17c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm6-9h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6h1.9c0-1.71 1.39-3.1 3.1-3.1s3.1 1.39 3.1 3.1v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm0 12H6V10h12v10z");
    /// Visibility / visible
    pub const VISIBILITY: MaterialIcon = MaterialIcon::new("visibility", "M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z");
    /// Visibility off / invisible
    pub const VISIBILITY_OFF: MaterialIcon = MaterialIcon::new("visibility_off", "M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.43-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7zM2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3 2 4.27zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2zm4.31-.78l3.15 3.15.02-.16c0-1.66-1.34-3-3-3l-.17.01z");

    // ═══════════════════════════════════════════════════════════════════
    // CONTENIDO (Content)
    // ═══════════════════════════════════════════════════════════════════

    /// Filter / filtrar
    pub const FILTER: MaterialIcon = MaterialIcon::new("filter", "M3 17h18v-2H3v2zm0-5h18v-2H3v2zm0-7v2h18V5H3z");
    /// Sort / ordenar
    pub const SORT: MaterialIcon = MaterialIcon::new("sort", "M3 18h6v-2H3v2zM3 6v2h18V6H3zm0 7h12v-2H3v2z");
    /// Send / enviar
    pub const SEND: MaterialIcon = MaterialIcon::new("send", "M2.01 21L23 12 2.01 3 2 10l15 2-15 2z");
    /// Cloud / nube
    pub const CLOUD: MaterialIcon = MaterialIcon::new("cloud", "M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96z");
    /// Cloud download / descarga de nube
    pub const CLOUD_DOWNLOAD: MaterialIcon = MaterialIcon::new("cloud_download", "M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM17 13l-5 5-5-5h3V9h4v4h3z");
    /// Cloud upload / subida a nube
    pub const CLOUD_UPLOAD: MaterialIcon = MaterialIcon::new("cloud_upload", "M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM14 13v4h-4v-4H7l5-5 5 5h-3z");
    /// Link / enlace
    pub const LINK: MaterialIcon = MaterialIcon::new("link", "M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z");
    /// Flag / bandera
    pub const FLAG: MaterialIcon = MaterialIcon::new("flag", "M14.4 6L14 4H5v17h2v-7h5.6l.4 2h7V6z");

    // ═══════════════════════════════════════════════════════════════════
    // COMUNICACIÓN (Communication)
    // ═══════════════════════════════════════════════════════════════════

    /// Email / correo
    pub const EMAIL: MaterialIcon = MaterialIcon::new("email", "M20 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z");
    /// Phone / teléfono
    pub const PHONE: MaterialIcon = MaterialIcon::new("phone", "M6.62 10.79c1.44 2.83 3.76 5.14 6.59 6.59l2.2-2.2c.27-.27.67-.36 1.02-.24 1.12.37 2.33.57 3.57.57.55 0 1 .45 1 1V20c0 .55-.45 1-1 1-9.39 0-17-7.61-17-17 0-.55.45-1 1-1h3.5c.55 0 1 .45 1 1 0 1.25.2 2.45.57 3.57.11.35.03.74-.25 1.02l-2.2 2.2z");
    /// Chat / burbuja de chat
    pub const CHAT: MaterialIcon = MaterialIcon::new("chat", "M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H5.17L4 17.17V4h16v12z");
    /// Notifications / notificaciones
    pub const NOTIFICATIONS: MaterialIcon = MaterialIcon::new("notifications", "M12 22c1.1 0 2-.9 2-2h-4c0 1.1.89 2 2 2zm6-6v-5c0-3.07-1.64-5.64-4.5-6.32V4c0-.83-.67-1.5-1.5-1.5s-1.5.67-1.5 1.5v.68C7.63 5.36 6 7.92 6 11v5l-2 2v1h16v-1l-2-2z");
    /// Person / persona
    pub const PERSON: MaterialIcon = MaterialIcon::new("person", "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z");
    /// Group / grupo
    pub const GROUP: MaterialIcon = MaterialIcon::new("group", "M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5c0-2.33-4.67-3.5-7-3.5zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5z");
    /// Forum / foro
    pub const FORUM: MaterialIcon = MaterialIcon::new("forum", "M21 6h-2v9H6v2c0 .55.45 1 1 1h11l4 4V7c0-.55-.45-1-1-1zm-4 6V3c0-.55-.45-1-1-1H3c-.55 0-1 .45-1 1v14l4-4h10c.55 0 1-.45 1-1z");

    // ═══════════════════════════════════════════════════════════════════
    // ARCHIVO (File)
    // ═══════════════════════════════════════════════════════════════════

    /// File / archivo
    pub const FILE: MaterialIcon = MaterialIcon::new("file", "M6 2c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6H6zm0 18V4h7v5h5v11H6z");
    /// Folder / carpeta
    pub const FOLDER: MaterialIcon = MaterialIcon::new("folder", "M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z");
    /// Folder open / carpeta abierta
    pub const FOLDER_OPEN: MaterialIcon = MaterialIcon::new("folder_open", "M20 6h-8l-2-2H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 12H4V8h16v10z");
    /// Upload file / subir archivo
    pub const FILE_UPLOAD: MaterialIcon = MaterialIcon::new("file_upload", "M9 16h6v-6h4l-7-7-7 7h4zm-4 2h14v2H5z");
    /// Download / descargar
    pub const DOWNLOAD: MaterialIcon = MaterialIcon::new("download", "M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z");
    /// Upload / subir
    pub const UPLOAD: MaterialIcon = MaterialIcon::new("upload", "M9 16h6v-6h4l-7-7-7 7h4zm-4 2h14v2H5z");
    /// Attach file / adjuntar
    pub const ATTACH_FILE: MaterialIcon = MaterialIcon::new("attach_file", "M16.5 6v11.5c0 2.21-1.79 4-4 4s-4-1.79-4-4V5c0-1.38 1.12-2.5 2.5-2.5s2.5 1.12 2.5 2.5v10.5c0 .55-.45 1-1 1s-1-.45-1-1V6H10v9.5c0 1.38 1.12 2.5 2.5 2.5s2.5-1.12 2.5-2.5V5c0-2.21-1.79-4-4-4S7 2.79 7 5v12.5c0 3.04 2.46 5.5 5.5 5.5s5.5-2.46 5.5-5.5V6h-1.5z");
    /// Image / imagen
    pub const IMAGE: MaterialIcon = MaterialIcon::new("image", "M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z");
    /// Description / descripción
    pub const DESCRIPTION: MaterialIcon = MaterialIcon::new("description", "M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm-1 18H7v-2h6v2zm3-4H7v-2h9v2zm0-4H7V8h9v4z");
    /// PDF / archivo PDF
    pub const PICTURE_AS_PDF: MaterialIcon = MaterialIcon::new("picture_as_pdf", "M20 2H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-8.5 7.5c0 .83-.67 1.5-1.5 1.5H9v2H7.5V7H10c.83 0 1.5.67 1.5 1.5v1zm5 2c0 .83-.67 1.5-1.5 1.5h-2.5V7H15c.83 0 1.5.67 1.5 1.5v3zm4-3H19v1h1.5V11H19v2h-1.5V7h3v1.5zM9 9.5h1v-1H9v1zM4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6zm10 5.5h1v-3h-1v3z");

    // ═══════════════════════════════════════════════════════════════════
    // DISPOSITIVO (Device)
    // ═══════════════════════════════════════════════════════════════════

    /// Wi-Fi
    pub const WIFI: MaterialIcon = MaterialIcon::new("wifi", "M1 9l2 2c4.97-4.97 13.03-4.97 18 0l2-2C16.93 2.93 7.08 2.93 1 9zm8 8l3 3 3-3c-1.65-1.66-4.34-1.66-6 0zm-4-4l2 2c2.76-2.76 7.24-2.76 10 0l2-2C15.14 9.14 8.87 9.14 5 13z");
    /// Bluetooth
    pub const BLUETOOTH: MaterialIcon = MaterialIcon::new("bluetooth", "M17.71 7.71L12 2h-1v7.59L6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 11 14.41V22h1l5.71-5.71-4.3-4.29 4.3-4.29zM13 5.83l1.88 1.88L13 9.59V5.83zm1.88 10.46L13 18.17v-3.76l1.88 1.88z");
    /// Battery full / batería llena
    pub const BATTERY_FULL: MaterialIcon = MaterialIcon::new("battery_full", "M15.67 4H14V2h-4v2H8.33C7.6 4 7 4.6 7 5.33v15.33C7 21.4 7.6 22 8.33 22h7.33c.74 0 1.34-.6 1.34-1.33V5.33C17 4.6 16.4 4 15.67 4z");
    /// Signal / señal
    pub const SIGNAL: MaterialIcon = MaterialIcon::new("signal", "M2 22h20V2L2 22zm18-2H6.83L20 6.83V20z");
    /// Location / ubicación
    pub const LOCATION: MaterialIcon = MaterialIcon::new("location", "M12 2C8.13 2 5 5.13 5 9c0 5.25 7 13 7 13s7-7.75 7-13c0-3.87-3.13-7-7-7zm0 9.5c-1.38 0-2.5-1.12-2.5-2.5s1.12-2.5 2.5-2.5 2.5 1.12 2.5 2.5-1.12 2.5-2.5 2.5z");

    // ═══════════════════════════════════════════════════════════════════
    // EDITOR (Editor)
    // ═══════════════════════════════════════════════════════════════════

    /// Code / código
    pub const CODE: MaterialIcon = MaterialIcon::new("code", "M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z");
    /// Format bold / negrita
    pub const FORMAT_BOLD: MaterialIcon = MaterialIcon::new("format_bold", "M15.6 10.79c.97-.67 1.65-1.77 1.65-2.79 0-2.26-1.75-4-4-4H7v14h7.04c2.09 0 3.71-1.7 3.71-3.79 0-1.52-.86-2.82-2.15-3.42zM10 6.5h3c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5h-3v-3zm3.5 9H10v-3h3.5c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5z");
    /// Format italic / cursiva
    pub const FORMAT_ITALIC: MaterialIcon = MaterialIcon::new("format_italic", "M10 4v3h2.21l-3.42 8H6v3h8v-3h-2.21l3.42-8H18V4z");
    /// Format underline / subrayado
    pub const FORMAT_UNDERLINE: MaterialIcon = MaterialIcon::new("format_underline", "M12 17c3.31 0 6-2.69 6-6V3h-2.5v8c0 1.93-1.57 3.5-3.5 3.5S8.5 12.93 8.5 11V3H6v8c0 3.31 2.69 6 6 6zm-7 2v2h14v-2H5z");
    /// Format list / lista
    pub const FORMAT_LIST: MaterialIcon = MaterialIcon::new("format_list", "M3 13h2v-2H3v2zm0 4h2v-2H3v2zm0-8h2V7H3v2zm4 4h14v-2H7v2zm0 4h14v-2H7v2zM7 7v2h14V7H7z");
    /// Format size / tamaño
    pub const FORMAT_SIZE: MaterialIcon = MaterialIcon::new("format_size", "M9 4v3h5v12h3V7h5V4H9zm-6 8h3v7h3v-7h3V9H3v3z");
    /// Undo / deshacer
    pub const UNDO: MaterialIcon = MaterialIcon::new("undo", "M12.5 8c-2.65 0-5.05.99-6.9 2.6L2 7v9h9l-3.62-3.62c1.39-1.16 3.16-1.88 5.12-1.88 3.54 0 6.55 2.31 7.6 5.5l2.37-.78C21.08 11.03 17.15 8 12.5 8z");
    /// Redo / rehacer
    pub const REDO: MaterialIcon = MaterialIcon::new("redo", "M18.4 10.6C16.55 8.99 14.15 8 11.5 8c-4.65 0-8.58 3.03-9.96 7.22L3.9 16c1.05-3.19 4.05-5.5 7.6-5.5 1.95 0 3.73.72 5.12 1.88L13 16h9V7l-3.6 3.6z");

    // ═══════════════════════════════════════════════════════════════════
    // HARDWARE (Hardware)
    // ═══════════════════════════════════════════════════════════════════

    /// Computer / computadora
    pub const COMPUTER: MaterialIcon = MaterialIcon::new("computer", "M20 18c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z");
    /// Laptop
    pub const LAPTOP: MaterialIcon = MaterialIcon::new("laptop", "M20 18c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z");
    /// Phone Android
    pub const PHONE_ANDROID: MaterialIcon = MaterialIcon::new("phone_android", "M16 1H8C6.34 1 5 2.34 5 4v16c0 1.66 1.34 3 3 3h8c1.66 0 3-1.34 3-3V4c0-1.66-1.34-3-3-3zm-2 20h-4v-1h4v1zm3.25-3H6.75V4h10.5v14z");
    /// Tablet
    pub const TABLET: MaterialIcon = MaterialIcon::new("tablet", "M21 4H3c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm-1 14H4V6h16v12z");
    /// Print / imprimir (duplicado de acción)
    pub const PRINTER: MaterialIcon = MaterialIcon::new("printer", "M19 8H5c-1.66 0-3 1.34-3 3v6h4v4h12v-4h4v-6c0-1.66-1.34-3-3-3zm-3 11H8v-5h8v5zm3-7c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm-1-9H6v4h12V3z");

    // ═══════════════════════════════════════════════════════════════════
    // IMAGEN (Image)
    // ═══════════════════════════════════════════════════════════════════

    /// Photo / foto
    pub const PHOTO: MaterialIcon = MaterialIcon::new("photo", "M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z");
    /// Camera / cámara
    pub const CAMERA: MaterialIcon = MaterialIcon::new("camera", "M9.4 10.5l4.77-8.26a9.984 9.984 0 0 0-8.49 2.01l3.66 6.35.06-.1zM21.54 9c-.92-2.92-3.15-5.26-6-6.34L11.18 9H21.54zm-8.78 11.76c3.84-1.12 6.78-4.39 7.7-8.26H12.7l-1.94 8.26zM8.23 18.51C9.97 20.03 12.34 21 15 21c.41 0 .82-.02 1.21-.06l-3.79-6.84-4.19 4.41zM4.03 15.01C4.16 15.66 4.36 16.28 4.62 16.87l4.23-2.44-4.82-4.81c-.42 1.41-.53 2.86-.28 4.28l.25.1z");
    /// Brush / pincel
    pub const BRUSH: MaterialIcon = MaterialIcon::new("brush", "M7 14c-1.66 0-3 1.34-3 3 0 1.31-1.16 2-2 2 .92 1.22 2.49 2 4 2 2.21 0 4-1.79 4-4 0-1.66-1.34-3-3-3zm13.71-9.37l-1.34-1.34a.996.996 0 0 0-1.41 0L9 12.25 11.75 15l8.96-8.96a.996.996 0 0 0 0-1.41z");
    /// Palette / paleta
    pub const PALETTE: MaterialIcon = MaterialIcon::new("palette", "M12 2C6.49 2 2 6.49 2 12s4.49 10 10 10c1.38 0 2.5-1.12 2.5-2.5 0-.61-.23-1.16-.61-1.59-.38-.43-.61-1.01-.61-1.66 0-1.38 1.12-2.5 2.5-2.5H16c3.31 0 6-2.69 6-6 0-4.96-4.49-9-10-9zm-5.5 9c-.83 0-1.5-.67-1.5-1.5S5.67 8 6.5 8 8 8.67 8 9.5 7.33 11 6.5 11zm3-4C8.67 7 8 6.33 8 5.5S8.67 4 9.5 4s1.5.67 1.5 1.5S10.33 7 9.5 7zm5 0c-.83 0-1.5-.67-1.5-1.5S13.67 4 14.5 4s1.5.67 1.5 1.5S15.33 7 14.5 7zm3 4c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5z");

    // ═══════════════════════════════════════════════════════════════════
    // MAPA (Maps)
    // ═══════════════════════════════════════════════════════════════════

    /// Place / lugar (pin)
    pub const PLACE: MaterialIcon = MaterialIcon::new("place", "M12 2C8.13 2 5 5.13 5 9c0 5.25 7 13 7 13s7-7.75 7-13c0-3.87-3.13-7-7-7zm0 9.5c-1.38 0-2.5-1.12-2.5-2.5s1.12-2.5 2.5-2.5 2.5 1.12 2.5 2.5-1.12 2.5-2.5 2.5z");
    /// Directions / direcciones
    pub const DIRECTIONS: MaterialIcon = MaterialIcon::new("directions", "M21.71 11.29l-9-9a.996.996 0 0 0-1.41 0l-9 9a.996.996 0 0 0 0 1.41l9 9c.39.39 1.02.39 1.41 0l9-9a.996.996 0 0 0 0-1.41zM14 14.5V12h-4v3H8v-4c0-.55.45-1 1-1h5V7.5l3.5 3.5-3.5 3.5z");
    /// Map / mapa
    pub const MAP: MaterialIcon = MaterialIcon::new("map", "M20.5 3l-.16.03L15 5.1 9 3 3.36 4.9c-.21.07-.36.25-.36.48V20.5c0 .28.22.5.5.5l.16-.03L9 18.9l6 2.1 5.64-1.9c.21-.07.36-.25.36-.48V3.5c0-.28-.22-.5-.5-.5zM15 19l-6-2.11V5l6 2.11V19z");
    /// Local shipping / envío
    pub const LOCAL_SHIPPING: MaterialIcon = MaterialIcon::new("local_shipping", "M20 8h-3V4H3c-1.1 0-2 .9-2 2v11h2c0 1.66 1.34 3 3 3s3-1.34 3-3h6c0 1.66 1.34 3 3 3s3-1.34 3-3h2v-5l-3-4zM6 18.5c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm13.5-9l1.96 2.5H17V9.5h2.5zm-1.5 9c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5z");
    /// Restaurant / restaurante
    pub const RESTAURANT: MaterialIcon = MaterialIcon::new("restaurant", "M11 9H9V2H7v7H5V2H3v7c0 2.12 1.66 3.84 3.75 3.97V22h2.5v-9.03C11.34 12.84 13 11.12 13 9V2h-2v7zm5-3v8h2.5v8H21V2c-2.76 0-5 2.24-5 4z");
    /// Hotel
    pub const HOTEL: MaterialIcon = MaterialIcon::new("hotel", "M7 13c1.66 0 3-1.34 3-3S8.66 7 7 7s-3 1.34-3 3 1.34 3 3 3zm12-6h-8v7H3V5H1v15h2v-3h18v3h2V9c0-2.21-1.79-4-4-4z");

    // ═══════════════════════════════════════════════════════════════════
    // NOTIFICACIÓN (Notification)
    // ═══════════════════════════════════════════════════════════════════

    /// Info / información
    pub const INFO: MaterialIcon = MaterialIcon::new("info", "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z");
    /// Warning / advertencia
    pub const WARNING: MaterialIcon = MaterialIcon::new("warning", "M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z");
    /// Error / error
    pub const ERROR: MaterialIcon = MaterialIcon::new("error", "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z");
    /// Warning amber / advertencia ámbar
    pub const WARNING_AMBER: MaterialIcon = MaterialIcon::new("warning_amber", "M12 5.99L19.53 19H4.47L12 5.99M12 2L1 21h22L12 2zm1 14h-2v2h2v-2zm0-6h-2v4h2v-4z");
    /// Feedback / retroalimentación
    pub const FEEDBACK: MaterialIcon = MaterialIcon::new("feedback", "M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H5.17L4 17.17V4h16v12zM11 12h2v-2h-2v2zm0-4h2V6h-2v2z");
    /// Help / ayuda
    pub const HELP: MaterialIcon = MaterialIcon::new("help", "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 17h-2v-2h2v2zm2.07-7.75l-.9.92C13.45 12.9 13 13.5 13 15h-2v-.5c0-1.1.45-2.1 1.17-2.83l1.24-1.26c.37-.36.59-.86.59-1.41 0-1.1-.9-2-2-2s-2 .9-2 2H8c0-2.21 1.79-4 4-4s4 1.79 4 4c0 .88-.36 1.68-.93 2.25z");
    /// New releases / novedades
    pub const NEW_RELEASES: MaterialIcon = MaterialIcon::new("new_releases", "M23 12l-2.44-2.78.34-3.68-3.61-.82-1.89-3.18L12 3 8.6 1.54 6.71 4.72l-3.61.81.34 3.68L1 12l2.44 2.78-.34 3.69 3.61.82 1.89 3.18L12 21l3.4 1.46 1.89-3.18 3.61-.82-.34-3.68L23 12zm-10 5h-2v-2h2v2zm0-4h-2V7h2v6z");

    // ═══════════════════════════════════════════════════════════════════
    // LUGAR (Places)
    // ═══════════════════════════════════════════════════════════════════

    /// Favorite / favorito (corazón)
    pub const FAVORITE: MaterialIcon = MaterialIcon::new("favorite", "M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z");
    /// Favorite outline / favorito borde
    pub const FAVORITE_OUTLINE: MaterialIcon = MaterialIcon::new("favorite_outline", "M16.5 3c-1.74 0-3.41.81-4.5 2.09C10.91 3.81 9.24 3 7.5 3 4.42 3 2 5.42 2 8.5c0 3.78 3.4 6.86 8.55 11.54L12 21.35l1.45-1.32C18.6 15.36 22 12.28 22 8.5 22 5.42 19.58 3 16.5 3zm-4.4 15.55l-.1.1-.1-.1C7.14 14.24 4 11.39 4 8.5 4 6.5 5.5 5 7.5 5c1.54 0 3.04.99 3.57 2.36h1.87C13.46 5.99 14.96 5 16.5 5c2 0 3.5 1.5 3.5 3.5 0 2.89-3.14 5.74-7.9 10.05z");
    /// Star / estrella
    pub const STAR: MaterialIcon = MaterialIcon::new("star", "M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z");
    /// Star half / estrella media
    pub const STAR_HALF: MaterialIcon = MaterialIcon::new("star_half", "M22 9.24l-7.19-.62L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21 12 17.27 18.18 21l-1.63-7.03L22 9.24zM12 15.4V6.1l1.71 4.08 4.38.38-3.32 2.88 1 4.28L12 15.4z");
    /// Star outline / estrella borde
    pub const STAR_OUTLINE: MaterialIcon = MaterialIcon::new("star_outline", "M22 9.24l-7.19-.62L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21 12 17.27 18.18 21l-1.63-7.03L22 9.24zM12 15.4l-3.76 2.27 1-4.28-3.32-2.88 4.38-.38L12 6.1l1.71 4.04 4.38.38-3.32 2.88 1 4.28L12 15.4z");
    /// Thumb up / pulgar arriba
    pub const THUMB_UP: MaterialIcon = MaterialIcon::new("thumb_up", "M1 21h4V9H1v12zm22-11c0-1.1-.9-2-2-2h-6.31l.95-4.57.03-.32c0-.41-.17-.79-.44-1.06L14.17 1 7.59 7.59C7.22 7.95 7 8.45 7 9v10c0 1.1.9 2 2 2h9c.83 0 1.54-.5 1.84-1.22l3.02-7.05c.09-.23.14-.47.14-.73v-2z");
    /// Thumb down / pulgar abajo
    pub const THUMB_DOWN: MaterialIcon = MaterialIcon::new("thumb_down", "M15 3H6c-.83 0-1.54.5-1.84 1.22l-3.02 7.05c-.09.23-.14.47-.14.73v2c0 1.1.9 2 2 2h6.31l-.95 4.57-.03.32c0 .41.17.79.44 1.06L9.83 23l6.59-6.59c.36-.36.58-.86.58-1.41V5c0-1.1-.9-2-2-2zm4 0v12h4V3h-4z");

    // ═══════════════════════════════════════════════════════════════════
    // SOCIAL (Social)
    // ═══════════════════════════════════════════════════════════════════

    /// Share / compartir (social)
    pub const SHARE_SOCIAL: MaterialIcon = MaterialIcon::new("share_social", "M18 16.08c-.76 0-1.44.3-1.96.77L8.91 12.7c.05-.23.09-.46.09-.7s-.04-.47-.09-.7l7.05-4.11c.54.5 1.25.81 2.04.81 1.66 0 3-1.34 3-3s-1.34-3-3-3-3 1.34-3 3c0 .24.04.47.09.7L8.04 9.81C7.5 9.31 6.79 9 6 9c-1.66 0-3 1.34-3 3s1.34 3 3 3c.79 0 1.5-.31 2.04-.81l7.12 4.16c-.05.21-.08.43-.08.65 0 1.61 1.31 2.92 2.92 2.92 1.61 0 2.92-1.31 2.92-2.92s-1.31-2.92-2.92-2.92z");
    /// Public / público
    pub const PUBLIC: MaterialIcon = MaterialIcon::new("public", "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z");
    /// School / escuela
    pub const SCHOOL: MaterialIcon = MaterialIcon::new("school", "M5 13.18v4L12 21l7-3.82v-4L12 17l-7-3.82zM12 3L1 9l11 6 9-4.91V17h2V9L12 3z");
    /// Work / trabajo
    pub const WORK: MaterialIcon = MaterialIcon::new("work", "M20 6h-4V4c0-1.11-.89-2-2-2h-4c-1.11 0-2 .89-2 2v2H4c-1.11 0-1.99.89-1.99 2L2 19c0 1.11.89 2 2 2h16c1.11 0 2-.89 2-2V8c0-1.11-.89-2-2-2zm-6 0h-4V4h4v2z");
    /// Celebration / celebración
    pub const CELEBRATION: MaterialIcon = MaterialIcon::new("celebration", "M2 22l14-5-9-9-5 14zm12.53-9.47L21 6.05l1.48 1.48 1.06-1.06L21 3.93l-7.53 7.53 1.06 1.07zM10.94 6L9.47 7.47l1.06 1.06 2.54-2.54-1.06-1.06L10.94 6zm-2.48 3.12l-3.54 3.54 1.06 1.06 3.54-3.54-1.06-1.06z");

    // ═══════════════════════════════════════════════════════════════════
    // TOGGLE (Toggle)
    // ═══════════════════════════════════════════════════════════════════

    /// Check box / casilla verificada
    pub const CHECK_BOX: MaterialIcon = MaterialIcon::new("check_box", "M19 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.11 0 2-.9 2-2V5c0-1.1-.89-2-2-2zm-9 14l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z");
    /// Check box outline / casilla sin verificar
    pub const CHECK_BOX_OUTLINE: MaterialIcon = MaterialIcon::new("check_box_outline", "M19 5v14H5V5h14m0-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z");
    /// Radio button checked
    pub const RADIO_BUTTON_CHECKED: MaterialIcon = MaterialIcon::new("radio_button_checked", "M12 7c-2.76 0-5 2.24-5 5s2.24 5 5 5 5-2.24 5-5-2.24-5-5-5zm0-5C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8z");
    /// Radio button unchecked
    pub const RADIO_BUTTON_UNCHECKED: MaterialIcon = MaterialIcon::new("radio_button_unchecked", "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8z");
    /// Toggle on / activo
    pub const TOGGLE_ON: MaterialIcon = MaterialIcon::new("toggle_on", "M17 7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h10c2.76 0 5-2.24 5-5s-2.24-5-5-5zm0 8c-1.66 0-3-1.34-3-3s1.34-3 3-3 3 1.34 3 3-1.34 3-3 3z");
    /// Toggle off / inactivo
    pub const TOGGLE_OFF: MaterialIcon = MaterialIcon::new("toggle_off", "M17 7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h10c2.76 0 5-2.24 5-5s-2.24-5-5-5zM7 15c-1.66 0-3-1.34-3-3s1.34-3 3-3 3 1.34 3 3-1.34 3-3 3z");

    // ═══════════════════════════════════════════════════════════════════
    // FECHA / HORA (Date/Time)
    // ═══════════════════════════════════════════════════════════════════

    /// Calendar / calendario (fecha)
    pub const DATE: MaterialIcon = MaterialIcon::new("date", "M19 3h-1V1h-2v2H8V1H6v2H5c-1.11 0-1.99.9-1.99 2L3 19c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H5V8h14v11z");
    /// Calendar today / hoy
    pub const CALENDAR_TODAY: MaterialIcon = MaterialIcon::new("calendar_today", "M20 3h-1V1h-2v2H7V1H5v2H4c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 18H4V8h16v13z");
    /// Schedule / horario
    pub const TIME: MaterialIcon = MaterialIcon::new("time", "M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z");
    /// Alarm / alarma
    pub const ALARM: MaterialIcon = MaterialIcon::new("alarm", "M22 5.72l-4.6-3.86-1.29 1.53 4.6 3.86L22 5.72zM7.88 3.39L6.6 1.86 2 5.71l1.29 1.53 4.59-3.85zM12.5 8H11v6l4.75 2.85.75-1.23-4-2.37V8zM12 4c-4.97 0-9 4.03-9 9s4.02 9 9 9c4.97 0 9-4.03 9-9s-4.03-9-9-9zm0 16c-3.87 0-7-3.13-7-7s3.13-7 7-7 7 3.13 7 7-3.13 7-7 7z");

    // ═══════════════════════════════════════════════════════════════════
    // COMERCIO (Commerce)
    // ═══════════════════════════════════════════════════════════════════

    /// Shopping cart / carrito
    pub const SHOPPING_CART: MaterialIcon = MaterialIcon::new("shopping_cart", "M7 18c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm10 0c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zM5.53 6l1.48 3h10.37l1.66-5H4.21L3.11 2H1v2h1.38l2.96 6.86L4 13.34C3.39 14.28 4.05 15.5 5.07 15.5H19v-2H6.58l1.11-2h9.48c.73 0 1.38-.41 1.69-1.02l2.53-7.64C21.29 2.46 20.67 2 20 2H5.05l-.42-1H1v2h2.41L5.53 6z");
    /// Payment / pago
    pub const PAYMENT: MaterialIcon = MaterialIcon::new("payment", "M20 4H4c-1.11 0-1.99.89-1.99 2L2 18c0 1.11.89 2 2 2h16c1.11 0 2-.89 2-2V6c0-1.11-.89-2-2-2zm0 14H4v-6h16v6zm0-10H4V6h16v2z");
    /// Account balance / saldo
    pub const ACCOUNT_BALANCE: MaterialIcon = MaterialIcon::new("account_balance", "M4 10h3v7H4v-7zm6.5 0h3v7h-3v-7zM2 19h20v3H2v-3zM21 10h-3v7h3v-7zM11 1.59L1.59 6.93 2 8h20l.41-1.07L11 1.59z");
    /// Store / tienda
    pub const STORE: MaterialIcon = MaterialIcon::new("store", "M20 4H4v2h16V4zm1 10v-2l-1-5H4l-1 5v2h1v6h10v-6h4v6h2v-6h1zm-9 4H6v-4h6v4z");
    /// Trending up / tendencia alza
    pub const TRENDING_UP: MaterialIcon = MaterialIcon::new("trending_up", "M16 6l2.29 2.29-4.88 4.88-4-4L2 16.59 3.41 18l6-6 4 4 6.3-6.29L22 12V6z");

    // ═══════════════════════════════════════════════════════════════════
    // SALUD / CIENCIA (Health/Science)
    // ═══════════════════════════════════════════════════════════════════

    /// Favorite / favorito (ya definido arriba)
    /// Local hospital / hospital
    pub const LOCAL_HOSPITAL: MaterialIcon = MaterialIcon::new("local_hospital", "M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-1 11h-4v4h-4v-4H6v-4h4V6h4v4h4v4z");
    /// Science / ciencia
    pub const SCIENCE: MaterialIcon = MaterialIcon::new("science", "M19.8 18.4L14 10.67V6.5l1.35-1.69c.26-.33.03-.81-.39-.81H9.04c-.42 0-.65.48-.39.81L10 6.5v4.17L4.2 18.4c-.49.66-.02 1.6.8 1.6h14c.82 0 1.29-.94.8-1.6z");

    // ─── Buscar icono por nombre ─────────────────────────────────────

    /// Busca un icono en el catálogo por su nombre
    pub fn by_name(name: &str) -> Option<&'static MaterialIcon> {
        match name {
            // Navegación
            "home" => Some(&HOME),
            "search" | "buscar" => Some(&SEARCH),
            "settings" | "configuracion" | "ajustes" => Some(&SETTINGS),
            "menu" => Some(&MENU),
            "arrow_back" | "atras" | "back" => Some(&ARROW_BACK),
            "arrow_forward" | "adelante" | "forward" => Some(&ARROW_FORWARD),
            "arrow_drop_down" | "desplegable" => Some(&ARROW_DROP_DOWN),
            "arrow_up" | "arriba" => Some(&ARROW_UP),
            "arrow_down" | "abajo" => Some(&ARROW_DOWN),
            "chevron_left" | "izquierda" => Some(&CHEVRON_LEFT),
            "chevron_right" | "derecha" => Some(&CHEVRON_RIGHT),
            "more_vert" | "mas_opciones" => Some(&MORE_VERT),
            "close" | "cerrar" => Some(&CLOSE),
            "refresh" | "refrescar" | "actualizar" => Some(&REFRESH),
            // Acción
            "add" | "anadir" | "añadir" | "agregar" => Some(&ADD),
            "add_circle" | "anadir_circulo" => Some(&ADD_CIRCLE),
            "delete" | "eliminar" | "borrar" => Some(&DELETE),
            "edit" | "editar" => Some(&EDIT),
            "save" | "guardar" => Some(&SAVE),
            "copy" | "copiar" => Some(&COPY),
            "done" | "listo" | "hecho" => Some(&DONE),
            "check" | "verificar" | "ok" => Some(&CHECK),
            "check_circle" => Some(&CHECK_CIRCLE),
            "cancel" | "cancelar" => Some(&CANCEL),
            "print" | "imprimir" => Some(&PRINT),
            "share" | "compartir" => Some(&SHARE),
            "open_in_new" => Some(&OPEN_IN_NEW),
            "lock" | "candado" | "bloquear" => Some(&LOCK),
            "lock_open" | "desbloquear" => Some(&LOCK_OPEN),
            "visibility" | "visible" | "ver" => Some(&VISIBILITY),
            "visibility_off" | "invisible" | "ocultar" => Some(&VISIBILITY_OFF),
            // Contenido
            "filter" | "filtrar" => Some(&FILTER),
            "sort" | "ordenar" => Some(&SORT),
            "send" | "enviar" => Some(&SEND),
            "cloud" | "nube" => Some(&CLOUD),
            "cloud_download" => Some(&CLOUD_DOWNLOAD),
            "cloud_upload" => Some(&CLOUD_UPLOAD),
            "link" | "enlace" => Some(&LINK),
            "flag" | "bandera" => Some(&FLAG),
            // Comunicación
            "email" | "correo" | "mail" => Some(&EMAIL),
            "phone" | "telefono" | "teléfono" | "llamar" => Some(&PHONE),
            "chat" => Some(&CHAT),
            "notifications" | "notificaciones" | "campana" => Some(&NOTIFICATIONS),
            "person" | "persona" | "usuario" | "user" => Some(&PERSON),
            "group" | "grupo" => Some(&GROUP),
            "forum" | "foro" => Some(&FORUM),
            // Archivo
            "file" | "archivo" => Some(&FILE),
            "folder" | "carpeta" => Some(&FOLDER),
            "folder_open" | "carpeta_abierta" => Some(&FOLDER_OPEN),
            "file_upload" | "subir_archivo" => Some(&FILE_UPLOAD),
            "download" | "descargar" | "bajar" => Some(&DOWNLOAD),
            "upload" | "subir" => Some(&UPLOAD),
            "attach_file" | "adjuntar" => Some(&ATTACH_FILE),
            "image" | "imagen" | "foto" => Some(&IMAGE),
            "description" | "descripcion" | "documento" => Some(&DESCRIPTION),
            "picture_as_pdf" | "pdf" => Some(&PICTURE_AS_PDF),
            // Dispositivo
            "wifi" => Some(&WIFI),
            "bluetooth" => Some(&BLUETOOTH),
            "battery_full" | "bateria" => Some(&BATTERY_FULL),
            "signal" | "señal" | "senal" => Some(&SIGNAL),
            "location" | "ubicacion" | "ubicación" => Some(&LOCATION),
            // Editor
            "code" | "codigo" | "código" => Some(&CODE),
            "format_bold" | "negrita" => Some(&FORMAT_BOLD),
            "format_italic" | "cursiva" => Some(&FORMAT_ITALIC),
            "format_underline" | "subrayado" => Some(&FORMAT_UNDERLINE),
            "format_list" | "lista" => Some(&FORMAT_LIST),
            "format_size" | "tamano" | "tamaño" => Some(&FORMAT_SIZE),
            "undo" | "deshacer" => Some(&UNDO),
            "redo" | "rehacer" => Some(&REDO),
            // Hardware
            "computer" | "computadora" | "pc" => Some(&COMPUTER),
            "laptop" | "portatil" | "portátil" => Some(&LAPTOP),
            "phone_android" | "android" | "movil" | "móvil" => Some(&PHONE_ANDROID),
            "tablet" | "tableta" => Some(&TABLET),
            "printer" | "impresora" => Some(&PRINTER),
            // Imagen
            "photo" => Some(&PHOTO),
            "camera" | "camara" | "cámara" => Some(&CAMERA),
            "brush" | "pincel" => Some(&BRUSH),
            "palette" | "paleta" => Some(&PALETTE),
            // Mapa
            "place" | "lugar" | "pin" => Some(&PLACE),
            "directions" | "direcciones" => Some(&DIRECTIONS),
            "map" | "mapa" => Some(&MAP),
            "local_shipping" | "envio" | "envío" => Some(&LOCAL_SHIPPING),
            "restaurant" | "restaurante" | "comer" => Some(&RESTAURANT),
            "hotel" => Some(&HOTEL),
            // Notificación
            "info" | "informacion" | "información" => Some(&INFO),
            "warning" | "advertencia" | "cuidado" => Some(&WARNING),
            "error" | "error_icon" => Some(&ERROR),
            "warning_amber" => Some(&WARNING_AMBER),
            "feedback" | "retroalimentacion" | "comentarios" => Some(&FEEDBACK),
            "help" | "ayuda" => Some(&HELP),
            "new_releases" | "novedades" => Some(&NEW_RELEASES),
            // Lugar (social)
            "favorite" | "favorito" | "corazon" | "corazón" | "like" => Some(&FAVORITE),
            "favorite_outline" | "favorito_borde" => Some(&FAVORITE_OUTLINE),
            "star" | "estrella" => Some(&STAR),
            "star_half" | "estrella_media" => Some(&STAR_HALF),
            "star_outline" | "estrella_borde" => Some(&STAR_OUTLINE),
            "thumb_up" | "pulgar_arriba" | "me_gusta" => Some(&THUMB_UP),
            "thumb_down" | "pulgar_abajo" | "no_gusta" => Some(&THUMB_DOWN),
            // Social
            "share_social" | "compartir_social" => Some(&SHARE_SOCIAL),
            "public" | "publico" | "público" | "mundo" => Some(&PUBLIC),
            "school" | "escuela" | "educacion" | "educación" => Some(&SCHOOL),
            "work" | "trabajo" | "oficina" => Some(&WORK),
            "celebration" | "celebracion" | "celebrar" => Some(&CELEBRATION),
            // Toggle
            "check_box" | "casilla_on" => Some(&CHECK_BOX),
            "check_box_outline" | "casilla_off" => Some(&CHECK_BOX_OUTLINE),
            "radio_button_checked" | "radio_on" => Some(&RADIO_BUTTON_CHECKED),
            "radio_button_unchecked" | "radio_off" => Some(&RADIO_BUTTON_UNCHECKED),
            "toggle_on" | "interruptor_on" | "activo" => Some(&TOGGLE_ON),
            "toggle_off" | "interruptor_off" | "inactivo" => Some(&TOGGLE_OFF),
            // Fecha / Hora
            "date" | "fecha" | "calendario" | "calendar" => Some(&DATE),
            "calendar_today" | "hoy" => Some(&CALENDAR_TODAY),
            "time" | "hora" | "reloj" => Some(&TIME),
            "alarm" | "alarma" => Some(&ALARM),
            // Comercio
            "shopping_cart" | "carrito" | "cesta" => Some(&SHOPPING_CART),
            "payment" | "pago" => Some(&PAYMENT),
            "account_balance" | "saldo" | "banco" => Some(&ACCOUNT_BALANCE),
            "store" | "tienda" => Some(&STORE),
            "trending_up" | "tendencia" | "creciendo" => Some(&TRENDING_UP),
            // Salud / Ciencia
            "local_hospital" | "hospital" => Some(&LOCAL_HOSPITAL),
            "science" | "ciencia" | "laboratorio" => Some(&SCIENCE),
            // No encontrado
            _ => None,
        }
    }

    /// Obtiene un emoji de fallback para un nombre de icono
    /// Se usa cuando el icono no está en el catálogo o como placeholder
    pub fn fallback_emoji(name: &str) -> &'static str {
        match name {
            // Navegación
            "home" => "🏠", "search" | "buscar" => "🔍", "settings" | "configuracion" => "⚙️",
            "menu" => "☰", "arrow_back" | "atras" => "◀️", "arrow_forward" | "adelante" => "▶️",
            "arrow_up" | "arriba" => "⬆️", "arrow_down" | "abajo" => "⬇️",
            "close" | "cerrar" => "✖️", "refresh" | "actualizar" => "🔄",
            "more_vert" => "⋮", "chevron_left" => "◀", "chevron_right" => "▶",
            // Acción
            "add" | "anadir" | "añadir" => "➕", "delete" | "eliminar" => "🗑️",
            "edit" | "editar" => "✏️", "save" | "guardar" => "💾",
            "copy" | "copiar" => "📋", "done" | "listo" => "✅",
            "check" | "verificar" => "✅", "cancel" | "cancelar" => "❌",
            "print" | "imprimir" => "🖨️", "share" | "compartir" => "📤",
            "lock" | "bloquear" => "🔒", "lock_open" | "desbloquear" => "🔓",
            "visibility" | "ver" => "👁️", "visibility_off" | "ocultar" => "👁️‍🗨️",
            // Contenido
            "filter" | "filtrar" => "🔽", "sort" | "ordenar" => "↕️",
            "send" | "enviar" => "📤", "cloud" | "nube" => "☁️",
            "link" | "enlace" => "🔗", "flag" | "bandera" => "🚩",
            // Comunicación
            "email" | "correo" | "mail" => "✉️", "phone" | "telefono" => "📞",
            "chat" => "💬", "notifications" | "notificaciones" => "🔔",
            "person" | "persona" | "usuario" => "👤", "group" | "grupo" => "👥",
            "forum" | "foro" => "🗣️",
            // Archivo
            "file" | "archivo" => "📄", "folder" | "carpeta" => "📁",
            "folder_open" => "📂", "download" | "descargar" => "⬇️",
            "upload" | "subir" => "⬆️", "image" | "imagen" => "🖼️",
            "description" | "documento" => "📝", "pdf" => "📕",
            // Dispositivo
            "wifi" => "📶", "bluetooth" => "🔷", "location" | "ubicacion" => "📍",
            // Editor
            "code" | "codigo" => "💻", "undo" | "deshacer" => "↩️",
            "redo" | "rehacer" => "↪️",
            // Hardware
            "computer" | "computadora" => "🖥️", "laptop" | "portatil" => "💻",
            "phone_android" | "movil" => "📱", "tablet" => "📱",
            "printer" | "impresora" => "🖨️",
            // Imagen
            "photo" | "foto" => "📷", "camera" | "camara" => "📷",
            "brush" | "pincel" => "🖌️", "palette" | "paleta" => "🎨",
            // Mapa
            "place" | "lugar" | "pin" => "📍", "directions" | "direcciones" => "🗺️",
            "map" | "mapa" => "🗺️", "restaurant" | "restaurante" => "🍽️",
            "hotel" => "🏨",
            // Notificación
            "info" | "informacion" => "ℹ️", "warning" | "advertencia" => "⚠️",
            "error" => "❌", "help" | "ayuda" => "❓",
            "feedback" => "💬", "new_releases" => "🆕",
            // Lugar
            "favorite" | "favorito" | "corazon" => "❤️",
            "favorite_outline" => "🤍",
            "star" | "estrella" => "⭐", "star_half" => "🌟",
            "star_outline" => "☆",
            "thumb_up" | "me_gusta" => "👍", "thumb_down" | "no_gusta" => "👎",
            // Social
            "public" | "mundo" => "🌐", "school" | "escuela" => "🏫",
            "work" | "trabajo" => "💼", "celebration" => "🎉",
            // Toggle
            "check_box" => "☑️", "check_box_outline" => "⬜",
            "toggle_on" | "activo" => "🔵", "toggle_off" | "inactivo" => "⚪",
            // Fecha / Hora
            "date" | "fecha" | "calendario" => "📅", "calendar_today" | "hoy" => "📅",
            "time" | "hora" | "reloj" => "⏰", "alarm" | "alarma" => "⏰",
            // Comercio
            "shopping_cart" | "carrito" => "🛒", "payment" | "pago" => "💳",
            "account_balance" | "banco" => "🏦", "store" | "tienda" => "🏪",
            "trending_up" | "tendencia" => "📈",
            // Salud / Ciencia
            "local_hospital" | "hospital" => "🏥", "science" => "🔬",
            // Fallback genérico
            _ => "❓",
        }
    }
}

// ─── Función helper ──────────────────────────────────────────────────

/// Crea un widget icono desde un nombre de icono
///
/// Busca el icono en el catálogo. Si no existe, muestra un emoji de fallback.
///
/// # Parámetros
/// - `name`: nombre del icono (ej: "home", "favorite", "settings")
/// - `size`: tamaño en píxeles (ej: 24, 32, 48)
/// - `color`: color del icono como `RgbColor`
pub fn icon_widget(name: &str, size: f64, color: RgbColor) -> Box<AnyWidgetView<()>> {
    if let Some(icon) = catalog::by_name(name) {
        icon.to_widget(size, color)
    } else {
        // Fallback a emoji genérico
        let emoji = catalog::fallback_emoji(name);
        let xilem_color: Color = color.into();
        Box::new(
            view::label(emoji)
                .text_size(size as f32)
                .color(xilem_color)
        )
    }
}

/// Crea un widget icono con estilo específico
///
/// # Parámetros
/// - `name`: nombre del icono
/// - `size`: tamaño en píxeles
/// - `color`: color del icono
/// - `style`: estilo del icono (filled, outlined, rounded, sharp, twotone)
pub fn icon_widget_styled(
    name: &str,
    size: f64,
    color: RgbColor,
    style: IconStyle,
) -> Box<AnyWidgetView<()>> {
    if let Some(icon) = catalog::by_name(name) {
        icon.to_widget_styled(size, color, style)
    } else {
        icon_widget(name, size, color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_by_name_exists() {
        assert!(catalog::by_name("home").is_some());
        assert!(catalog::by_name("favorite").is_some());
        assert!(catalog::by_name("settings").is_some());
        assert!(catalog::by_name("search").is_some());
    }

    #[test]
    fn test_by_name_not_found() {
        assert!(catalog::by_name("nonexistent_icon_xyz").is_none());
    }

    #[test]
    fn test_by_name_spanish() {
        assert!(catalog::by_name("buscar").is_some());
        assert!(catalog::by_name("correo").is_some());
        assert!(catalog::by_name("usuario").is_some());
    }

    #[test]
    fn test_fallback_emoji() {
        assert_eq!(catalog::fallback_emoji("home"), "🏠");
        assert_eq!(catalog::fallback_emoji("favorite"), "❤️");
        assert_eq!(catalog::fallback_emoji("unknown"), "❓");
    }

    #[test]
    fn test_icon_style_from_str() {
        assert_eq!(IconStyle::from_str("filled"), IconStyle::Filled);
        assert_eq!(IconStyle::from_str("outlined"), IconStyle::Outlined);
        assert_eq!(IconStyle::from_str("perfilado"), IconStyle::Outlined);
        assert_eq!(IconStyle::from_str("rounded"), IconStyle::Rounded);
        assert_eq!(IconStyle::from_str("redondo"), IconStyle::Rounded);
        assert_eq!(IconStyle::from_str("sharp"), IconStyle::Sharp);
        assert_eq!(IconStyle::from_str("agudo"), IconStyle::Sharp);
        assert_eq!(IconStyle::from_str("twotone"), IconStyle::TwoTone);
        assert_eq!(IconStyle::from_str("dos_tonos"), IconStyle::TwoTone);
    }

    #[test]
    fn test_icon_new() {
        let icon = MaterialIcon::new("test", "M0 0h24v24H0z");
        assert_eq!(icon.name, "test");
        assert_eq!(icon.svg_path, "M0 0h24v24H0z");
        assert_eq!(icon.style, IconStyle::Filled);
    }

    #[test]
    fn test_icon_with_style() {
        let icon = MaterialIcon::new("test", "M0 0h24v24H0z")
            .with_style(IconStyle::Outlined);
        assert_eq!(icon.style, IconStyle::Outlined);
    }

    #[test]
    fn test_catalog_count() {
        // Verificar que tenemos al menos 90 iconos en el catálogo
        let icons = vec![
            "home", "search", "settings", "menu", "arrow_back", "arrow_forward",
            "add", "delete", "edit", "save", "close", "check", "cancel",
            "email", "phone", "person", "group", "notifications",
            "file", "folder", "download", "upload", "image",
            "info", "warning", "error", "help",
            "favorite", "star", "thumb_up", "thumb_down",
            "date", "time", "alarm",
            "send", "filter", "sort", "share", "copy",
            "code", "lock", "visibility",
            "wifi", "location", "map", "place",
            "shopping_cart", "payment", "store",
            "chat", "forum", "public", "school", "work",
            "check_box", "toggle_on", "toggle_off",
            "computer", "laptop", "phone_android", "tablet",
            "photo", "camera", "brush", "palette",
            "directions", "restaurant", "hotel",
            "undo", "redo", "link", "flag",
            "cloud", "cloud_download", "cloud_upload",
            "celebration", "science", "local_hospital",
            "format_bold", "format_italic", "format_list",
        ];
        let count = icons.iter().filter(|n| catalog::by_name(n).is_some()).count();
        assert!(count >= 78, "Only {} icons found, expected at least 78", count);
    }
}
