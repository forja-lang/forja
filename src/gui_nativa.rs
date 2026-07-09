// Forja GUI Nativa — construye widgets xilem directamente desde el AST
// con soporte completo de tema Material You
#![allow(dead_code)]

use std::collections::HashMap;
use crate::ast::*;
use forja_gui_rt::*;
use forja_gui_rt::{map_message, MessageResult};
use forja_gui_rt::view::{self, Axis};
use forja_gui_rt::Length;
use forja_gui_rt::{FontWeight, palette};
use forja_gui_rt::{
    MaterialTheme,       // Tema Material You completo
    ColorScheme,         // Esquema de color con roles
    RgbColor,            // Color RGB (convierte a xilem::Color vía From)
    TypeScale,           // Escala tipográfica
    TextStyle,           // Estilo de texto individual
    ShapeSystem,         // Sistema de formas (radios de borde)
    ShapeFamily,         // Familia de componentes para formas
    VariableStore,       // Store reactivo de variables
};
use forja_gui_rt::icons;

#[derive(Debug, Clone)]
pub enum ValorGUI {
    Texto(String),
    Entero(i64),
    Decimal(f64),
    Booleano(bool),
    Nulo,
}

impl ValorGUI {
    fn to_string(&self) -> String {
        match self {
            ValorGUI::Texto(s) => s.clone(),
            ValorGUI::Entero(n) => n.to_string(),
            ValorGUI::Decimal(f) => f.to_string(),
            ValorGUI::Booleano(b) => if *b { "verdadero".to_string() } else { "falso".to_string() },
            ValorGUI::Nulo => "nulo".to_string(),
        }
    }

    fn to_f64(&self) -> f64 {
        match self {
            ValorGUI::Entero(n) => *n as f64,
            ValorGUI::Decimal(f) => *f,
            ValorGUI::Texto(s) => s.parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    fn to_bool(&self) -> bool {
        match self {
            ValorGUI::Booleano(b) => *b,
            ValorGUI::Texto(s) => s == "verdadero" || s == "true",
            ValorGUI::Entero(n) => *n != 0,
            _ => false,
        }
    }
}

impl From<&str> for ValorGUI {
    fn from(s: &str) -> Self { ValorGUI::Texto(s.to_string()) }
}

#[derive(Debug, Clone)]
pub struct AppStateNativo {
    pub store: VariableStore,
    pub window_size: WindowSizeClass,
    pub window_width: f64,
}

// ─── Conversión ValorGUI ↔ serde_json::Value ─────────────────────

impl From<ValorGUI> for serde_json::Value {
    fn from(val: ValorGUI) -> Self {
        match val {
            ValorGUI::Texto(s) => serde_json::Value::String(s),
            ValorGUI::Entero(n) => serde_json::Value::Number(n.into()),
            ValorGUI::Decimal(f) => serde_json::Value::String(f.to_string()),
            ValorGUI::Booleano(b) => serde_json::Value::Bool(b),
            ValorGUI::Nulo => serde_json::Value::Null,
        }
    }
}

impl From<serde_json::Value> for ValorGUI {
    fn from(val: serde_json::Value) -> Self {
        match val {
            serde_json::Value::String(s) => ValorGUI::Texto(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ValorGUI::Entero(i)
                } else if let Some(f) = n.as_f64() {
                    ValorGUI::Decimal(f)
                } else {
                    ValorGUI::Nulo
                }
            }
            serde_json::Value::Bool(b) => ValorGUI::Booleano(b),
            serde_json::Value::Null => ValorGUI::Nulo,
            _ => ValorGUI::Nulo,
        }
    }
}

impl AppStateNativo {
    pub fn new() -> Self {
        AppStateNativo {
            store: VariableStore::new(),
            window_size: WindowSizeClass::Compact,
            window_width: 800.0, // Valor por defecto razonable
        }
    }
    
    /// Actualiza el tamaño de ventana y la clase de tamaño
    pub fn update_window_size(&mut self, width: f64) {
        self.window_width = width;
        self.window_size = WindowSizeClass::from_width(width);
    }
    
    pub fn leer(&self, nombre: &str) -> ValorGUI {
        self.store.get(nombre)
            .map(ValorGUI::from)
            .unwrap_or(ValorGUI::Nulo)
    }
    
    pub fn escribir(&mut self, nombre: &str, valor: ValorGUI) {
        let json_val: serde_json::Value = valor.into();
        self.store.set(nombre, json_val);
    }
}

impl Default for AppStateNativo {
    fn default() -> Self { Self::new() }
}

// ─── Window Size Class (Material Design 3) ─────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowSizeClass {
    Compact,  // < 600dp
    Medium,   // 600-840dp
    Expanded, // > 840dp
}

impl WindowSizeClass {
    pub fn from_width(width: f64) -> Self {
        if width < 600.0 {
            WindowSizeClass::Compact
        } else if width < 840.0 {
            WindowSizeClass::Medium
        } else {
            WindowSizeClass::Expanded
        }
    }
}

// ─── Variantes de botón Material Design 3 ────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ButtonVariant {
    Filled,
    Tonal,
    Outlined,
    Text,
    Elevated,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FabSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IconButtonVariant {
    Standard,
    Filled,
    Tonal,
    Outlined,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChipVariant {
    Assist,
    Filter,
    Input,
    Suggestion,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextFieldVariant {
    Filled,
    Outlined,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CardVariant {
    Filled,
    Elevated,
    Outlined,
    Selectable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SheetVariant {
    Standard,
    Modal,
    Expanded,
}

// ─── Navegación Material You ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NavItem {
    pub icono: String,
    pub label: String,
    pub badge: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IconAction {
    pub icono: String,
    pub callback: String,
}

// ─── Navigator (sistema de navegación por pantallas) ──────────────

/// Una pantalla dentro del Navigator
#[derive(Debug)]
pub struct NavigatorScreen {
    /// Identificador único de la pantalla
    pub id: String,
    /// Título mostrado en la barra de navegación/tabs
    pub titulo: String,
    /// Icono opcional (para NavigationBar/Tabs)
    pub icono: Option<String>,
    /// Contenido de la pantalla
    pub contenido: Box<Layout>,
    /// Badge opcional (notificaciones)
    pub badge: Option<String>,
}

impl NavigatorScreen {
    pub fn new(id: &str, titulo: &str, contenido: Layout) -> Self {
        NavigatorScreen {
            id: id.to_string(),
            titulo: titulo.to_string(),
            icono: None,
            contenido: Box::new(contenido),
            badge: None,
        }
    }
}

/// Tipo de navegación visual del Navigator
#[derive(Debug, Clone, PartialEq)]
pub enum NavigatorType {
    /// Solo las pantallas, sin barra de navegación
    None,
    /// NavigationBar inferior (mobile style)
    BottomBar,
    /// NavigationRail lateral
    Rail,
    /// Pestañas superiores
    Tabs,
    /// Cajón lateral (Drawer)
    Drawer,
}

/// Animación de transición entre pantallas
#[derive(Debug, Clone, PartialEq)]
pub enum NavigatorAnim {
    /// Sin animación
    None,
    /// Fundido
    Fade,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopAppBarVariant {
    Small,
    Medium,
    Large,
}

// ─── Avatar variant ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AvatarVariant {
    Text,
    Icon,
    Image,
}

// ─── Layout (representación intermedia) ───────────────────────────

#[derive(Debug)]
enum Layout {
    // ─── Layouts básicos ──────────────────────────────────────────
    Column { children: Vec<Layout>, gap: f64, alignment: String },
    CenteredColumn(Vec<Layout>),
    Row { children: Vec<Layout>, gap: f64, alignment: String },
    ZStack(Vec<Layout>),
    Portal(Box<Layout>),
    Container { child: Box<Layout>, max_width: f64 },
    Label { texto: String, es_variable: bool },
    VariableLabel { variable: String },
    Title(String),
    ColoredLabel { texto: String, color: String },
    // ─── Variantes de tema Material You ───────────────────────────
    /// Proveedor de tema: cambia el color semilla para los hijos
    ThemeProvider { child: Box<Layout>, theme: String },
    /// Aplica un color del esquema al hijo (primary, secondary, etc.)
    ColoredBox { child: Box<Layout>, color_role: String },
    /// Etiqueta con estilo tipográfico predefinido (display_large, body_medium, etc.)
    StyledLabel { texto: String, style: String },
    /// Aplica una familia de forma (border-radius) al hijo
    ShapedBox { child: Box<Layout>, shape_family: String },
    /// Aplica elevación (sombra) al hijo
    ElevatedBox { child: Box<Layout>, level: u8 },
    /// Aplica un estado visual al hijo (hover, pressed, etc.)
    StateLayerBox { child: Box<Layout>, state: String },
    /// Layout responsivo con 3 variantes
    ResponsiveLayout { compact: Box<Layout>, medium: Box<Layout>, expanded: Box<Layout> },
    /// Aplica padding alrededor del hijo
    Padding { child: Box<Layout>, amount: f64 },
    /// Hace que el hijo se expanda para llenar el espacio disponible
    Expanded { child: Box<Layout> },
    /// Centra al hijo en el eje transversal
    Centered { child: Box<Layout> },
    Button { texto: String, callback: String },
    TextInput { variable: String, multiline: bool, placeholder: String },
    ProgressBar { variable: String },
    Slider { variable: String, min: f64, max: f64 },
    Checkbox { variable: String },
    Prose(String),
    Spinner,
    Separator,
    Spacer(f64),
    // ─── Layout responsive avanzado ───────────────────────────────
    /// Flex layout con gap, axis y wrap configurables
    FlexLayout { children: Vec<Layout>, axis: String, gap: f64, wrap: bool },
    /// Flow layout con gap y wrap automático
    FlowLayout { children: Vec<Layout>, gap: f64 },
    /// Caja con relación de aspecto fija
    AspectRatioBox { child: Box<Layout>, ratio: f64 },
    // ─── Botones Material Design 3 ──────────────────────────────────
    MaterialButton {
        texto: String,
        callback: String,
        variant: ButtonVariant,
        icono: Option<String>,
        disabled: bool,
    },
    FAB {
        icono: String,
        callback: String,
        size: FabSize,
        texto_extendido: Option<String>,
    },
    IconButton {
        icono: String,
        callback: String,
        variant: IconButtonVariant,
        seleccionado: bool,
    },
    SegmentedButton {
        opciones: Vec<String>,
        seleccionados: Vec<bool>,
        callback: String,
        multiple: bool,
    },
    Chip {
        texto: String,
        callback: String,
        variant: ChipVariant,
        activo: bool,
        on_remove: Option<String>,
    },

    // ─── Inputs Material Design 3 ────────────────────────────────
    MaterialTextField {
        variable: String,
        label: String,
        placeholder: String,
        variant: TextFieldVariant,
        multiline: bool,
        error: String,
        counter: bool,
    },
    MaterialPasswordField {
        variable: String,
        label: String,
        visible: bool,
    },
    MaterialNumberField {
        variable: String,
        label: String,
        min: f64,
        max: f64,
        decimales: i32,
    },
    MaterialSearchField {
        variable: String,
        placeholder: String,
    },
    MaterialDropdown {
        opciones: Vec<String>,
        seleccionada: usize,
        placeholder: String,
    },
    MaterialSelect {
        opciones: Vec<String>,
        seleccionada: usize,
        label: String,
    },
    MaterialAutocomplete {
        opciones: Vec<String>,
        variable: String,
    },
    MaterialRadioGroup {
        nombre: String,
        opciones: Vec<String>,
        seleccion: usize,
        callback: String,
        direction: String,
    },
    MaterialSwitch {
        label: String,
        variable: String,
    },
    MaterialSliderDiscrete {
        variable: String,
        min: f64,
        max: f64,
        steps: i32,
    },
    MaterialSliderRange {
        variable_inicio: String,
        variable_fin: String,
        min: f64,
        max: f64,
    },
    MaterialChipGroup {
        chips: Vec<String>,
        seleccion: Vec<bool>,
        callback: String,
        multiple: bool,
    },
    MaterialDatePicker {
        variable: String,
    },
    MaterialTimePicker {
        variable: String,
    },

    // ─── Tarjetas, Listas y Tablas ──────────────────────────────
    MaterialCard {
        child: Box<Layout>,
        variant: CardVariant,
        on_click: Option<String>,
        seleccionado: bool,
    },
    MaterialListItem {
        leading: Option<Box<Layout>>,
        titulo: String,
        subtitulo: Option<String>,
        trailing: Option<Box<Layout>>,
        on_click: Option<String>,
    },
    MaterialList {
        items: Vec<Layout>,
        dividers: bool,
    },
    MaterialListControl {
        items: Vec<Layout>,
        control_type: String,
        variables: Vec<String>,
    },
    MaterialListSelection {
        items: Vec<Layout>,
        seleccion: Vec<bool>,
        callback: String,
        multiple: bool,
    },
    MaterialDataTable {
        columnas: Vec<String>,
        filas: Vec<Vec<String>>,
        ordenable: bool,
        seleccionable: bool,
        col_orden: usize,
        orden_asc: bool,
    },
    MaterialSurface {
        child: Box<Layout>,
        color_role: String,
    },
    MaterialScaffold {
        top: Option<Box<Layout>>,
        body: Box<Layout>,
        bottom: Option<Box<Layout>>,
        fab: Option<Box<Layout>>,
    },

    // ─── Feedback y Superposiciones ──────────────────────────────

    // Dialogs
    DialogOverlay {
        dialog: Box<Layout>,
        visible: String, // nombre variable bool
    },
    DialogAlert {
        titulo: String,
        mensaje: String,
        confirmar_texto: String,
        cancelar_texto: String,
        on_confirm: String,
        on_cancel: String,
    },
    DialogCustom {
        titulo: String,
        child: Box<Layout>,
        on_close: String,
    },

    // Bottom Sheets
    BottomSheet {
        child: Box<Layout>,
        variant: SheetVariant, // Standard | Modal | Expanded
        visible: String,
        on_dismiss: Option<String>,
    },

    // Snackbar
    Snackbar {
        mensaje: String,
        accion_texto: Option<String>,
        accion_callback: Option<String>,
        duracion: f64, // en ms
        visible: String,
    },

    // Tooltip
    Tooltip {
        child: Box<Layout>,
        texto: String,
    },

    // Menu
    Menu {
        items: Vec<String>,
        on_select: String,
        visible: String,
    },
    ContextMenu {
        items: Vec<String>,
        on_select: String,
        visible: String,
    },

    // === NAVEGACIÓN ===
    /// Navigator: sistema de navegación por pantallas con push/pop
    Navigator {
        screens: Vec<NavigatorScreen>,
        /// Variable que almacena el ID de la pantalla actual
        current_var: String,
        /// Variable para pila de navegación (historial)
        history_var: String,
        /// Tipo de navegación visual
        nav_type: NavigatorType,
        /// Animación de transición
        anim: NavigatorAnim,
    },
    NavigationBar {
        items: Vec<NavItem>,
        seleccion: usize,
        on_change: String,
    },
    NavigationRail {
        items: Vec<NavItem>,
        seleccion: usize,
        on_change: String,
        extended: bool, // muestra labels
    },
    NavigationDrawer {
        items: Vec<NavItem>,
        seleccion: usize,
        on_change: String,
        modal: bool,
        visible: String,
    },
    TopAppBar {
        titulo: String,
        acciones: Vec<IconAction>,
        menu_visible: bool,
        variant: TopAppBarVariant, // Small | Medium | Large
    },
    BottomAppBar {
        acciones: Vec<IconAction>,
        fab: Option<Box<Layout>>,
    },
    Tabs {
        tabs: Vec<String>,
        seleccion: usize,
        on_change: String,
        scrollable: bool,
    },
    SearchBar {
        placeholder: String,
        on_search: String,
        variable: String,
    },
    SearchView {
        query: String,
        resultados: Vec<Layout>,
        visible: String,
    },

    // ═══════════════════════════════════════════════════════════════════
    // INDICADORES (Fase 7)
    // ═══════════════════════════════════════════════════════════════════

    /// Barra de progreso lineal
    LinearProgress { variable: String, indeterminado: bool },
    /// Círculo de progreso
    CircularProgress { variable: String, size: f64, indeterminado: bool },
    /// Badge (distintivo) sobre un hijo
    Badge { child: Box<Layout>, valor: Option<String>, dot: bool },
    /// Skeleton (placeholder de carga)
    Skeleton { ancho: f64, alto: f64, tipo: String },
    /// Empty state (estado vacío)
    EmptyState { icono: String, mensaje: String, accion_texto: Option<String>, accion_cb: Option<String> },
    /// Error state (estado de error)
    ErrorState { mensaje: String, on_retry: Option<String> },

    // ═══════════════════════════════════════════════════════════════════
    // AVATARES (Fase 7)
    // ═══════════════════════════════════════════════════════════════════

    /// Avatar individual
    Avatar { texto: String, variant: AvatarVariant, tamaño: f64 },
    /// Grupo de avatares
    AvatarGroup { avatares: Vec<String>, max: usize },

    // ═══════════════════════════════════════════════════════════════════
    // MOTION (Fase 8)
    // ═══════════════════════════════════════════════════════════════════

    /// Transición Fade (muestra/oculta con opacidad)
    FadeTransition { child: Box<Layout>, visible: String, duracion: f64 },
    /// Efecto ripple (onda al hacer clic)
    RippleEffect { child: Box<Layout>, color: String },

    // ═══════════════════════════════════════════════════════════════════
    // INTERACCIONES (Pull-to-refresh, Swipe-to-dismiss)
    // ═══════════════════════════════════════════════════════════════════

    /// Pull-to-refresh: envuelve un hijo con capacidad de recargar al tirar hacia abajo
    PullToRefresh {
        child: Box<Layout>,
        callback: String,
        refreshing: String,
    },
    /// Swipe-to-dismiss: deslizar para descartar con opción de deshacer
    SwipeToDismiss {
        child: Box<Layout>,
        on_dismiss: String,
        label: String,
        dismissed: String,
    },

    // ═══════════════════════════════════════════════════════════════════
    // GRÁFICOS (Fase 9)
    // ═══════════════════════════════════════════════════════════════════

    /// Gráfico de líneas
    LineChart { datos: Vec<f64>, color: String, etiquetas: Vec<String> },
    /// Gráfico de barras
    BarChart { datos: Vec<f64>, colores: Vec<String>, etiquetas: Vec<String>, apilado: bool },
    /// Gráfico de pastel / donut
    PieChart { datos: Vec<f64>, etiquetas: Vec<String>, donut: bool },
    /// Gráfico indicador tipo Gauge
    GaugeChart { valor: f64, min: f64, max: f64, color: String },
    /// Mini gráfico (Sparkline)
    Sparkline { datos: Vec<f64>, color: String },

    // ═══════════════════════════════════════════════════════════════════
    // AVANZADOS (Fase 9)
    // ═══════════════════════════════════════════════════════════════════

    /// Calificación por estrellas
    StarRating { valor: usize, max: usize, callback: String },
    /// Asistente de pasos (Stepper)
    Stepper { pasos: Vec<String>, actual: usize, callback: String },
    /// Migas de pan (Breadcrumbs)
    Breadcrumbs { items: Vec<String>, separador: String },
    /// Calendario
    Calendar { mes: i32, año: i32, seleccionado: Option<String>, callback: String },
    /// Visor de Markdown
    MarkdownViewer { texto: String },
    /// Generador de QR
    QRCode { texto: String, tamaño: f64 },
    /// Selector de archivos
    FilePicker { tipos: Vec<String>, multiple: bool, callback: String },

    // ═══════════════════════════════════════════════════════════════════
    // EXPRESSIVE (Fase 10)
    // ═══════════════════════════════════════════════════════════════════

    /// Tarjeta con efecto vidrio (Glassmorphism)
    GlassCard { child: Box<Layout>, blur: f64, opacity: f64 },
    /// Caja con gradiente
    GradientBox { child: Box<Layout>, colores: Vec<String>, direccion: String },
    /// Botón morphing (icono → texto)
    MorphingButton { icono: String, texto_extendido: String, callback: String },
    /// Fondo expresivo animado
    ExpressiveBackground { colores: Vec<String>, animado: bool },
    /// Borde con brillo (Glow)
    GlowBorder { child: Box<Layout>, color: String, ancho: f64 },

    // ═══════════════════════════════════════════════════════════════════
    // ICONOS MATERIAL (Fase Iconos)
    // ═══════════════════════════════════════════════════════════════════

    /// Icono Material Design vectorial
    MaterialIconLayout {
        nombre: String,       // nombre del icono: "home", "favorite"
        tamaño: f64,           // tamaño en píxeles (ej: 24, 32, 48)
        color: String,         // color (hex o role del tema)
        estilo: String,        // "filled", "outlined", "rounded", "sharp", "twotone"
    },
}

// ─── AST → Layout ─────────────────────────────────────────────────

/// Extrae el layout del AST (recursivo: soporta desplazable, pila, etc.)
fn extraer_layout(decls: &[Declaracion]) -> Layout {
    for decl in decls {
        if let Declaracion::Funcion { nombre, cuerpo, .. } = decl {
            if nombre == "main" {
                for d in cuerpo {
                    // Convertir Declaracion::LlamadaFuncion → Expresion::LlamadaFuncion
                    // para usar expr_a_layout que ya maneja todos los wrappers
                    if let Declaracion::LlamadaFuncion { nombre, argumentos } = d {
                        let expr = Expresion::LlamadaFuncion {
                            nombre: nombre.clone(),
                            argumentos: argumentos.clone(),
                        };
                        if let Some(layout) = expr_a_layout(&expr) {
                            return layout;
                        }
                    }
                }
            }
        }
    }
    Layout::Column { children: vec![], gap: 0.0, alignment: "start".to_string() }
}

fn procesar_args(args: &[Expresion]) -> Vec<Layout> {
    args.iter().filter_map(expr_a_layout).collect()
}

// ─── Helpers para extraer argumentos de funciones Forja ─────────────

fn extraer_texto(args: &[Expresion], index: usize) -> String {
    args.get(index)
        .map(|a| match a {
            Expresion::LiteralTexto(s) => s.clone(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn extraer_callback(args: &[Expresion], index: usize) -> String {
    args.get(index)
        .map(|a| match a {
            Expresion::Referencia { expr, .. } => match expr.as_ref() {
                Expresion::Identificador(n, ..) => n.clone(),
                _ => String::new(),
            },
            Expresion::Identificador(n, ..) => n.clone(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn extraer_booleano(args: &[Expresion], index: usize) -> bool {
    args.get(index)
        .and_then(|a| match a {
            Expresion::LiteralBooleano(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false)
}

fn extraer_array_strings(args: &[Expresion], index: usize) -> Vec<String> {
    args.get(index)
        .and_then(|a| match a {
            Expresion::Arreglo(exprs) => Some(
                exprs.iter().filter_map(|e| match e {
                    Expresion::LiteralTexto(s) => Some(s.clone()),
                    _ => None,
                }).collect::<Vec<_>>()
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn extraer_array_bool(args: &[Expresion], index: usize) -> Vec<bool> {
    args.get(index)
        .and_then(|a| match a {
            Expresion::Arreglo(exprs) => Some(
                exprs.iter().filter_map(|e| match e {
                    Expresion::LiteralBooleano(b) => Some(*b),
                    _ => None,
                }).collect::<Vec<_>>()
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn extraer_array_arrays_strings(args: &[Expresion], index: usize) -> Vec<Vec<String>> {
    args.get(index)
        .and_then(|a| match a {
            Expresion::Arreglo(exprs) => Some(
                exprs.iter().filter_map(|e| match e {
                    Expresion::Arreglo(inner) => Some(
                        inner.iter().filter_map(|x| match x {
                            Expresion::LiteralTexto(s) => Some(s.clone()),
                            _ => None,
                        }).collect::<Vec<_>>()
                    ),
                    _ => None,
                }).collect::<Vec<_>>()
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn extraer_nav_items(args: &[Expresion], index: usize) -> Vec<NavItem> {
    args.get(index)
        .and_then(|a| match a {
            Expresion::Arreglo(exprs) => Some(
                exprs.iter().filter_map(|e| match e {
                    Expresion::LlamadaFuncion { nombre, argumentos } if nombre == "item_navegacion" => {
                        let icono = argumentos.first()
                            .map(|a| match a {
                                Expresion::LiteralTexto(s) => s.clone(),
                                _ => String::new(),
                            }).unwrap_or_default();
                        let label = argumentos.get(1)
                            .map(|a| match a {
                                Expresion::LiteralTexto(s) => s.clone(),
                                _ => String::new(),
                            }).unwrap_or_default();
                        let badge = argumentos.get(2)
                            .map(|a| match a {
                                Expresion::LiteralTexto(s) => Some(s.clone()),
                                _ => None,
                            }).unwrap_or(None);
                        Some(NavItem { icono, label, badge })
                    }
                    _ => None,
                }).collect::<Vec<_>>()
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn extraer_navigator_screens(args: &[Expresion], index: usize) -> Vec<NavigatorScreen> {
    args.get(index)
        .and_then(|a| match a {
            Expresion::Arreglo(exprs) => Some(
                exprs.iter().filter_map(|e| match e {
                    Expresion::LlamadaFuncion { nombre, argumentos } if nombre == "pantalla" || nombre == "screen" => {
                        let id = argumentos.first()
                            .map(|a| match a {
                                Expresion::LiteralTexto(s) => s.clone(),
                                _ => String::new(),
                            }).unwrap_or_default();
                        let titulo = argumentos.get(1)
                            .map(|a| match a {
                                Expresion::LiteralTexto(s) => s.clone(),
                                _ => String::new(),
                            }).unwrap_or_default();
                        let contenido = argumentos.get(2)
                            .and_then(expr_a_layout)
                            .unwrap_or(Layout::Spacer(0.0));
                        let icono = argumentos.get(3)
                            .map(|a| match a {
                                Expresion::LiteralTexto(s) => Some(s.clone()),
                                _ => None,
                            }).unwrap_or(None);
                        Some(NavigatorScreen {
                            id,
                            titulo,
                            icono,
                            contenido: Box::new(contenido),
                            badge: None,
                        })
                    }
                    _ => None,
                }).collect::<Vec<_>>()
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn extraer_icon_actions(args: &[Expresion], index: usize) -> Vec<IconAction> {
    args.get(index)
        .and_then(|a| match a {
            Expresion::Arreglo(exprs) => Some(
                exprs.iter().filter_map(|e| match e {
                    Expresion::LlamadaFuncion { nombre, argumentos }
                        if nombre == "boton_icono" || nombre == "icon_button" =>
                    {
                        let icono = argumentos.first()
                            .map(|a| match a {
                                Expresion::LiteralTexto(s) => s.clone(),
                                _ => String::new(),
                            }).unwrap_or_default();
                        let callback = argumentos.get(1)
                            .map(|a| match a {
                                Expresion::Referencia { expr, .. } => match expr.as_ref() {
                                    Expresion::Identificador(n, ..) => n.clone(),
                                    _ => String::new(),
                                },
                                Expresion::Identificador(n, ..) => n.clone(),
                                _ => String::new(),
                            }).unwrap_or_default();
                        Some(IconAction { icono, callback })
                    }
                    _ => None,
                }).collect::<Vec<_>>()
            ),
            _ => {
                let result = args.get(index).and_then(|e| match e {
                    Expresion::LlamadaFuncion { nombre, argumentos }
                        if nombre == "boton_icono" || nombre == "icon_button" =>
                    {
                        let icono = argumentos.first()
                            .map(|a| match a {
                                Expresion::LiteralTexto(s) => s.clone(),
                                _ => String::new(),
                            }).unwrap_or_default();
                        let callback = argumentos.get(1)
                            .map(|a| match a {
                                Expresion::Referencia { expr, .. } => match expr.as_ref() {
                                    Expresion::Identificador(n, ..) => n.clone(),
                                    _ => String::new(),
                                },
                                Expresion::Identificador(n, ..) => n.clone(),
                                _ => String::new(),
                            }).unwrap_or_default();
                        Some(vec![IconAction { icono, callback }])
                    }
                    _ => None,
                });
                result
            }
        })
        .unwrap_or_default()
}

fn extraer_f64(args: &[Expresion], index: usize) -> f64 {
    args.get(index)
        .and_then(|a| match a {
            Expresion::LiteralNumero(n) => Some(*n as f64),
            Expresion::LiteralDecimal(f) => Some(*f),
            _ => None,
        })
        .unwrap_or(0.0)
}

fn extraer_usize(args: &[Expresion], index: usize) -> usize {
    args.get(index)
        .and_then(|a| match a {
            Expresion::LiteralNumero(n) => Some(*n as usize),
            _ => None,
        })
        .unwrap_or(0)
}

fn extraer_array_f64(args: &[Expresion], index: usize) -> Vec<f64> {
    args.get(index)
        .and_then(|a| match a {
            Expresion::Arreglo(exprs) => Some(
                exprs.iter().filter_map(|e| match e {
                    Expresion::LiteralNumero(n) => Some(*n as f64),
                    Expresion::LiteralDecimal(f) => Some(*f),
                    _ => None,
                }).collect::<Vec<_>>()
            ),
            Expresion::LiteralNumero(n) => Some(vec![*n as f64]),
            Expresion::LiteralDecimal(f) => Some(vec![*f]),
            _ => None,
        })
        .unwrap_or_default()
}

// ─── AST → Layout ─────────────────────────────────────────────────

fn expr_a_layout(expr: &Expresion) -> Option<Layout> {
    match expr {
        Expresion::LlamadaFuncion { nombre, argumentos } => {
            match nombre.as_str() {
                "escribir" | "etiqueta" | "label" | "text" => {
                    if let Some(arg) = argumentos.first() {
                        match arg {
Expresion::Identificador(v, ..) =>
                                Some(Layout::Label { texto: v.clone(), es_variable: true }),
                            Expresion::LiteralTexto(s) =>
                                Some(Layout::Label { texto: s.clone(), es_variable: false }),
                            _ => Some(Layout::Spacer(0.0)),
                        }
                    } else { Some(Layout::Spacer(0.0)) }
                }
                "etiqueta_titulo" | "titulo" | "title" => {
                    let texto = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::Title(texto))
                }
                "etiqueta_color" | "texto_color" | "colored_label" => {
                    let texto = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    let color = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => "defecto".to_string(),
                        }).unwrap_or_else(|| "defecto".to_string());
                    Some(Layout::ColoredLabel { texto, color })
                }
                "etiqueta_dinamica" | "varlabel" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::Identificador(s, ..) => s.clone(),
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::VariableLabel { variable })
                }
                "boton" | "button" | "btn" => {
                    let texto = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::MaterialButton {
                        texto,
                        callback,
                        variant: ButtonVariant::Filled,
                        icono: None,
                        disabled: false,
                    })
                }
                "entrada_texto" | "text_input" | "input" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s, ..) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    let placeholder = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::TextInput { variable, multiline: false, placeholder })
                }
                "area_texto" | "textarea" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s, ..) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    let placeholder = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::TextInput { variable, multiline: true, placeholder })
                }
                "gui_barra_progreso" | "progress_bar" | "progress" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s, ..) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::ProgressBar { variable })
                }
                "deslizante" | "gui_deslizante" | "slider" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s, ..) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    let min = argumentos.get(1)
                        .and_then(|a| match a { Expresion::LiteralNumero(n) => Some(*n as f64), _ => None })
                        .unwrap_or(0.0);
                    let max = argumentos.get(2)
                        .and_then(|a| match a { Expresion::LiteralNumero(n) => Some(*n as f64), _ => None })
                        .unwrap_or(100.0);
                    Some(Layout::Slider { variable, min, max })
                }
                "casilla" | "gui_casilla" | "checkbox" | "check" => {
                    let variable = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s, ..) => s.clone(),
                            _ => String::new(),
                        }).or_else(|| {
                            argumentos.first().map(|a| match a {
                                Expresion::LiteralTexto(s) => s.clone(),
                                Expresion::Identificador(s, ..) => s.clone(),
                                _ => String::new(),
                            })
                        }).unwrap_or_default();
                    Some(Layout::Checkbox { variable })
                }
                "texto_enriquecido" | "prose" => {
                    let texto = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::Prose(texto))
                }
                "cargando" | "spinner" => {
                    Some(Layout::Spinner)
                }
                "separador" | "divider" => {
                    Some(Layout::Separator)
                }
                "espacio" | "spacer" => {
                    let tamano = argumentos.first()
                        .and_then(|a| match a { Expresion::LiteralNumero(n) => Some(*n as f64), _ => None })
                        .unwrap_or(10.0);
                    Some(Layout::Spacer(tamano))
                }
                "columna" | "gui_columna" => Some(Layout::Column { children: procesar_args(argumentos), gap: 0.0, alignment: "start".to_string() }),
                "columna_centrada" | "centered_column" | "center_col" => {
                    Some(Layout::CenteredColumn(procesar_args(argumentos)))
                }
                "fila" | "gui_fila" => Some(Layout::Row { children: procesar_args(argumentos), gap: 0.0, alignment: "start".to_string() }),
                "pila" | "gui_pila" | "zstack" => Some(Layout::ZStack(procesar_args(argumentos))),
                "desplazable" | "gui_desplazable" | "scroll" => {
                    argumentos.first().and_then(|a| expr_a_layout(a))
                        .map(|child| Layout::Portal(Box::new(child)))
                }
                "contenedor" | "container" | "caja" => {
                    let child = argumentos.first().and_then(|a| expr_a_layout(a));
                    let max_width = argumentos.get(1)
                        .and_then(|a| match a { Expresion::LiteralNumero(n) => Some(*n as f64), _ => None })
                        .unwrap_or(400.0);
                    child.map(|c| Layout::Container { child: Box::new(c), max_width })
                }
                // ─── Funciones de tema Material You ────────────────
                // Proveedor de tema: cambia el color semilla
                "tema_material" | "theme_provider" => {
                    let seed = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => "#6750A4".to_string(),
                        }).unwrap_or_else(|| "#6750A4".to_string());
                    let child = argumentos.get(1).and_then(expr_a_layout);
                    child.map(|c| Layout::ThemeProvider { child: Box::new(c), theme: seed })
                }
                // Colores del tema (envuelven un hijo con un color role)
                "color_primario" | "color_primary" =>
                    wrap_with_color_role(argumentos, "primary"),
                "color_sobre_primario" | "color_on_primary" =>
                    wrap_with_color_role(argumentos, "on_primary"),
                "color_primario_contenedor" | "color_primary_container" =>
                    wrap_with_color_role(argumentos, "primary_container"),
                "color_sobre_primario_contenedor" | "color_on_primary_container" =>
                    wrap_with_color_role(argumentos, "on_primary_container"),
                "color_secundario" | "color_secondary" =>
                    wrap_with_color_role(argumentos, "secondary"),
                "color_sobre_secundario" | "color_on_secondary" =>
                    wrap_with_color_role(argumentos, "on_secondary"),
                "color_secundario_contenedor" | "color_secondary_container" =>
                    wrap_with_color_role(argumentos, "secondary_container"),
                "color_sobre_secundario_contenedor" | "color_on_secondary_container" =>
                    wrap_with_color_role(argumentos, "on_secondary_container"),
                "color_terciario" | "color_tertiary" =>
                    wrap_with_color_role(argumentos, "tertiary"),
                "color_sobre_terciario" | "color_on_tertiary" =>
                    wrap_with_color_role(argumentos, "on_tertiary"),
                "color_error" | "color_error_role" =>
                    wrap_with_color_role(argumentos, "error"),
                "color_sobre_error" | "color_on_error" =>
                    wrap_with_color_role(argumentos, "on_error"),
                "color_superficie" | "color_surface" =>
                    wrap_with_color_role(argumentos, "surface"),
                "color_sobre_superficie" | "color_on_surface" =>
                    wrap_with_color_role(argumentos, "on_surface"),
                "color_fondo" | "color_background" =>
                    wrap_with_color_role(argumentos, "background"),
                "color_sobre_fondo" | "color_on_background" =>
                    wrap_with_color_role(argumentos, "on_background"),
                "color_perfil" | "color_outline" =>
                    wrap_with_color_role(argumentos, "outline"),

                // Estilos tipográficos (15 estilos Material You)
                "texto_grande" | "display_large" =>
                    styled_label(argumentos, "display_large"),
                "texto_mediano" | "display_medium" =>
                    styled_label(argumentos, "display_medium"),
                "texto_pequeño" | "display_small" =>
                    styled_label(argumentos, "display_small"),
                "titular_grande" | "headline_large" =>
                    styled_label(argumentos, "headline_large"),
                "titular_mediano" | "headline_medium" =>
                    styled_label(argumentos, "headline_medium"),
                "titular_pequeño" | "headline_small" =>
                    styled_label(argumentos, "headline_small"),
                "encabezado_grande" | "title_large" =>
                    styled_label(argumentos, "title_large"),
                "encabezado_mediano" | "title_medium" =>
                    styled_label(argumentos, "title_medium"),
                "encabezado_pequeño" | "title_small" =>
                    styled_label(argumentos, "title_small"),
                "cuerpo_grande" | "body_large" =>
                    styled_label(argumentos, "body_large"),
                "cuerpo_mediano" | "body_medium" =>
                    styled_label(argumentos, "body_medium"),
                "cuerpo_pequeño" | "body_small" =>
                    styled_label(argumentos, "body_small"),
                "etiqueta_grande" | "label_large" =>
                    styled_label(argumentos, "label_large"),
                "etiqueta_mediana" | "label_medium" =>
                    styled_label(argumentos, "label_medium"),
                "etiqueta_pequeña" | "label_small" =>
                    styled_label(argumentos, "label_small"),

                // Formas (border-radius)
                "esquinas_pequeñas" | "shape_small" => {
                    wrap_with_shape(argumentos, "small")
                }
                "esquinas_medianas" | "shape_medium" => {
                    wrap_with_shape(argumentos, "medium")
                }
                "esquinas_grandes" | "shape_large" => {
                    wrap_with_shape(argumentos, "large")
                }
                "esquinas_completas" | "shape_full" => {
                    wrap_with_shape(argumentos, "full")
                }

                // Layout avanzado
                "relleno" | "padding" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let amount = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            _ => None,
                        }).unwrap_or(8.0);
                    child.map(|c| Layout::Padding { child: Box::new(c), amount })
                }
                "expansor" | "expanded" => {
                    argumentos.first().and_then(expr_a_layout)
                        .map(|c| Layout::Expanded { child: Box::new(c) })
                }
                "centrado" | "centered" | "center" => {
                    argumentos.first().and_then(expr_a_layout)
                        .map(|c| Layout::Centered { child: Box::new(c) })
                }
                "sombra" | "shadow" | "elevated" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let level = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as u8),
                            _ => None,
                        }).unwrap_or(1);
                    child.map(|c| Layout::ElevatedBox { child: Box::new(c), level })
                }
                "adaptable" | "responsive" => {
                    let compact = argumentos.get(0).and_then(expr_a_layout);
                    let medium = argumentos.get(1).and_then(expr_a_layout);
                    let expanded = argumentos.get(2).and_then(expr_a_layout);
                    match (compact, medium, expanded) {
                        (Some(c), Some(m), Some(e)) =>
                            Some(Layout::ResponsiveLayout {
                                compact: Box::new(c),
                                medium: Box::new(m),
                                expanded: Box::new(e),
                            }),
                        (Some(c), _, _) => Some(c), // fallback: solo compact
                        _ => None,
                    }
                }

                // ─── Layout responsive avanzado ────────────────
                // columna_con_gap(hijos, gap, alinear)
                // hijos se pasa como tupla: (hijo1, hijo2, ...)
                "columna_con_gap" | "column_with_gap" => {
                    let children = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(procesar_args(exprs)),
                            _ => None,
                        }).unwrap_or_default();
                    let gap = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(0.0);
                    let alignment = argumentos.get(2)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => "start".to_string(),
                        }).unwrap_or_else(|| "start".to_string());
                    Some(Layout::Column { children, gap, alignment })
                }

                // fila_con_gap(hijos, gap, alinear)
                "fila_con_gap" | "row_with_gap" => {
                    let children = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(procesar_args(exprs)),
                            _ => None,
                        }).unwrap_or_default();
                    let gap = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(0.0);
                    let alignment = argumentos.get(2)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => "start".to_string(),
                        }).unwrap_or_else(|| "start".to_string());
                    Some(Layout::Row { children, gap, alignment })
                }

                // flujo(hijos, gap) — Flow layout con wrap
                "flujo" | "flow_layout" | "flow" => {
                    let children = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(procesar_args(exprs)),
                            _ => None,
                        }).unwrap_or_default();
                    let gap = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(0.0);
                    Some(Layout::FlowLayout { children, gap })
                }

                // caja_relativa(hijo, proporcion) — Aspect ratio
                "caja_relativa" | "aspect_ratio" | "aspectratio" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let ratio = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(1.0);
                    child.map(|c| Layout::AspectRatioBox { child: Box::new(c), ratio })
                }

                // flex_layout(hijos, axis, gap, wrap) — Flex layout configurable
                "flex_layout" | "flex" => {
                    let children = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(procesar_args(exprs)),
                            _ => None,
                        }).unwrap_or_default();
                    let axis = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => "vertical".to_string(),
                        }).unwrap_or_else(|| "vertical".to_string());
                    let gap = argumentos.get(2)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(0.0);
                    let wrap = argumentos.get(3)
                        .and_then(|a| match a {
                            Expresion::LiteralBooleano(b) => Some(*b),
                            _ => None,
                        }).unwrap_or(false);
                    Some(Layout::FlexLayout { children, axis, gap, wrap })
                }

                // ─── Botones Material Design 3 ──────────────────────
                
                // Variantes principales (5)
                "boton_relleno" | "filled_button" => {
                    let texto = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::MaterialButton {
                        texto,
                        callback,
                        variant: ButtonVariant::Filled,
                        icono: None,
                        disabled: false,
                    })
                }
                "boton_tonal" | "tonal_button" => {
                    let texto = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::MaterialButton {
                        texto,
                        callback,
                        variant: ButtonVariant::Tonal,
                        icono: None,
                        disabled: false,
                    })
                }
                "boton_perfilado" | "outlined_button" => {
                    let texto = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::MaterialButton {
                        texto,
                        callback,
                        variant: ButtonVariant::Outlined,
                        icono: None,
                        disabled: false,
                    })
                }
                "boton_texto" | "text_button_cmd" => {
                    let texto = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::MaterialButton {
                        texto,
                        callback,
                        variant: ButtonVariant::Text,
                        icono: None,
                        disabled: false,
                    })
                }
                "boton_elevado" | "elevated_button" => {
                    let texto = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::MaterialButton {
                        texto,
                        callback,
                        variant: ButtonVariant::Elevated,
                        icono: None,
                        disabled: false,
                    })
                }
                
                // FAB (Floating Action Button)
                "fab" => {
                    let icono = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::FAB {
                        icono,
                        callback,
                        size: FabSize::Medium,
                        texto_extendido: None,
                    })
                }
                "fab_pequeño" | "fab_pequeno" | "fab_small" => {
                    let icono = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::FAB {
                        icono,
                        callback,
                        size: FabSize::Small,
                        texto_extendido: None,
                    })
                }
                "fab_grande" | "fab_large" => {
                    let icono = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::FAB {
                        icono,
                        callback,
                        size: FabSize::Large,
                        texto_extendido: None,
                    })
                }
                "fab_extendido" | "fab_extended" => {
                    let texto = extraer_texto(argumentos, 0);
                    let icono = extraer_texto(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::FAB {
                        icono,
                        callback,
                        size: FabSize::Medium,
                        texto_extendido: Some(texto),
                    })
                }
                
                // Icon Buttons
                "boton_icono" | "icon_button" => {
                    let icono = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::IconButton {
                        icono,
                        callback,
                        variant: IconButtonVariant::Standard,
                        seleccionado: false,
                    })
                }
                "boton_icono_relleno" | "icon_button_filled" => {
                    let icono = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::IconButton {
                        icono,
                        callback,
                        variant: IconButtonVariant::Filled,
                        seleccionado: false,
                    })
                }
                "boton_icono_tonal" | "icon_button_tonal" => {
                    let icono = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::IconButton {
                        icono,
                        callback,
                        variant: IconButtonVariant::Tonal,
                        seleccionado: false,
                    })
                }
                "boton_icono_perfilado" | "icon_button_outlined" => {
                    let icono = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::IconButton {
                        icono,
                        callback,
                        variant: IconButtonVariant::Outlined,
                        seleccionado: false,
                    })
                }
                
                // Segmented Buttons
                "segmentado" | "segmented_button" => {
                    let opciones = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(exprs.iter().filter_map(|e| match e {
                                Expresion::LiteralTexto(s) => Some(s.clone()),
                                _ => None,
                            }).collect::<Vec<_>>()),
                            _ => None,
                        }).unwrap_or_default();
                    let seleccion_idx = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as usize),
                            _ => None,
                        }).unwrap_or(0);
                    let callback = extraer_callback(argumentos, 2);
                    let mut seleccionados = vec![false; opciones.len()];
                    if seleccion_idx < seleccionados.len() {
                        seleccionados[seleccion_idx] = true;
                    }
                    Some(Layout::SegmentedButton {
                        opciones,
                        seleccionados,
                        callback,
                        multiple: false,
                    })
                }
                "segmentado_multiple" | "segmented_button_multiple" => {
                    let opciones = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(exprs.iter().filter_map(|e| match e {
                                Expresion::LiteralTexto(s) => Some(s.clone()),
                                _ => None,
                            }).collect::<Vec<_>>()),
                            _ => None,
                        }).unwrap_or_default();
                    let seleccionados = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(exprs.iter().filter_map(|e| match e {
                                Expresion::LiteralBooleano(b) => Some(*b),
                                _ => None,
                            }).collect::<Vec<_>>()),
                            _ => None,
                        }).unwrap_or_default();
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::SegmentedButton {
                        opciones,
                        seleccionados,
                        callback,
                        multiple: true,
                    })
                }
                
                // Chips
                "subconjunto_asistente" | "chip_assist" => {
                    let texto = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::Chip {
                        texto,
                        callback,
                        variant: ChipVariant::Assist,
                        activo: false,
                        on_remove: None,
                    })
                }
                "subconjunto_filtro" | "chip_filter" => {
                    let texto = extraer_texto(argumentos, 0);
                    let activo = extraer_booleano(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::Chip {
                        texto,
                        callback,
                        variant: ChipVariant::Filter,
                        activo,
                        on_remove: None,
                    })
                }
                "subconjunto_entrada" | "chip_input" => {
                    let texto = extraer_texto(argumentos, 0);
                    let cb_fn = extraer_callback(argumentos, 1);
                    let cb_clone = cb_fn.clone();
                    Some(Layout::Chip {
                        texto,
                        callback: cb_fn,
                        variant: ChipVariant::Input,
                        activo: true,
                        on_remove: Some(cb_clone),
                    })
                }
                "subconjunto_sugerencia" | "chip_suggestion" => {
                    let texto = extraer_texto(argumentos, 0);
                    let callback = extraer_callback(argumentos, 1);
                    Some(Layout::Chip {
                        texto,
                        callback,
                        variant: ChipVariant::Suggestion,
                        activo: false,
                        on_remove: None,
                    })
                }

                // ─── Inputs Material Design 3 ──────────────────────────

                // Text Fields
                "campo_texto" | "campo_texto_error" => {
                    let variable = extraer_texto(argumentos, 0);
                    let label = extraer_texto(argumentos, 1);
                    let error = if nombre == "campo_texto_error" { extraer_texto(argumentos, 2) } else { String::new() };
                    Some(Layout::MaterialTextField {
                        variable,
                        label,
                        placeholder: String::new(),
                        variant: TextFieldVariant::Filled,
                        multiline: false,
                        error,
                        counter: false,
                    })
                }
                "campo_perfilado" => {
                    let variable = extraer_texto(argumentos, 0);
                    let label = extraer_texto(argumentos, 1);
                    Some(Layout::MaterialTextField {
                        variable,
                        label,
                        placeholder: String::new(),
                        variant: TextFieldVariant::Outlined,
                        multiline: false,
                        error: String::new(),
                        counter: false,
                    })
                }
                "campo_email" => {
                    let variable = extraer_texto(argumentos, 0);
                    let label = extraer_texto(argumentos, 1);
                    Some(Layout::MaterialTextField {
                        variable,
                        label,
                        placeholder: "email@ejemplo.com".to_string(),
                        variant: TextFieldVariant::Outlined,
                        multiline: false,
                        error: String::new(),
                        counter: false,
                    })
                }
                "campo_telefono" => {
                    let variable = extraer_texto(argumentos, 0);
                    let label = extraer_texto(argumentos, 1);
                    Some(Layout::MaterialTextField {
                        variable,
                        label,
                        placeholder: "+54 11 1234-5678".to_string(),
                        variant: TextFieldVariant::Outlined,
                        multiline: false,
                        error: String::new(),
                        counter: false,
                    })
                }
                "campo_url" => {
                    let variable = extraer_texto(argumentos, 0);
                    let label = extraer_texto(argumentos, 1);
                    Some(Layout::MaterialTextField {
                        variable,
                        label,
                        placeholder: "https://".to_string(),
                        variant: TextFieldVariant::Outlined,
                        multiline: false,
                        error: String::new(),
                        counter: false,
                    })
                }
                "campo_contraseña" | "campo_contrasena" => {
                    let variable = extraer_texto(argumentos, 0);
                    let label = extraer_texto(argumentos, 1);
                    Some(Layout::MaterialPasswordField {
                        variable,
                        label,
                        visible: false,
                    })
                }
                "campo_numero" => {
                    let variable = extraer_texto(argumentos, 0);
                    let label = extraer_texto(argumentos, 1);
                    let min = extraer_f64(argumentos, 2);
                    let max = extraer_f64(argumentos, 3);
                    Some(Layout::MaterialNumberField {
                        variable,
                        label,
                        min,
                        max,
                        decimales: 0,
                    })
                }
                "campo_busqueda" | "campo_search" => {
                    let variable = extraer_texto(argumentos, 0);
                    Some(Layout::MaterialSearchField {
                        variable,
                        placeholder: "Buscar...".to_string(),
                    })
                }

                // Dropdown & Select
                "contraer_desplegable" | "dropdown" => {
                    let opciones = extraer_array_strings(argumentos, 0);
                    let seleccionada = extraer_usize(argumentos, 1);
                    Some(Layout::MaterialDropdown {
                        opciones,
                        seleccionada,
                        placeholder: "Seleccionar...".to_string(),
                    })
                }
                "menu_seleccion" | "select_menu" => {
                    let opciones = extraer_array_strings(argumentos, 0);
                    let seleccionada = extraer_usize(argumentos, 1);
                    let label = extraer_texto(argumentos, 2);
                    Some(Layout::MaterialSelect {
                        opciones,
                        seleccionada,
                        label,
                    })
                }
                "autocompletar" | "autocomplete" => {
                    let opciones = extraer_array_strings(argumentos, 0);
                    let variable = extraer_texto(argumentos, 1);
                    Some(Layout::MaterialAutocomplete {
                        opciones,
                        variable,
                    })
                }

                // Radio Group
                "grupo_radio" | "radio_group" => {
                    let opciones = extraer_array_strings(argumentos, 0);
                    let seleccion = extraer_usize(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    let direction = extraer_texto(argumentos, 3);
                    Some(Layout::MaterialRadioGroup {
                        nombre: String::new(),
                        opciones,
                        seleccion,
                        callback,
                        direction: if direction.is_empty() { "vertical".to_string() } else { direction },
                    })
                }

                // Switch
                "interruptor" | "switch_widget" => {
                    let label = extraer_texto(argumentos, 0);
                    let variable = extraer_texto(argumentos, 1);
                    Some(Layout::MaterialSwitch {
                        label,
                        variable,
                    })
                }

                // Sliders
                "deslizante_discreto" | "discrete_slider" => {
                    let variable = extraer_texto(argumentos, 0);
                    let min = extraer_f64(argumentos, 1);
                    let max = extraer_f64(argumentos, 2);
                    let steps = extraer_usize(argumentos, 3) as i32;
                    Some(Layout::MaterialSliderDiscrete {
                        variable,
                        min,
                        max,
                        steps,
                    })
                }
                "deslizante_rango" | "range_slider" => {
                    let variable_inicio = extraer_texto(argumentos, 0);
                    let variable_fin = extraer_texto(argumentos, 1);
                    let min = extraer_f64(argumentos, 2);
                    let max = extraer_f64(argumentos, 3);
                    Some(Layout::MaterialSliderRange {
                        variable_inicio,
                        variable_fin,
                        min,
                        max,
                    })
                }

                // Chip Group
                "grupo_subconjuntos" | "chip_group" => {
                    let chips = extraer_array_strings(argumentos, 0);
                    let seleccion = extraer_array_bool(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::MaterialChipGroup {
                        chips,
                        seleccion,
                        callback,
                        multiple: false,
                    })
                }

                // Date & Time Pickers
                "selector_fecha" | "date_picker" => {
                    let variable = extraer_texto(argumentos, 0);
                    Some(Layout::MaterialDatePicker { variable })
                }
                "selector_hora" | "time_picker" => {
                    let variable = extraer_texto(argumentos, 0);
                    Some(Layout::MaterialTimePicker { variable })
                }
                "selector_color" | "color_picker" => {
                    let variable = extraer_texto(argumentos, 0);
                    Some(Layout::MaterialTextField {
                        variable,
                        label: "Color".to_string(),
                        placeholder: "#RRGGBB".to_string(),
                        variant: TextFieldVariant::Outlined,
                        multiline: false,
                        error: String::new(),
                        counter: false,
                    })
                }

                // ─── Tarjetas (Cards) ─────────────────────────────
                "tarjeta" | "card" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    child.map(|c| Layout::MaterialCard {
                        child: Box::new(c),
                        variant: CardVariant::Filled,
                        on_click: None,
                        seleccionado: false,
                    })
                }
                "tarjeta_elevada" | "elevated_card" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    child.map(|c| Layout::MaterialCard {
                        child: Box::new(c),
                        variant: CardVariant::Elevated,
                        on_click: None,
                        seleccionado: false,
                    })
                }
                "tarjeta_perfilada" | "outlined_card" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    child.map(|c| Layout::MaterialCard {
                        child: Box::new(c),
                        variant: CardVariant::Outlined,
                        on_click: None,
                        seleccionado: false,
                    })
                }
                "tarjeta_seleccionable" | "selectable_card" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let _variable = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::Identificador(s, ..) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    child.map(|c| Layout::MaterialCard {
                        child: Box::new(c),
                        variant: CardVariant::Selectable,
                        on_click: None,
                        seleccionado: false,
                    })
                }

                // ─── List Items ──────────────────────────────────
                "elemento_lista" | "list_item" => {
                    let leading = argumentos.first().and_then(expr_a_layout).map(Box::new);
                    let titulo = extraer_texto(argumentos, 1);
                    let trailing = argumentos.get(2).and_then(expr_a_layout).map(Box::new);
                    Some(Layout::MaterialListItem {
                        leading,
                        titulo,
                        subtitulo: None,
                        trailing,
                        on_click: None,
                    })
                }
                "elemento_lista_doble" | "two_line_list_item" => {
                    let leading = argumentos.first().and_then(expr_a_layout).map(Box::new);
                    let titulo = extraer_texto(argumentos, 1);
                    let subtitulo = Some(extraer_texto(argumentos, 2));
                    let trailing = argumentos.get(3).and_then(expr_a_layout).map(Box::new);
                    Some(Layout::MaterialListItem {
                        leading,
                        titulo,
                        subtitulo,
                        trailing,
                        on_click: None,
                    })
                }
                "lista" | "list_widget" => {
                    let items = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(procesar_args(exprs)),
                            _ => None,
                        }).unwrap_or_default();
                    Some(Layout::MaterialList { items, dividers: false })
                }
                "lista_con_dividores" | "list_with_dividers" => {
                    let items = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(procesar_args(exprs)),
                            _ => None,
                        }).unwrap_or_default();
                    Some(Layout::MaterialList { items, dividers: true })
                }
                "lista_control" | "list_control" => {
                    let items = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(procesar_args(exprs)),
                            _ => None,
                        }).unwrap_or_default();
                    let control_type = extraer_texto(argumentos, 1);
                    let variables = extraer_array_strings(argumentos, 2);
                    Some(Layout::MaterialListControl { items, control_type, variables })
                }
                "lista_seleccion" | "list_selection" => {
                    let items = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(procesar_args(exprs)),
                            _ => None,
                        }).unwrap_or_default();
                    let seleccion = extraer_array_bool(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::MaterialListSelection {
                        items,
                        seleccion,
                        callback,
                        multiple: false,
                    })
                }

                // ─── Data Tables ─────────────────────────────────
                "tabla_datos" | "data_table" => {
                    let columnas = extraer_array_strings(argumentos, 0);
                    let filas = extraer_array_arrays_strings(argumentos, 1);
                    Some(Layout::MaterialDataTable {
                        columnas,
                        filas,
                        ordenable: false,
                        seleccionable: false,
                        col_orden: 0,
                        orden_asc: true,
                    })
                }
                "tabla_ordenable" | "sortable_table" => {
                    let columnas = extraer_array_strings(argumentos, 0);
                    let filas = extraer_array_arrays_strings(argumentos, 1);
                    Some(Layout::MaterialDataTable {
                        columnas,
                        filas,
                        ordenable: true,
                        seleccionable: false,
                        col_orden: 0,
                        orden_asc: true,
                    })
                }
                "tabla_seleccion" | "selectable_table" => {
                    let columnas = extraer_array_strings(argumentos, 0);
                    let filas = extraer_array_arrays_strings(argumentos, 1);
                    Some(Layout::MaterialDataTable {
                        columnas,
                        filas,
                        ordenable: false,
                        seleccionable: true,
                        col_orden: 0,
                        orden_asc: true,
                    })
                }

                // ─── Surfaces ────────────────────────────────────
                "superficie" | "surface_widget" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    child.map(|c| Layout::MaterialSurface {
                        child: Box::new(c),
                        color_role: "surface".to_string(),
                    })
                }
                "superficie_tonal" | "tonal_surface" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    child.map(|c| Layout::MaterialSurface {
                        child: Box::new(c),
                        color_role: "tonal".to_string(),
                    })
                }
                "andamio" | "scaffold" => {
                    let top = argumentos.get(0).and_then(expr_a_layout).map(Box::new);
                    let body = argumentos.get(1).and_then(expr_a_layout);
                    let bottom = argumentos.get(2).and_then(expr_a_layout).map(Box::new);
                    body.map(|b| Layout::MaterialScaffold {
                        top,
                        body: Box::new(b),
                        bottom,
                        fab: None,
                    })
                }

                // ─── FEEDBACK: Diálogos ──────────────────────────────
                "dialogo_alerta" | "dialog_alert" => {
                    let titulo = extraer_texto(argumentos, 0);
                    let mensaje = extraer_texto(argumentos, 1);
                    Some(Layout::DialogAlert {
                        titulo,
                        mensaje,
                        confirmar_texto: "Aceptar".to_string(),
                        cancelar_texto: String::new(),
                        on_confirm: String::new(),
                        on_cancel: String::new(),
                    })
                }
                "dialogo_confirmacion" | "dialog_confirm" => {
                    let titulo = extraer_texto(argumentos, 0);
                    let mensaje = extraer_texto(argumentos, 1);
                    let on_confirm = extraer_callback(argumentos, 2);
                    let on_cancel = extraer_callback(argumentos, 3);
                    Some(Layout::DialogAlert {
                        titulo,
                        mensaje,
                        confirmar_texto: "Confirmar".to_string(),
                        cancelar_texto: "Cancelar".to_string(),
                        on_confirm,
                        on_cancel,
                    })
                }
                "dialogo_personalizado" | "dialog_custom" => {
                    let titulo = extraer_texto(argumentos, 0);
                    let child = argumentos.get(1).and_then(expr_a_layout);
                    child.map(|c| Layout::DialogCustom {
                        titulo,
                        child: Box::new(c),
                        on_close: String::new(),
                    })
                }
                "dialogo_completo" | "dialog_full" => {
                    let titulo = extraer_texto(argumentos, 0);
                    let child = argumentos.get(1).and_then(expr_a_layout);
                    child.map(|c| Layout::DialogCustom {
                        titulo,
                        child: Box::new(c),
                        on_close: String::new(),
                    })
                }

                // ─── FEEDBACK: Bottom Sheets ─────────────────────────
                "hoja_inferior" | "bottom_sheet" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let visible = extraer_texto(argumentos, 1);
                    child.map(|c| Layout::BottomSheet {
                        child: Box::new(c),
                        variant: SheetVariant::Standard,
                        visible,
                        on_dismiss: None,
                    })
                }
                "hoja_inferior_modal" | "modal_sheet" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let visible = extraer_texto(argumentos, 1);
                    child.map(|c| Layout::BottomSheet {
                        child: Box::new(c),
                        variant: SheetVariant::Modal,
                        visible,
                        on_dismiss: None,
                    })
                }
                "hoja_inferior_grande" | "expanded_sheet" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let visible = extraer_texto(argumentos, 1);
                    child.map(|c| Layout::BottomSheet {
                        child: Box::new(c),
                        variant: SheetVariant::Expanded,
                        visible,
                        on_dismiss: None,
                    })
                }

                // ─── FEEDBACK: Snackbar ──────────────────────────────
                "notificación" | "snackbar" | "notification" => {
                    let mensaje = extraer_texto(argumentos, 0);
                    Some(Layout::Snackbar {
                        mensaje,
                        accion_texto: None,
                        accion_callback: None,
                        duracion: 3000.0,
                        visible: String::new(),
                    })
                }
                "notificación_accion" | "snackbar_action" => {
                    let mensaje = extraer_texto(argumentos, 0);
                    let accion_texto = Some(extraer_texto(argumentos, 1));
                    let accion_callback = {
                        let cb = extraer_callback(argumentos, 2);
                        if cb.is_empty() { None } else { Some(cb) }
                    };
                    Some(Layout::Snackbar {
                        mensaje,
                        accion_texto,
                        accion_callback,
                        duracion: 4000.0,
                        visible: String::new(),
                    })
                }

                // ─── FEEDBACK: Tooltip ───────────────────────────────
                "información" | "tooltip" | "info" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let texto = extraer_texto(argumentos, 1);
                    child.map(|c| Layout::Tooltip {
                        child: Box::new(c),
                        texto,
                    })
                }

                // ─── FEEDBACK: Menús ─────────────────────────────────
                "menú_desplegable" | "menu_desplegable" | "dropdown_menu" => {
                    let items = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(
                                exprs.iter().filter_map(|e| match e {
                                    Expresion::LiteralTexto(s) => Some(s.clone()),
                                    _ => None,
                                }).collect::<Vec<_>>()
                            ),
                            Expresion::LiteralTexto(s) => Some(vec![s.clone()]),
                            _ => None,
                        }).unwrap_or_default();
                    let _seleccion = extraer_usize(argumentos, 1);
                    let on_select = extraer_callback(argumentos, 2);
                    Some(Layout::Menu {
                        items,
                        on_select,
                        visible: "true".to_string(),
                    })
                }
                "menú_contexto" | "menu_contexto" | "context_menu" => {
                    let items = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(
                                exprs.iter().filter_map(|e| match e {
                                    Expresion::LiteralTexto(s) => Some(s.clone()),
                                    _ => None,
                                }).collect::<Vec<_>>()
                            ),
                            Expresion::LiteralTexto(s) => Some(vec![s.clone()]),
                            _ => None,
                        }).unwrap_or_default();
                    let _seleccion = extraer_usize(argumentos, 1);
                    let on_select = extraer_callback(argumentos, 2);
                    Some(Layout::ContextMenu {
                        items,
                        on_select,
                        visible: "true".to_string(),
                    })
                }

                // ═══════════════════════════════════════════════════════
                // Navegación Material You
                // ═══════════════════════════════════════════════════════

                // ─── Navigator (navegación por pantallas) ──────────
                "navegador" | "navigator" => {
                    let screens = extraer_navigator_screens(argumentos, 0);
                    let current_var = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => "pantalla_actual".to_string(),
                        }).unwrap_or_else(|| "pantalla_actual".to_string());
                    let current_var_clone = current_var.clone();
                    let nav_type_str = argumentos.get(2)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => "ninguno".to_string(),
                        }).unwrap_or_else(|| "ninguno".to_string());
                    let nav_type = match nav_type_str.to_lowercase().as_str() {
                        "barra" | "bottom" | "inferior" => NavigatorType::BottomBar,
                        "riel" | "rail" | "lateral" => NavigatorType::Rail,
                        "pestañas" | "tabs" | "tab" => NavigatorType::Tabs,
                        "cajon" | "drawer" => NavigatorType::Drawer,
                        _ => NavigatorType::None,
                    };
                    if screens.is_empty() {
                        Some(Layout::Spacer(0.0))
                    } else {
                        Some(Layout::Navigator {
                            screens,
                            current_var: current_var_clone,
                            history_var: format!("{}_historial", current_var),
                            nav_type,
                            anim: NavigatorAnim::None,
                        })
                    }
                }

                // ─── Pantalla individual (para usar dentro de navigator) ──
                "pantalla" | "screen" => {
                    // Las pantallas individuales se procesan dentro de extraer_navigator_screens
                    // Como llamada directa, devolvemos placeholder
                    let titulo = extraer_texto(argumentos, 0);
                    Some(Layout::StyledLabel {
                        texto: format!("[Pantalla: {}]", titulo),
                        style: "body_medium".to_string(),
                    })
                }

                // ─── NavigationBar ───────────────────────────────────
                "barra_navegacion" | "navigation_bar" => {
                    let items = extraer_nav_items(argumentos, 0);
                    let seleccion = extraer_usize(argumentos, 1);
                    let on_change = extraer_callback(argumentos, 2);
                    Some(Layout::NavigationBar { items, seleccion, on_change })
                }

                // ─── NavigationRail ──────────────────────────────────
                "riel_navegacion" | "navigation_rail" => {
                    let items = extraer_nav_items(argumentos, 0);
                    let seleccion = extraer_usize(argumentos, 1);
                    let on_change = extraer_callback(argumentos, 2);
                    let extended = extraer_booleano(argumentos, 3);
                    Some(Layout::NavigationRail { items, seleccion, on_change, extended })
                }

                // ─── NavigationDrawer ────────────────────────────────
                "cajon_navegacion" | "navigation_drawer" => {
                    let items = extraer_nav_items(argumentos, 0);
                    let seleccion = extraer_usize(argumentos, 1);
                    let on_change = extraer_callback(argumentos, 2);
                    Some(Layout::NavigationDrawer {
                        items, seleccion, on_change,
                        modal: false,
                        visible: String::new(),
                    })
                }

                // ─── NavigationDrawer Modal ──────────────────────────
                "cajon_modal" | "modal_drawer" => {
                    let items = extraer_nav_items(argumentos, 0);
                    let seleccion = extraer_usize(argumentos, 1);
                    let on_change = extraer_callback(argumentos, 2);
                    let visible = extraer_texto(argumentos, 3);
                    Some(Layout::NavigationDrawer {
                        items, seleccion, on_change,
                        modal: true,
                        visible,
                    })
                }

                // ─── TopAppBar ───────────────────────────────────────
                "barra_superior" | "top_app_bar" => {
                    let titulo = extraer_texto(argumentos, 0);
                    let acciones = extraer_icon_actions(argumentos, 1);
                    Some(Layout::TopAppBar {
                        titulo,
                        acciones,
                        menu_visible: false,
                        variant: TopAppBarVariant::Small,
                    })
                }
                "barra_superior_media" | "top_app_bar_medium" => {
                    let titulo = extraer_texto(argumentos, 0);
                    Some(Layout::TopAppBar {
                        titulo,
                        acciones: vec![],
                        menu_visible: false,
                        variant: TopAppBarVariant::Medium,
                    })
                }
                "barra_superior_grande" | "top_app_bar_large" => {
                    let titulo = extraer_texto(argumentos, 0);
                    Some(Layout::TopAppBar {
                        titulo,
                        acciones: vec![],
                        menu_visible: false,
                        variant: TopAppBarVariant::Large,
                    })
                }

                // ─── BottomAppBar ────────────────────────────────────
                "barra_inferior" | "bottom_app_bar" => {
                    let acciones = extraer_icon_actions(argumentos, 0);
                    Some(Layout::BottomAppBar {
                        acciones,
                        fab: None,
                    })
                }

                // ─── Tabs ────────────────────────────────────────────
                "pestañas" | "tabs_widget" => {
                    let tabs = extraer_array_strings(argumentos, 0);
                    let seleccion = extraer_usize(argumentos, 1);
                    let on_change = extraer_callback(argumentos, 2);
                    Some(Layout::Tabs { tabs, seleccion, on_change, scrollable: false })
                }
                "pestañas_desplazables" | "scrollable_tabs" => {
                    let tabs = extraer_array_strings(argumentos, 0);
                    let seleccion = extraer_usize(argumentos, 1);
                    let on_change = extraer_callback(argumentos, 2);
                    Some(Layout::Tabs { tabs, seleccion, on_change, scrollable: true })
                }

                // ─── SearchBar ───────────────────────────────────────
                "barra_busqueda" | "search_bar_widget" => {
                    let placeholder = extraer_texto(argumentos, 0);
                    let variable = extraer_texto(argumentos, 1);
                    Some(Layout::SearchBar {
                        placeholder,
                        on_search: String::new(),
                        variable,
                    })
                }

                // ─── item_navegacion (helper, se procesa en extraer_nav_items) ──
                "item_navegacion" | "nav_item" => {
                    // item_navegacion se procesa dentro de extraer_nav_items,
                    // pero como llamada directa devolvemos un placeholder
                    let icono = extraer_texto(argumentos, 0);
                    let label = extraer_texto(argumentos, 1);
                    Some(Layout::StyledLabel {
                        texto: format!("{} {}", icono, label),
                        style: "label_medium".to_string(),
                    })
                }

                // ═══════════════════════════════════════════════════════
                // INDICADORES (Fase 7)
                // ═══════════════════════════════════════════════════════

                // ─── LinearProgress ──────────────────────────────────
                "barra_progreso" | "progress_bar_linear" => {
                    let variable = extraer_texto(argumentos, 0);
                    Some(Layout::LinearProgress { variable, indeterminado: false })
                }
                "barra_progreso_indeterminada" | "progress_bar_indeterminate" => {
                    Some(Layout::LinearProgress { variable: String::new(), indeterminado: true })
                }

                // ─── CircularProgress ────────────────────────────────
                "circulo_progreso" | "circular_progress" => {
                    let variable = extraer_texto(argumentos, 0);
                    let size = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(40.0);
                    Some(Layout::CircularProgress { variable, size, indeterminado: false })
                }
                "circulo_progreso_indeterminado" | "circular_progress_indeterminate" => {
                    let size = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(40.0);
                    Some(Layout::CircularProgress { variable: String::new(), size, indeterminado: true })
                }

                // ─── Badge ───────────────────────────────────────────
                "distintivo" | "badge" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let valor = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => Some(s.clone()),
                            Expresion::Identificador(s, ..) => Some(s.clone()),
                            Expresion::LiteralNumero(n) => Some(n.to_string()),
                            _ => None,
                        }).unwrap_or(None);
                    child.map(|c| Layout::Badge {
                        child: Box::new(c),
                        valor,
                        dot: false,
                    })
                }
                "distintivo_punto" | "badge_dot" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    child.map(|c| Layout::Badge {
                        child: Box::new(c),
                        valor: None,
                        dot: true,
                    })
                }

                // ─── Skeleton ───────────────────────────────────────
                "esqueleto" | "skeleton" => {
                    let ancho = extraer_f64(argumentos, 0);
                    let alto = extraer_f64(argumentos, 1);
                    let tipo = extraer_texto(argumentos, 2);
                    Some(Layout::Skeleton {
                        ancho,
                        alto,
                        tipo: if tipo.is_empty() { "linea".to_string() } else { tipo },
                    })
                }
                "esqueleto_tarjeta" | "skeleton_card" => {
                    Some(Layout::Skeleton { ancho: 300.0, alto: 180.0, tipo: "tarjeta".to_string() })
                }
                "esqueleto_linea" | "skeleton_line" => {
                    Some(Layout::Skeleton { ancho: 200.0, alto: 16.0, tipo: "linea".to_string() })
                }

                // ─── EmptyState ─────────────────────────────────────
                "estado_vacio" | "empty_state" => {
                    let icono = extraer_texto(argumentos, 0);
                    let mensaje = extraer_texto(argumentos, 1);
                    let accion_texto = argumentos.get(2).map(|a| match a {
                        Expresion::LiteralTexto(s) => Some(s.clone()),
                        _ => None,
                    }).unwrap_or(None);
                    let accion_cb = argumentos.get(3).map(|a| match a {
                        Expresion::Identificador(s, ..) => Some(s.clone()),
                        _ => None,
                    }).unwrap_or(None);
                    Some(Layout::EmptyState {
                        icono,
                        mensaje,
                        accion_texto,
                        accion_cb,
                    })
                }

                // ─── ErrorState ─────────────────────────────────────
                "estado_error" | "error_state" => {
                    let mensaje = extraer_texto(argumentos, 0);
                    let on_retry = argumentos.get(1).map(|a| match a {
                        Expresion::Identificador(s, ..) => Some(s.clone()),
                        _ => None,
                    }).unwrap_or(None);
                    Some(Layout::ErrorState { mensaje, on_retry })
                }

                // ═══════════════════════════════════════════════════════
                // AVATARES (Fase 7)
                // ═══════════════════════════════════════════════════════

                // ─── Avatar ─────────────────────────────────────────
                "avatar" | "avatar_text" => {
                    let texto = extraer_texto(argumentos, 0);
                    let tamaño = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(40.0);
                    Some(Layout::Avatar { texto, variant: AvatarVariant::Text, tamaño })
                }
                "avatar_icono" | "avatar_icon" => {
                    let icono = extraer_texto(argumentos, 0);
                    let tamaño = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(40.0);
                    Some(Layout::Avatar { texto: icono, variant: AvatarVariant::Icon, tamaño })
                }
                "avatar_imagen" | "avatar_image" => {
                    let ruta = extraer_texto(argumentos, 0);
                    let tamaño = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(40.0);
                    Some(Layout::Avatar { texto: ruta, variant: AvatarVariant::Image, tamaño })
                }

                // ─── AvatarGroup ─────────────────────────────────────
                "grupo_avatar" | "avatar_group" => {
                    let avatares = argumentos.first()
                        .and_then(|a| match a {
                            Expresion::Arreglo(exprs) => Some(
                                exprs.iter().filter_map(|e| match e {
                                    Expresion::LiteralTexto(s) => Some(s.clone()),
                                    _ => None,
                                }).collect::<Vec<_>>()
                            ),
                            _ => None,
                        }).unwrap_or_default();
                    let max = argumentos.get(1)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as usize),
                            _ => None,
                        }).unwrap_or(3);
                    Some(Layout::AvatarGroup { avatares, max })
                }

                // ═══════════════════════════════════════════════════════
                // MOTION (Fase 8)
                // ═══════════════════════════════════════════════════════

                // ─── FadeTransition ──────────────────────────────────
                "transición" | "fade_transition" | "transition" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let visible = extraer_texto(argumentos, 1);
                    let duracion = argumentos.get(2)
                        .and_then(|a| match a {
                            Expresion::LiteralNumero(n) => Some(*n as f64),
                            Expresion::LiteralDecimal(f) => Some(*f),
                            _ => None,
                        }).unwrap_or(0.3);
                    child.map(|c| Layout::FadeTransition {
                        child: Box::new(c),
                        visible,
                        duracion,
                    })
                }

                // ─── RippleEffect ────────────────────────────────────
                "efecto_onda" | "ripple_effect" | "ripple" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let color = extraer_texto(argumentos, 1);
                    child.map(|c| Layout::RippleEffect {
                        child: Box::new(c),
                        color: if color.is_empty() { "primary".to_string() } else { color },
                    })
                }

                // ─── PullToRefresh ──────────────────────────────────
                "pull_to_refresh" | "pulltorefresh" | "tirar_recargar" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let callback = extraer_callback(argumentos, 1);
                    let refreshing = argumentos.get(2)
                        .map(|a| match a {
                            Expresion::Identificador(s, ..) => s.clone(),
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    child.map(|c| Layout::PullToRefresh {
                        child: Box::new(c),
                        callback,
                        refreshing: if refreshing.is_empty() { "refreshing".to_string() } else { refreshing },
                    })
                }

                // ─── SwipeToDismiss ─────────────────────────────────
                "swipe_to_dismiss" | "swipetodismiss" | "deslizar_descartar" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let on_dismiss = extraer_callback(argumentos, 1);
                    let label = extraer_texto(argumentos, 2);
                    let dismissed = argumentos.get(3)
                        .map(|a| match a {
                            Expresion::Identificador(s, ..) => s.clone(),
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    child.map(|c| Layout::SwipeToDismiss {
                        child: Box::new(c),
                        on_dismiss: if on_dismiss.is_empty() { String::new() } else { on_dismiss },
                        label: if label.is_empty() { "Descartar".to_string() } else { label },
                        dismissed: if dismissed.is_empty() { "dismissed".to_string() } else { dismissed },
                    })
                }

                // ═══════════════════════════════════════════════════════
                // GRÁFICOS (Fase 9)
                // ═══════════════════════════════════════════════════════

                // ─── LineChart ────────────────────────────────────────
                "gráfico_linea" | "grafico_linea" | "line_chart" | "chart_line" => {
                    let datos = extraer_array_f64(argumentos, 0);
                    let etiquetas = extraer_array_strings(argumentos, 1);
                    let color = extraer_texto(argumentos, 2);
                    Some(Layout::LineChart {
                        datos,
                        color: if color.is_empty() { "primary".to_string() } else { color },
                        etiquetas,
                    })
                }

                // ─── BarChart ─────────────────────────────────────────
                "gráfico_barras" | "grafico_barras" | "bar_chart" | "chart_bar" => {
                    let datos = extraer_array_f64(argumentos, 0);
                    let etiquetas = extraer_array_strings(argumentos, 1);
                    let colores = extraer_array_strings(argumentos, 2);
                    let apilado = extraer_booleano(argumentos, 3);
                    Some(Layout::BarChart {
                        datos,
                        colores,
                        etiquetas,
                        apilado,
                    })
                }

                // ─── PieChart ─────────────────────────────────────────
                "gráfico_pastel" | "grafico_pastel" | "pie_chart" | "chart_pie" => {
                    let datos = extraer_array_f64(argumentos, 0);
                    let etiquetas = extraer_array_strings(argumentos, 1);
                    Some(Layout::PieChart { datos, etiquetas, donut: false })
                }

                // ─── Donut ────────────────────────────────────────────
                "gráfico_donut" | "grafico_donut" | "donut_chart" | "chart_donut" => {
                    let datos = extraer_array_f64(argumentos, 0);
                    let etiquetas = extraer_array_strings(argumentos, 1);
                    Some(Layout::PieChart { datos, etiquetas, donut: true })
                }

                // ─── GaugeChart ───────────────────────────────────────
                "gráfico_indicador" | "grafico_indicador" | "gauge_chart" | "gauge" => {
                    let valor = extraer_f64(argumentos, 0);
                    let min = extraer_f64(argumentos, 1);
                    let max = extraer_f64(argumentos, 2);
                    let color = extraer_texto(argumentos, 3);
                    Some(Layout::GaugeChart {
                        valor,
                        min: if min == 0.0 && max == 0.0 { 0.0 } else { min },
                        max: if max == 0.0 { 100.0 } else { max },
                        color: if color.is_empty() { "primary".to_string() } else { color },
                    })
                }

                // ─── Sparkline ────────────────────────────────────────
                "minigráfico" | "minigrafico" | "sparkline" | "mini_chart" => {
                    let datos = extraer_array_f64(argumentos, 0);
                    let color = extraer_texto(argumentos, 1);
                    Some(Layout::Sparkline {
                        datos,
                        color: if color.is_empty() { "primary".to_string() } else { color },
                    })
                }

                // ═══════════════════════════════════════════════════════
                // AVANZADOS (Fase 9)
                // ═══════════════════════════════════════════════════════

                // ─── StarRating ───────────────────────────────────────
                "calificación" | "calificacion" | "star_rating" | "rating" => {
                    let valor = extraer_usize(argumentos, 0);
                    let max = extraer_usize(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::StarRating {
                        valor,
                        max: if max == 0 { 5 } else { max },
                        callback,
                    })
                }

                // ─── Stepper ──────────────────────────────────────────
                "asistente_pasos" | "stepper" | "step_wizard" => {
                    let pasos = extraer_array_strings(argumentos, 0);
                    let actual = extraer_usize(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::Stepper { pasos, actual, callback })
                }

                // ─── Breadcrumbs ──────────────────────────────────────
                "migaja_de_pan" | "breadcrumbs" | "breadcrumb" | "migajas" => {
                    let items = extraer_array_strings(argumentos, 0);
                    let separador = extraer_texto(argumentos, 1);
                    Some(Layout::Breadcrumbs {
                        items,
                        separador: if separador.is_empty() { "›".to_string() } else { separador },
                    })
                }

                // ─── Calendar ─────────────────────────────────────────
                "calendario" | "calendar" | "month_calendar" => {
                    let mes = extraer_usize(argumentos, 0) as i32;
                    let año = extraer_usize(argumentos, 1) as i32;
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::Calendar {
                        mes: if mes == 0 { 1 } else { mes },
                        año: if año == 0 { 2025 } else { año },
                        seleccionado: None,
                        callback,
                    })
                }

                // ─── MarkdownViewer ───────────────────────────────────
                "visor_markdown" | "markdown_viewer" | "markdown" | "ver_markdown" => {
                    let texto = extraer_texto(argumentos, 0);
                    Some(Layout::MarkdownViewer { texto })
                }

                // ─── QRCode ───────────────────────────────────────────
                "visor_qr" | "qr_code" | "qr" | "codigo_qr" => {
                    let texto = extraer_texto(argumentos, 0);
                    let tamaño = extraer_f64(argumentos, 1);
                    Some(Layout::QRCode {
                        texto,
                        tamaño: if tamaño <= 0.0 { 128.0 } else { tamaño },
                    })
                }

                // ─── FilePicker ───────────────────────────────────────
                "selector_archivo" | "file_picker" | "file_selector" | "seleccionar_archivo" => {
                    let tipos = extraer_array_strings(argumentos, 0);
                    let multiple = extraer_booleano(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::FilePicker {
                        tipos,
                        multiple,
                        callback,
                    })
                }

                // ═══════════════════════════════════════════════════════
                // EXPRESSIVE (Fase 10)
                // ═══════════════════════════════════════════════════════

                // ─── GlassCard ────────────────────────────────────────
                "tarjeta_vidrio" | "glass_card" | "glassmorphism" | "glass" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let blur = extraer_f64(argumentos, 1);
                    let opacity = extraer_f64(argumentos, 2);
                    child.map(|c| Layout::GlassCard {
                        child: Box::new(c),
                        blur: if blur <= 0.0 { 20.0 } else { blur },
                        opacity: if opacity <= 0.0 { 0.65 } else { opacity.min(1.0) },
                    })
                }

                // ─── GradientBox ──────────────────────────────────────
                "gradiente_lineal" | "gradiente_radial" | "gradient_box" | "gradient" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let colores = extraer_array_strings(argumentos, 1);
                    let direccion = extraer_texto(argumentos, 2);
                    child.map(|c| Layout::GradientBox {
                        child: Box::new(c),
                        colores,
                        direccion: if direccion.is_empty() { "horizontal".to_string() } else { direccion },
                    })
                }

                // ─── MorphingButton ───────────────────────────────────
                "boton_morphing" | "morphing_button" | "morph_button" => {
                    let icono = extraer_texto(argumentos, 0);
                    let texto_extendido = extraer_texto(argumentos, 1);
                    let callback = extraer_callback(argumentos, 2);
                    Some(Layout::MorphingButton {
                        icono,
                        texto_extendido,
                        callback,
                    })
                }

                // ─── ExpressiveBackground ─────────────────────────────
                "fondo_expresivo" | "expressive_background" | "bg_expressive" => {
                    let colores = extraer_array_strings(argumentos, 0);
                    let animado = extraer_booleano(argumentos, 1);
                    Some(Layout::ExpressiveBackground {
                        colores,
                        animado,
                    })
                }

                // ─── GlowBorder ───────────────────────────────────────
                "efecto_brillo" | "glow_border" | "glow" | "brillo" => {
                    let child = argumentos.first().and_then(expr_a_layout);
                    let color = extraer_texto(argumentos, 1);
                    let ancho = extraer_f64(argumentos, 2);
                    child.map(|c| Layout::GlowBorder {
                        child: Box::new(c),
                        color: if color.is_empty() { "primary".to_string() } else { color },
                        ancho: if ancho <= 0.0 { 2.0 } else { ancho },
                    })
                }

                // ═══════════════════════════════════════════════════════════
                // ICONOS MATERIAL DESIGN
                // ═══════════════════════════════════════════════════════════

                // icono_material(nombre, tamaño, color)
                // icono_material(nombre, tamaño, color, estilo)
                "icono_material" | "material_icon" => {
                    let nombre = extraer_texto(argumentos, 0);
                    let tamaño = extraer_f64(argumentos, 1);
                    let color = extraer_texto(argumentos, 2);
                    let estilo = extraer_texto(argumentos, 3);
                    Some(Layout::MaterialIconLayout {
                        nombre,
                        tamaño: if tamaño <= 0.0 { 24.0 } else { tamaño },
                        color: if color.is_empty() { "primary".to_string() } else { color },
                        estilo: if estilo.is_empty() { "filled".to_string() } else { estilo },
                    })
                }

                // icono_relleno(nombre, tamaño) — estilo filled
                "icono_relleno" | "icon_filled" => {
                    let nombre = extraer_texto(argumentos, 0);
                    let tamaño = extraer_f64(argumentos, 1);
                    Some(Layout::MaterialIconLayout {
                        nombre,
                        tamaño: if tamaño <= 0.0 { 24.0 } else { tamaño },
                        color: "primary".to_string(),
                        estilo: "filled".to_string(),
                    })
                }

                // icono_perfilado(nombre, tamaño) — estilo outlined
                "icono_perfilado" | "icon_outlined" => {
                    let nombre = extraer_texto(argumentos, 0);
                    let tamaño = extraer_f64(argumentos, 1);
                    Some(Layout::MaterialIconLayout {
                        nombre,
                        tamaño: if tamaño <= 0.0 { 24.0 } else { tamaño },
                        color: "primary".to_string(),
                        estilo: "outlined".to_string(),
                    })
                }

                // icono_redondo(nombre, tamaño) — estilo rounded
                "icono_redondo" | "icon_rounded" => {
                    let nombre = extraer_texto(argumentos, 0);
                    let tamaño = extraer_f64(argumentos, 1);
                    Some(Layout::MaterialIconLayout {
                        nombre,
                        tamaño: if tamaño <= 0.0 { 24.0 } else { tamaño },
                        color: "primary".to_string(),
                        estilo: "rounded".to_string(),
                    })
                }

                // icono_agudo(nombre, tamaño) — estilo sharp
                "icono_agudo" | "icon_sharp" => {
                    let nombre = extraer_texto(argumentos, 0);
                    let tamaño = extraer_f64(argumentos, 1);
                    Some(Layout::MaterialIconLayout {
                        nombre,
                        tamaño: if tamaño <= 0.0 { 24.0 } else { tamaño },
                        color: "primary".to_string(),
                        estilo: "sharp".to_string(),
                    })
                }

                // icono_dos_tonos(nombre, tamaño) — estilo twotone
                "icono_dos_tonos" | "icon_twotone" => {
                    let nombre = extraer_texto(argumentos, 0);
                    let tamaño = extraer_f64(argumentos, 1);
                    Some(Layout::MaterialIconLayout {
                        nombre,
                        tamaño: if tamaño <= 0.0 { 24.0 } else { tamaño },
                        color: "primary".to_string(),
                        estilo: "twotone".to_string(),
                    })
                }

                _ => None,
            }
        }
        Expresion::LiteralTexto(s) =>
            Some(Layout::Label { texto: s.clone(), es_variable: false }),
        _ => None,
    }
}

// ─── Helpers de tema Material You ──────────────────────────────────

/// Envuelve un child con un color role del tema
fn wrap_with_color_role(args: &[Expresion], role: &str) -> Option<Layout> {
    args.first().and_then(expr_a_layout)
        .map(|c| Layout::ColoredBox {
            child: Box::new(c),
            color_role: role.to_string(),
        })
}

/// Crea una etiqueta con estilo tipográfico predefinido
fn styled_label(args: &[Expresion], style: &str) -> Option<Layout> {
    let texto = args.first()
        .map(|a| match a {
            Expresion::LiteralTexto(s) => s.clone(),
            Expresion::Identificador(s, ..) => s.clone(),
            _ => String::new(),
        }).unwrap_or_default();
    Some(Layout::StyledLabel {
        texto,
        style: style.to_string(),
    })
}

/// Envuelve un child con una familia de forma
fn wrap_with_shape(args: &[Expresion], family: &str) -> Option<Layout> {
    args.first().and_then(expr_a_layout)
        .map(|c| Layout::ShapedBox {
            child: Box::new(c),
            shape_family: family.to_string(),
        })
}

/// Obtiene un color del esquema por su nombre de role
fn get_color_role(scheme: &ColorScheme, role: &str) -> RgbColor {
    match role {
        "primary" => scheme.primary,
        "on_primary" => scheme.on_primary,
        "primary_container" => scheme.primary_container,
        "on_primary_container" => scheme.on_primary_container,
        "secondary" => scheme.secondary,
        "on_secondary" => scheme.on_secondary,
        "secondary_container" => scheme.secondary_container,
        "on_secondary_container" => scheme.on_secondary_container,
        "tertiary" => scheme.tertiary,
        "on_tertiary" => scheme.on_tertiary,
        "tertiary_container" => scheme.tertiary_container,
        "on_tertiary_container" => scheme.on_tertiary_container,
        "error" => scheme.error,
        "on_error" => scheme.on_error,
        "error_container" => scheme.error_container,
        "on_error_container" => scheme.on_error_container,
        "surface" => scheme.surface,
        "on_surface" => scheme.on_surface,
        "surface_variant" => scheme.surface_variant,
        "on_surface_variant" => scheme.on_surface_variant,
        "background" => scheme.background,
        "on_background" => scheme.on_background,
        "outline" => scheme.outline,
        "outline_variant" => scheme.outline_variant,
        "inverse_surface" => scheme.inverse_surface,
        "inverse_on_surface" => scheme.inverse_on_surface,
        "inverse_primary" => scheme.inverse_primary,
        _ => scheme.primary,
    }
}

/// Obtiene el TextStyle de la escala tipográfica por nombre de estilo
fn get_text_style(typography: &TypeScale, style: &str) -> TextStyle {
    match style {
        "display_large" => typography.display_large,
        "display_medium" => typography.display_medium,
        "display_small" => typography.display_small,
        "headline_large" => typography.headline_large,
        "headline_medium" => typography.headline_medium,
        "headline_small" => typography.headline_small,
        "title_large" => typography.title_large,
        "title_medium" => typography.title_medium,
        "title_small" => typography.title_small,
        "body_large" => typography.body_large,
        "body_medium" => typography.body_medium,
        "body_small" => typography.body_small,
        "label_large" => typography.label_large,
        "label_medium" => typography.label_medium,
        "label_small" => typography.label_small,
        _ => typography.body_medium,
    }
}

/// Obtiene el radio de forma del sistema por nombre de familia
fn get_shape_radius(shapes: &ShapeSystem, family: &str) -> f64 {
    match family {
        "none" => shapes.none,
        "extra_small" | "extrasmall" => shapes.extra_small,
        "small" => shapes.small,
        "medium" => shapes.medium,
        "large" => shapes.large,
        "extra_large" | "extralarge" => shapes.extra_large,
        "full" => shapes.full,
        "button" => shapes.for_family(ShapeFamily::Button),
        "surface" => shapes.for_family(ShapeFamily::Surface),
        "container" => shapes.for_family(ShapeFamily::Container),
        _ => shapes.small,
    }
}

// ─── Helpers de alineación ─────────────────────────────────────────

/// Parsea un string de alineación a MainAxisAlignment de Xilem
fn parse_alignment(s: &str) -> MainAxisAlignment {
    match s.to_lowercase().as_str() {
        "start" | "inicio" | "izquierda" => MainAxisAlignment::Start,
        "center" | "centro" | "centrado" => MainAxisAlignment::Center,
        "end" | "fin" | "derecha" => MainAxisAlignment::End,
        "space_between" | "espacio_entre" => MainAxisAlignment::SpaceBetween,
        "space_around" | "espacio_alrededor" => MainAxisAlignment::SpaceAround,
        "space_evenly" | "espacio_igual" => MainAxisAlignment::SpaceEvenly,
        _ => MainAxisAlignment::Start,
    }
}

// ─── Colores ────────────────────────────────────────────────────────

/// Parsea un nombre de color a `Color` de Vello (usado por ColoredLabel legacy)
fn color_desde_nombre(nombre: &str) -> forja_gui_rt::Color {
    match nombre.to_lowercase().as_str() {
        "rojo" | "red" => palette::css::RED,
        "azul" | "blue" => palette::css::BLUE,
        "verde" | "green" => palette::css::GREEN,
        "blanco" | "white" => palette::css::WHITE,
        "negro" | "black" => palette::css::BLACK,
        "gris" | "gray" | "grey" => palette::css::GRAY,
        "naranja" | "orange" => palette::css::ORANGE,
        "morado" | "purple" => palette::css::PURPLE,
        "amarillo" | "yellow" => palette::css::YELLOW,
        "cian" | "cyan" => palette::css::CYAN,
        "rosa" | "pink" => palette::css::PINK,
        "azul_marino" | "navy" => palette::css::NAVY,
        "plateado" | "silver" => palette::css::SILVER,
        "marron" | "brown" => palette::css::BROWN,
        "defecto" | "default" => palette::css::WHITE,
        _ => palette::css::WHITE,
    }
}

/// Parsea un string de color (hex #RRGGBB o nombre/role del tema) a RgbColor.
/// Soporta roles del tema: "primary", "secondary", "tertiary", etc.
fn parse_color(s: &str) -> Option<RgbColor> {
    if s.starts_with('#') {
        return RgbColor::from_hex(s);
    }
    match s.to_lowercase().as_str() {
        "primary" | "secundario" => Some(RgbColor(103, 80, 164)),   // #6750A4
        "secondary" => Some(RgbColor(98, 91, 113)),                  // #625B71
        "tertiary" | "terciario" => Some(RgbColor(125, 82, 96)),    // #7D5260
        "error" => Some(RgbColor(179, 38, 30)),                      // #B3261E
        "surface" | "superficie" => Some(RgbColor(255, 251, 254)),  // #FFFBFE
        "primary_container" => Some(RgbColor(234, 221, 255)),        // #EADDFF
        "secondary_container" => Some(RgbColor(232, 222, 248)),      // #E8DEF8
        "tertiary_container" => Some(RgbColor(255, 216, 228)),       // #FFD8E4
        _ => {
            // Intentar como nombre de color estándar
            let c = RgbColor::from(s);
            if c != RgbColor(0, 0, 0) || s == "negro" || s == "black" {
                Some(c)
            } else {
                None
            }
        }
    }
}

// ─── Helpers para Navigator ─────────────────────────────────────

/// Renderiza una NavigationBar para el Navigator
fn render_navigator_bottom_bar<'a>(
    screens: &[NavigatorScreen],
    current_idx: usize,
    current_var: &str,
    prog: &[Declaracion],
    scheme: &ColorScheme,
    theme: &MaterialTheme,
) -> Box<AnyWidgetView<AppStateNativo>> {
    let cv = current_var.to_string();
    let p = prog.to_vec();
    let label_style = get_text_style(&theme.typography, "label_small");

    let mut nav_items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
    for (i, screen) in screens.iter().enumerate() {
        let cv_inner = cv.clone();
        let p_inner = p.clone();
        let idx = i;
        let is_selected = i == current_idx;
        let fg_color: Color = if is_selected {
            scheme.primary.into()
        } else {
            scheme.on_surface_variant.into()
        };
        let icon_text = screen.icono.clone().unwrap_or_else(|| "•".to_string());
        let item_widget = view::flex(Axis::Vertical, (
            view::label(icon_text)
                .text_size(24.0)
                .color(fg_color),
            view::label(screen.titulo.clone())
                .text_size(label_style.font_size as f32)
                .weight(if is_selected { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                .color(fg_color),
        )).gap(Length::px(2.0));

        // Capturar el ID de la pantalla antes del closure para evitar capturar screens
        let screen_id = screens[idx].id.clone();
        let btn = view::button(item_widget, move |data: &mut AppStateNativo| {
            data.escribir(&cv_inner, ValorGUI::Texto(screen_id.clone()));
            ejecutar_callback_y_actualizar(&cv_inner, data, &p_inner);
        });
        nav_items.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
    }

    Box::new(
        view::flex(Axis::Horizontal, (nav_items,))
            .gap(Length::px(0.0))
            .background(Background::Color(scheme.surface.into()))
    )
}

/// Renderiza Tabs para el Navigator
fn render_navigator_tabs(
    screens: Vec<NavigatorScreen>,
    current_idx: usize,
    current_var: &str,
    prog: &[Declaracion],
    scheme: &ColorScheme,
    theme: &MaterialTheme,
) -> Box<AnyWidgetView<AppStateNativo>> {
    let cv = current_var.to_string();
    let p = prog.to_vec();
    let label_style = get_text_style(&theme.typography, "label_large");

    let mut tab_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
    for (i, screen) in screens.iter().enumerate() {
        let cv_inner = cv.clone();
        let p_inner = p.clone();
        let idx = i;
        let is_selected = i == current_idx;

        let fg_color: Color = if is_selected {
            scheme.primary.into()
        } else {
            scheme.on_surface_variant.into()
        };

        let tab_content = view::flex(Axis::Vertical, (
            view::label(screen.titulo.clone())
                .text_size(label_style.font_size as f32)
                .weight(if is_selected { FontWeight::BOLD } else { FontWeight::MEDIUM })
                .color(fg_color),
            if is_selected {
                Box::new(
                    view::sized_box(view::label(String::new()))
                        .height(Length::px(3.0))
                        .background(Background::Color(scheme.primary.into()))
                ) as Box<AnyWidgetView<AppStateNativo>>
            } else {
                Box::new(
                    view::sized_box(view::label(String::new()))
                        .height(Length::px(3.0))
                ) as Box<AnyWidgetView<AppStateNativo>>
            },
        )).gap(Length::px(4.0));

        let screen_id = screens[idx].id.clone();
        let btn = view::button(tab_content, move |data: &mut AppStateNativo| {
            data.escribir(&cv_inner, ValorGUI::Texto(screen_id.clone()));
            ejecutar_callback_y_actualizar(&cv_inner, data, &p_inner);
        });
        tab_widgets.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
    }

    Box::new(
        view::flex(Axis::Horizontal, (tab_widgets,))
            .gap(Length::px(0.0))
    )
}

/// Renderiza un NavigationRail para el Navigator
fn render_navigator_rail(
    screens: Vec<NavigatorScreen>,
    current_idx: usize,
    current_var: &str,
    prog: &[Declaracion],
    scheme: &ColorScheme,
    theme: &MaterialTheme,
) -> Box<AnyWidgetView<AppStateNativo>> {
    let cv = current_var.to_string();
    let p = prog.to_vec();

    let mut rail_items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
    for (i, screen) in screens.iter().enumerate() {
        let cv_inner = cv.clone();
        let p_inner = p.clone();
        let idx = i;
        let is_selected = i == current_idx;

        let fg_color: Color = if is_selected {
            scheme.on_secondary_container.into()
        } else {
            scheme.on_surface_variant.into()
        };
        let bg_color: Color = if is_selected {
            scheme.secondary_container.into()
        } else {
            Color::TRANSPARENT
        };

        let icon_text = screen.icono.clone().unwrap_or_else(|| "•".to_string());
        let content = view::flex(Axis::Vertical, (
            view::label(icon_text)
                .text_size(24.0)
                .color(fg_color),
            view::label(screen.titulo.clone())
                .text_size(10.0)
                .color(fg_color),
        )).gap(Length::px(2.0));

        let screen_id = screens[idx].id.clone();
        let btn = view::button(content, move |data: &mut AppStateNativo| {
            data.escribir(&cv_inner, ValorGUI::Texto(screen_id.clone()));
            ejecutar_callback_y_actualizar(&cv_inner, data, &p_inner);
        });
        let styled_btn = view::sized_box(btn)
            .background(Background::Color(bg_color))
            .corner_radius(16.0);
        rail_items.push(Box::new(styled_btn) as Box<AnyWidgetView<AppStateNativo>>);
    }

    Box::new(
        view::flex(Axis::Vertical, (rail_items,))
            .gap(Length::px(4.0))
            .background(Background::Color(scheme.surface.into()))
    )
}

// ─── Layout → xilem widgets (con tema Material You) ──────────────

/// Convierte Layout a xilem usando AnyWidgetView para type erasure.
/// Recibe el tema actual como referencia para aplicar colores, tipografía y formas.
fn layout_a_view<'a>(
    layout: &'a Layout,
    data: &'a mut AppStateNativo,
    _prog: &'a [Declaracion],
    theme: &'a MaterialTheme,
) -> Box<AnyWidgetView<AppStateNativo>> {
    match layout {
        // ─── Layouts existentes (compatibilidad hacia atrás) ──────
        Layout::Column { children, gap, alignment } => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in children {
                widgets.push(layout_a_view(h, data, _prog, theme));
            }
            let ma = parse_alignment(alignment);
            Box::new(view::flex(Axis::Vertical, (widgets,)).gap(Length::px(*gap)).main_axis_alignment(ma))
        }
        Layout::CenteredColumn(hijos) => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in hijos {
                widgets.push(layout_a_view(h, data, _prog, theme));
            }
            Box::new(
                view::flex(Axis::Vertical, (widgets,))
                    .must_fill_major_axis(true)
                    .main_axis_alignment(MainAxisAlignment::Center)
            )
        }
        Layout::Row { children, gap, alignment } => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in children {
                widgets.push(layout_a_view(h, data, _prog, theme));
            }
            let ma = parse_alignment(alignment);
            Box::new(view::flex(Axis::Horizontal, (widgets,)).gap(Length::px(*gap)).main_axis_alignment(ma))
        }
        Layout::ZStack(hijos) => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in hijos {
                widgets.push(layout_a_view(h, data, _prog, theme));
            }
            Box::new(view::zstack((widgets,)))
        }
        Layout::Portal(child) => {
            let inner = layout_a_view(child, data, _prog, theme);
            Box::new(view::portal(inner))
        }
        Layout::Container { child, max_width } => {
            let inner = layout_a_view(child, data, _prog, theme);
            Box::new(view::sized_box(inner).width(Length::px(*max_width)))
        }
        Layout::Label { texto, es_variable } => {
            if *es_variable {
                let var_name = texto.clone();
                let txt = data.leer(&var_name).to_string();
                let gen = data.store.generation(&var_name);
                let color: forja_gui_rt::Color = get_color_role(&theme.scheme, "on_surface").into();
                Box::new(memoize(
                    (gen, txt, color),
                    move |(_, text, clr): &(u64, String, forja_gui_rt::Color)| {
                        view::label(text.clone()).color(*clr)
                    }
                ))
            } else {
                let txt = texto.clone();
                let color: forja_gui_rt::Color = get_color_role(&theme.scheme, "on_surface").into();
                Box::new(view::label(txt).color(color))
            }
        }
        Layout::Title(texto) => {
            // Usar estilo headline_medium del tema
            let style = get_text_style(&theme.typography, "headline_medium");
            let color: forja_gui_rt::Color = get_color_role(&theme.scheme, "on_surface").into();
            Box::new(
                view::label(texto.clone())
                    .text_size(style.font_size as f32)
                    .weight(FontWeight::BOLD)
                    .color(color)
            )
        }
        Layout::ColoredLabel { texto, color } => {
            let c = color_desde_nombre(color);
            Box::new(
                view::label(texto.clone())
                    .color(c)
            )
        }
        Layout::VariableLabel { variable } => {
            let var_name = variable.clone();
            // Leer valor Y generación para memoize reactivo
            let txt = data.leer(&var_name).to_string();
            let gen = data.store.generation(&var_name);
            // Usar memoize para que este widget solo se reconstruya
            // cuando la generación de la variable cambie
            Box::new(memoize(
                (gen, txt),
                move |(_, text): &(u64, String)| {
                    view::variable_label(text.clone())
                }
            ))
        }
        Layout::Button { texto, callback } => {
            let cb = callback.clone();
            let t = texto.clone();
            let prog = _prog.to_vec();
            Box::new(view::text_button(t, move |data: &mut AppStateNativo| {
                ejecutar_callback_y_actualizar(&cb, data, &prog);
            }))
            // Nota: xilem 0.4 text_button no expone color de fondo directamente;
            // el color primario se aplica donde la API lo permita
        }
        Layout::TextInput { variable, multiline: _, placeholder } => {
            let var_name = variable.clone();
            let val = data.leer(&var_name).to_string();
            let gen = data.store.generation(&var_name);
            let ph = placeholder.clone();
            Box::new(memoize(
                (gen, val.clone(), var_name.clone(), ph.clone()),
                move |(_, text, _, _): &(u64, String, String, String)| {
                    let vn = var_name.clone();
                    let pl = ph.clone();
                    let mut ti = view::text_input(text.clone(), move |data: &mut AppStateNativo, new_val: String| {
                        data.escribir(&vn, ValorGUI::Texto(new_val));
                    });
                    if !pl.is_empty() {
                        ti = ti.placeholder(pl.as_str());
                    }
                    map_message(ti, |_data: &mut AppStateNativo, result: MessageResult<()>| {
                        match result {
                            MessageResult::Action(()) => MessageResult::Nop,
                            other => other,
                        }
                    })
                }
            ))
        }
        Layout::ProgressBar { variable } => {
            let var_name = variable.clone();
            let val_str = data.leer(&var_name).to_string();
            let num: f64 = val_str.parse().unwrap_or(0.0);
            let gen = data.store.generation(&var_name);
            Box::new(memoize(
                (gen, num),
                move |(_, n): &(u64, f64)| {
                    view::progress_bar(Some(*n))
                }
            ))
        }
        Layout::Slider { variable, min, max } => {
            let var_name = variable.clone();
            let val = data.leer(&var_name).to_f64();
            let gen = data.store.generation(&var_name);
            let mn = *min;
            let mx = *max;
            Box::new(memoize(
                (gen, val, mn, mx),
                move |(_, v, mn2, mx2): &(u64, f64, f64, f64)| {
                    let vn = var_name.clone();
                    view::slider(*mn2, *mx2, *v, move |data: &mut AppStateNativo, new_val: f64| {
                        data.escribir(&vn, ValorGUI::Decimal(new_val));
                    })
                }
            ))
        }
        Layout::Checkbox { variable } => {
            let var_name = variable.clone();
            let txt = variable.clone();
            let checked = data.leer(&var_name).to_bool();
            let gen = data.store.generation(&var_name);
            Box::new(memoize(
                (gen, checked, txt),
                move |(_, chk, label_txt): &(u64, bool, String)| {
                    let vn = var_name.clone();
                    view::checkbox(label_txt.clone(), *chk, move |data: &mut AppStateNativo, new_checked: bool| {
                        data.escribir(&vn, ValorGUI::Booleano(new_checked));
                    })
                }
            ))
        }
        Layout::Prose(texto) => {
            Box::new(view::prose(texto.clone()))
        }
        Layout::Spinner => {
            Box::new(view::spinner())
        }
        Layout::Separator => {
            let _color: forja_gui_rt::Color = get_color_role(&theme.scheme, "outline").into();
            Box::new(view::sized_box(view::label(String::new())).height(Length::px(1.0)))
        }
        Layout::Spacer(tamano) => {
            let t = *tamano;
            Box::new(view::sized_box(view::label(String::new())).width(Length::px(t)).height(Length::px(t)))
        }

        // ─── Nuevos: variantes de tema Material You ───────────────

        // ThemeProvider: cambia el tema para los hijos
        Layout::ThemeProvider { child, theme: seed } => {
            let new_theme = MaterialTheme::from_seed(seed, theme.is_dark);
            layout_a_view(child, data, _prog, &new_theme)
        }

        // ColoredBox: aplica un color del esquema como fondo
        Layout::ColoredBox { child, color_role: _ } => {
            let inner = layout_a_view(child, data, _prog, theme);
            // Nota: xilem 0.4 sized_box no expone background_color directamente.
            // El color role está disponible para uso futuro con renderizado personalizado.
            // let _bg: forja_gui_rt::Color = get_color_role(&theme.scheme, color_role).into();
            Box::new(view::sized_box(inner))
        }

        // StyledLabel: etiqueta con estilo tipográfico predefinido
        Layout::StyledLabel { texto, style } => {
            let text_style = get_text_style(&theme.typography, style);
            let color: forja_gui_rt::Color = get_color_role(&theme.scheme, "on_surface").into();
            Box::new(
                view::label(texto.clone())
                    .text_size(text_style.font_size as f32)
                    .weight(text_style.weight.to_xilem_weight())
                    .color(color)
            )
        }

        // ShapedBox: aplica border-radius al hijo
        Layout::ShapedBox { child, shape_family } => {
            let _radius = get_shape_radius(&theme.shapes, shape_family);
            let inner = layout_a_view(child, data, _prog, theme);
            // Nota: xilem 0.4 no expone border-radius directamente en sized_box.
            // El radio se usa donde la API de renderizado lo permita.
            Box::new(view::sized_box(inner))
        }

        // ElevatedBox: aplica elevación (sombra)
        Layout::ElevatedBox { child, level } => {
            let _shadow = theme.elevation.shadow_for_level(*level);
            let inner = layout_a_view(child, data, _prog, theme);
            // Nota: xilem 0.4 no tiene sombras nativas; la config de sombra
            // está disponible para uso futuro con renderizado personalizado.
            Box::new(view::sized_box(inner))
        }

        // StateLayerBox: overlay de estado visual
        Layout::StateLayerBox { child, state: _state } => {
            // Los estados (hover, pressed, focused) se manejan internamente
            // por xilem; este wrapper es un placeholder para futura personalización.
            layout_a_view(child, data, _prog, theme)
        }

        // ResponsiveLayout: 3 variantes según WindowSizeClass
        Layout::ResponsiveLayout { compact, medium, expanded } => {
            match data.window_size {
                WindowSizeClass::Expanded => layout_a_view(expanded, data, _prog, theme),
                WindowSizeClass::Medium => layout_a_view(medium, data, _prog, theme),
                WindowSizeClass::Compact => layout_a_view(compact, data, _prog, theme),
            }
        }

        // Padding: añade espacio alrededor (usando sized_box con dimensiones)
        Layout::Padding { child, amount } => {
            let inner = layout_a_view(child, data, _prog, theme);
            Box::new(view::sized_box(inner).width(Length::px(*amount)))
        }

        // Expanded: llena el espacio disponible
        Layout::Expanded { child } => {
            layout_a_view(child, data, _prog, theme)
        }

        // Centered: centra el hijo en el eje transversal
        Layout::Centered { child } => {
            let inner = layout_a_view(child, data, _prog, theme);
            let widgets = vec![inner];
            Box::new(
                view::flex(Axis::Vertical, (widgets,))
                    .must_fill_major_axis(true)
                    .main_axis_alignment(MainAxisAlignment::Center)
            )
        }

        // ─── Layout responsive avanzado ────────────────────────

        // FlexLayout: flex con axis, gap y wrap configurables
        Layout::FlexLayout { children, axis, gap, wrap: _ } => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in children {
                widgets.push(layout_a_view(h, data, _prog, theme));
            }
            let ax = match axis.to_lowercase().as_str() {
                "horizontal" | "h" | "fila" => Axis::Horizontal,
                _ => Axis::Vertical,
            };
            // Nota: xilem 0.4 no soporta wrap nativamente; se implementará
            // cuando la API lo exponga. Por ahora se usa flex con gap.
            Box::new(view::flex(ax, (widgets,)).gap(Length::px(*gap)))
        }

        // FlowLayout: flex con wrap automático
        Layout::FlowLayout { children, gap } => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in children {
                widgets.push(layout_a_view(h, data, _prog, theme));
            }
            // FlowLayout se renderiza como flex horizontal con gap
            // El wrap automático se añadirá cuando xilem lo soporte
            Box::new(view::flex(Axis::Horizontal, (widgets,)).gap(Length::px(*gap)))
        }

        // ─── Botones Material Design 3 ──────────────────────────────
        // NOTA: En xilem 0.4, Button no soporta ContentColor ni FontSize directamente.
        // Usamos view::button(view::label(...).color(...).text_size(...), callback)
        // para poder estilizar el texto, y .background()/.corner_radius()/.border_color()
        // en el Button wrapper para el contenedor.

        // ─── MaterialButton (5 variantes) ───────────────────────────
        Layout::MaterialButton { texto, callback, variant, icono: _, disabled: _ } => {
            let cb = callback.clone();
            let t = texto.clone();
            let prog = _prog.to_vec();
            let scheme = &theme.scheme;
            let label_style = get_text_style(&theme.typography, "label_large");

            match variant {
                ButtonVariant::Filled => {
                    let fg: Color = scheme.on_primary.into();
                    let bg: Color = scheme.primary.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.background(Background::Color(bg)).corner_radius(20.0))
                }
                ButtonVariant::Tonal => {
                    let fg: Color = scheme.on_secondary_container.into();
                    let bg: Color = scheme.secondary_container.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.background(Background::Color(bg)).corner_radius(20.0))
                }
                ButtonVariant::Outlined => {
                    let fg: Color = scheme.primary.into();
                    let border: Color = scheme.outline.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.border_color(border).border_width(1.0).corner_radius(20.0))
                }
                ButtonVariant::Text => {
                    let fg: Color = scheme.primary.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.corner_radius(20.0))
                }
                ButtonVariant::Elevated => {
                    let fg: Color = scheme.primary.into();
                    let bg: Color = scheme.surface.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.background(Background::Color(bg)).corner_radius(20.0))
                }
            }
        }

        // ─── FAB (Floating Action Button) ───────────────────────────
        Layout::FAB { icono, callback, size, texto_extendido } => {
            let cb = callback.clone();
            let prog = _prog.to_vec();
            let scheme = &theme.scheme;

            let (texto, font_size) = match size {
                FabSize::Small => (icono.clone(), 16.0),
                FabSize::Medium => {
                    match texto_extendido {
                        Some(ext) => (format!("{} {}", icono, ext), 24.0),
                        None => (icono.clone(), 24.0),
                    }
                }
                FabSize::Large => (icono.clone(), 36.0),
            };

            let fg: Color = scheme.on_primary_container.into();
            let bg: Color = scheme.primary_container.into();
            let label = view::label(texto)
                .text_size(font_size as f32)
                .weight(FontWeight::MEDIUM)
                .color(fg);
            let btn = view::button(label, move |data: &mut AppStateNativo| {
                ejecutar_callback_y_actualizar(&cb, data, &prog);
            });
            Box::new(btn.background(Background::Color(bg)).corner_radius(16.0))
        }

        // ─── IconButton (4 variantes) ──────────────────────────────
        Layout::IconButton { icono, callback, variant, seleccionado: _ } => {
            let cb = callback.clone();
            let prog = _prog.to_vec();
            let scheme = &theme.scheme;

            match variant {
                IconButtonVariant::Standard => {
                    let fg: Color = scheme.on_surface_variant.into();
                    let label = view::label(icono.clone()).text_size(24.0).color(fg);
                    Box::new(view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    }))
                }
                IconButtonVariant::Filled => {
                    let fg: Color = scheme.on_primary.into();
                    let bg: Color = scheme.primary.into();
                    let label = view::label(icono.clone()).text_size(24.0).color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.background(Background::Color(bg)).corner_radius(20.0))
                }
                IconButtonVariant::Tonal => {
                    let fg: Color = scheme.on_secondary_container.into();
                    let bg: Color = scheme.secondary_container.into();
                    let label = view::label(icono.clone()).text_size(24.0).color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.background(Background::Color(bg)).corner_radius(20.0))
                }
                IconButtonVariant::Outlined => {
                    let fg: Color = scheme.primary.into();
                    let border: Color = scheme.outline.into();
                    let label = view::label(icono.clone()).text_size(24.0).color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.border_color(border).border_width(1.0).corner_radius(20.0))
                }
            }
        }

        // ─── SegmentedButton ────────────────────────────────────────
        Layout::SegmentedButton { opciones, seleccionados, callback, multiple: _ } => {
            let cb = callback.clone();
            let prog = _prog.to_vec();
            let scheme = &theme.scheme;
            let label_style = get_text_style(&theme.typography, "label_large");

            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();

            for (i, texto) in opciones.iter().enumerate() {
                let cb_inner = cb.clone();
                let t = texto.clone();
                let prog_inner = prog.clone();
                let is_selected = seleccionados.get(i).copied().unwrap_or(false);

                if is_selected {
                    let fg: Color = scheme.on_secondary_container.into();
                    let bg: Color = scheme.secondary_container.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb_inner, data, &prog_inner);
                    });
                    widgets.push(Box::new(btn.background(Background::Color(bg)).corner_radius(8.0)));
                } else {
                    let fg: Color = scheme.on_surface.into();
                    let border: Color = scheme.outline.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb_inner, data, &prog_inner);
                    });
                    widgets.push(Box::new(btn.border_color(border).border_width(1.0).corner_radius(8.0)));
                }
            }

            Box::new(view::flex(Axis::Horizontal, (widgets,)).gap(Length::px(0.0)))
        }

        // ─── Chip (4 variantes) ────────────────────────────────────
        Layout::Chip { texto, callback, variant, activo, on_remove: _ } => {
            let cb = callback.clone();
            let t = texto.clone();
            let prog = _prog.to_vec();
            let scheme = &theme.scheme;
            let label_style = get_text_style(&theme.typography, "label_small");

            match variant {
                ChipVariant::Assist | ChipVariant::Suggestion => {
                    let fg: Color = scheme.on_surface.into();
                    let border: Color = scheme.outline.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.border_color(border).border_width(1.0).corner_radius(8.0))
                }
                ChipVariant::Filter => {
                    if *activo {
                        let fg: Color = scheme.on_secondary_container.into();
                        let bg: Color = scheme.secondary_container.into();
                        let label = view::label(t.clone())
                            .text_size(label_style.font_size as f32)
                            .weight(FontWeight::MEDIUM)
                            .color(fg);
                        let btn = view::button(label, move |data: &mut AppStateNativo| {
                            ejecutar_callback_y_actualizar(&cb, data, &prog);
                        });
                        Box::new(btn.background(Background::Color(bg)).corner_radius(8.0))
                    } else {
                        let fg: Color = scheme.on_surface.into();
                        let border: Color = scheme.outline.into();
                        let label = view::label(t.clone())
                            .text_size(label_style.font_size as f32)
                            .weight(FontWeight::MEDIUM)
                            .color(fg);
                        let btn = view::button(label, move |data: &mut AppStateNativo| {
                            ejecutar_callback_y_actualizar(&cb, data, &prog);
                        });
                        Box::new(btn.border_color(border).border_width(1.0).corner_radius(8.0))
                    }
                }
                ChipVariant::Input => {
                    let fg: Color = scheme.on_secondary_container.into();
                    let bg: Color = scheme.secondary_container.into();
                    let label = view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    });
                    Box::new(btn.background(Background::Color(bg)).corner_radius(8.0))
                }
            }
        }

        // ─── Inputs Material Design 3 ──────────────────────────────────

        // ─── MaterialTextField ─────────────────────────────────────────
        Layout::MaterialTextField { variable, label, placeholder, variant, multiline: _, error, counter: _ } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let label_text = label.clone();
            let placeholder_text = placeholder.clone();
            let err_text = error.clone();

            // Label flotante
            let label_color: Color = if !err_text.is_empty() {
                scheme.error.into()
            } else {
                scheme.on_surface_variant.into()
            };
            let label_widget = if label_text.is_empty() {
                None
            } else {
                Some(view::label(label_text.clone())
                    .text_size(12.0)
                    .color(label_color))
            };

            // Campo de texto
            let mut ti = view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                data.escribir(&var_name, ValorGUI::Texto(new_val));
            });
            if !placeholder_text.is_empty() {
                ti = ti.placeholder(placeholder_text.as_str());
            }

            // Aplicar colores según variante
            let input_widget = match variant {
                TextFieldVariant::Filled => {
                    // Filled: fondo surface_variant, borde inferior al hacer focus
                    let bg: Color = scheme.surface_variant.into();
                    // Nota: xilem 0.4 no expone background_color en text_input,
                    // así que envolvemos en un sized_box con fondo
                    Box::new(view::sized_box(ti)
                        .background(Background::Color(bg))
                        .corner_radius(4.0))
                        as Box<AnyWidgetView<AppStateNativo>>
                }
                TextFieldVariant::Outlined => {
                    // Outlined: fondo transparente, borde outline
                    let border: Color = if !err_text.is_empty() {
                        scheme.error.into()
                    } else {
                        scheme.outline.into()
                    };
                    Box::new(view::sized_box(ti)
                        .border_color(border)
                        .border_width(1.0)
                        .corner_radius(4.0))
                        as Box<AnyWidgetView<AppStateNativo>>
                }
            };

            // Error text
            let children: Vec<Box<AnyWidgetView<AppStateNativo>>> = if !err_text.is_empty() {
                let err_color: Color = scheme.error.into();
                let err_label = view::label(err_text.clone())
                    .text_size(11.0)
                    .color(err_color);
                match label_widget {
                    Some(lw) => vec![
                        Box::new(lw) as Box<AnyWidgetView<AppStateNativo>>,
                        input_widget,
                        Box::new(err_label) as Box<AnyWidgetView<AppStateNativo>>,
                    ],
                    None => vec![
                        input_widget,
                        Box::new(err_label) as Box<AnyWidgetView<AppStateNativo>>,
                    ],
                }
            } else {
                match label_widget {
                    Some(lw) => vec![
                        Box::new(lw) as Box<AnyWidgetView<AppStateNativo>>,
                        input_widget,
                    ],
                    None => vec![input_widget],
                }
            };

            Box::new(view::flex(Axis::Vertical, (children,)).gap(Length::px(4.0)))
        }

        // ─── MaterialPasswordField ─────────────────────────────────────
        Layout::MaterialPasswordField { variable, label, visible: _ } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let label_text = label.clone();

            let label_color: Color = scheme.on_surface_variant.into();
            let bg: Color = scheme.surface_variant.into();

            let ti = view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                data.escribir(&var_name, ValorGUI::Texto(new_val));
            }).placeholder("••••••••");

            let input_widget = Box::new(view::sized_box(ti)
                .background(Background::Color(bg))
                .corner_radius(4.0))
                as Box<AnyWidgetView<AppStateNativo>>;

            let children: Vec<Box<AnyWidgetView<AppStateNativo>>> = if label_text.is_empty() {
                vec![input_widget]
            } else {
                vec![
                    Box::new(view::label(label_text.clone())
                        .text_size(12.0)
                        .color(label_color))
                        as Box<AnyWidgetView<AppStateNativo>>,
                    input_widget,
                ]
            };

            Box::new(view::flex(Axis::Vertical, (children,)).gap(Length::px(4.0)))
        }

        // ─── MaterialNumberField ───────────────────────────────────────
        Layout::MaterialNumberField { variable, label, min, max, decimales: _ } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let label_text = label.clone();
            let mn = *min;
            let mx = *max;

            let label_color: Color = scheme.on_surface_variant.into();
            let border: Color = scheme.outline.into();

            let range_text = format!("{}-{}", mn, mx);
            let ti = view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                // Validar que sea numérico
                if new_val.parse::<f64>().is_ok() || new_val.is_empty() || new_val == "-" || new_val == "." {
                    data.escribir(&var_name, ValorGUI::Texto(new_val));
                }
            }).placeholder(range_text.as_str());

            let input_widget: Box<AnyWidgetView<AppStateNativo>> = if label_text.is_empty() {
                Box::new(view::sized_box(ti)
                    .border_color(border)
                    .border_width(1.0)
                    .corner_radius(4.0))
            } else {
                Box::new(view::flex(Axis::Vertical, (
                    view::label(label_text.clone())
                        .text_size(12.0)
                        .color(label_color),
                    view::sized_box(ti)
                        .border_color(border)
                        .border_width(1.0)
                        .corner_radius(4.0),
                )).gap(Length::px(4.0)))
            };

            input_widget
        }

        // ─── MaterialSearchField ───────────────────────────────────────
        Layout::MaterialSearchField { variable, placeholder } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let ph = placeholder.clone();

            let bg: Color = scheme.surface_variant.into();
            let ti = view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                data.escribir(&var_name, ValorGUI::Texto(new_val));
            }).placeholder(ph.as_str());

            Box::new(view::flex(Axis::Horizontal, (
                view::label("🔍 ").text_size(16.0).color(scheme.on_surface_variant.into()),
                view::sized_box(ti)
                    .background(Background::Color(bg))
                    .corner_radius(20.0),
            )).gap(Length::px(4.0)))
        }

        // ─── MaterialDropdown ──────────────────────────────────────────
        Layout::MaterialDropdown { opciones, seleccionada, placeholder } => {
            let scheme = &theme.scheme;
            let opts = opciones.clone();
            let sel = *seleccionada;
            let ph = placeholder.clone();

            let display_text = opts.get(sel).cloned().unwrap_or(ph);
            let fg: Color = scheme.on_surface.into();
            let border: Color = scheme.outline.into();
            let bg: Color = scheme.surface_variant.into();

            // Botón que cicla a la siguiente opción
            let cb_btn = move |_data: &mut AppStateNativo| {
                // No hacemos nada en el placeholder de dropdown (ciclo)
                // El ciclo se maneja con indices de estado
            };

            Box::new(view::button(
                view::label(display_text)
                    .text_size(14.0)
                    .color(fg),
                cb_btn,
            ).background(Background::Color(bg))
              .border_color(border)
              .border_width(1.0)
              .corner_radius(4.0))
        }

        // ─── MaterialSelect ────────────────────────────────────────────
        Layout::MaterialSelect { opciones, seleccionada, label } => {
            let scheme = &theme.scheme;
            let opts = opciones.clone();
            let sel = *seleccionada;
            let label_text = label.clone();

            let display_text = opts.get(sel).cloned().unwrap_or_else(|| "Seleccionar...".to_string());
            let fg: Color = scheme.on_surface.into();
            let border: Color = scheme.outline.into();
            let label_color: Color = scheme.on_surface_variant.into();

            Box::new(view::flex(Axis::Vertical, (
                view::label(label_text.clone())
                    .text_size(12.0)
                    .color(label_color),
                view::button(
                    view::label(display_text)
                        .text_size(14.0)
                        .color(fg),
                    move |data: &mut AppStateNativo| {
                        // Placeholder: ciclo de selección no implementado en Xilem 0.4
                        let _ = data;
                    },
                ).border_color(border)
                  .border_width(1.0)
                  .corner_radius(4.0),
            )).gap(Length::px(4.0)))
        }

        // ─── MaterialAutocomplete ──────────────────────────────────────
        Layout::MaterialAutocomplete { opciones: _, variable } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let border: Color = scheme.outline.into();

            let ti = view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                data.escribir(&var_name, ValorGUI::Texto(new_val));
            }).placeholder("Escribir...");

            Box::new(view::sized_box(ti)
                .border_color(border)
                .border_width(1.0)
                .corner_radius(4.0))
        }

        // ─── MaterialRadioGroup ────────────────────────────────────────
        Layout::MaterialRadioGroup { nombre: _, opciones, seleccion, callback, direction } => {
            let scheme = &theme.scheme;
            let cb = callback.clone();
            let prog = _prog.to_vec();
            let opts = opciones.clone();
            let sel = *seleccion;

            let mut radios: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (i, opcion) in opts.iter().enumerate() {
                let cb_inner = cb.clone();
                let t = opcion.clone();
                let prog_inner = prog.clone();
                let is_selected = i == sel;

                let fg: Color = if is_selected { scheme.primary.into() } else { scheme.on_surface_variant.into() };
                let radio_color: Color = if is_selected { scheme.primary.into() } else { scheme.outline.into() };

                // Círculo + texto como botón
                let radio_widget = view::flex(Axis::Horizontal, (
                    // Círculo del radio button
                    view::sized_box(view::label(if is_selected { "◉" } else { "○" }.to_string())
                        .text_size(20.0)
                        .color(radio_color))
                        .width(Length::px(24.0))
                        .height(Length::px(24.0)),
                    view::label(t.clone())
                        .text_size(14.0)
                        .color(fg),
                )).gap(Length::px(8.0));

                let btn = view::button(radio_widget, move |data: &mut AppStateNativo| {
                    ejecutar_callback_y_actualizar(&cb_inner, data, &prog_inner);
                });

                radios.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
            }

            let ax = if direction == "horizontal" { Axis::Horizontal } else { Axis::Vertical };
            Box::new(view::flex(ax, (radios,)).gap(Length::px(8.0)))
        }

        // ─── MaterialSwitch ────────────────────────────────────────────
        Layout::MaterialSwitch { label, variable } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let lbl = label.clone();
            let checked = data.leer(variable).to_bool();

            let _track_color: Color = if checked {
                scheme.primary.into()
            } else {
                scheme.surface_variant.into()
            };
            let _thumb_color: Color = if checked {
                scheme.on_primary.into()
            } else {
                scheme.outline.into()
            };

            // Usamos checkbox de xilem estilizado como switch
            let checkbox = view::checkbox(lbl.clone(), checked, move |data: &mut AppStateNativo, new_checked: bool| {
                data.escribir(&var_name, ValorGUI::Booleano(new_checked));
            });

            // Envolvemos en un flex con colores del tema
            Box::new(view::flex(Axis::Horizontal, (
                checkbox,
            )).gap(Length::px(8.0)))
        }

        // ─── MaterialSliderDiscrete ────────────────────────────────────
        Layout::MaterialSliderDiscrete { variable, min, max, steps: _ } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_f64();
            let mn = *min;
            let mx = *max;

            let slider = view::slider(mn, mx, val, move |data: &mut AppStateNativo, new_val: f64| {
                data.escribir(&var_name, ValorGUI::Decimal(new_val));
            });

            // Mostrar valor actual
            let display_val = format!("{:.1}", val);
            let fg: Color = scheme.on_surface.into();

            Box::new(view::flex(Axis::Vertical, (
                view::label(display_val)
                    .text_size(12.0)
                    .color(fg),
                slider,
            )).gap(Length::px(4.0)))
        }

        // ─── MaterialSliderRange ───────────────────────────────────────
        Layout::MaterialSliderRange { variable_inicio, variable_fin, min, max } => {
            let scheme = &theme.scheme;
            let var1 = variable_inicio.clone();
            let var2 = variable_fin.clone();
            let val1 = data.leer(variable_inicio).to_f64();
            let val2 = data.leer(variable_fin).to_f64();
            let mn = *min;
            let mx = *max;

            let slider1 = view::slider(mn, mx, val1, move |data: &mut AppStateNativo, new_val: f64| {
                data.escribir(&var1, ValorGUI::Decimal(new_val));
            });
            let slider2 = view::slider(mn, mx, val2, move |data: &mut AppStateNativo, new_val: f64| {
                data.escribir(&var2, ValorGUI::Decimal(new_val));
            });

            let fg: Color = scheme.on_surface.into();

            Box::new(view::flex(Axis::Vertical, (
                view::label(format!("Inicio: {:.1}", val1))
                    .text_size(11.0)
                    .color(fg),
                slider1,
                view::label(format!("Fin: {:.1}", val2))
                    .text_size(11.0)
                    .color(fg),
                slider2,
            )).gap(Length::px(4.0)))
        }

        // ─── MaterialChipGroup ─────────────────────────────────────────
        Layout::MaterialChipGroup { chips, seleccion, callback, multiple: _ } => {
            let scheme = &theme.scheme;
            let cb = callback.clone();
            let prog = _prog.to_vec();
            let chip_texts = chips.clone();
            let sels = seleccion.clone();

            let mut chip_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (i, chip_text) in chip_texts.iter().enumerate() {
                let cb_inner = cb.clone();
                let t = chip_text.clone();
                let prog_inner = prog.clone();
                let is_selected = sels.get(i).copied().unwrap_or(false);

                if is_selected {
                    let fg: Color = scheme.on_secondary_container.into();
                    let bg: Color = scheme.secondary_container.into();
                    let label = view::label(t.clone())
                        .text_size(12.0)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb_inner, data, &prog_inner);
                    });
                    chip_widgets.push(Box::new(btn.background(Background::Color(bg)).corner_radius(8.0))
                        as Box<AnyWidgetView<AppStateNativo>>);
                } else {
                    let fg: Color = scheme.on_surface.into();
                    let border: Color = scheme.outline.into();
                    let label = view::label(t.clone())
                        .text_size(12.0)
                        .weight(FontWeight::MEDIUM)
                        .color(fg);
                    let btn = view::button(label, move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb_inner, data, &prog_inner);
                    });
                    chip_widgets.push(Box::new(btn.border_color(border).border_width(1.0).corner_radius(8.0))
                        as Box<AnyWidgetView<AppStateNativo>>);
                }
            }

            Box::new(view::flex(Axis::Horizontal, (chip_widgets,)).gap(Length::px(8.0)))
        }

        // ─── MaterialDatePicker ────────────────────────────────────────
        Layout::MaterialDatePicker { variable } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let border: Color = scheme.outline.into();
            let bg: Color = scheme.surface_variant.into();

            // Nota: Xilem 0.4 no tiene date picker nativo.
            // Usamos text_input con formato de fecha.
            let ti = view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                data.escribir(&var_name, ValorGUI::Texto(new_val));
            }).placeholder("YYYY-MM-DD");

            Box::new(view::flex(Axis::Horizontal, (
                view::label("📅 ").text_size(16.0).color(scheme.on_surface_variant.into()),
                view::sized_box(ti)
                    .background(Background::Color(bg))
                    .border_color(border)
                    .border_width(1.0)
                    .corner_radius(4.0),
            )).gap(Length::px(4.0)))
        }

        // ─── MaterialTimePicker ────────────────────────────────────────
        Layout::MaterialTimePicker { variable } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let border: Color = scheme.outline.into();
            let bg: Color = scheme.surface_variant.into();

            let ti = view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                data.escribir(&var_name, ValorGUI::Texto(new_val));
            }).placeholder("HH:MM");

            Box::new(view::flex(Axis::Horizontal, (
                view::label("🕐 ").text_size(16.0).color(scheme.on_surface_variant.into()),
                view::sized_box(ti)
                    .background(Background::Color(bg))
                    .border_color(border)
                    .border_width(1.0)
                    .corner_radius(4.0),
            )).gap(Length::px(4.0)))
        }

        // AspectRatioBox: caja con relación de aspecto fija
        Layout::AspectRatioBox { child, ratio: _ } => {
            let inner = layout_a_view(child, data, _prog, theme);
            // Nota: xilem 0.4 no tiene aspect ratio nativo en sized_box.
            // Se renderiza el hijo directamente; el constraint de aspecto
            // se implementará en una versión futura con layout personalizado.
            Box::new(view::sized_box(inner))
        }

        // ─── Tarjetas, Listas y Tablas ──────────────────────────────

        Layout::MaterialCard { child, variant, on_click: _, seleccionado } => {
            let scheme = &theme.scheme;
            let inner = layout_a_view(child, data, _prog, theme);
            let base = view::sized_box(inner).corner_radius(12.0);
            match variant {
                CardVariant::Filled => {
                    let bg: Color = scheme.surface_variant.into();
                    Box::new(base.background(Background::Color(bg)))
                }
                CardVariant::Elevated => {
                    let bg: Color = scheme.surface.into();
                    Box::new(base.background(Background::Color(bg)))
                }
                CardVariant::Outlined => {
                    let bg: Color = scheme.surface.into();
                    let border: Color = scheme.outline_variant.into();
                    Box::new(base.background(Background::Color(bg)).border_color(border).border_width(1.0))
                }
                CardVariant::Selectable => {
                    if *seleccionado {
                        let bg: Color = scheme.secondary_container.into();
                        let border: Color = scheme.secondary.into();
                        Box::new(base.background(Background::Color(bg)).border_color(border).border_width(1.0))
                    } else {
                        let bg: Color = scheme.surface_variant.into();
                        Box::new(base.background(Background::Color(bg)))
                    }
                }
            }
        }

        Layout::MaterialListItem { leading, titulo, subtitulo, trailing, on_click: _ } => {
            let scheme = &theme.scheme;
            let mut children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();

            // Leading (icon/avatar)
            if let Some(l) = leading {
                children.push(layout_a_view(l, data, _prog, theme));
            }

            // Texto (título + subtítulo)
            let fg: Color = scheme.on_surface.into();
            let fg_var: Color = scheme.on_surface_variant.into();

            let mut text_children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            text_children.push(Box::new(
                view::label(titulo.clone())
                    .text_size(16.0)
                    .color(fg)
            ) as Box<AnyWidgetView<AppStateNativo>>);

            if let Some(sub) = subtitulo {
                text_children.push(Box::new(
                    view::label(sub.clone())
                        .text_size(14.0)
                        .color(fg_var)
                ) as Box<AnyWidgetView<AppStateNativo>>);
            }

            children.push(Box::new(
                view::flex(Axis::Vertical, (text_children,)).gap(Length::px(2.0))
            ) as Box<AnyWidgetView<AppStateNativo>>);

            // Trailing
            if let Some(t) = trailing {
                children.push(layout_a_view(t, data, _prog, theme));
            }

            Box::new(view::flex(Axis::Horizontal, (children,)).gap(Length::px(16.0)))
        }

        Layout::MaterialList { items, dividers } => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (i, item) in items.iter().enumerate() {
                widgets.push(layout_a_view(item, data, _prog, theme));
                if *dividers && i < items.len() - 1 {
                    widgets.push(Box::new(
                        view::sized_box(view::label(String::new()))
                            .height(Length::px(1.0))
                    ) as Box<AnyWidgetView<AppStateNativo>>);
                }
            }
            Box::new(view::flex(Axis::Vertical, (widgets,)).gap(Length::px(0.0)))
        }

        Layout::MaterialListControl { items, control_type: _, variables: _ } => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for item in items.iter() {
                widgets.push(layout_a_view(item, data, _prog, theme));
            }
            Box::new(view::flex(Axis::Vertical, (widgets,)).gap(Length::px(0.0)))
        }

        Layout::MaterialListSelection { items, seleccion: _, callback: _, multiple: _ } => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for item in items.iter() {
                widgets.push(layout_a_view(item, data, _prog, theme));
            }
            Box::new(view::flex(Axis::Vertical, (widgets,)).gap(Length::px(0.0)))
        }

        Layout::MaterialDataTable { columnas, filas, ordenable, seleccionable: _, col_orden, orden_asc } => {
            let scheme = &theme.scheme;
            let label_style = get_text_style(&theme.typography, "label_small");
            let fg_header: Color = scheme.on_surface.into();
            let fg_body: Color = scheme.on_surface.into();
            let bg_header: Color = scheme.surface_variant.into();
            let bg_row1: Color = scheme.surface.into();
            let bg_row2: Color = scheme.surface_variant.into();

            // Header row
            let mut header_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (i, col) in columnas.iter().enumerate() {
                let is_ordered = *ordenable && i == *col_orden;
                let display = if is_ordered {
                    format!("{} {}", col, if *orden_asc { "↑" } else { "↓" })
                } else {
                    col.clone()
                };
                let hdr = view::label(display)
                    .text_size(label_style.font_size as f32)
                    .weight(FontWeight::BOLD)
                    .color(fg_header);
                header_widgets.push(Box::new(view::sized_box(hdr).padding(8.0))
                    as Box<AnyWidgetView<AppStateNativo>>);
            }

            let header_row = Box::new(
                view::flex(Axis::Horizontal, (header_widgets,)).gap(Length::px(8.0))
            ) as Box<AnyWidgetView<AppStateNativo>>;

            let header_container = Box::new(
                view::sized_box(header_row)
                    .background(Background::Color(bg_header))
                    .corner_radius(4.0)
            );

            // Body rows
            let mut body_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (row_idx, fila) in filas.iter().enumerate() {
                let mut cell_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                for celda in fila.iter() {
                    let cell = view::label(celda.clone())
                        .text_size(14.0)
                        .color(fg_body);
                    cell_widgets.push(Box::new(view::sized_box(cell).padding(8.0))
                        as Box<AnyWidgetView<AppStateNativo>>);
                }
                let row_bg = if row_idx % 2 == 0 { bg_row1 } else { bg_row2 };
                let row = Box::new(
                    view::flex(Axis::Horizontal, (cell_widgets,)).gap(Length::px(8.0))
                ) as Box<AnyWidgetView<AppStateNativo>>;
                body_widgets.push(
                    Box::new(view::sized_box(row).background(Background::Color(row_bg)))
                        as Box<AnyWidgetView<AppStateNativo>>
                );
            }

            let body = Box::new(
                view::flex(Axis::Vertical, (body_widgets,)).gap(Length::px(0.0))
            );

            Box::new(view::flex(Axis::Vertical, (
                header_container,
                body,
            )).gap(Length::px(4.0)))
        }

        Layout::MaterialSurface { child, color_role } => {
            let scheme = &theme.scheme;
            let inner = layout_a_view(child, data, _prog, theme);
            let bg: Color = match color_role.as_str() {
                "tonal" => scheme.secondary_container.into(),
                "primary" => scheme.primary.into(),
                _ => scheme.surface.into(),
            };
            Box::new(view::sized_box(inner).background(Background::Color(bg)).corner_radius(12.0))
        }

        Layout::MaterialScaffold { top, body, bottom, fab: _ } => {
            let mut children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();

            if let Some(t) = top {
                children.push(layout_a_view(t, data, _prog, theme));
            }

            children.push(layout_a_view(body, data, _prog, theme));

            if let Some(b) = bottom {
                children.push(layout_a_view(b, data, _prog, theme));
            }

            Box::new(view::flex(Axis::Vertical, (children,)).gap(Length::px(0.0)))
        }

        // ═══════════════════════════════════════════════════════════════
        // FEEDBACK Y SUPERPOSICIONES (Material Design 3)
        // ═══════════════════════════════════════════════════════════════

        // ─── DialogOverlay ───────────────────────────────────────────
        // Diálogo superpuesto con overlay semitransparente
        Layout::DialogOverlay { dialog, visible } => {
            let show = data.leer(visible).to_string() == "true";
            if show {
                let inner = layout_a_view(dialog, data, _prog, theme);
                let overlay_color: Color = RgbColor(0, 0, 0).with_alpha(0.32);
                // ZStack: overlay semitransparente + contenido del diálogo centrado
                Box::new(view::zstack((
                    view::sized_box(view::label(String::new()))
                        .background(Background::Color(overlay_color)),
                    view::sized_box(inner)
                        .width(Length::px(312.0)),
                )))
            } else {
                Box::new(view::sized_box(view::label(String::new())))
            }
        }

        // ─── DialogAlert ─────────────────────────────────────────────
        // Tarjeta de diálogo con título, mensaje y botones
        Layout::DialogAlert { titulo, mensaje, confirmar_texto, cancelar_texto, on_confirm, on_cancel } => {
            let scheme = &theme.scheme;
            let overlay_color: Color = RgbColor(0, 0, 0).with_alpha(0.32);
            let surface_bg: Color = scheme.surface.into();
            let fg: Color = scheme.on_surface.into();
            let fg_var: Color = scheme.on_surface_variant.into();
            let primary_color: Color = scheme.primary.into();
            let prog = _prog.to_vec();
            let cb_confirm = on_confirm.clone();
            let cb_cancel = on_cancel.clone();

            // Botón Confirmar
            let confirm_btn = {
                let cb = cb_confirm.clone();
                let p = prog.clone();
                view::button(
                    view::label(confirmar_texto.clone())
                        .text_size(14.0)
                        .weight(FontWeight::MEDIUM)
                        .color(primary_color),
                    move |data: &mut AppStateNativo| {
                        if !cb.is_empty() {
                            ejecutar_callback_y_actualizar(&cb, data, &p);
                        }
                    },
                )
            };

            // Botón Cancelar (solo si hay texto)
            let has_cancel = !cancelar_texto.is_empty();
            let cancel_btn = if has_cancel {
                let cb = cb_cancel.clone();
                let p = prog.clone();
                Some(view::button(
                    view::label(cancelar_texto.clone())
                        .text_size(14.0)
                        .weight(FontWeight::MEDIUM)
                        .color(primary_color),
                    move |data: &mut AppStateNativo| {
                        if !cb.is_empty() {
                            ejecutar_callback_y_actualizar(&cb, data, &p);
                        }
                    },
                ))
            } else {
                None
            };

            // Fila de botones
            let button_row: Box<AnyWidgetView<AppStateNativo>> = if has_cancel {
                Box::new(view::flex(Axis::Horizontal, (
                    cancel_btn.unwrap(),
                    view::sized_box(view::label(String::new())).width(Length::px(8.0)),
                    confirm_btn,
                )).main_axis_alignment(MainAxisAlignment::End))
            } else {
                Box::new(view::flex(Axis::Horizontal, (
                    confirm_btn,
                )).main_axis_alignment(MainAxisAlignment::End))
            };

            // Título
            let title_label = {
                let title_style = get_text_style(&theme.typography, "title_large");
                view::label(titulo.clone())
                    .text_size(title_style.font_size as f32)
                    .weight(FontWeight::BOLD)
                    .color(fg)
            };

            // Mensaje
            let msg_label = {
                let body_style = get_text_style(&theme.typography, "body_medium");
                view::label(mensaje.clone())
                    .text_size(body_style.font_size as f32)
                    .color(fg_var)
            };

            // Card del diálogo
            let dialog_card = view::flex(Axis::Vertical, (
                title_label,
                view::sized_box(view::label(String::new())).height(Length::px(8.0)),
                msg_label,
                view::sized_box(view::label(String::new())).height(Length::px(16.0)),
                button_row,
            ));

            // Card del diálogo con padding interior
            let dialog_inner = view::sized_box(dialog_card)
                .background(Background::Color(surface_bg))
                .corner_radius(16.0)
                .padding(24.0);

            // Contenedor externo con ancho máximo
            let dialog_container = view::sized_box(dialog_inner)
                .width(Length::px(312.0));

            // Overlay completo
            Box::new(view::zstack((
                view::sized_box(view::label(String::new()))
                    .background(Background::Color(overlay_color)),
                dialog_container,
            )))
        }

        // ─── DialogCustom ────────────────────────────────────────────
        // Diálogo personalizado con hijo arbitrario
        Layout::DialogCustom { titulo, child, on_close: _ } => {
            let scheme = &theme.scheme;
            let overlay_color: Color = RgbColor(0, 0, 0).with_alpha(0.32);
            let surface_bg: Color = scheme.surface.into();
            let fg: Color = scheme.on_surface.into();

            let title_style = get_text_style(&theme.typography, "title_large");
            let title_label = view::label(titulo.clone())
                .text_size(title_style.font_size as f32)
                .weight(FontWeight::BOLD)
                .color(fg);

            let inner = layout_a_view(child, data, _prog, theme);

            let dialog_card = view::flex(Axis::Vertical, (
                title_label,
                view::sized_box(view::label(String::new())).height(Length::px(8.0)),
                inner,
            ));

            let dialog_container = view::sized_box(dialog_card)
                .background(Background::Color(surface_bg))
                .corner_radius(16.0)
                .padding(24.0);

            Box::new(view::zstack((
                view::sized_box(view::label(String::new()))
                    .background(Background::Color(overlay_color)),
                dialog_container,
            )))
        }

        // ─── BottomSheet ─────────────────────────────────────────────
        // Panel que emerge desde abajo (Standard, Modal, Expanded)
        Layout::BottomSheet { child, variant, visible, on_dismiss: _ } => {
            let show = data.leer(visible).to_string() == "true";
            if show {
                let scheme = &theme.scheme;
                let inner = layout_a_view(child, data, _prog, theme);
                let sheet_bg: Color = scheme.surface.into();

                match variant {
                    SheetVariant::Standard => {
                        // Sin overlay, solo panel con esquinas redondeadas arriba
                        let sheet = view::sized_box(inner)
                            .background(Background::Color(sheet_bg))
                            .corner_radius(16.0);
                        // En un ZStack para que aparezca sobre el contenido
                        Box::new(view::zstack((
                            view::sized_box(view::label(String::new())),
                            Box::new(sheet) as Box<AnyWidgetView<AppStateNativo>>,
                        )))
                    }
                    SheetVariant::Modal => {
                        // Overlay semitransparente + panel
                        let overlay_color: Color = RgbColor(0, 0, 0).with_alpha(0.32);
                        let sheet = view::sized_box(inner)
                            .background(Background::Color(sheet_bg))
                            .corner_radius(16.0);
                        Box::new(view::zstack((
                            view::sized_box(view::label(String::new()))
                                .background(Background::Color(overlay_color)),
                            Box::new(sheet) as Box<AnyWidgetView<AppStateNativo>>,
                        )))
                    }
                    SheetVariant::Expanded => {
                        // Overlay + panel que ocupa ~90% de altura
                        let overlay_color: Color = RgbColor(0, 0, 0).with_alpha(0.32);
                        let sheet = view::sized_box(inner)
                            .background(Background::Color(sheet_bg))
                            .corner_radius(16.0);
                        Box::new(view::zstack((
                            view::sized_box(view::label(String::new()))
                                .background(Background::Color(overlay_color)),
                            Box::new(sheet) as Box<AnyWidgetView<AppStateNativo>>,
                        )))
                    }
                }
            } else {
                Box::new(view::sized_box(view::label(String::new())))
            }
        }

        // ─── Snackbar ────────────────────────────────────────────────
        // Barra inferior de notificación temporal
        Layout::Snackbar { mensaje, accion_texto, accion_callback, duracion: _, visible } => {
            let show = data.leer(visible).to_string() == "true";
            if show {
                let scheme = &theme.scheme;
                let bg: Color = scheme.inverse_surface.into();
                let fg: Color = scheme.inverse_on_surface.into();
                let action_fg: Color = scheme.inverse_primary.into();
                let prog = _prog.to_vec();

                // Texto del mensaje
                let body_style = get_text_style(&theme.typography, "body_medium");
                let msg_label = view::label(mensaje.clone())
                    .text_size(body_style.font_size as f32)
                    .color(fg);

                // Botón de acción (opcional)
                let has_action = accion_texto.is_some() && accion_callback.is_some();
                let action_btn: Option<Box<AnyWidgetView<AppStateNativo>>> = if has_action {
                    let at = accion_texto.clone().unwrap_or_default();
                    let cb = accion_callback.clone().unwrap_or_default();
                    let p = prog.clone();
                    Some(Box::new(view::button(
                        view::label(at.clone())
                            .text_size(14.0)
                            .weight(FontWeight::MEDIUM)
                            .color(action_fg),
                        move |data: &mut AppStateNativo| {
                            if !cb.is_empty() {
                                ejecutar_callback_y_actualizar(&cb, data, &p);
                            }
                        },
                    )) as Box<AnyWidgetView<AppStateNativo>>)
                } else {
                    None
                };

                // Construir fila: mensaje + botón
                let mut row_children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                row_children.push(Box::new(msg_label) as Box<AnyWidgetView<AppStateNativo>>);
                if let Some(btn) = action_btn {
                    row_children.push(Box::new(view::sized_box(view::label(String::new())).width(Length::px(16.0)))
                        as Box<AnyWidgetView<AppStateNativo>>);
                    row_children.push(btn);
                }

                let row = view::flex(Axis::Horizontal, (row_children,))
                    .gap(Length::px(8.0));

                let snackbar = view::sized_box(row)
                    .background(Background::Color(bg))
                    .corner_radius(8.0)
                    .padding(16.0);

                Box::new(snackbar)
            } else {
                Box::new(view::sized_box(view::label(String::new())))
            }
        }

        // ─── Tooltip ─────────────────────────────────────────────────
        // Pequeña etiqueta de ayuda contextual
        Layout::Tooltip { child, texto } => {
            let scheme = &theme.scheme;
            let tooltip_bg: Color = scheme.on_surface.into();
            let tooltip_fg: Color = scheme.surface.into();

            let inner = layout_a_view(child, data, _prog, theme);
            let label_style = get_text_style(&theme.typography, "label_small");
            let tooltip_label = view::label(texto.clone())
                .text_size(label_style.font_size as f32)
                .color(tooltip_fg);

            let tooltip_box = view::sized_box(tooltip_label)
                .background(Background::Color(tooltip_bg))
                .corner_radius(8.0)
                .padding(8.0);

            // Mostrar tooltip debajo del contenido
            Box::new(view::flex(Axis::Vertical, (
                inner,
                tooltip_box,
            )).gap(Length::px(4.0)))
        }

        // ─── Menu ────────────────────────────────────────────────────
        // Menú desplegable con lista de opciones
        Layout::Menu { items, on_select, visible } => {
            let show = data.leer(visible).to_string() == "true";
            if show && !items.is_empty() {
                let scheme = &theme.scheme;
                let surface_bg: Color = scheme.surface.into();
                let fg: Color = scheme.on_surface.into();
                let prog = _prog.to_vec();
                let cb = on_select.clone();

                let mut menu_items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let cb_inner = cb.clone();
                    let t = item.clone();
                    let p = prog.clone();
                    let item_label = view::label(t.clone())
                        .text_size(14.0)
                        .color(fg);
                    let item_btn = view::button(item_label, move |data: &mut AppStateNativo| {
                        if !cb_inner.is_empty() {
                            ejecutar_callback_y_actualizar(&cb_inner, data, &p);
                        }
                    });
                    menu_items.push(Box::new(item_btn) as Box<AnyWidgetView<AppStateNativo>>);
                    if i < items.len() - 1 {
                        menu_items.push(Box::new(
                            view::sized_box(view::label(String::new()))
                                .height(Length::px(1.0))
                        ) as Box<AnyWidgetView<AppStateNativo>>);
                    }
                }

                let menu_box = view::sized_box(
                    view::flex(Axis::Vertical, (menu_items,)).gap(Length::px(0.0))
                ).background(Background::Color(surface_bg))
                 .corner_radius(8.0);

                Box::new(menu_box)
            } else {
                Box::new(view::sized_box(view::label(String::new())))
            }
        }

        // ─── ContextMenu ─────────────────────────────────────────────
        // Menú contextual con lista de opciones
        Layout::ContextMenu { items, on_select, visible } => {
            let show = data.leer(visible).to_string() == "true";
            if show && !items.is_empty() {
                let scheme = &theme.scheme;
                let surface_bg: Color = scheme.surface.into();
                let fg: Color = scheme.on_surface.into();
                let _prog = _prog;
                let prog = _prog.to_vec();
                let cb = on_select.clone();

                let mut menu_items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let cb_inner = cb.clone();
                    let t = item.clone();
                    let p = prog.clone();
                    let item_label = view::label(t.clone())
                        .text_size(14.0)
                        .color(fg);
                    let item_btn = view::button(item_label, move |data: &mut AppStateNativo| {
                        if !cb_inner.is_empty() {
                            ejecutar_callback_y_actualizar(&cb_inner, data, &p);
                        }
                    });
                    menu_items.push(Box::new(item_btn) as Box<AnyWidgetView<AppStateNativo>>);
                    if i < items.len() - 1 {
                        menu_items.push(Box::new(
                            view::sized_box(view::label(String::new()))
                                .height(Length::px(1.0))
                        ) as Box<AnyWidgetView<AppStateNativo>>);
                    }
                }

                let menu_box = view::sized_box(
                    view::flex(Axis::Vertical, (menu_items,)).gap(Length::px(0.0))
                ).background(Background::Color(surface_bg))
                 .corner_radius(8.0);

                Box::new(menu_box)
            } else {
                Box::new(view::sized_box(view::label(String::new())))
            }
        }

        // ═══════════════════════════════════════════════════════════
        // Navegación Material You
        // ═══════════════════════════════════════════════════════════

        // ─── Navigator (navegación por pantallas) ────────────────────
        Layout::Navigator { screens, current_var, history_var: _, nav_type, anim: _ } => {
            let scheme = &theme.scheme;
            let p = _prog.to_vec();

            // Leer la pantalla actual desde el store reactivo
            let current_id = data.leer(current_var).to_string();
            let current_idx = screens.iter().position(|s| s.id == current_id)
                .unwrap_or(0);
            let idx = current_idx % screens.len();

            // Obtener la pantalla actual y renderizar su contenido
            let current_screen = &screens[idx];
            let content = layout_a_view(&current_screen.contenido, data, _prog, theme);

            // Pre-extraer datos de navegación para evitar ownership issues en closures
            let nav_ids: Vec<String> = screens.iter().map(|s| s.id.clone()).collect();
            let nav_titles: Vec<String> = screens.iter().map(|s| s.titulo.clone()).collect();
            let nav_icons: Vec<String> = screens.iter()
                .map(|s| s.icono.clone().unwrap_or_else(|| "•".to_string()))
                .collect();

            // Construir la navegación según el tipo
            match nav_type {
                NavigatorType::None => content,
                NavigatorType::BottomBar => {
                    let cv = current_var.to_string();
                    let sc = scheme.clone();
                    let label_style = get_text_style(&theme.typography, "label_small");
                    let mut items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                    for i in 0..screens.len() {
                        let cv_inner = cv.clone();
                        let sid = nav_ids[i].clone();
                        let titulo = nav_titles[i].clone();
                        let icono = nav_icons[i].clone();
                        let sel = i == idx;
                        let fg: Color = if sel { sc.primary.into() } else { sc.on_surface_variant.into() };
                        let w = view::flex(Axis::Vertical, (
                            view::label(icono).text_size(24.0).color(fg),
                            view::label(titulo).text_size(label_style.font_size as f32)
                                .weight(if sel { FontWeight::MEDIUM } else { FontWeight::NORMAL }).color(fg),
                        )).gap(Length::px(2.0));
                        let btn = view::button(w, move |data: &mut AppStateNativo| {
                            data.escribir(&cv_inner, ValorGUI::Texto(sid.clone()));
                            ejecutar_callback_y_actualizar(&cv_inner, data, &p);
                        });
                        items.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
                    }
                    let bar = Box::new(view::flex(Axis::Horizontal, (items,))
                        .gap(Length::px(0.0)).background(Background::Color(sc.surface.into())));
                    Box::new(view::flex(Axis::Vertical, (content, bar)).gap(Length::px(0.0)))
                }
                NavigatorType::Tabs => {
                    let cv = current_var.to_string();
                    let sc = scheme.clone();
                    let label_style = get_text_style(&theme.typography, "label_large");
                    let mut items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                    for i in 0..screens.len() {
                        let cv_inner = cv.clone();
                        let sid = nav_ids[i].clone();
                        let titulo = nav_titles[i].clone();
                        let sel = i == idx;
                        let fg: Color = if sel { sc.primary.into() } else { sc.on_surface_variant.into() };
                        let tab = view::flex(Axis::Vertical, (
                            view::label(titulo).text_size(label_style.font_size as f32)
                                .weight(if sel { FontWeight::BOLD } else { FontWeight::MEDIUM }).color(fg),
                            if sel {
                                Box::new(view::sized_box(view::label(String::new()))
                                    .height(Length::px(3.0)).background(Background::Color(sc.primary.into())))
                                    as Box<AnyWidgetView<AppStateNativo>>
                            } else {
                                Box::new(view::sized_box(view::label(String::new())).height(Length::px(3.0)))
                                    as Box<AnyWidgetView<AppStateNativo>>
                            },
                        )).gap(Length::px(4.0));
                        let btn = view::button(tab, move |data: &mut AppStateNativo| {
                            data.escribir(&cv_inner, ValorGUI::Texto(sid.clone()));
                            ejecutar_callback_y_actualizar(&cv_inner, data, &p);
                        });
                        items.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
                    }
                    let tabs = Box::new(view::flex(Axis::Horizontal, (items,)).gap(Length::px(0.0)));
                    Box::new(view::flex(Axis::Vertical, (tabs, content)).gap(Length::px(0.0)))
                }
                NavigatorType::Rail | NavigatorType::Drawer => {
                    let cv = current_var.to_string();
                    let sc = scheme.clone();
                    let mut items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                    for i in 0..screens.len() {
                        let cv_inner = cv.clone();
                        let sid = nav_ids[i].clone();
                        let titulo = nav_titles[i].clone();
                        let icono = nav_icons[i].clone();
                        let sel = i == idx;
                        let fg: Color = if sel { sc.on_secondary_container.into() } else { sc.on_surface_variant.into() };
                        let bg: Color = if sel { sc.secondary_container.into() } else { Color::TRANSPARENT };
                        let w = view::flex(Axis::Vertical, (
                            view::label(icono).text_size(24.0).color(fg),
                            view::label(titulo).text_size(10.0).color(fg),
                        )).gap(Length::px(2.0));
                        let btn = view::button(w, move |data: &mut AppStateNativo| {
                            data.escribir(&cv_inner, ValorGUI::Texto(sid.clone()));
                            ejecutar_callback_y_actualizar(&cv_inner, data, &p);
                        });
                        items.push(Box::new(view::sized_box(btn).background(Background::Color(bg)).corner_radius(16.0))
                            as Box<AnyWidgetView<AppStateNativo>>);
                    }
                    let rail = Box::new(view::flex(Axis::Vertical, (items,))
                        .gap(Length::px(4.0)).background(Background::Color(sc.surface.into())));
                    Box::new(view::flex(Axis::Horizontal, (rail, content)).gap(Length::px(0.0)))
                }
            }
        }

        // ─── NavigationBar ───────────────────────────────────────────
        // Bottom navigation estilo mobile con fondo surface
        Layout::NavigationBar { items, seleccion, on_change } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = on_change.clone();

            let mut nav_items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let cb_inner = cb.clone();
                let p_inner = prog.clone();
                let idx = i;
                let is_selected = i == *seleccion;

                let fg_color: Color = if is_selected {
                    scheme.primary.into()
                } else {
                    scheme.on_surface_variant.into()
                };

                let label_style = get_text_style(&theme.typography, "label_small");
                let item_widget = view::flex(Axis::Vertical, (
                    view::label(item.icono.clone())
                        .text_size(24.0)
                        .color(fg_color),
                    view::label(item.label.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(if is_selected { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                        .color(fg_color),
                )).gap(Length::px(2.0));

                let btn = view::button(item_widget, move |data: &mut AppStateNativo| {
                    data.escribir(&cb_inner, ValorGUI::Entero(idx as i64));
                    ejecutar_callback_y_actualizar(&cb_inner, data, &p_inner);
                });

                nav_items.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
            }

            Box::new(
                view::flex(Axis::Horizontal, (nav_items,))
                    .gap(Length::px(0.0))
                    .background(Background::Color(scheme.surface.into()))
            )
        }

        // ─── NavigationRail ──────────────────────────────────────────
        // Navegación lateral (compacta o extendida)
        Layout::NavigationRail { items, seleccion, on_change, extended } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = on_change.clone();

            let mut nav_items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let cb_inner = cb.clone();
                let p_inner = prog.clone();
                let idx = i;
                let is_selected = i == *seleccion;

                let fg_color: Color = if is_selected {
                    scheme.primary.into()
                } else {
                    scheme.on_surface_variant.into()
                };
                let bg_color: Color = if is_selected {
                    scheme.primary_container.into()
                } else {
                    Color::TRANSPARENT
                };

                let icon = view::label(item.icono.clone())
                    .text_size(24.0)
                    .color(fg_color);

                let content: Box<AnyWidgetView<AppStateNativo>> = if *extended {
                    let label_style = get_text_style(&theme.typography, "label_medium");
                    Box::new(view::flex(Axis::Horizontal, (
                        icon,
                        view::label(item.label.clone())
                            .text_size(label_style.font_size as f32)
                            .color(fg_color),
                    )).gap(Length::px(8.0))) as Box<AnyWidgetView<AppStateNativo>>
                } else {
                    Box::new(view::flex(Axis::Vertical, (
                        icon,
                        view::label(item.label.clone())
                            .text_size(10.0)
                            .color(fg_color),
                    )).gap(Length::px(2.0))) as Box<AnyWidgetView<AppStateNativo>>
                };

                let btn = view::button(content, move |data: &mut AppStateNativo| {
                    data.escribir(&cb_inner, ValorGUI::Entero(idx as i64));
                    ejecutar_callback_y_actualizar(&cb_inner, data, &p_inner);
                });

                let styled_btn = view::sized_box(btn)
                    .background(Background::Color(bg_color))
                    .corner_radius(16.0);

                nav_items.push(Box::new(styled_btn) as Box<AnyWidgetView<AppStateNativo>>);
            }

            let axis = if *extended { Axis::Horizontal } else { Axis::Vertical };
            Box::new(
                view::flex(axis, (nav_items,))
                    .gap(Length::px(4.0))
                    .background(Background::Color(scheme.surface.into()))
            )
        }

        // ─── NavigationDrawer ────────────────────────────────────────
        // Cajón lateral con overlay (modal) o sin overlay
        Layout::NavigationDrawer { items, seleccion, on_change, modal, visible } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = on_change.clone();

            if *modal {
                let show = data.leer(visible).to_string() == "true";
                if !show {
                    return Box::new(view::sized_box(view::label(String::new())));
                }
            }

            let mut nav_items: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let cb_inner = cb.clone();
                let p_inner = prog.clone();
                let idx = i;
                let is_selected = i == *seleccion;

                let fg_color: Color = if is_selected {
                    scheme.on_secondary_container.into()
                } else {
                    scheme.on_surface_variant.into()
                };
                let bg_color: Color = if is_selected {
                    scheme.secondary_container.into()
                } else {
                    Color::TRANSPARENT
                };

                let label_style = get_text_style(&theme.typography, "label_large");
                let item_content = view::flex(Axis::Horizontal, (
                    view::label(item.icono.clone())
                        .text_size(24.0)
                        .color(fg_color),
                    view::label(item.label.clone())
                        .text_size(label_style.font_size as f32)
                        .color(fg_color),
                )).gap(Length::px(16.0));

                let btn = view::button(item_content, move |data: &mut AppStateNativo| {
                    data.escribir(&cb_inner, ValorGUI::Entero(idx as i64));
                    ejecutar_callback_y_actualizar(&cb_inner, data, &p_inner);
                });

                let styled_item = view::sized_box(btn)
                    .background(Background::Color(bg_color))
                    .corner_radius(12.0);

                nav_items.push(Box::new(styled_item) as Box<AnyWidgetView<AppStateNativo>>);
            }

            let drawer = view::sized_box(
                view::flex(Axis::Vertical, (nav_items,)).gap(Length::px(4.0))
            ).background(Background::Color(scheme.surface.into()))
             .corner_radius(16.0);

            if *modal {
                let overlay_color: Color = RgbColor(0, 0, 0).with_alpha(0.32);
                Box::new(view::zstack((
                    view::sized_box(view::label(String::new()))
                        .background(Background::Color(overlay_color)),
                    Box::new(drawer) as Box<AnyWidgetView<AppStateNativo>>,
                )))
            } else {
                Box::new(drawer)
            }
        }

        // ─── TopAppBar ───────────────────────────────────────────────
        // Barra superior con título, iconos de acción y variantes
        Layout::TopAppBar { titulo, acciones, menu_visible: _, variant } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();

            let _title_style = match variant {
                TopAppBarVariant::Small => get_text_style(&theme.typography, "title_large"),
                TopAppBarVariant::Medium => get_text_style(&theme.typography, "title_large"),
                TopAppBarVariant::Large => get_text_style(&theme.typography, "headline_medium"),
            };

            let title_size = if *variant == TopAppBarVariant::Large { 28.0 } else { 22.0 };
            let fg_title: Color = scheme.on_surface.into();

            let title_label = view::label(titulo.clone())
                .text_size(title_size as f32)
                .weight(FontWeight::BOLD)
                .color(fg_title);

            // Iconos de acción
            let mut action_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for action in acciones.iter() {
                let cb_inner = action.callback.clone();
                let p_inner = prog.clone();
                let icon_fg: Color = scheme.on_surface_variant.into();
                let icon_btn = view::button(
                    view::label(action.icono.clone())
                        .text_size(24.0)
                        .color(icon_fg),
                    move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb_inner, data, &p_inner);
                    },
                );
                action_widgets.push(Box::new(icon_btn) as Box<AnyWidgetView<AppStateNativo>>);
            }

            let bar = view::flex(Axis::Horizontal, (
                title_label,
                view::flex(Axis::Horizontal, (action_widgets,)).gap(Length::px(4.0)),
            )).gap(Length::px(8.0));

            Box::new(
                view::sized_box(bar)
                    .background(Background::Color(scheme.surface.into()))
                    .padding(16.0)
            )
        }

        // ─── BottomAppBar ────────────────────────────────────────────
        // Barra inferior con acciones y FAB opcional
        Layout::BottomAppBar { acciones, fab } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();

            let mut action_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for action in acciones.iter() {
                let cb_inner = action.callback.clone();
                let p_inner = prog.clone();
                let icon_fg: Color = scheme.on_surface_variant.into();
                let icon_btn = view::button(
                    view::label(action.icono.clone())
                        .text_size(24.0)
                        .color(icon_fg),
                    move |data: &mut AppStateNativo| {
                        ejecutar_callback_y_actualizar(&cb_inner, data, &p_inner);
                    },
                );
                action_widgets.push(Box::new(icon_btn) as Box<AnyWidgetView<AppStateNativo>>);
            }

            let mut children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            children.push(Box::new(
                view::flex(Axis::Horizontal, (action_widgets,)).gap(Length::px(8.0))
            ) as Box<AnyWidgetView<AppStateNativo>>);

            if let Some(f) = fab {
                children.push(layout_a_view(f, data, _prog, theme));
            }

            Box::new(
                view::sized_box(
                    view::flex(Axis::Horizontal, (children,)).gap(Length::px(16.0))
                )
                .background(Background::Color(scheme.surface.into()))
                .padding(8.0)
            )
        }

        // ─── Tabs ────────────────────────────────────────────────────
        // Pestañas con indicador de selección
        Layout::Tabs { tabs, seleccion, on_change, scrollable: _ } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = on_change.clone();

            let mut tab_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for (i, tab) in tabs.iter().enumerate() {
                let cb_inner = cb.clone();
                let p_inner = prog.clone();
                let idx = i;
                let t = tab.clone();
                let is_selected = i == *seleccion;

                let fg_color: Color = if is_selected {
                    scheme.primary.into()
                } else {
                    scheme.on_surface_variant.into()
                };

                let label_style = get_text_style(&theme.typography, "label_large");
                let tab_content = view::flex(Axis::Vertical, (
                    view::label(t.clone())
                        .text_size(label_style.font_size as f32)
                        .weight(if is_selected { FontWeight::BOLD } else { FontWeight::MEDIUM })
                        .color(fg_color),
                    // Indicador de selección
                    if is_selected {
                        Box::new(
                            view::sized_box(view::label(String::new()))
                                .height(Length::px(3.0))
                                .background(Background::Color(scheme.primary.into()))
                        ) as Box<AnyWidgetView<AppStateNativo>>
                    } else {
                        Box::new(
                            view::sized_box(view::label(String::new()))
                                .height(Length::px(3.0))
                        ) as Box<AnyWidgetView<AppStateNativo>>
                    },
                )).gap(Length::px(4.0));

                let btn = view::button(tab_content, move |data: &mut AppStateNativo| {
                    data.escribir(&cb_inner, ValorGUI::Entero(idx as i64));
                    ejecutar_callback_y_actualizar(&cb_inner, data, &p_inner);
                });

                tab_widgets.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
            }

            Box::new(
                view::flex(Axis::Horizontal, (tab_widgets,))
                    .gap(Length::px(0.0))
            )
        }

        // ─── SearchBar ───────────────────────────────────────────────
        // Barra de búsqueda con placeholder
        Layout::SearchBar { placeholder, on_search: _, variable } => {
            let scheme = &theme.scheme;
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let ph = placeholder.clone();

            let bg: Color = scheme.surface_variant.into();
            let _fg: Color = scheme.on_surface.into();
            let icon_fg: Color = scheme.on_surface_variant.into();

            let ti = view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                data.escribir(&var_name, ValorGUI::Texto(new_val));
            }).placeholder(ph.as_str());

            Box::new(
                view::flex(Axis::Horizontal, (
                    view::label("🔍 ")
                        .text_size(18.0)
                        .color(icon_fg),
                    ti,
                )).gap(Length::px(8.0))
                .background(Background::Color(bg))
                .corner_radius(24.0)
                .padding(12.0)
            )
        }

        // ─── SearchView ──────────────────────────────────────────────
        // Vista de búsqueda con resultados
        Layout::SearchView { query: _, resultados, visible } => {
            let scheme = &theme.scheme;
            let show = data.leer(visible).to_string() == "true";
            if show {
                let mut result_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                for r in resultados.iter() {
                    result_widgets.push(layout_a_view(r, data, _prog, theme));
                }
                Box::new(
                    view::sized_box(
                        view::flex(Axis::Vertical, (result_widgets,)).gap(Length::px(8.0))
                    )
                    .background(Background::Color(scheme.surface.into()))
                    .corner_radius(16.0)
                )
            } else {
                Box::new(view::sized_box(view::label(String::new())))
            }
        }

        // ═══════════════════════════════════════════════════════════════
        // INDICADORES (Fase 7)
        // ═══════════════════════════════════════════════════════════════

        // ─── LinearProgress ──────────────────────────────────────────
        Layout::LinearProgress { variable, indeterminado } => {
            let scheme = &theme.scheme;
            let track_color: Color = scheme.surface_variant.into();
            let indicator_color: Color = scheme.primary.into();
            if *indeterminado {
                Box::new(view::zstack((
                    view::sized_box(view::label(String::new())).height(Length::px(4.0)).background(Background::Color(track_color)).corner_radius(2.0),
                    view::sized_box(view::label(String::new())).width(Length::px(60.0)).height(Length::px(4.0)).background(Background::Color(indicator_color)).corner_radius(2.0),
                )))
            } else {
                let valor = data.leer(variable).to_f64() / 100.0;
                Box::new(view::zstack((
                    view::sized_box(view::label(String::new())).height(Length::px(4.0)).background(Background::Color(track_color)).corner_radius(2.0),
                    view::sized_box(view::label(String::new())).width(Length::px(300.0 * valor)).height(Length::px(4.0)).background(Background::Color(indicator_color)).corner_radius(2.0),
                )))
            }
        }

        // ─── CircularProgress ────────────────────────────────────────
        Layout::CircularProgress { variable, size, indeterminado } => {
            let scheme = &theme.scheme;
            let s = *size;
            let track_color: Color = scheme.surface_variant.into();
            let indicator_color: Color = scheme.primary.into();
            if *indeterminado {
                Box::new(view::sized_box(view::label("⟳").text_size((s * 0.5) as f32).color(indicator_color)).width(Length::px(s)).height(Length::px(s)).background(Background::Color(track_color)).corner_radius(s / 2.0))
            } else {
                let _valor = data.leer(variable).to_f64();
                Box::new(view::sized_box(view::label(format!("{:.0}%", _valor)).text_size((s * 0.3) as f32).color(indicator_color)).width(Length::px(s)).height(Length::px(s)).border_color(indicator_color).border_width(4.0).corner_radius(s / 2.0).background(Background::Color(track_color)))
            }
        }

        // ─── Badge ──────────────────────────────────────────────────
        Layout::Badge { child, valor, dot } => {
            let scheme = &theme.scheme;
            let inner = layout_a_view(child, data, _prog, theme);
            let bg_color: Color = scheme.error.into();
            let fg_color: Color = scheme.on_error.into();
            if *dot {
                let dot_w = view::sized_box(view::label(String::new())).width(Length::px(8.0)).height(Length::px(8.0)).background(Background::Color(bg_color)).corner_radius(4.0);
                Box::new(view::zstack((inner, Box::new(dot_w) as Box<AnyWidgetView<AppStateNativo>>)))
            } else {
                let num = valor.clone().unwrap_or_default();
                let badge = view::sized_box(view::label(num).text_size(11.0).color(fg_color)).width(Length::px(18.0)).height(Length::px(18.0)).background(Background::Color(bg_color)).corner_radius(9.0);
                Box::new(view::zstack((inner, Box::new(badge) as Box<AnyWidgetView<AppStateNativo>>)))
            }
        }

        // ─── Skeleton ───────────────────────────────────────────────
        Layout::Skeleton { ancho, alto, tipo } => {
            let scheme = &theme.scheme;
            let sk_color: Color = scheme.surface_variant.into();
            let radius = match tipo.as_str() { "circulo" => ancho / 2.0, "tarjeta" => 12.0, _ => 4.0 };
            Box::new(view::sized_box(view::label(String::new())).width(Length::px(*ancho)).height(Length::px(*alto)).background(Background::Color(sk_color)).corner_radius(radius))
        }

        // ─── EmptyState ─────────────────────────────────────────────
        Layout::EmptyState { icono, mensaje, accion_texto, accion_cb } => {
            let scheme = &theme.scheme;
            let fg_var: Color = scheme.on_surface_variant.into();
            let prog = _prog.to_vec();
            let mut children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            if !icono.is_empty() { children.push(Box::new(view::label(icono.clone()).text_size(48.0)) as Box<AnyWidgetView<AppStateNativo>>); }
            children.push(Box::new(view::label(mensaje.clone()).text_size(16.0).color(fg_var)) as Box<AnyWidgetView<AppStateNativo>>);
            if let Some(texto) = accion_texto {
                if let Some(cb_name) = accion_cb {
                    let cb = cb_name.clone(); let p = prog.clone();
                    let btn = view::button(view::label(texto.clone()).text_size(14.0).weight(FontWeight::MEDIUM).color(scheme.primary.into()), move |data: &mut AppStateNativo| { if !cb.is_empty() { ejecutar_callback_y_actualizar(&cb, data, &p); } });
                    children.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
                } else { children.push(Box::new(view::label(texto.clone()).text_size(14.0).weight(FontWeight::MEDIUM).color(scheme.primary.into())) as Box<AnyWidgetView<AppStateNativo>>); }
            }
            Box::new(view::flex(Axis::Vertical, (children,)).gap(Length::px(12.0)).main_axis_alignment(MainAxisAlignment::Center))
        }

        // ─── ErrorState ─────────────────────────────────────────────
        Layout::ErrorState { mensaje, on_retry } => {
            let scheme = &theme.scheme;
            let fg: Color = scheme.on_surface.into();
            let error_color: Color = scheme.error.into();
            let prog = _prog.to_vec();
            let mut children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            children.push(Box::new(view::label("⚠️").text_size(48.0)) as Box<AnyWidgetView<AppStateNativo>>);
            children.push(Box::new(view::label(mensaje.clone()).text_size(16.0).color(fg)) as Box<AnyWidgetView<AppStateNativo>>);
            if let Some(cb_name) = on_retry {
                let cb = cb_name.clone(); let p = prog.clone();
                let btn = view::button(view::label("Reintentar").text_size(14.0).weight(FontWeight::MEDIUM).color(error_color), move |data: &mut AppStateNativo| { if !cb.is_empty() { ejecutar_callback_y_actualizar(&cb, data, &p); } });
                children.push(Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>);
            }
            Box::new(view::flex(Axis::Vertical, (children,)).gap(Length::px(12.0)).main_axis_alignment(MainAxisAlignment::Center))
        }

        // ═══════════════════════════════════════════════════════════════
        // AVATARES (Fase 7)
        // ═══════════════════════════════════════════════════════════════

        // ─── Avatar ─────────────────────────────────────────────────
        Layout::Avatar { texto, variant, tamaño } => {
            let scheme = &theme.scheme;
            let t = *tamaño;
            let bg_color: Color = scheme.primary_container.into();
            let fg_color: Color = scheme.on_primary_container.into();
            match variant {
                AvatarVariant::Text => {
                    let initials: String = texto.chars().take(2).collect();
                    Box::new(view::sized_box(view::label(initials).text_size((t * 0.4) as f32).weight(FontWeight::BOLD).color(fg_color)).width(Length::px(t)).height(Length::px(t)).background(Background::Color(bg_color)).corner_radius(t / 2.0))
                }
                AvatarVariant::Icon => {
                    Box::new(view::sized_box(view::label(texto.clone()).text_size((t * 0.5) as f32).color(fg_color)).width(Length::px(t)).height(Length::px(t)).background(Background::Color(bg_color)).corner_radius(t / 2.0))
                }
                AvatarVariant::Image => {
                    Box::new(view::sized_box(view::label("🖼").text_size((t * 0.5) as f32)).width(Length::px(t)).height(Length::px(t)).background(Background::Color(bg_color)).corner_radius(t / 2.0))
                }
            }
        }

        // ─── AvatarGroup ────────────────────────────────────────────
        Layout::AvatarGroup { avatares, max } => {
            let scheme = &theme.scheme;
            let bg_color: Color = scheme.primary_container.into();
            let fg_color: Color = scheme.on_primary_container.into();
            let avatar_size = 32.0; let overlap = 12.0;
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            let count = avatares.len().min(*max);
            for i in 0..count {
                let initials: String = avatares[i].chars().take(2).collect();
                let avatar = view::sized_box(view::label(initials).text_size(12.0).weight(FontWeight::BOLD).color(fg_color)).width(Length::px(avatar_size)).height(Length::px(avatar_size)).background(Background::Color(bg_color)).corner_radius(avatar_size / 2.0).border_color(scheme.surface.into()).border_width(2.0);
                widgets.push(Box::new(view::sized_box(avatar).width(Length::px(avatar_size + i as f64 * overlap))) as Box<AnyWidgetView<AppStateNativo>>);
            }
            if avatares.len() > *max {
                let remaining = avatares.len() - *max;
                let more = view::sized_box(view::label(format!("+{}", remaining)).text_size(11.0).weight(FontWeight::BOLD).color(fg_color)).width(Length::px(avatar_size)).height(Length::px(avatar_size)).background(Background::Color(scheme.surface_variant.into())).corner_radius(avatar_size / 2.0).border_color(scheme.surface.into()).border_width(2.0);
                widgets.push(Box::new(more) as Box<AnyWidgetView<AppStateNativo>>);
            }
            Box::new(view::flex(Axis::Horizontal, (widgets,)).gap(Length::px(-overlap)))
        }

        // ═══════════════════════════════════════════════════════════════
        // MOTION (Fase 8)
        // ═══════════════════════════════════════════════════════════════

        // ─── FadeTransition ──────────────────────────────────────────
        Layout::FadeTransition { child, visible, duracion: _ } => {
            let show = data.leer(visible).to_string() == "true";
            if show { layout_a_view(child, data, _prog, theme) } else { Box::new(view::sized_box(view::label(String::new()))) }
        }

        // ─── RippleEffect ───────────────────────────────────────────
        Layout::RippleEffect { child, color: _ } => {
            let inner = layout_a_view(child, data, _prog, theme);
            Box::new(view::sized_box(inner).corner_radius(4.0))
        }

        // ─── PullToRefresh ────────────────────────────────────────────
        Layout::PullToRefresh { child, callback, refreshing } => {
            let is_refreshing = data.leer(refreshing).to_bool();
            let prog = _prog.to_vec();
            let cb = callback.clone();
            let ref_var = refreshing.clone();

            // Botón de recargar en la parte superior
            let scheme = &theme.scheme;
            let label_style = get_text_style(&theme.typography, "label_medium");
            let fg: Color = scheme.primary.into();
            let refresh_label = if is_refreshing {
                view::label("🔄 Actualizando...")
                    .text_size(label_style.font_size as f32)
                    .color(fg)
            } else {
                view::label("⬇️ Tira para recargar")
                    .text_size(label_style.font_size as f32)
                    .color(fg)
            };

            let btn = view::button(refresh_label, move |data: &mut AppStateNativo| {
                if !cb.is_empty() {
                    data.escribir(&ref_var, ValorGUI::Booleano(true));
                    ejecutar_callback_y_actualizar(&cb, data, &prog);
                }
            });
            let header = Box::new(view::sized_box(btn).padding(8.0)) as Box<AnyWidgetView<AppStateNativo>>;

            let inner = layout_a_view(child, data, _prog, theme);

            // TODO xilem 0.4: cuando soporte gesture detection, reemplazar por drag-down gesture.
            // Por ahora usamos un botón como activador manual.
            let mut column_children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            column_children.push(header);
            column_children.push(inner);
            Box::new(view::portal(view::flex(Axis::Vertical, (column_children,)).gap(Length::px(4.0))))
        }

        // ─── SwipeToDismiss ───────────────────────────────────────────
        Layout::SwipeToDismiss { child, on_dismiss, label, dismissed } => {
            let is_dismissed = data.leer(dismissed).to_bool();
            let prog = _prog.to_vec();
            let cb = on_dismiss.clone();
            let dismiss_var = dismissed.clone();
            let action_text = label.clone();

            if is_dismissed {
                // Ya descartado: mostrar opción de deshacer
                let scheme = &theme.scheme;
                let fg: Color = scheme.on_surface.into();
                let btn_fg: Color = scheme.primary.into();
                let undo_label = view::label("🗑️ Elemento descartado")
                    .text_size(14.0)
                    .color(fg);
                let undo_btn = view::button(
                    view::label(format!("↩️ {}", action_text))
                        .text_size(14.0)
                        .weight(FontWeight::MEDIUM)
                        .color(btn_fg),
                    move |data: &mut AppStateNativo| {
                        data.escribir(&dismiss_var, ValorGUI::Booleano(false));
                    },
                );
                let mut row_children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                row_children.push(Box::new(undo_label) as Box<AnyWidgetView<AppStateNativo>>);
                row_children.push(Box::new(view::sized_box(view::label(String::new())).width(Length::px(8.0)))
                    as Box<AnyWidgetView<AppStateNativo>>);
                row_children.push(Box::new(undo_btn) as Box<AnyWidgetView<AppStateNativo>>);
                Box::new(view::sized_box(
                    view::flex(Axis::Horizontal, (row_children,)).gap(Length::px(4.0))
                ).padding(12.0).corner_radius(8.0))
            } else {
                // Mostrar contenido + botón de descartar
                let inner = layout_a_view(child, data, _prog, theme);
                let scheme = &theme.scheme;
                let dismiss_fg: Color = scheme.error.into();
                let prog_clone = prog.clone();
                let cb_clone = cb.clone();
                let dismiss_var_clone = dismiss_var.clone();

                let action_btn = view::button(
                    view::label("✕")
                        .text_size(18.0)
                        .weight(FontWeight::BOLD)
                        .color(dismiss_fg),
                    move |data: &mut AppStateNativo| {
                        data.escribir(&dismiss_var_clone, ValorGUI::Booleano(true));
                        if !cb_clone.is_empty() {
                            ejecutar_callback_y_actualizar(&cb_clone, data, &prog_clone);
                        }
                    },
                );
                let dismiss_btn = Box::new(view::sized_box(action_btn).padding(8.0))
                    as Box<AnyWidgetView<AppStateNativo>>;

                let mut swipe_children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                swipe_children.push(inner);
                swipe_children.push(dismiss_btn);

                // TODO xilem 0.4: cuando soporte gestos de deslizamiento, reemplazar por swipe gesture.
                // Por ahora usamos un botón ✕ como activador manual.
                Box::new(view::flex(Axis::Horizontal, (swipe_children,)).gap(Length::px(4.0)))
            }
        }

        // ═══════════════════════════════════════════════════════════════
        // GRÁFICOS (Fase 9)
        // ═══════════════════════════════════════════════════════════════

        // ─── Constante de colores para gráficos ─────────────────────
        // Usamos los colores del tema en lugar de constantes fijas

        // ─── LineChart ────────────────────────────────────────────────
        Layout::LineChart { datos, color, etiquetas } => {
            let scheme = &theme.scheme;
            let max = datos.iter().cloned().fold(0.0_f64, f64::max);
            let bar_color: Color = get_color_role(&theme.scheme, color).into();
            let child_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = datos.iter().enumerate().map(|(i, &v)| {
                let altura = if max > 0.0 { (v / max) * 80.0 } else { 0.0 };
                let label_txt = etiquetas.get(i).cloned().unwrap_or_default();
                let col = if max > 0.0 {
                    let bar = view::sized_box(view::label(String::new()))
                        .height(Length::px(altura.max(2.0)))
                        .width(Length::px(24.0))
                        .background(Background::Color(bar_color))
                        .corner_radius(4.0);
                    Box::new(view::flex(Axis::Vertical, (bar,)).gap(Length::px(2.0)).cross_axis_alignment(CrossAxisAlignment::Center)) as Box<AnyWidgetView<AppStateNativo>>
                } else {
                    Box::new(view::sized_box(view::label(String::new())).height(Length::px(2.0)).width(Length::px(24.0))) as Box<AnyWidgetView<AppStateNativo>>
                };
                if !label_txt.is_empty() {
                    let lbl = view::label(label_txt).text_size(10.0).color(scheme.on_surface.into());
                    Box::new(view::flex(Axis::Vertical, (col, Box::new(lbl) as Box<AnyWidgetView<AppStateNativo>>)).gap(Length::px(4.0)).cross_axis_alignment(CrossAxisAlignment::Center)) as Box<AnyWidgetView<AppStateNativo>>
                } else { col }
            }).collect();
            Box::new(view::flex(Axis::Horizontal, (child_widgets,)).gap(Length::px(8.0)).cross_axis_alignment(CrossAxisAlignment::End))
        }

        // ─── BarChart ────────────────────────────────────────────────
        Layout::BarChart { datos, colores, etiquetas, apilado: _ } => {
            let scheme = &theme.scheme;
            let max = datos.iter().cloned().fold(0.0_f64, f64::max);
            let child_widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = datos.iter().enumerate().map(|(i, &v)| {
                let altura = if max > 0.0 { (v / max) * 100.0 } else { 0.0 };
                let c = colores.get(i).cloned().unwrap_or_else(|| "primary".to_string());
                let bar_color: Color = get_color_role(&theme.scheme, &c).into();
                let label_txt = etiquetas.get(i).cloned().unwrap_or_default();
                let bar = view::sized_box(view::label(format!("{:.0}", v)).text_size(9.0).color(scheme.on_primary.into()))
                    .height(Length::px(altura.max(2.0)))
                    .width(Length::px(40.0))
                    .background(Background::Color(bar_color))
                    .corner_radius(4.0);
                let lbl = view::label(label_txt).text_size(10.0).color(scheme.on_surface.into());
                Box::new(view::flex(Axis::Vertical, (Box::new(bar) as Box<AnyWidgetView<AppStateNativo>>, Box::new(lbl) as Box<AnyWidgetView<AppStateNativo>>)).gap(Length::px(4.0)).cross_axis_alignment(CrossAxisAlignment::Center)) as Box<AnyWidgetView<AppStateNativo>>
            }).collect();
            Box::new(view::flex(Axis::Horizontal, (child_widgets,)).gap(Length::px(8.0)).cross_axis_alignment(CrossAxisAlignment::End))
        }

        // ─── PieChart / Donut ─────────────────────────────────────────
        Layout::PieChart { datos, etiquetas, donut } => {
            let scheme = &theme.scheme;
            let colores_graficos = [
                scheme.primary, scheme.secondary, scheme.tertiary,
                scheme.error, scheme.primary_container, scheme.secondary_container,
                scheme.tertiary_container, scheme.error_container,
            ];
            let total: f64 = datos.iter().sum();
            let children: Vec<Box<AnyWidgetView<AppStateNativo>>> = datos.iter().enumerate().map(|(i, &v)| {
                let pct = if total > 0.0 { (v / total) * 100.0 } else { 0.0 };
                let c: Color = colores_graficos[i % colores_graficos.len()].into();
                let dot = view::sized_box(view::label(String::new())).width(Length::px(12.0)).height(Length::px(12.0)).corner_radius(6.0).background(Background::Color(c));
                let lbl = view::label(format!("{}: {:.1}%", etiquetas.get(i).cloned().unwrap_or_default(), pct)).text_size(13.0).color(scheme.on_surface.into());
                Box::new(view::flex(Axis::Horizontal, (Box::new(dot) as Box<AnyWidgetView<AppStateNativo>>, Box::new(lbl) as Box<AnyWidgetView<AppStateNativo>>)).gap(Length::px(8.0)).cross_axis_alignment(CrossAxisAlignment::Center)) as Box<AnyWidgetView<AppStateNativo>>
            }).collect();
            let inner = view::flex(Axis::Vertical, (children,)).gap(Length::px(6.0));
            if *donut {
                let center = view::label("◯").text_size(32.0).color(scheme.primary.into());
                Box::new(view::flex(Axis::Horizontal, (Box::new(inner) as Box<AnyWidgetView<AppStateNativo>>, Box::new(center) as Box<AnyWidgetView<AppStateNativo>>)).gap(Length::px(12.0)).cross_axis_alignment(CrossAxisAlignment::Center))
            } else {
                Box::new(inner)
            }
        }

        // ─── GaugeChart ──────────────────────────────────────────────
        Layout::GaugeChart { valor, min, max, color } => {
            let scheme = &theme.scheme;
            let gauge_color: Color = get_color_role(&theme.scheme, color).into();
            let rango = if max > min { max - min } else { 1.0 };
            let pct = ((valor - min) / rango).clamp(0.0, 1.0);
            let ancho_barra = 200.0;
            let track = view::sized_box(view::label(String::new())).width(Length::px(ancho_barra)).height(Length::px(12.0)).background(Background::Color(scheme.surface_variant.into())).corner_radius(6.0);
            let fill = view::sized_box(view::label(String::new())).width(Length::px(ancho_barra * pct)).height(Length::px(12.0)).background(Background::Color(gauge_color)).corner_radius(6.0);
            let bar = view::zstack((Box::new(track) as Box<AnyWidgetView<AppStateNativo>>, Box::new(fill) as Box<AnyWidgetView<AppStateNativo>>));
            let txt = view::label(format!("{:.0} / {:.0}", valor, max)).text_size(14.0).weight(FontWeight::BOLD).color(scheme.on_surface.into());
            Box::new(view::flex(Axis::Vertical, (Box::new(bar) as Box<AnyWidgetView<AppStateNativo>>, Box::new(txt) as Box<AnyWidgetView<AppStateNativo>>)).gap(Length::px(8.0)).cross_axis_alignment(CrossAxisAlignment::Center))
        }

        // ─── Sparkline ───────────────────────────────────────────────
        Layout::Sparkline { datos, color } => {
            let scheme = &theme.scheme;
            let spark_color: Color = get_color_role(scheme, color).into();
            let max = datos.iter().cloned().fold(0.0_f64, f64::max).max(1.0);
            let ancho_bar = if !datos.is_empty() { 20.0 / datos.len() as f64 } else { 4.0 };
            let bars: Vec<Box<AnyWidgetView<AppStateNativo>>> = datos.iter().map(|&v| {
                let h = (v / max) * 24.0;
                Box::new(view::sized_box(view::label(String::new())).width(Length::px(ancho_bar.max(2.0))).height(Length::px(h.max(2.0))).background(Background::Color(spark_color)).corner_radius(1.0)) as Box<AnyWidgetView<AppStateNativo>>
            }).collect();
            Box::new(view::flex(Axis::Horizontal, (bars,)).gap(Length::px(1.0)).cross_axis_alignment(CrossAxisAlignment::End))
        }

        // ═══════════════════════════════════════════════════════════════
        // AVANZADOS (Fase 9)
        // ═══════════════════════════════════════════════════════════════

        // ─── StarRating ──────────────────────────────────────────────
        Layout::StarRating { valor, max, callback } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = callback.clone();
            let stars: Vec<Box<AnyWidgetView<AppStateNativo>>> = (0..*max).map(|i| {
                let llena = i < *valor;
                let cb_name = cb.clone();
                let p = prog.clone();
                let star = if llena { "★" } else { "☆" };
                let star_color: Color = if llena { scheme.primary.into() } else { scheme.on_surface_variant.into() };
                let btn = view::button(
                    view::label(star.to_string()).text_size(24.0).color(star_color),
                    move |data: &mut AppStateNativo| {
                        data.escribir("rating", ValorGUI::Entero((i + 1) as i64));
                        if !cb_name.is_empty() {
                            ejecutar_callback_y_actualizar(&cb_name, data, &p);
                        }
                    },
                );
                Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>
            }).collect();
            Box::new(view::flex(Axis::Horizontal, (stars,)).gap(Length::px(4.0)))
        }

        // ─── Stepper ─────────────────────────────────────────────────
        Layout::Stepper { pasos, actual, callback } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = callback.clone();
            let steps: Vec<Box<AnyWidgetView<AppStateNativo>>> = pasos.iter().enumerate().map(|(i, paso)| {
                let active = i == *actual;
                let completed = i < *actual;
                let num_color: Color = if active { scheme.on_primary.into() } else if completed { scheme.on_primary_container.into() } else { scheme.on_surface_variant.into() };
                let bg_color = if active { scheme.primary.into() } else if completed { scheme.primary_container.into() } else { scheme.surface_variant.into() };
                let num_text = if completed { "✓".to_string() } else { (i+1).to_string() };
                let num = view::label(num_text).text_size(14.0).weight(FontWeight::BOLD).color(num_color);
                let circle = view::sized_box(num).width(Length::px(32.0)).height(Length::px(32.0)).background(Background::Color(bg_color)).corner_radius(16.0);
                let label = view::label(paso.clone()).text_size(12.0).color(scheme.on_surface.into());
                let cb_name = cb.clone();
                let p = prog.clone();
                let btn = view::button(view::flex(Axis::Vertical, (Box::new(circle) as Box<AnyWidgetView<AppStateNativo>>, Box::new(label) as Box<AnyWidgetView<AppStateNativo>>)).gap(Length::px(4.0)).cross_axis_alignment(CrossAxisAlignment::Center), move |data: &mut AppStateNativo| {
                    data.escribir("step", ValorGUI::Entero(i as i64));
                    if !cb_name.is_empty() {
                        ejecutar_callback_y_actualizar(&cb_name, data, &p);
                    }
                });
                Box::new(btn) as Box<AnyWidgetView<AppStateNativo>>
            }).collect();
            Box::new(view::flex(Axis::Horizontal, (steps,)).gap(Length::px(16.0)).cross_axis_alignment(CrossAxisAlignment::Center))
        }

        // ─── Breadcrumbs ─────────────────────────────────────────────
        Layout::Breadcrumbs { items, separador } => {
            let scheme = &theme.scheme;
            let crumbs: Vec<Box<AnyWidgetView<AppStateNativo>>> = items.iter().enumerate().flat_map(|(i, item)| {
                let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                if i > 0 {
                    let sep = view::label(separador.clone()).text_size(16.0).color(scheme.on_surface_variant.into());
                    widgets.push(Box::new(sep) as Box<AnyWidgetView<AppStateNativo>>);
                }
                let is_last = i == items.len() - 1;
                let lbl = view::label(item.clone()).text_size(14.0).weight(if is_last { FontWeight::BOLD } else { FontWeight::NORMAL }).color(if is_last { scheme.on_surface.into() } else { scheme.primary.into() });
                widgets.push(Box::new(lbl) as Box<AnyWidgetView<AppStateNativo>>);
                widgets
            }).collect();
            Box::new(view::flex(Axis::Horizontal, (crumbs,)).gap(Length::px(4.0)).cross_axis_alignment(CrossAxisAlignment::Center))
        }

        // ─── Calendar ────────────────────────────────────────────────
        Layout::Calendar { mes, año, seleccionado: _, callback } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = callback.clone();
            // Copiar valores para el closure (evita capturar referencias al layout)
            let mes_val = *mes;
            let año_val = *año;
            let dias_semana = vec!["L", "M", "M", "J", "V", "S", "D"];
            let header_dias: Vec<Box<AnyWidgetView<AppStateNativo>>> = dias_semana.iter().map(|d| {
                Box::new(view::sized_box(view::label(d.to_string()).text_size(11.0).weight(FontWeight::BOLD).color(scheme.on_surface_variant.into())).width(Length::px(36.0)).height(Length::px(36.0))) as Box<AnyWidgetView<AppStateNativo>>
            }).collect();
            let header = view::flex(Axis::Horizontal, (header_dias,)).gap(Length::px(2.0));
            let title = view::label(format!("{}/{}", mes_val, año_val)).text_size(16.0).weight(FontWeight::BOLD).color(scheme.on_surface.into());
            // Generar algunos días del mes (simplificado: 28 días)
            let mut day_rows: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for semana in 0..4 {
                let mut dias_fila: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
                for dia in 0..7 {
                    let dia_num = semana * 7 + dia + 1;
                    if dia_num <= 28 {
                        let cb_name = cb.clone();
                        let p = prog.clone();
                        let dia_btn = view::button(
                            view::label(dia_num.to_string()).text_size(12.0).color(scheme.on_surface.into()),
                            move |data: &mut AppStateNativo| {
                                data.escribir("fecha", ValorGUI::Texto(format!("{}/{}/{}", dia_num, mes_val, año_val)));
                                if !cb_name.is_empty() {
                                    ejecutar_callback_y_actualizar(&cb_name, data, &p);
                                }
                            },
                        );
                        dias_fila.push(Box::new(view::sized_box(dia_btn).width(Length::px(36.0)).height(Length::px(36.0))) as Box<AnyWidgetView<AppStateNativo>>);
                    } else {
                        dias_fila.push(Box::new(view::sized_box(view::label(String::new())).width(Length::px(36.0)).height(Length::px(36.0))) as Box<AnyWidgetView<AppStateNativo>>);
                    }
                }
                day_rows.push(Box::new(view::flex(Axis::Horizontal, (dias_fila,)).gap(Length::px(2.0))) as Box<AnyWidgetView<AppStateNativo>>);
            }
            let mut all: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            all.push(Box::new(title) as Box<AnyWidgetView<AppStateNativo>>);
            all.push(Box::new(header) as Box<AnyWidgetView<AppStateNativo>>);
            for row in day_rows { all.push(row); }
            Box::new(view::flex(Axis::Vertical, (all,)).gap(Length::px(4.0)).cross_axis_alignment(CrossAxisAlignment::Center))
        }

        // ─── MarkdownViewer ──────────────────────────────────────────
        Layout::MarkdownViewer { texto } => {
            let scheme = &theme.scheme;
            // Renderizado simplificado: reemplaza # y ** para simular formato
            let lines: Vec<&str> = texto.split('\n').collect();
            let mut children: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for line in lines {
                let txt = line.to_string();
                if txt.starts_with("# ") {
                    let content = txt.trim_start_matches("# ");
                    children.push(Box::new(view::label(content.to_string()).text_size(24.0).weight(FontWeight::BOLD).color(scheme.on_surface.into())) as Box<AnyWidgetView<AppStateNativo>>);
                } else if txt.starts_with("## ") {
                    let content = txt.trim_start_matches("## ");
                    children.push(Box::new(view::label(content.to_string()).text_size(20.0).weight(FontWeight::BOLD).color(scheme.on_surface.into())) as Box<AnyWidgetView<AppStateNativo>>);
                } else if txt.starts_with("### ") {
                    let content = txt.trim_start_matches("### ");
                    children.push(Box::new(view::label(content.to_string()).text_size(16.0).weight(FontWeight::BOLD).color(scheme.on_surface.into())) as Box<AnyWidgetView<AppStateNativo>>);
                } else if txt.starts_with("- ") || txt.starts_with("* ") {
                    let content = txt.trim_start_matches("- ").trim_start_matches("* ");
                    children.push(Box::new(view::label(format!("  • {}", content)).text_size(14.0).color(scheme.on_surface.into())) as Box<AnyWidgetView<AppStateNativo>>);
                } else if txt.trim().is_empty() {
                    children.push(Box::new(view::sized_box(view::label(String::new())).height(Length::px(8.0))) as Box<AnyWidgetView<AppStateNativo>>);
                } else {
                    // Bold y cursiva simulados
                    let processed = txt.replace("**", "").replace("*", "").replace("__", "");
                    children.push(Box::new(view::label(processed).text_size(14.0).color(scheme.on_surface.into())) as Box<AnyWidgetView<AppStateNativo>>);
                }
            }
            Box::new(view::flex(Axis::Vertical, (children,)).gap(Length::px(4.0)))
        }

        // ─── QRCode ──────────────────────────────────────────────────
        Layout::QRCode { texto, tamaño } => {
            let scheme = &theme.scheme;
            let t = *tamaño;
            // Representación visual simplificada: muestra un marco con texto
            let fg: Color = scheme.primary.into();
            let bg: Color = scheme.surface.into();
            let content = if texto.len() > 20 { format!("{}…", &texto[..20]) } else { texto.clone() };
            let qr_art = view::sized_box(view::label("▄▄▄▄▄▄▄▄\n█  █  █\n█ ▀▀█ █\n█  █  █\n▀▀▀▀▀▀▀▀").text_size(8.0).color(fg)).width(Length::px(t)).height(Length::px(t)).background(Background::Color(bg)).corner_radius(8.0).border_color(fg).border_width(2.0);
            let lbl = view::label(content).text_size(10.0).color(scheme.on_surface.into());
            Box::new(view::flex(Axis::Vertical, (Box::new(qr_art) as Box<AnyWidgetView<AppStateNativo>>, Box::new(lbl) as Box<AnyWidgetView<AppStateNativo>>)).gap(Length::px(8.0)).cross_axis_alignment(CrossAxisAlignment::Center))
        }

        // ─── FilePicker ──────────────────────────────────────────────
        Layout::FilePicker { tipos: _, multiple: _, callback } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = callback.clone();
            let btn = view::button(
                view::flex(Axis::Horizontal, (
                    Box::new(view::label("📁").text_size(18.0)) as Box<AnyWidgetView<AppStateNativo>>,
                    Box::new(view::label("Seleccionar archivo").text_size(14.0).weight(FontWeight::MEDIUM)) as Box<AnyWidgetView<AppStateNativo>>,
                )).gap(Length::px(8.0)).cross_axis_alignment(CrossAxisAlignment::Center),
                move |data: &mut AppStateNativo| {
                    data.escribir("archivo", ValorGUI::Texto("archivo_seleccionado.txt".to_string()));
                    if !cb.is_empty() {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    }
                },
            );
            Box::new(view::sized_box(btn).corner_radius(8.0).background(Background::Color(scheme.primary_container.into())).border_color(scheme.primary.into()).border_width(1.0))
        }

        // ═══════════════════════════════════════════════════════════════
        // EXPRESSIVE (Fase 10)
        // ═══════════════════════════════════════════════════════════════

        // ─── GlassCard ───────────────────────────────────────────────
        Layout::GlassCard { child, blur: _, opacity } => {
            let _scheme = &theme.scheme;
            let inner = layout_a_view(child, data, _prog, theme);
            // Glassmorphism simulado con alpha blending
            // with_alpha() ya retorna xilem::Color (alias Color)
            let bg_color = RgbColor(255, 255, 255).with_alpha(*opacity);
            let border_color = RgbColor(255, 255, 255).with_alpha(0.3);
            Box::new(
                view::sized_box(inner)
                    .background(Background::Color(bg_color))
                    .corner_radius(16.0)
                    .border_color(border_color)
                    .border_width(1.0)
            )
        }

        // ─── GradientBox ─────────────────────────────────────────────
        Layout::GradientBox { child, colores, direccion: _ } => {
            let scheme = &theme.scheme;
            let inner = layout_a_view(child, data, _prog, theme);
            // Gradiente simulado: usar el primer color como fondo sólido
            let color = if !colores.is_empty() {
                let c = parse_color(&colores[0]).unwrap_or(scheme.primary);
                c.into()
            } else {
                scheme.primary.into()
            };
            Box::new(
                view::sized_box(inner)
                    .background(Background::Color(color))
                    .corner_radius(12.0)
            )
        }

        // ─── MorphingButton ──────────────────────────────────────────
        Layout::MorphingButton { icono, texto_extendido, callback } => {
            let scheme = &theme.scheme;
            let prog = _prog.to_vec();
            let cb = callback.clone();
            let icon = icono.clone();
            let txt = texto_extendido.clone();
            let btn = view::button(
                view::flex(Axis::Horizontal, (
                    Box::new(view::label(icon).text_size(20.0)) as Box<AnyWidgetView<AppStateNativo>>,
                    Box::new(view::label(txt).text_size(14.0).weight(FontWeight::MEDIUM)) as Box<AnyWidgetView<AppStateNativo>>,
                )).gap(Length::px(8.0)).cross_axis_alignment(CrossAxisAlignment::Center),
                move |data: &mut AppStateNativo| {
                    if !cb.is_empty() {
                        ejecutar_callback_y_actualizar(&cb, data, &prog);
                    }
                },
            );
            Box::new(view::sized_box(btn).corner_radius(24.0).background(Background::Color(scheme.primary_container.into())).border_color(scheme.primary.into()).border_width(2.0))
        }

        // ─── ExpressiveBackground ────────────────────────────────────
        Layout::ExpressiveBackground { colores, animado: _ } => {
            let scheme = &theme.scheme;
            let bg_color = if !colores.is_empty() {
                let c = parse_color(&colores[0]).unwrap_or(scheme.primary);
                c.into()
            } else {
                scheme.primary_container.into()
            };
            // Layout de fondo expansivo con gradiente simulado (múltiples capas)
            let base = view::sized_box(view::label(String::new())).width(Length::px(300.0)).height(Length::px(120.0)).background(Background::Color(bg_color)).corner_radius(16.0);
            if colores.len() > 1 {
                let c2 = parse_color(&colores[1]).unwrap_or(scheme.secondary);
                let overlay = view::sized_box(view::label(String::new())).width(Length::px(300.0)).height(Length::px(60.0)).background(Background::Color(c2.into())).corner_radius(16.0);
                Box::new(view::zstack((Box::new(base) as Box<AnyWidgetView<AppStateNativo>>, Box::new(overlay) as Box<AnyWidgetView<AppStateNativo>>)))
            } else {
                Box::new(base)
            }
        }

        // ─── GlowBorder ──────────────────────────────────────────────
        Layout::GlowBorder { child, color, ancho } => {
            let scheme = &theme.scheme;
            let inner = layout_a_view(child, data, _prog, theme);
            let glow_color: Color = get_color_role(scheme, color).into();
            Box::new(
                view::sized_box(inner)
                    .border_color(glow_color)
                    .border_width(*ancho)
                    .corner_radius(12.0)
            )
        }

        // ═══════════════════════════════════════════════════════════════════
        // ICONOS MATERIAL DESIGN — renderizado
        // ═══════════════════════════════════════════════════════════════════

        // MaterialIconLayout: icono vectorial con tamaño y color
        // Por ahora renderiza como emoji con tamaño y color (Xilem 0.4 no tiene SVG)
        // Cuando Xilem soporte SVG nativo, aquí se renderizarán los paths reales.
        Layout::MaterialIconLayout { nombre, tamaño, color, estilo: _ } => {
            let scheme = &theme.scheme;
            let icon_color: RgbColor = parse_color(color).unwrap_or(scheme.primary);
            let icon_size = *tamaño;

            // Obtener el emoji de fallback para el nombre del icono
            let emoji = icons::catalog::fallback_emoji(nombre);

            // Convertir color a Color de xilem (accesible vía crate re-export)
            let xilem_color: Color = icon_color.into();

            // Renderizar como label con emoji, tamaño y color
            // (Xilem 0.4 no tiene renderizado SVG nativo)
            Box::new(view::label(emoji)
                .text_size(icon_size as f32)
                .color(xilem_color))
        }

    }
}

// ═══════════════════════════════════════════════════════════════════
// Window Width Observer — detección de resize vía widget personalizado
// ═══════════════════════════════════════════════════════════════════

use std::cell::Cell;
use std::marker::PhantomData;

// Almacena el ancho actual de la ventana para que la función de vista lo lea.
thread_local! {
    static WINDOW_WIDTH_TL: Cell<f64> = const { Cell::new(800.0) };
}

/// Obtiene el ancho de ventana almacenado en el thread-local.
pub fn current_window_width() -> f64 {
    WINDOW_WIDTH_TL.with(|w| w.get())
}

/// Widget Masonry que observa el ancho disponible durante el layout
/// y lo almacena en el thread-local para que la vista Xilem lo lea.
pub struct WindowWidthProbe {
    child: Option<WidgetPod<dyn Widget>>,
}

impl WindowWidthProbe {
    /// Crea un nuevo probe que envuelve al widget hijo.
    pub fn new(child: NewWidget<impl Widget + ?Sized>) -> Self {
        Self {
            child: Some(child.erased().to_pod()),
        }
    }

    /// Obtiene una referencia mutable al widget hijo.
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        let child = this.widget.child.as_mut().expect("WidthProbe always has child");
        this.ctx.get_mut(child)
    }
}

impl Widget for WindowWidthProbe {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        if let Some(ref mut child) = self.child {
            ctx.register_child(child);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        // Guardar el ancho disponible en el thread-local
        let width = bc.max().width;
        WINDOW_WIDTH_TL.with(|w| w.set(width));

        // Layout del hijo ocupando todo el espacio disponible
        match self.child.as_mut() {
            Some(child) => {
                let size = ctx.run_layout(child, bc);
                ctx.place_child(child, Point::ORIGIN);
                size
            }
            None => bc.constrain((800.0, 600.0)),
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {}

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        if let Some(child) = &self.child {
            ChildrenIds::from_slice(&[child.id()])
        } else {
            ChildrenIds::from_slice(&[])
        }
    }
}

// ─── View wrapper que inserta el WindowWidthProbe en el árbol ─────

/// Wrapper de Xilem View que envuelve la vista raíz con `WindowWidthProbe`
/// para detectar resize automáticamente.
pub struct WidthObservedView<V, State> {
    inner: V,
    phantom: PhantomData<fn() -> State>,
}

/// Envuelve cualquier `WidgetView` con detección de resize.
pub fn observe_width<State, V: WidgetView<State>>(inner: V) -> WidthObservedView<V, State> {
    WidthObservedView {
        inner,
        phantom: PhantomData,
    }
}

impl<V: WidgetView<State>, State: 'static> ViewMarker for WidthObservedView<V, State> {}

impl<V: WidgetView<State>, State: 'static> View<State, (), ViewCtx> for WidthObservedView<V, State> {
    type Element = Pod<WindowWidthProbe>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child, child_state) = self.inner.build(ctx, app_state);
        let widget = WindowWidthProbe::new(child.new_widget);
        (ctx.create_pod(widget), child_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let mut child = WindowWidthProbe::child_mut(&mut element);
        self.inner
            .rebuild(&prev.inner, view_state, ctx, child.downcast(), app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let mut child = WindowWidthProbe::child_mut(&mut element);
        self.inner.teardown(view_state, ctx, child.downcast());
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageContext,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<()> {
        let mut child = WindowWidthProbe::child_mut(&mut element);
        self.inner
            .message(view_state, message, child.downcast(), app_state)
    }
}

// WidthObservedView implementa WidgetView automáticamente vía el blanket impl de xilem
// (requiere View<..., Element=Pod<W>> + Send + Sync, que WidthObservedView cumple)

// ─── Punto de entrada ─────────────────────────────────────────────

/// Construye y ejecuta la ventana GUI nativa.
///
/// Acepta un `theme` opcional; si es `None`, usa `MaterialTheme::light()`.
///
/// El tamaño inicial de ventana es 1200×800 (suficiente para Expanded).
/// En cada render, se lee el ancho real desde el widget WindowWidthProbe
/// para actualizar `window_size` dinámicamente según el resize.
///
/// `auto_theme`: si es `true`, detecta automáticamente el tema del sistema
pub fn build_and_run(
    programa: &Programa,
    theme: Option<MaterialTheme>,
    auto_theme: bool,
) -> Result<(), String> {
    let theme = if auto_theme {
        MaterialTheme::system("#6750A4")
    } else {
        theme.unwrap_or_else(MaterialTheme::light)
    };

    let mut state = AppStateNativo::new();
    inicializar_estado(&programa.declaraciones, &mut state);
    let layout = extraer_layout(&programa.declaraciones);
    let prog = programa.declaraciones.clone();

    println!("  🪟 Lanzando ventana GUI nativa...");
    if theme.is_dark {
        println!("  🌙 Tema oscuro activo (seed: {})", theme.seed_color);
    } else {
        println!("  ☀️ Tema claro activo (seed: {})", theme.seed_color);
    }

    // Inicializar window_width con un valor por defecto (se actualizará en cada render)
    let initial_width: f64 = 1200.0;
    state.update_window_size(initial_width);
    WINDOW_WIDTH_TL.with(|w| w.set(initial_width));
    println!("  📐 Clase de tamaño inicial: {:?} ({:.0}px)", state.window_size, initial_width);

    // Crear la app con WindowOptions que fija el tamaño inicial de la ventana
    let app = Xilem::new_simple(
        state,
        move |data: &mut AppStateNativo| -> Box<AnyWidgetView<AppStateNativo>> {
            // Leer el ancho actual desde el thread-local (actualizado por WindowWidthProbe)
            let current_width = current_window_width();
            data.update_window_size(current_width);
            // Envolver con WindowWidthProbe para que el width se actualice en cada layout
            Box::new(observe_width(layout_a_view(&layout, data, &prog, &theme)))
        },
        WindowOptions::new("Forja GUI - Material You")
            .with_initial_inner_size(LogicalSize::new(initial_width, 800.0)),
    );

    app.run_in(EventLoop::with_user_event())
        .map_err(|e| format!("Error en GUI: {}", e))
}

// ─── Callback: ejecutar funciones Forja ──────────────────────────

/// Busca una función en el AST y retorna (parametros, cuerpo)
fn buscar_funcion<'a>(decls: &'a [Declaracion], nombre: &str) -> Option<(&'a [Parametro], &'a [Declaracion])> {
    for decl in decls {
        if let Declaracion::Funcion { nombre: n, parametros, cuerpo, .. } = decl {
            if n == nombre {
                return Some((parametros, cuerpo));
            }
        }
    }
    None
}

/// Ejecuta una función Forja inline evaluando el AST (para callbacks de botones)
/// Soporta: si, retornar, comparación ==, string concat +, variables de estado
pub fn ejecutar_callback_forja(
    nombre_fn: &str,
    state: &AppStateNativo,
    programa: &[Declaracion],
) -> ValorGUI {
    // Buscar la función
    let (params, cuerpo) = match buscar_funcion(programa, nombre_fn) {
        Some(p) => p,
        None => return ValorGUI::Texto(format!("Error: función '{}' no encontrada", nombre_fn)),
    };

    // Crear scope local: parámetros se obtienen de state
    let mut locals: HashMap<String, ValorGUI> = HashMap::new();
    for param in params {
        let val = state.leer(&param.nombre);
        locals.insert(param.nombre.clone(), val);
    }

    // Evaluar el cuerpo
    evaluar_bloque(cuerpo, &mut locals, state, programa)
}

/// Evalúa un bloque de declaraciones y retorna el valor de retorno
fn evaluar_bloque(
    decls: &[Declaracion],
    locals: &mut HashMap<String, ValorGUI>,
    state: &AppStateNativo,
    programa: &[Declaracion],
) -> ValorGUI {
    for decl in decls {
        match decl {
            Declaracion::Retornar { valor, .. } => {
                if let Some(expr) = valor {
                    return evaluar_expresion(expr, locals, state, programa);
                }
                return ValorGUI::Nulo;
            }
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let cond_val = evaluar_expresion(condicion, locals, state, programa);
                if cond_val.to_string() == "verdadero" || cond_val.to_string() == "true" {
                    let result = evaluar_bloque(bloque_verdadero, locals, state, programa);
                    if !matches!(result, ValorGUI::Nulo) {
                        return result;
                    }
                } else if let Some(bloque_falso) = bloque_falso {
                    let result = evaluar_bloque(bloque_falso, locals, state, programa);
                    if !matches!(result, ValorGUI::Nulo) {
                        return result;
                    }
                }
            }
            Declaracion::LlamadaFuncion { .. } => {
                // Ignorar llamadas a funciones _ (efectos secundarios)
            }
            _ => {}
        }
    }
    ValorGUI::Nulo
}

/// Evalúa una expresión Forja y retorna su valor
fn evaluar_expresion(
    expr: &Expresion,
    locals: &HashMap<String, ValorGUI>,
    state: &AppStateNativo,
    programa: &[Declaracion],
) -> ValorGUI {
    match expr {
        Expresion::LiteralTexto(s) => ValorGUI::Texto(s.clone()),
        Expresion::LiteralNumero(n) => ValorGUI::Entero(*n),
        Expresion::LiteralBooleano(b) => ValorGUI::Texto(if *b { "verdadero".to_string() } else { "falso".to_string() }),
        Expresion::LiteralExacto(_, _) => ValorGUI::Nulo,
        Expresion::LiteralNulo => ValorGUI::Nulo,
        Expresion::Identificador(v, ..) => {
            // Buscar en locales primero, luego en state
            locals.get(v)
                .cloned()
                .or_else(|| {
                    // Buscar en variables de función (ámbito global de Forja)
                    for decl in programa {
                        if let Declaracion::Funcion { nombre, cuerpo, .. } = decl {
                            if nombre == "main" {
                                for d in cuerpo {
                                    if let Declaracion::Variable { nombre: n, valor: _, .. } = d {
                                        if n == v {
                                            return Some(state.leer(v));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    None
                })
                .unwrap_or(ValorGUI::Texto(v.clone()))
        }
        Expresion::Binaria { izquierda, operador, derecha } => {
            let izq = evaluar_expresion(izquierda, locals, state, programa);
            let der = evaluar_expresion(derecha, locals, state, programa);
            match operador {
                Operador::Suma => {
                    // Concat: "a" + "b" → "ab"
                    ValorGUI::Texto(izq.to_string() + &der.to_string())
                }
                Operador::IgualIgual => {
                    let result = izq.to_string() == der.to_string();
                    ValorGUI::Texto(if result { "verdadero" } else { "falso" }.to_string())
                }
                Operador::Diferente => {
                    let result = izq.to_string() != der.to_string();
                    ValorGUI::Texto(if result { "verdadero" } else { "falso" }.to_string())
                }
                _ => ValorGUI::Texto(izq.to_string()),
            }
        }
        _ => ValorGUI::Texto("?".to_string()),
    }
}

/// Actualiza el state con el resultado de un callback
pub fn ejecutar_callback_y_actualizar(
    nombre_fn: &str,
    state: &mut AppStateNativo,
    programa: &[Declaracion],
) {
    let resultado = ejecutar_callback_forja(nombre_fn, state, programa);
    // Guardar en la variable 'resultado' por convención
    state.escribir("resultado", resultado);
}

fn inicializar_estado(decls: &[Declaracion], state: &mut AppStateNativo) {
    for decl in decls {
        if let Declaracion::Funcion { nombre, cuerpo, .. } = decl {
            if nombre == "main" {
                for d in cuerpo {
                    if let Declaracion::Variable { nombre, valor, .. } = d {
                        let v = match valor {
                            Some(Expresion::LiteralTexto(s)) => ValorGUI::Texto(s.clone()),
                            Some(Expresion::LiteralNumero(n)) => ValorGUI::Entero(*n),
                            Some(Expresion::LiteralExacto(_, _)) => ValorGUI::Nulo,
                            _ => ValorGUI::Texto(String::new()),
                        };
                        state.escribir(nombre, v);
                    }
                }
                return;
            }
        }
    }
}
