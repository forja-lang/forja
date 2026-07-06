use crate::ast::*;
use crate::error::ErrorForja;
use std::collections::HashMap;

/// Analiza si una variable es realmente mutable (se reasigna) dentro de un conjunto de declaraciones.
/// Busca declaraciones `Asignacion` o `AsignacionIndex` que modifiquen la variable,
/// respetando límites de ámbito (no cruza fronteras de función).
fn es_variable_mutable(nombre: &str, declaraciones: &[Declaracion], _ambito_actual: usize) -> bool {
    for decl in declaraciones {
        match decl {
            Declaracion::Asignacion { nombre: var, .. } if var == nombre => return true,
            Declaracion::AsignacionIndex { nombre: var, .. } if var == nombre => return true,
            // Buscar en ámbitos anidados (bloques si, mientras, para, repetir)
            Declaracion::Si { bloque_verdadero, bloque_falso, .. } => {
                if es_variable_mutable(nombre, bloque_verdadero, _ambito_actual + 1) {
                    return true;
                }
                if let Some(bf) = bloque_falso {
                    if es_variable_mutable(nombre, bf, _ambito_actual + 1) {
                        return true;
                    }
                }
            }
            Declaracion::Mientras { bloque, .. } => {
                if es_variable_mutable(nombre, bloque, _ambito_actual + 1) {
                    return true;
                }
            }
            Declaracion::Para { bloque, incremento, .. } => {
                // El incremento del bucle `para` también es una asignación
                if let Some(inc) = incremento {
                    if es_variable_mutable(nombre, &[inc.as_ref().clone()], _ambito_actual + 1) {
                        return true;
                    }
                }
                if es_variable_mutable(nombre, bloque, _ambito_actual + 1) {
                    return true;
                }
            }
            Declaracion::Repetir { bloque, .. } => {
                if es_variable_mutable(nombre, bloque, _ambito_actual + 1) {
                    return true;
                }
            }
            // No cruzar fronteras de función: las funciones tienen su propio ámbito
            Declaracion::Funcion { .. } => {}
            _ => {}
        }
    }
    false
}

/// Generador de código Rust a partir del AST de Forja
pub struct Transpiler {
    output: String,
    indent_level: usize,
    #[allow(dead_code)]
    errors: Vec<ErrorForja>,
    /// Conteo de variables temporales para el bucle `para`
    temp_counter: usize,
    /// Si es true, no genera el fn main() automático
    pub saltar_main: bool,
    /// Clases declaradas (para generar impls)
    clases: HashMap<String, ClaseInfo>,
    /// Declaraciones globales del programa (para análisis de mutabilidad)
    declaraciones_globales: Vec<Declaracion>,
    /// Nombres de funciones externas (FFI)
    funciones_externas: Vec<String>,
    /// Variables detectadas para GUI AppState dinámico
    gui_vars: Vec<(String, String)>,
    /// Si es true, transpilar_expresion referencia campos como data.nombre
    gui_mode: bool,
    /// Pila de cierres de layout (columna/fila anidados)
    layout_stack: Vec<String>,
    /// True si se usó columna/fila como layout contenedor (para no duplicar flex)
    gui_container_layout: bool,
}

struct ClaseInfo {
    #[allow(dead_code)]
    campos: Vec<(String, String)>,    // (nombre_campo, tipo)
    #[allow(dead_code)]
    metodos: Vec<String>,             // nombres de métodos
    /// Mapa campo -> tipo inferido desde constructor
    tipos_campos: HashMap<String, String>,
}

/// Determina si una expresión hija necesita paréntesis según el operador padre.
/// Las reglas de precedencia (de menor a mayor) son:
///   Nivel 1: O (or lógico)
///   Nivel 2: Y (and lógico)
///   Nivel 3: Igual, Distinto
///   Nivel 4: Mayor, Menor, MayorIgual, MenorIgual
///   Nivel 5: Suma, Resta
///   Nivel 6: Multiplicacion, Division, Modulo
///   Nivel 7: Unario/primario
fn necesita_parentesis(expr: &Expresion, op_padre: &Operador) -> bool {
    let prec_hijo = match expr {
        Expresion::Binaria { operador, .. } => match operador {
            Operador::O => 1,
            Operador::Y => 2,
            Operador::IgualIgual | Operador::Diferente => 3,
            Operador::Mayor | Operador::Menor | Operador::MayorIgual | Operador::MenorIgual => 4,
            Operador::Suma | Operador::Resta => 5,
            Operador::Multiplicacion | Operador::Division | Operador::Modulo => 6,
        },
        _ => 7, // Unario/primario: máxima precedencia
    };
    let prec_padre = match op_padre {
        Operador::O => 1,
        Operador::Y => 2,
        Operador::IgualIgual | Operador::Diferente => 3,
        Operador::Mayor | Operador::Menor | Operador::MayorIgual | Operador::MenorIgual => 4,
        Operador::Suma | Operador::Resta => 5,
        Operador::Multiplicacion | Operador::Division | Operador::Modulo => 6,
    };
    // El hijo necesita paréntesis si tiene menor precedencia que el padre
    prec_hijo < prec_padre
}

impl Transpiler {
    pub fn new() -> Self {
        Transpiler {
            output: String::new(),
            indent_level: 0,
            errors: Vec::new(),
            temp_counter: 0,
            clases: HashMap::new(),
            declaraciones_globales: Vec::new(),
            funciones_externas: Vec::new(),
            gui_vars: Vec::new(),
            gui_mode: false,
            saltar_main: false,
            layout_stack: Vec::new(),
            gui_container_layout: false,
        }
    }

    /// Indica si el programa usa el paquete GUI
    pub fn usa_gui(&self) -> bool {
        self.declaraciones_globales.iter().any(|d| {
            matches!(d, Declaracion::Importar(ruta) if ruta == "gui")
        })
    }

    /// Exporta un programa Forja a código Rust (opcional, Forja ya ejecuta directo con VM)
    pub fn transpilar(&mut self, programa: &Programa) -> Result<String, Vec<ErrorForja>> {
        // Almacenar declaraciones globales para análisis de mutabilidad
        self.declaraciones_globales = programa.declaraciones.clone();

        // Primera pasada: recolectar clases
        self.recolectar_clases(&programa.declaraciones);

        // Segunda pasada: generar código
        self.emit_line("// Código exportado desde Forja (fa) — https://github.com/lococoi/forja");
        self.emit_line("// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust");
        self.emit_line("");

        // Detectar si hay concurrencia para añadir imports
        let tiene_concurrencia = self.detectar_concurrencia(&programa.declaraciones);
        if tiene_concurrencia {
            self.emit_line("use std::thread;");
            self.emit_line("use std::sync::mpsc;");
            self.emit_line("");
        }

        // Detectar si se usa el paquete GUI para emitir código Xilem REAL
        if self.usa_gui() {
            self.emit_line("// ─── GUI: Forja GUI Runtime (xilem precompilado) ───");
            self.emit_line("use forja_gui_rt::view::{self, Axis, flex, label, text_button, text_input, progress_bar, sized_box, button, checkbox, grid, portal, prose, slider, spinner, split, variable_label, zstack, image};");
            self.emit_line("use forja_gui_rt::{WidgetView, Xilem, WindowOptions, EventLoop, EventLoopError, Color, Affine, FontWeight};");
            self.emit_line("use forja_gui_rt::core::{lens, memoize};");
            self.emit_line("");
        }

        // Recolectar funciones externas
        self.funciones_externas = programa.declaraciones.iter()
            .filter(|d| matches!(d, Declaracion::Funcion { externa: true, .. }))
            .filter_map(|d| {
                if let Declaracion::Funcion { nombre, .. } = d {
                    Some(nombre.clone())
                } else {
                    None
                }
            })
            .collect();

        // Generar bloque extern "C" para funciones externas
        if !self.funciones_externas.is_empty() {
            self.emit_line("extern \"C\" {");
            self.indent();
            for decl in &programa.declaraciones {
                if let Declaracion::Funcion { nombre, parametros, tipo_retorno, externa: true, .. } = decl {
                    let params_str: Vec<String> = parametros.iter()
                        .map(|p| format!("{}: {}", p.nombre, self.tipo_a_rust(p.tipo.as_ref().unwrap_or(&Tipo::Entero))))
                        .collect();
                    let ret = tipo_retorno.as_ref()
                        .map(|t| self.tipo_a_rust(t))
                        .unwrap_or_else(|| "()".to_string());
                    self.emit_line(&format!("fn {}({}) -> {};", nombre, params_str.join(", "), ret));
                }
            }
            self.dedent();
            self.emit_line("}");
            self.emit_line("");
        }

        // Detectar si hay función main o clases para generar el fn main()
        let tiene_main = programa.declaraciones.iter().any(|d| {
            matches!(d, Declaracion::Funcion { nombre, .. } if nombre == "main")
        });
        let _tiene_clases = !self.clases.is_empty();

        // Generar clases como struct + impl
        self.generar_clases(&programa.declaraciones);

        // Generar funciones globales (saltar externas ya declaradas, y main si hay GUI)
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion { externa: true, .. } => {} // ya declaradas en extern "C"
                Declaracion::Funcion { nombre, .. } if self.usa_gui() && nombre == "main" => {
                    // Saltar main de Forja cuando hay GUI (Xilem genera su propio main)
                    self.emit_line(&format!("// fn main() de Forja omitido (GUI usa Xilem)"));
                }
                Declaracion::Funcion { .. } => {
                    self.transpilar_declaracion(decl);
                    self.emit_line("");
                }
                _ => {}
            }
        }

        // Generar traits e implementaciones después de las funciones
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Trait { .. } | Declaracion::Implementacion { .. } => {
                    self.transpilar_declaracion(decl);
                    self.emit_line("");
                }
                _ => {}
            }
        }

        // Si hay GUI: recolectar widgets recorriendo el AST recursivamente
        if self.usa_gui() {
            // Analizar variables para AppState dinámico
            let mut gui_vars = self.analizar_variables_gui(&programa.declaraciones);
            // Remover duplicados (mismo nombre, primer tipo se mantiene)
            let mut seen = std::collections::HashSet::new();
            gui_vars.retain(|(nombre, _)| seen.insert(nombre.clone()));
            self.gui_vars = gui_vars;

            // Generar AppState dinámico
            self.emit_line("#[derive(Default)]");
            self.emit_line("struct AppState {");
            let gui_vars = self.gui_vars.clone();
            if gui_vars.is_empty() {
                self.emit_line("    _placeholder: (),");
            } else {
                for (nombre, tipo_rust) in &gui_vars {
                    self.emit_line(&format!("    {}: {},", nombre, tipo_rust));
                }
            }
            self.emit_line("}");
            self.emit_line("");

            // Activar gui_mode para que transpilar_expresion prefije con data.
            self.gui_mode = true;

            // Recolectar widgets recursivamente desde todo el AST
            // Resetear estado de layout
            self.gui_container_layout = false;

            let mut widgets: Vec<String> = Vec::new();
            self.recolectar_widgets(&programa.declaraciones, &mut widgets);

            // Si no hay widgets, poner uno default
            if widgets.is_empty() {
                widgets.push("    view::label(String::from(\"Forja + Xilem GUI\")),".to_string());
            }

            // Emitir app_logic() con los widgets recolectados
            self.emit_line("fn app_logic(data: &mut AppState) -> impl WidgetView<AppState> {");
            if self.gui_container_layout {
                // Si el layout principal ya es columna/fila, los widgets tienen indentación base
                for w in &widgets {
                    self.emit_line(w);
                }
            } else {
                // Layout legacy: envolver en flex vertical con indentación extra
                self.emit_line("    view::flex(Axis::Vertical, (");
                for w in &widgets {
                    self.emit_line(&format!("    {}", w));
                }
                self.emit_line("    ))");
            }
            self.emit_line("}");
            self.emit_line("");

            // Emitir main() Xilem con tema oscuro (default en Windows)
            self.emit_line("fn main() -> Result<(), EventLoopError> {");
            self.emit_line("    // Modo oscuro: Xilem usa tema dark por defecto en Windows");
            self.emit_line("    Xilem::new_simple(");
            self.emit_line("        AppState::default(),");
            self.emit_line("        app_logic,");
            self.emit_line("        WindowOptions::new(\"Forja GUI\".to_string()),");
            self.emit_line("    ).run_in(EventLoop::with_user_event())");
            self.emit_line("}");
            self.gui_mode = false;
            return Ok(self.output.clone());
        }

        // Si no hay GUI, generar main con código global
        if !tiene_main && !self.saltar_main {
            self.emit_line("fn main() {");
            self.indent();

            for decl in &programa.declaraciones {
                match decl {
                    Declaracion::Funcion { .. } | Declaracion::Clase { .. } => {}
                    _ => {
                        self.transpilar_declaracion(decl);
                    }
                }
            }

            self.dedent();
            self.emit_line("}");
        }

        Ok(self.output.clone())
    }

    fn es_funcion_externa(&self, nombre: &str) -> bool {
        self.funciones_externas.contains(&nombre.to_string())
    }

    /// Recolecta widgets Xilem recorriendo el AST recursivamente
    fn recolectar_widgets(&mut self, declaraciones: &[Declaracion], widgets: &mut Vec<String>) {
        for decl in declaraciones {
            match decl {
                Declaracion::LlamadaFuncion { nombre, argumentos } => {
                    match nombre.as_str() {
                        "columna" | "gui_columna" => {
                            self.gui_container_layout = true;
                            widgets.push("    view::sized_box(view::flex(Axis::Vertical, (".to_string());
                            for arg in argumentos {
                                self.procesar_expresion_widget(arg, widgets);
                            }
                            widgets.push("    )))".to_string());
                        }
                        "fila" | "gui_fila" => {
                            self.gui_container_layout = true;
                            widgets.push("    view::flex(Axis::Horizontal, (".to_string());
                            for arg in argumentos {
                                self.procesar_expresion_widget(arg, widgets);
                            }
                            widgets.push("    ))".to_string());
                        }
                        "pila" | "gui_pila" | "zstack" => {
                            self.gui_container_layout = true;
                            widgets.push("    view::zstack((".to_string());
                            for arg in argumentos {
                                self.procesar_expresion_widget(arg, widgets);
                            }
                            widgets.push("    ))".to_string());
                        }
                        "desplazable" | "gui_desplazable" | "scroll" => {
                            if let Some(arg) = argumentos.first() {
                                widgets.push("    view::portal(".to_string());
                                self.procesar_expresion_widget(arg, widgets);
                                widgets.push("    ),".to_string());
                            }
                        }
                        "grilla" | "gui_grilla" | "grid" => {
                            // último arg = columnas
                            if let Some(Expresion::LiteralNumero(cols)) = argumentos.last() {
                                let _hijos = &argumentos[..argumentos.len()-1];
                                widgets.push(format!("    view::grid((\"_tmp\",), 1, {}),", cols));
                            }
                        }
                        _ => {
                            let args: Vec<String> = argumentos.iter()
                                .map(|a| self.transpilar_expresion(a))
                                .collect();
                            match nombre.as_str() {
                                "escribir" | "etiqueta" | "gui_etiqueta" | "text" | "label" => {
                                    if let Some(arg) = args.first() {
                                        widgets.push(format!("    view::label({}),", arg));
                                    }
                                }
                                "etiqueta_dinamica" | "varlabel" => {
                                    if let Some(arg) = args.first() {
                                        let var = arg.trim_start_matches("data.");
                                        widgets.push(format!("    view::variable_label(|d: &mut AppState| d.{}.clone()),", var));
                                    }
                                }
                                "boton" | "gui_boton" | "btn" | "button" => {
                                    let texto = args.first().map(|s| s.as_str()).unwrap_or("String::from(\"\")");
                                    if args.len() >= 2 {
                                        let callback = args[1].trim_start_matches('&').to_string();
                                        widgets.push(format!(
                                            "    view::text_button({}, |d: &mut AppState| {{ {}(); }}),",
                                            texto, callback
                                        ));
                                    } else {
                                        widgets.push(format!(
                                            "    view::text_button({}, |d: &mut AppState| {{ println!(\"Boton: {}\"); }}),",
                                            texto, texto
                                        ));
                                    }
                                }
                                "entrada_texto" | "gui_entrada_texto" | "input" => {
                                    if let Some(val) = args.first() {
                                        widgets.push(format!("    view::text_input({}),", val));
                                    }
                                }
                                "area_texto" | "textarea" => {
                                    if let Some(val) = args.first() {
                                        // text_input multiline (Masonry soporta multi-line natively)
                                        widgets.push(format!("    view::text_input({}).insert_newline(true),", val));
                                    }
                                }
                                "barra_progreso" | "gui_barra_progreso" | "progress" => {
                                    if let Some(val) = args.first() {
                                        widgets.push(format!("    view::progress_bar({}),", val));
                                    }
                                }
                                "deslizante" | "gui_deslizante" | "slider" => {
                                    if args.len() >= 3 {
                                        widgets.push(format!(
                                            "    view::slider({}, {}, {}, |d: &mut AppState| {{ }}),",
                                            args[0], args[1], args[2]
                                        ));
                                    }
                                }
                                "casilla" | "checkbox" | "gui_casilla" | "check" => {
                                    let etiqueta = args.first().map(|s| s.as_str()).unwrap_or("\"\"");
                                    widgets.push(format!("    view::checkbox({}, false),", etiqueta));
                                }
                                "texto_enriquecido" | "prose" => {
                                    if let Some(arg) = args.first() {
                                        widgets.push(format!("    view::prose({}),", arg));
                                    }
                                }
                                "cargando" | "spinner" => {
                                    widgets.push("    view::spinner(),".to_string());
                                }
                                "separador" | "divider" => {
                                    widgets.push("    view::sized_box(view::label(String::new())).height(1.0).width_full(),".to_string());
                                }
                                "espacio" | "spacer" => {
                                    if let Some(arg) = args.first() {
                                        widgets.push(format!("    view::sized_box(view::label(String::new())).width({}).height({}),", arg, arg));
                                    }
                                }
                                "caja_fija" | "sized" => {
                                    if args.len() >= 3 {
                                        widgets.push(format!(
                                            "    view::sized_box(view::label(String::new())).width({}).height({}),",
                                            args[1], args[2]
                                        ));
                                    }
                                }
                                "panel_dividido" | "split" => {
                                    // No implementado directamente en transpiler, se maneja mejor en nativo
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Declaracion::Funcion { cuerpo, .. } => {
                    self.recolectar_widgets(cuerpo, widgets);
                }
                Declaracion::Si { bloque_verdadero, bloque_falso, .. } => {
                    self.recolectar_widgets(bloque_verdadero, widgets);
                    if let Some(bf) = bloque_falso {
                        self.recolectar_widgets(bf, widgets);
                    }
                }
                Declaracion::Mientras { bloque, .. } => {
                    self.recolectar_widgets(bloque, widgets);
                }
                Declaracion::Para { bloque, .. } => {
                    self.recolectar_widgets(bloque, widgets);
                }
                Declaracion::Repetir { bloque, .. } => {
                    self.recolectar_widgets(bloque, widgets);
                }
                _ => {}
            }
        }
    }

    /// Procesa una expresión que representa un widget, usada como argumento de columna/fila
    fn procesar_expresion_widget(&mut self, expr: &Expresion, widgets: &mut Vec<String>) {
        match expr {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                match nombre.as_str() {
                    "columna" | "gui_columna" => {
                        widgets.push("    view::sized_box(view::flex(Axis::Vertical, (".to_string());
                        for arg in argumentos {
                            self.procesar_expresion_widget(arg, widgets);
                        }
                        widgets.push("    ))),".to_string());
                    }
                    "fila" | "gui_fila" => {
                        widgets.push("    view::flex(Axis::Horizontal, (".to_string());
                        for arg in argumentos {
                            self.procesar_expresion_widget(arg, widgets);
                        }
                        widgets.push("    )),".to_string());
                    }
                    "pila" | "gui_pila" | "zstack" => {
                        widgets.push("    view::zstack((".to_string());
                        for arg in argumentos {
                            self.procesar_expresion_widget(arg, widgets);
                        }
                        widgets.push("    )),".to_string());
                    }
                    "desplazable" | "gui_desplazable" | "scroll" => {
                        if let Some(arg) = argumentos.first() {
                            widgets.push("    view::portal(".to_string());
                            self.procesar_expresion_widget(arg, widgets);
                            widgets.push("    ),".to_string());
                        }
                    }
                    _ => {
                        let args: Vec<String> = argumentos.iter()
                            .map(|a| self.transpilar_expresion(a))
                            .collect();
                        match nombre.as_str() {
                            "escribir" | "etiqueta" | "gui_etiqueta" | "text" | "label" => {
                                if let Some(arg) = args.first() {
                                    widgets.push(format!("    view::label({}),", arg));
                                }
                            }
                            "etiqueta_dinamica" | "varlabel" => {
                                if let Some(arg) = args.first() {
                                    let var = arg.trim_start_matches("data.");
                                    widgets.push(format!("    view::variable_label(|d: &mut AppState| d.{}.clone()),", var));
                                }
                            }
                            "boton" | "gui_boton" | "btn" | "button" => {
                                let texto = args.first().map(|s| s.as_str()).unwrap_or("String::from(\"\")");
                                if args.len() >= 2 {
                                    let callback = args[1].trim_start_matches('&').to_string();
                                    widgets.push(format!(
                                        "    view::text_button({}, |d: &mut AppState| {{ {}(); }}),",
                                        texto, callback
                                    ));
                                } else {
                                    widgets.push(format!(
                                        "    view::text_button({}, |d: &mut AppState| {{ println!(\"Boton: {}\"); }}),",
                                        texto, texto
                                    ));
                                }
                            }
                            "entrada_texto" | "gui_entrada_texto" | "input" => {
                                if let Some(val) = args.first() {
                                    widgets.push(format!("    view::text_input({}),", val));
                                }
                            }
                            "area_texto" | "textarea" => {
                                if let Some(val) = args.first() {
                                    widgets.push(format!("    view::text_input({}).insert_newline(true),", val));
                                }
                            }
                            "barra_progreso" | "gui_barra_progreso" | "progress" => {
                                if let Some(val) = args.first() {
                                    widgets.push(format!("    view::progress_bar({}),", val));
                                }
                            }
                            "deslizante" | "gui_deslizante" | "slider" => {
                                if args.len() >= 3 {
                                    widgets.push(format!(
                                        "    view::slider({}, {}, {}, |d: &mut AppState| {{ }}),",
                                        args[0], args[1], args[2]
                                    ));
                                }
                            }
                            "casilla" | "checkbox" | "gui_casilla" | "check" => {
                                let etiqueta = args.first().map(|s| s.as_str()).unwrap_or("\"\"");
                                widgets.push(format!("    view::checkbox({}, false),", etiqueta));
                            }
                            "texto_enriquecido" | "prose" => {
                                if let Some(arg) = args.first() {
                                    widgets.push(format!("    view::prose({}),", arg));
                                }
                            }
                            "cargando" | "spinner" => {
                                widgets.push("    view::spinner(),".to_string());
                            }
                            "separador" | "divider" => {
                                widgets.push("    view::sized_box(view::label(String::new())).height(1.0).width_full(),".to_string());
                            }
                            "espacio" | "spacer" => {
                                if let Some(arg) = args.first() {
                                    widgets.push(format!("    view::sized_box(view::label(String::new())).width({}).height({}),", arg, arg));
                                }
                            }
                            "caja_fija" | "sized" => {
                                if args.len() >= 3 {
                                    widgets.push(format!(
                                        "    view::sized_box(view::label(String::new())).width({}).height({}),",
                                        args[1], args[2]
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {
                let s = self.transpilar_expresion(expr);
                widgets.push(format!("    {});", s));
            }
        }
    }

    /// Analiza recursivamente el AST y recolecta variables (nombre, tipo_rust) para
    /// generar AppState dinámicamente en programas GUI.
    fn analizar_variables_gui(&self, declaraciones: &[Declaracion]) -> Vec<(String, String)> {
        let mut vars = Vec::new();
        for decl in declaraciones {
            match decl {
                Declaracion::Variable { nombre, tipo, .. } => {
                    let tipo_rust = match tipo {
                        Some(Tipo::Entero) => "i32",
                        Some(Tipo::Decimal) => "f64",
                        Some(Tipo::Texto) => "String",
                        Some(Tipo::Booleano) => "bool",
                        _ => "String",
                    };
                    vars.push((nombre.clone(), tipo_rust.to_string()));
                }
                Declaracion::Funcion { cuerpo, .. } => {
                    vars.extend(self.analizar_variables_gui(cuerpo));
                }
                Declaracion::Si { bloque_verdadero, bloque_falso, .. } => {
                    vars.extend(self.analizar_variables_gui(bloque_verdadero));
                    if let Some(bf) = bloque_falso {
                        vars.extend(self.analizar_variables_gui(bf));
                    }
                }
                Declaracion::Mientras { bloque, .. } => {
                    vars.extend(self.analizar_variables_gui(bloque));
                }
                Declaracion::Para { bloque, .. } => {
                    vars.extend(self.analizar_variables_gui(bloque));
                }
                Declaracion::Repetir { bloque, .. } => {
                    vars.extend(self.analizar_variables_gui(bloque));
                }
                _ => {}
            }
        }
        vars
    }

    /// Detecta si el programa usa concurrencia (hilo, canal, enviar, recibir, unir, seleccionar)
    fn detectar_concurrencia(&self, declaraciones: &[Declaracion]) -> bool {
        for decl in declaraciones {
            match decl {
                Declaracion::Expresion(Expresion::Seleccionar { .. }) => return true,
                Declaracion::Expresion(Expresion::Hilo { .. }) => return true,
                Declaracion::Expresion(Expresion::CanalNuevo) => return true,
                Declaracion::Variable { valor: Some(val), .. } => {
                    if self.expr_tiene_concurrencia(val) { return true; }
                }
                Declaracion::AsignacionMultiple { valor, .. } => {
                    if self.expr_tiene_concurrencia(valor) { return true; }
                }
                Declaracion::LlamadaFuncion { nombre, .. } => {
                    if nombre.contains("enviar") || nombre.contains("recibir") || nombre.contains("unir") {
                        return true;
                    }
                }
                Declaracion::Si { bloque_verdadero, bloque_falso, .. } => {
                    if self.detectar_concurrencia(bloque_verdadero) { return true; }
                    if let Some(bf) = bloque_falso {
                        if self.detectar_concurrencia(bf) { return true; }
                    }
                }
                Declaracion::Mientras { bloque, .. } => {
                    if self.detectar_concurrencia(bloque) { return true; }
                }
                Declaracion::Para { bloque, .. } => {
                    if self.detectar_concurrencia(bloque) { return true; }
                }
                Declaracion::Repetir { bloque, .. } => {
                    if self.detectar_concurrencia(bloque) { return true; }
                }
                Declaracion::Funcion { cuerpo, .. } => {
                    if self.detectar_concurrencia(cuerpo) { return true; }
                }
                _ => {}
            }
        }
        false
    }

    fn expr_tiene_concurrencia(&self, expr: &Expresion) -> bool {
        match expr {
            Expresion::Seleccionar { .. } => true,
            Expresion::Hilo { .. } => true,
            Expresion::CanalNuevo => true,
            Expresion::LlamadaFuncion { nombre, .. } => {
                nombre.contains("enviar") || nombre.contains("recibir") || nombre.contains("unir")
            }
            _ => false,
        }
    }

    fn recolectar_clases(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Clase { nombre, campos, metodos, .. } = decl {
                let mut tipos_campos: HashMap<String, String> = HashMap::new();

                // Escanear constructores para inferir tipos de campos
                for metodo in metodos {
                    if metodo.nombre == "nuevo" {
                        for decl_cuerpo in &metodo.cuerpo {
                            if let Declaracion::AsignacionMiembro { objeto, miembro, valor } = decl_cuerpo {
                                // este.campo = expr → inferir tipo
                                if let Expresion::Identificador(ref nombre_self) = objeto.as_ref() {
                                    if nombre_self == "self" {
                                        let tipo_inferido = self.inferir_tipo_expr(valor, &metodo.parametros);
                                        tipos_campos.insert(miembro.clone(), tipo_inferido);
                                    }
                                }
                            }
                        }
                    }
                }

                let campos_info: Vec<(String, String)> = campos
                    .iter()
                    .map(|c| {
                        let tipo = tipos_campos.get(&c.nombre).cloned().unwrap_or_else(|| "String".to_string());
                        (c.nombre.clone(), tipo)
                    })
                    .collect();

                let metodos_info: Vec<String> = metodos
                    .iter()
                    .map(|m| m.nombre.clone())
                    .collect();

                self.clases.insert(
                    nombre.clone(),
                    ClaseInfo {
                        campos: campos_info,
                        metodos: metodos_info,
                        tipos_campos,
                    },
                );
            }
        }
    }

    /// Infiere el tipo Rust de una expresión usada como valor de campo
    fn inferir_tipo_expr(&self, expr: &Expresion, params: &[Parametro]) -> String {
        match expr {
            Expresion::LiteralNumero(_) => "i64".to_string(),
            Expresion::LiteralDecimal(_) => "f64".to_string(),
            Expresion::LiteralTexto(_) => "String".to_string(),
            Expresion::LiteralBooleano(_) => "bool".to_string(),
            Expresion::LiteralNulo => "()".to_string(),
            Expresion::Identificador(nombre) => {
                // Buscar si el identificador es un parámetro con tipo conocido
                for p in params {
                    if p.nombre == *nombre {
                        if let Some(ref tipo) = p.tipo {
                            return match tipo {
                                Tipo::Entero => "i64".to_string(),
                                Tipo::Decimal => "f64".to_string(),
                                Tipo::Texto => {
                                    if p.prestado { "&str".to_string() } else { "String".to_string() }
                                }
                                Tipo::Booleano => "bool".to_string(),
                                Tipo::Nulo => "()".to_string(),
                                Tipo::Clase(n) => n.clone(),
                                Tipo::Arreglo(t) => format!("Vec<{}>", self.tipo_a_rust(t)),
                                Tipo::Funcion(_, _) => "fn".to_string(),
                                Tipo::Resultado(_, _) => "Result<...>".to_string(),
                                Tipo::Opcion(_) => "Option<...>".to_string(),
                                Tipo::TraitObjeto(n) => format!("Box<dyn {}>", n),
                                Tipo::Parametro(n) => n.clone(),
                            };
                        }
                    }
                }
                // Si es un literal conocido (verdadero/falso)
                match nombre.as_str() {
                    "verdadero" | "falso" => "bool".to_string(),
                    _ => "String".to_string() // default
                }
            }
            Expresion::Binaria { izquierda, .. } => {
                // Para expresiones como a + b, inferir del lado izquierdo
                self.inferir_tipo_expr(izquierda, params)
            }
            Expresion::Unaria { expr: e, .. } => self.inferir_tipo_expr(e, params),
            _ => "String".to_string()
        }
    }

    fn generar_clases(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Clase { nombre, parametros_tipo, campos, metodos, atributos } = decl {
                // Generar parámetros genéricos si existen
                let gen_params_str = if parametros_tipo.is_empty() {
                    String::new()
                } else {
                    let gen_names: Vec<String> = parametros_tipo.iter()
                        .map(|p| p.nombre.clone())
                        .collect();
                    format!("<{}>", gen_names.join(", "))
                };

                // Generar #[derive(...)] desde @derive(Mostrar, Igual, ...)
                self.emit_derive_from_atributos(atributos);

                // Generar struct
                self.emit_line(&format!("#[derive(Debug)]"));
                self.emit_line(&format!("struct {}{} {{", nombre, gen_params_str));
                self.indent();

                for campo in campos {
                    let tipo = self.inferir_tipo_campo(campo);
                    self.emit_line(&format!("{}: {},", campo.nombre, tipo));
                }

                // Si no hay campos, agregar un placeholder
                if campos.is_empty() {
                    self.emit_line("// Campos de la clase");
                }

                self.dedent();
                self.emit_line("}");
                self.emit_line("");

                // Generar impl con genéricos
                if parametros_tipo.is_empty() {
                    self.emit_line(&format!("impl {} {{", nombre));
                } else {
                    let gen_names: Vec<String> = parametros_tipo.iter()
                        .map(|p| p.nombre.clone())
                        .collect();
                    let gen_params = format!("<{}>", gen_names.join(", "));
                    self.emit_line(&format!("impl{} {}{} {{", gen_params, nombre, gen_params));
                }
                self.indent();

                for metodo in metodos {
                    self.generar_metodo(metodo, nombre);
                    self.emit_line("");
                }

                self.dedent();
                self.emit_line("}");
                self.emit_line("");
            }
        }
    }

    fn inferir_tipo_campo(&self, campo: &VariableClase) -> String {
        // Buscar el tipo inferido desde el constructor (recolectar_clases)
        for (_nombre, info) in &self.clases {
            if let Some(tipo) = info.tipos_campos.get(&campo.nombre) {
                return tipo.clone();
            }
        }
        // Fallback: String
        "String".to_string()
    }

    fn generar_metodo(&mut self, metodo: &Metodo, nombre_clase: &str) {
        if metodo.nombre == "nuevo" {
            // Constructor: fn nuevo(...) -> Self
            let params: Vec<String> = metodo
                .parametros
                .iter()
                .map(|p| {
                    let mut param_str = String::new();
                    if p.prestado {
                        param_str.push_str("&");
                    }
                    if p.mutable {
                        param_str.push_str("mut ");
                    }
                    param_str.push_str(&p.nombre);
                    param_str.push_str(": ");
                    param_str.push_str(&self.inferir_tipo_parametro(p));
                    param_str
                })
                .collect();

            self.emit_line(&format!(
                "fn nuevo({}) -> Self {{",
                params.join(", ")
            ));
            self.indent();

            // Generar inicialización de campos basada en el cuerpo del constructor
            // Busca patrones: este.campo = param → Self { campo: param }
            let campos_inicializar: Vec<(String, String)> = metodo
                .cuerpo
                .iter()
                .filter_map(|decl| {
                    if let Declaracion::AsignacionMiembro { objeto, miembro, valor } = decl {
                        if let Expresion::Identificador(ref nombre_self) = objeto.as_ref() {
                            if nombre_self == "self" {
                                // El valor puede ser un identificador (param) o una expresión
                                let val_str = match valor.as_ref() {
                                    Expresion::Identificador(id) => id.clone(),
                                    other => self.transpilar_expresion(other),
                                };
                                return Some((miembro.clone(), val_str));
                            }
                        }
                    }
                    None
                })
                .collect();

            if campos_inicializar.is_empty() {
                self.emit_line(&format!("{} {{ }}", nombre_clase));
            } else {
                self.emit_line(&format!(
                    "{} {{",
                    nombre_clase
                ));
                self.indent();
                for (campo, valor) in &campos_inicializar {
                    self.emit_line(&format!("{}: {},", campo, valor));
                }
                self.dedent();
                self.emit_line("}");
            }

            self.dedent();
            self.emit_line("}");
        } else {
            // Método normal: fn nombre(&self, ...)
            let params: Vec<String> = metodo
                .parametros
                .iter()
                .map(|p| {
                    let mut param_str = String::new();
                    if p.prestado {
                        param_str.push_str("&");
                    }
                    if p.mutable {
                        param_str.push_str("mut ");
                    }
                    param_str.push_str(&p.nombre);
                    param_str.push_str(": ");
                    param_str.push_str(&self.inferir_tipo_parametro(p));
                    param_str
                })
                .collect();

            let mut sig = format!("fn {}(", metodo.nombre);

            // Verificar si el primer parámetro ya es self
            let tiene_self = metodo.parametros.first().map_or(false, |p| p.nombre == "self");
            if !tiene_self {
                sig.push_str("&self");
                if !params.is_empty() {
                    sig.push_str(", ");
                }
            }

            sig.push_str(&params.join(", "));
            sig.push_str(")");

            // Tipo de retorno opcional explícito
            if let Some(ref t) = metodo.tipo_retorno {
                sig.push_str(&format!(" -> {}", self.tipo_a_rust(t)));
            }

            sig.push_str(" {");

            self.emit_line(&sig);
            self.indent();

            for decl in &metodo.cuerpo {
                self.transpilar_declaracion(decl);
            }

            self.dedent();
            self.emit_line("}");
        }
    }

    /// Infiere el tipo Rust de un parámetro, analizando cómo se usa en el cuerpo de la función.
    /// Si tiene anotación explícita de tipo, la usa. Si no, infiere por contexto.
    fn inferir_tipo_parametro(&self, param: &Parametro) -> String {
        if let Some(ref tipo) = param.tipo {
            return match tipo {
                Tipo::Entero => "i64".to_string(),
                Tipo::Decimal => "f64".to_string(),
                Tipo::Texto => {
                    if param.prestado { "&str".to_string() } else { "String".to_string() }
                }
                Tipo::Booleano => "bool".to_string(),
                Tipo::Nulo => "()".to_string(),
                Tipo::Clase(nombre) => nombre.clone(),
                Tipo::Arreglo(_) => "Vec<...>".to_string(),
                Tipo::Funcion(_, _) => "fn".to_string(),
                Tipo::Resultado(_, _) => "Result<...>".to_string(),
                Tipo::Opcion(_) => "Option<...>".to_string(),
                Tipo::TraitObjeto(nombre) => format!("Box<dyn {}>", nombre),
                Tipo::Parametro(nombre) => nombre.clone(),
            };
        }

        if param.prestado {
            return "&str".to_string();
        }

        // Si no hay tipo explícito, usar i64 por defecto (es el caso más común
        // en programas educativos: parámetros numéricos sin anotación)
        "i64".to_string()
    }

    /// Escanea el cuerpo de una función para inferir tipos de parámetros no anotados.
    /// Retorna un mapa: nombre_del_parametro -> tipo_rust_inferido
    /// Infiere el tipo de retorno de una función analizando su cuerpo.
    /// Retorna Some(Tipo) si encuentra una declaración `retornar` con valor.
    fn inferir_tipo_retorno(&self, cuerpo: &[Declaracion]) -> Option<Tipo> {
        for decl in cuerpo {
            if let Declaracion::Retornar { valor: Some(val) } = decl {
                // Inferir tipo del valor retornado
                let tipo = match val {
                    Expresion::LiteralNumero(_) => Some(Tipo::Entero),
                    Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
                    Expresion::LiteralTexto(_) => Some(Tipo::Texto),
                    Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
                    Expresion::LiteralNulo => Some(Tipo::Nulo),
                    Expresion::Identificador(nombre) => {
                        // Buscar si la variable tiene tipo conocido
                        match nombre.as_str() {
                            "verdadero" | "falso" => Some(Tipo::Booleano),
                            _ => Some(Tipo::Entero), // asumir Entero por defecto
                        }
                    }
                    Expresion::Binaria { izquierda, .. } => {
                        // Inferir del lado izquierdo de la operación
                        if let Expresion::LiteralDecimal(_) = izquierda.as_ref() {
                            Some(Tipo::Decimal)
                        } else {
                            Some(Tipo::Entero)
                        }
                    }
                    _ => Some(Tipo::Entero), // default
                };
                return tipo;
            }
            // Buscar recursivamente en bloques
            if let Declaracion::Si { bloque_verdadero, bloque_falso, .. } = decl {
                if let Some(t) = self.inferir_tipo_retorno(bloque_verdadero) { return Some(t); }
                if let Some(bf) = bloque_falso {
                    if let Some(t) = self.inferir_tipo_retorno(bf) { return Some(t); }
                }
            }
            if let Declaracion::Mientras { bloque, .. } = decl {
                if let Some(t) = self.inferir_tipo_retorno(bloque) { return Some(t); }
            }
        }
        None
    }

    /// Escanea el cuerpo de una función para inferir tipos de parámetros no anotados.
    /// Retorna un mapa: nombre_del_parametro -> tipo_rust_inferido
    fn inferir_tipos_desde_cuerpo(&self, cuerpo: &[Declaracion], params: &[Parametro]) -> std::collections::HashMap<String, String> {
        let mut tipos: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        // Inicializar con i64 por defecto para parámetros sin tipo
        for p in params {
            if p.tipo.is_none() && !tipos.contains_key(&p.nombre) {
                tipos.insert(p.nombre.clone(), "i64".to_string());
            }
        }

        for decl in cuerpo {
            self.analizar_declaracion_para_tipos(decl, &mut tipos);
        }

        tipos
    }

    fn analizar_declaracion_para_tipos(&self, decl: &Declaracion, tipos: &mut std::collections::HashMap<String, String>) {
        match decl {
            Declaracion::Variable { valor: Some(val), .. } => {
                self.analizar_expr_para_tipos(val, tipos);
            }
            Declaracion::Asignacion { valor, .. } => {
                self.analizar_expr_para_tipos(valor, tipos);
            }
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                self.analizar_expr_para_tipos(condicion, tipos);
                for d in bloque_verdadero { self.analizar_declaracion_para_tipos(d, tipos); }
                if let Some(bf) = bloque_falso {
                    for d in bf { self.analizar_declaracion_para_tipos(d, tipos); }
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                self.analizar_expr_para_tipos(condicion, tipos);
                for d in bloque { self.analizar_declaracion_para_tipos(d, tipos); }
            }
            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                if let Some(init) = inicializacion { self.analizar_declaracion_para_tipos(init, tipos); }
                if let Some(cond) = condicion { self.analizar_expr_para_tipos(cond, tipos); }
                if let Some(inc) = incremento { self.analizar_declaracion_para_tipos(inc, tipos); }
                for d in bloque { self.analizar_declaracion_para_tipos(d, tipos); }
            }
            Declaracion::Repetir { cantidad, bloque } => {
                self.analizar_expr_para_tipos(cantidad, tipos);
                for d in bloque { self.analizar_declaracion_para_tipos(d, tipos); }
            }
            Declaracion::Retornar { valor: Some(val) } => {
                self.analizar_expr_para_tipos(val, tipos);
            }
            Declaracion::Expresion(expr) => {
                self.analizar_expr_para_tipos(expr, tipos);
            }
            Declaracion::LlamadaFuncion { argumentos, .. } => {
                for arg in argumentos { self.analizar_expr_para_tipos(arg, tipos); }
            }
            _ => {}
        }
    }

    fn analizar_expr_para_tipos(&self, expr: &Expresion, tipos: &mut std::collections::HashMap<String, String>) {
        match expr {
            Expresion::Identificador(nombre) => {
                // Si el parámetro se usa con literales numéricos, es Entero
                if !tipos.contains_key(nombre) {
                    tipos.insert(nombre.clone(), "i64".to_string());
                }
            }
            Expresion::Binaria { izquierda, derecha, operador } => {
                use Operador::*;
                match operador {
                    Suma => {
                        // Si alguno es Texto, el otro se convierte
                        if let Expresion::LiteralTexto(_) = izquierda.as_ref() {
                            self.asignar_tipo_si_parametro(izquierda, tipos, "String");
                            self.asignar_tipo_si_parametro(derecha, tipos, "String");
                        } else if let Expresion::LiteralTexto(_) = derecha.as_ref() {
                            self.asignar_tipo_si_parametro(izquierda, tipos, "String");
                            self.asignar_tipo_si_parametro(derecha, tipos, "String");
                        } else {
                            self.analizar_expr_para_tipos(izquierda, tipos);
                            self.analizar_expr_para_tipos(derecha, tipos);
                        }
                    }
                    _ => {
                        self.analizar_expr_para_tipos(izquierda, tipos);
                        self.analizar_expr_para_tipos(derecha, tipos);
                    }
                }
            }
            Expresion::LiteralNumero(_) => {}
            Expresion::LiteralDecimal(_) => {}
            Expresion::LiteralTexto(_) => {}
            Expresion::LiteralBooleano(_) => {}
            Expresion::Unaria { expr: e, .. } => self.analizar_expr_para_tipos(e, tipos),
            Expresion::LlamadaFuncion { argumentos, .. } => {
                for arg in argumentos { self.analizar_expr_para_tipos(arg, tipos); }
            }
            Expresion::Arreglo(elementos) => {
                for e in elementos { self.analizar_expr_para_tipos(e, tipos); }
            }
            Expresion::Grupo(expr) => self.analizar_expr_para_tipos(expr, tipos),
            _ => {}
        }
    }

    fn asignar_tipo_si_parametro(&self, expr: &Expresion, tipos: &mut std::collections::HashMap<String, String>, tipo: &str) {
        if let Expresion::Identificador(nombre) = expr {
            tipos.insert(nombre.clone(), tipo.to_string());
        }
    }

    // ============================================================
    // Transpilación de declaraciones
    // ============================================================

    fn transpilar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { mutable, nombre, tipo, valor } => {
                // Analizar si la variable es realmente mutable (se reasigna en el código)
                let realmente_mutable = *mutable && es_variable_mutable(
                    nombre,
                    &self.declaraciones_globales,
                    0,
                );
                let mut decl_str = if realmente_mutable {
                    format!("let mut {}", nombre)
                } else {
                    format!("let {}", nombre)
                };

                // Anotación de tipo explícita si se declaró (ej: variable x: Entero = 5)
                if let Some(t) = tipo {
                    let tipo_rust = self.tipo_a_rust(t);
                    decl_str.push_str(&format!(": {}", tipo_rust));
                }

                if let Some(val) = valor {
                    decl_str.push_str(" = ");
                    decl_str.push_str(&self.transpilar_expresion(val));
                }

                self.emit_line(&format!("{};", decl_str));
            }

            Declaracion::Asignacion { nombre, valor } => {
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{} = {};", nombre, val_str));
            }

            Declaracion::AsignacionMiembro { objeto, miembro, valor } => {
                let obj_str = self.transpilar_expresion(objeto);
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{}.{} = {};", obj_str, miembro, val_str));
            }

            Declaracion::AsignacionIndex { nombre, indice, valor } => {
                let idx_str = self.transpilar_expresion(indice);
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{}[{}] = {};", nombre, idx_str, val_str));
            }

            Declaracion::Funcion { nombre, parametros_tipo, parametros, tipo_retorno, cuerpo, externa: _, enlace_nombre: _, atributos, doc } => {
                // Emitir doc comment si existe
                if let Some(doc_text) = doc {
                    for line in doc_text.lines() {
                        self.emit_line(&format!("/// {}", line));
                    }
                }
                // Emitir #[test] si la función tiene @test
                if atributos.iter().any(|a| a.nombre == "test") {
                    self.emit_line("#[test]");
                }
                // Inferir tipos de parámetros desde el cuerpo de la función
                let tipos_inferidos = self.inferir_tipos_desde_cuerpo(cuerpo, parametros);

                // Inferir tipo de retorno desde el cuerpo si no está anotado
                let inferred_ret = tipo_retorno.clone().or_else(|| self.inferir_tipo_retorno(cuerpo));

                // Generar parámetros de tipo genérico <T, U> si existen
                let gen_params_str = if parametros_tipo.is_empty() {
                    String::new()
                } else {
                    let gen_names: Vec<String> = parametros_tipo.iter()
                        .map(|p| p.nombre.clone())
                        .collect();
                    format!("<{}> ", gen_names.join(", "))
                };

                let params: Vec<String> = parametros
                    .iter()
                    .map(|p| {
                        let mut s = String::new();
                        if p.prestado {
                            s.push_str("&");
                        }
                        if p.mutable {
                            s.push_str("mut ");
                        }
                        s.push_str(&p.nombre);
                        s.push_str(": ");
                        // Usar tipo inferido si no tiene anotación explícita
                        let tipo = if p.tipo.is_some() {
                            self.inferir_tipo_parametro(p)
                        } else {
                            tipos_inferidos.get(&p.nombre)
                                .cloned()
                                .unwrap_or_else(|| "i64".to_string())
                        };
                        s.push_str(&tipo);
                        s
                    })
                    .collect();

                let ret_str = if let Some(ref tipo) = inferred_ret {
                    format!(" -> {}", self.tipo_a_rust(tipo))
                } else {
                    String::new()
                };

                self.emit_line(&format!("fn {}{}({}){} {{", nombre, gen_params_str, params.join(", "), ret_str));
                self.indent();

                // Guardar contexto actual y poner el cuerpo de la función como ámbito de búsqueda
                // para que las variables locales también sean analizadas por es_variable_mutable
                let declaraciones_previas = std::mem::take(&mut self.declaraciones_globales);
                self.declaraciones_globales = cuerpo.clone();

                for d in cuerpo {
                    self.transpilar_declaracion(d);
                }

                // Restaurar contexto anterior
                self.declaraciones_globales = declaraciones_previas;

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::Trait { nombre, metodos } => {
                self.emit_line(&format!("trait {} {{", nombre));
                self.indent();
                for metodo in metodos {
                    let params: Vec<String> = metodo.parametros.iter().map(|p| {
                        let mut s = String::new();
                        if p.prestado { s.push_str("&"); }
                        if p.mutable { s.push_str("mut "); }
                        s.push_str(&p.nombre);
                        s.push_str(": ");
                        s.push_str(&self.inferir_tipo_parametro(p));
                        s
                    }).collect();
                    let ret = match &metodo.tipo_retorno {
                        Some(t) => format!(" -> {}", self.tipo_a_rust(t)),
                        None => String::new(),
                    };
                    self.emit_line(&format!("fn {}({}){};", metodo.nombre, params.join(", "), ret));
                }
                self.dedent();
                self.emit_line("}");
                self.emit_line("");
            }

            Declaracion::Implementacion { trait_nombre, clase_nombre, metodos } => {
                self.emit_line(&format!("impl {} for {} {{", trait_nombre, clase_nombre));
                self.indent();
                for metodo in metodos {
                    self.generar_metodo(metodo, clase_nombre);
                    self.emit_line("");
                }
                self.dedent();
                self.emit_line("}");
                self.emit_line("");
            }

            Declaracion::Clase { .. } => {
                // Las clases ya se generaron antes
            }

            Declaracion::Importar(ruta) => {
                self.emit_line(&format!("// importar \"{}\"", ruta));
            }

            Declaracion::Enum { nombre, variantes, atributos } => {
                // Generar #[derive(...)] desde @derive(Mostrar, Igual, ...)
                self.emit_derive_from_atributos(atributos);
                let vars: Vec<String> = variantes.iter().map(|v| {
                    let tipos: Vec<String> = v.tipos.iter().map(|t| self.tipo_a_rust(t)).collect();
                    if tipos.is_empty() {
                        v.nombre.clone()
                    } else {
                        format!("{}({})", v.nombre, tipos.join(", "))
                    }
                }).collect();
                self.emit_line(&format!("enum {} {{", nombre));
                self.indent();
                for v in &vars {
                    self.emit_line(&format!("{},", v));
                }
                self.dedent();
                self.emit_line("}");
                self.emit_line("");
            }

            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let cond_str = self.transpilar_expresion(condicion);
                self.emit_line(&format!("if {} {{", cond_str));
                self.indent();

                for d in bloque_verdadero {
                    self.transpilar_declaracion(d);
                }

                self.dedent();

                if let Some(bloque_falso) = bloque_falso {
                    self.emit_line("} else {");
                    self.indent();

                    for d in bloque_falso {
                        self.transpilar_declaracion(d);
                    }

                    self.dedent();
                    self.emit_line("}");
                } else {
                    self.emit_line("}");
                }
            }

            Declaracion::Mientras { condicion, bloque } => {
                let cond_str = self.transpilar_expresion(condicion);
                self.emit_line(&format!("while {} {{", cond_str));
                self.indent();

                for d in bloque {
                    self.transpilar_declaracion(d);
                }

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                // Forja: para (i = 0; i < N; i = i + 1) { ... }
                // Rust: for i in 0..N { ... }
                //
                // Si es el patrón estándar (i = 0; i < N; i = i + 1), optimizamos a range
                // De lo contrario, usamos while

                if let Some(cond) = condicion {
                    if let Expresion::Binaria { izquierda, operador: Operador::Menor, derecha } = cond.as_ref() {
                        if let Expresion::Identificador(ref var_name) = izquierda.as_ref() {
                            // Patrón detectado: for x in 0..N
                            let range_end = self.transpilar_expresion(derecha);
                            self.emit_line(&format!("for {} in 0..{} {{", var_name, range_end));
                            self.indent();

                            for d in bloque {
                                self.transpilar_declaracion(d);
                            }

                            self.dedent();
                            self.emit_line("}");
                            return;
                        }
                    }
                }

                // Fallback: generar como while
                let _temp_name = format!("__para_{}", self.temp_counter);
                self.temp_counter += 1;

                if let Some(init) = inicializacion {
                    self.transpilar_declaracion(init);
                }

                let cond_str = if let Some(cond) = condicion {
                    self.transpilar_expresion(cond)
                } else {
                    "true".to_string()
                };

                self.emit_line(&format!("while {} {{", cond_str));
                self.indent();

                // Generar el bloque encerrado en una lambda o scope para el continue
                // For simplicity, generate the block directly
                for d in bloque {
                    self.transpilar_declaracion(d);
                }

                if let Some(inc) = incremento {
                    self.transpilar_declaracion(inc);
                }

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::Repetir { cantidad, bloque } => {
                let cantidad_str = self.transpilar_expresion(cantidad);
                self.emit_line(&format!("for _ in 0..{} {{", cantidad_str));
                self.indent();

                for d in bloque {
                    self.transpilar_declaracion(d);
                }

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == "escribir" {
                    // escribir() -> println!()
                    let args: Vec<String> = argumentos
                        .iter()
                        .map(|a| self.transpilar_expresion(a))
                        .collect();
                    self.emit_line(&format!("println!(\"{}\", {});", "{}", args.join(", ")));
                } else if nombre == "longitud" {
                    let args: Vec<String> = argumentos
                        .iter()
                        .map(|a| self.transpilar_expresion(a))
                        .collect();
                    if args.len() == 1 {
                        self.emit_line(&format!("{}.len();", args[0]));
                    }
                } else if nombre == "BD" {
                    // BD("sqlite:memoria") -> rusqlite::Connection::open_in_memory()
                    self.emit_line("// TODO: Implementar conexión BD");
                    self.emit_line("// usar rusqlite::Connection::open_in_memory()");
                } else if self.es_funcion_externa(nombre) {
                    let args: Vec<String> = argumentos
                        .iter()
                        .map(|a| self.transpilar_expresion(a))
                        .collect();
                    self.emit_line(&format!("unsafe {{ {}({}); }}", nombre, args.join(", ")));
                } else if nombre.ends_with(".enviar") {
                    let obj = nombre.trim_end_matches(".enviar");
                    let args: Vec<String> = argumentos
                        .iter()
                        .map(|a| self.transpilar_expresion(a))
                        .collect();
                    self.emit_line(&format!("{}.send({}).unwrap();", obj, args.join(", ")));
                } else if nombre.ends_with(".recibir") {
                    let obj = nombre.trim_end_matches(".recibir");
                    self.emit_line(&format!("{}.recv().unwrap();", obj));
                } else if nombre.ends_with(".unir") {
                    let obj = nombre.trim_end_matches(".unir");
                    self.emit_line(&format!("{}.join().unwrap();", obj));
                } else if nombre == "asegurar" || nombre.ends_with(".asegurar") {
                    let args: Vec<String> = argumentos
                        .iter()
                        .map(|a| self.transpilar_expresion(a))
                        .collect();
                    if args.len() >= 1 {
                        self.emit_line(&format!("assert!({}, \"Aserción falló\");", args[0]));
                    }
                } else {
                    let args: Vec<String> = argumentos
                        .iter()
                        .map(|a| self.transpilar_expresion(a))
                        .collect();
                    self.emit_line(&format!("{}({});", nombre, args.join(", ")));
                }
            }

            Declaracion::AccesoMiembro { objeto, miembro } => {
                let obj_str = self.transpilar_expresion(objeto);
                self.emit_line(&format!("{}.{};", obj_str, miembro));
            }

            Declaracion::Retornar { valor } => {
                if let Some(val) = valor {
                    let val_str = self.transpilar_expresion(val);
                    self.emit_line(&format!("return {};", val_str));
                } else {
                    self.emit_line("return;");
                }
            }

            Declaracion::AsignacionMultiple { variables, mutable, valor } => {
                let valor_str = self.transpilar_expresion(valor);
                if *mutable {
                    self.emit_line(&format!("let mut ({}) = {};", variables.join(", "), valor_str));
                } else {
                    self.emit_line(&format!("let ({}) = {};", variables.join(", "), valor_str));
                }
            }

            Declaracion::Expresion(expr) => {
                let expr_str = self.transpilar_expresion(expr);
                self.emit_line(&format!("{};", expr_str));
            }
        }
    }

    // ============================================================
    // Transpilación de expresiones
    // ============================================================

    fn transpilar_expresion(&mut self, expr: &Expresion) -> String {
        match expr {
            Expresion::LiteralNumero(n) => n.to_string(),
            Expresion::LiteralDecimal(d) => d.to_string(),
            Expresion::LiteralTexto(s) => {
                    // Escapar TODOS los caracteres especiales (V-08)
                    let escaped = s
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r")
                        .replace('\t', "\\t")
                        .replace('\0', "\\0")
                        .replace('\x07', "\\x07")  // bell
                        .replace('\x08', "\\x08")  // backspace
                        .replace('\x0B', "\\x0B")  // vertical tab
                        .replace('\x0C', "\\x0C"); // form feed
                    format!("String::from(\"{}\")", escaped)
                }
            Expresion::LiteralBooleano(b) => b.to_string(),
            Expresion::LiteralNulo => "()".to_string(),

            Expresion::Identificador(nombre) => {
                if nombre == "self" {
                    "self".to_string()
                } else if nombre == "verdadero" {
                    "true".to_string()
                } else if nombre == "falso" {
                    "false".to_string()
                } else if self.gui_mode && self.gui_vars.iter().any(|(v, _)| v == nombre) {
                    format!("data.{}", nombre)
                } else {
                    nombre.clone()
                }
            }

            Expresion::Binaria { izquierda, operador, derecha } => {
                // Detectar concatenación String + número:
                // "texto" + 42  o  42 + "texto"  →  format!("{}{}", string, numero)
                if let Operador::Suma = operador {
                    let es_texto_izq = matches!(izquierda.as_ref(), Expresion::LiteralTexto(_));
                    let es_texto_der = matches!(derecha.as_ref(), Expresion::LiteralTexto(_));
                    let es_num_izq = matches!(izquierda.as_ref(), Expresion::LiteralNumero(_) | Expresion::LiteralDecimal(_));
                    let es_num_der = matches!(derecha.as_ref(), Expresion::LiteralNumero(_) | Expresion::LiteralDecimal(_));

                    if (es_texto_izq && es_num_der) || (es_num_izq && es_texto_der) {
                        let izq = self.transpilar_expresion(izquierda);
                        let der = self.transpilar_expresion(derecha);
                        return format!("format!(\"{{}}{{}}\", {}, {})", izq, der);
                    }
                }
                let izq_str = self.transpilar_expresion(izquierda);
                let der_str = self.transpilar_expresion(derecha);
                let op_str = match operador {
                    Operador::Suma => " + ",
                    Operador::Resta => " - ",
                    Operador::Multiplicacion => " * ",
                    Operador::Division => " / ",
                    Operador::Modulo => " % ",
                    Operador::Mayor => " > ",
                    Operador::Menor => " < ",
                    Operador::MayorIgual => " >= ",
                    Operador::MenorIgual => " <= ",
                    Operador::IgualIgual => " == ",
                    Operador::Diferente => " != ",
                    Operador::Y => " && ",
                    Operador::O => " || ",
                };
                // Solo añadir paréntesis donde sea necesario por precedencia
                let izq_final = if necesita_parentesis(izquierda, &operador) {
                    format!("({})", izq_str)
                } else {
                    izq_str
                };
                let der_final = if necesita_parentesis(derecha, &operador) {
                    format!("({})", der_str)
                } else {
                    der_str
                };
                format!("{}{}{}", izq_final, op_str, der_final)
            }

            Expresion::Unaria { operador, expr: e } => {
                let e_str = self.transpilar_expresion(e);
                let op_str = match operador {
                    OperadorUnario::Negar => "-",
                    OperadorUnario::No => "!",
                };
                format!("{}{}", op_str, e_str)
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos
                    .iter()
                    .map(|a| self.transpilar_expresion(a))
                    .collect();

                if nombre == "escribir" {
                    format!("println!(\"{}\", {})", "{}", args.join(", "))
                } else if nombre == "BD" {
                    "// BD()".to_string()
                } else if nombre == "longitud" {
                    if args.len() == 1 {
                        format!("{}.len()", args[0])
                    } else {
                        "0usize".to_string()
                    }
                } else if self.es_funcion_externa(nombre) {
                    format!("unsafe {{ {}({}) }}", nombre, args.join(", "))
                } else if nombre.ends_with(".enviar") {
                    let obj = nombre.trim_end_matches(".enviar");
                    format!("{}.send({}).unwrap()", obj, args.join(", "))
                } else if nombre.ends_with(".recibir") {
                    let obj = nombre.trim_end_matches(".recibir");
                    format!("{}.recv().unwrap()", obj)
                } else if nombre.ends_with(".unir") {
                    let obj = nombre.trim_end_matches(".unir");
                    format!("{}.join().unwrap()", obj)
                } else if nombre == "asegurar" || nombre.ends_with(".asegurar") {
                    if !args.is_empty() {
                        format!("assert!({}, \"Aserción falló\")", args[0])
                    } else {
                        format!("assert!(false, \"asegurar() sin argumentos\")")
                    }
                } else {
                    format!("{}({})", nombre, args.join(", "))
                }
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                let obj_str = self.transpilar_expresion(objeto);
                format!("{}.{}", obj_str, miembro)
            }

            Expresion::Instanciacion { clase, argumentos } => {
                let args: Vec<String> = argumentos
                    .iter()
                    .map(|a| self.transpilar_expresion(a))
                    .collect();
                format!("{}::nuevo({})", clase, args.join(", "))
            }

            Expresion::Referencia { expr: e, mutable } => {
                let e_str = self.transpilar_expresion(e);
                if *mutable {
                    format!("&mut {}", e_str)
                } else {
                    format!("&{}", e_str)
                }
            }

            Expresion::Arreglo(elementos) => {
                let elems: Vec<String> = elementos
                    .iter()
                    .map(|e| self.transpilar_expresion(e))
                    .collect();
                format!("vec![{}]", elems.join(", "))
            }

            Expresion::Grupo(expr) => {
                let inner = self.transpilar_expresion(expr);
                format!("({})", inner)
            }

            Expresion::Index { objeto, indice } => {
                let obj_str = self.transpilar_expresion(objeto);
                let idx_str = self.transpilar_expresion(indice);
                format!("{}[{}]", obj_str, idx_str)
            }

            Expresion::Mapa(pares) => {
                let entries: Vec<String> = pares.iter()
                    .map(|(k, v)| format!("({}, {})", self.transpilar_expresion(k), self.transpilar_expresion(v)))
                    .collect();
                format!("std::collections::HashMap::from([{}])", entries.join(", "))
            }

            Expresion::Coincidir { expr, brazos } => {
                let expr_str = self.transpilar_expresion(expr);
                let mut result = format!("match {} {{", expr_str);
                for brazo in brazos {
                    result.push_str(&format!(" {} => {{ ", self.patron_a_rust(&brazo.patron)));
                    result.push_str(" }},");
                }
                result.push_str(" }}");
                result
            }

            Expresion::Closure { parametros, cuerpo } => {
                let params: Vec<String> = parametros.iter()
                    .map(|p| format!("{}: {}", p.nombre, self.inferir_tipo_parametro(p)))
                    .collect();
                let _ = cuerpo;
                format!("|{}| {{}}", params.join(", "))
            }

            Expresion::Hilo { cuerpo } => {
                // Generar: thread::spawn(move || { ... })
                let mut body_parts = Vec::new();
                let prev_decls = std::mem::take(&mut self.declaraciones_globales);
                self.declaraciones_globales = cuerpo.clone();
                let prev_output = std::mem::take(&mut self.output);
                let prev_indent = self.indent_level;

                for d in cuerpo {
                    self.transpilar_declaracion(d);
                }

                body_parts.push(self.output.clone());
                self.output = prev_output;
                self.indent_level = prev_indent;
                self.declaraciones_globales = prev_decls;

                let body = body_parts.join("").trim().to_string();
                format!("thread::spawn(move || {{\n{}    \n}})", body)
            }

            Expresion::Try(expr) => {
                let inner = self.transpilar_expresion(expr);
                format!("{}.into()?", inner)
            }
            Expresion::CanalNuevo => {
                "mpsc::channel()".to_string()
            }
            Expresion::Seleccionar { brazos } => {
                // Transpilar a crossbeam::select! macro
                // Usamos un enfoque similar a Hilo: guardar/restaurar output state
                let mut arms = Vec::new();
                for brazo in brazos {
                    let prev_output = std::mem::take(&mut self.output);
                    let prev_indent = self.indent_level;

                    for d in &brazo.cuerpo {
                        self.transpilar_declaracion(d);
                    }

                    let cuerpo_str = self.output.trim().to_string();
                    self.output = prev_output;
                    self.indent_level = prev_indent;

                    if let Some((var, expr_recv)) = &brazo.recepcion {
                        // caso valor = rx.recibir() { ... }
                        let canal_str = self.transpilar_expresion(expr_recv);
                        arms.push(format!("    recv({}) -> {} => {{\n        {}\n    }},", canal_str, var, cuerpo_str));
                    } else if brazo.timeout_ms > 0 {
                        // tiempo ms { ... } -> default con Duration
                        arms.push(format!("    default(std::time::Duration::from_millis({})) => {{\n        {}\n    }},", brazo.timeout_ms, cuerpo_str));
                    } else {
                        // otro { ... } -> default (sin timeout)
                        arms.push(format!("    default => {{\n        {}\n    }},", cuerpo_str));
                    }
                }
                format!("crossbeam::select!{{\n{}\n}}", arms.join("\n"))
            }
            Expresion::Asignacion { variable, valor } => {
                let val_str = self.transpilar_expresion(valor);
                format!("{{ let __tmp = {}; {} = __tmp; __tmp }}", val_str, variable)
            }
            Expresion::AsignacionCampo { objeto, campo, valor } => {
                let obj_str = self.transpilar_expresion(objeto);
                let val_str = self.transpilar_expresion(valor);
                format!("{{ let __tmp = {}; {}.{} = __tmp; __tmp }}", val_str, obj_str, campo)
            }
            Expresion::ArraySet { array, valor } => {
                // arr[i] = val como expresión → tmp = val; array = tmp; tmp
                let arr_str = self.transpilar_expresion(array);
                let val_str = self.transpilar_expresion(valor);
                format!("{{ let __tmp = {}; {} = __tmp; __tmp }}", val_str, arr_str)
            }
            Expresion::Ok(expr) => {
                format!("Ok({})", self.transpilar_expresion(expr))
            }
            Expresion::Error(expr) => {
                format!("Err({})", self.transpilar_expresion(expr))
            }
            Expresion::Some(expr) => {
                format!("Some({})", self.transpilar_expresion(expr))
            }
        }
    }

    fn patron_a_rust(&self, patron: &Patron) -> String {
        match patron {
            Patron::Variable(n) => n.clone(),
            Patron::Constructor(n, ps) => {
                let sub: Vec<String> = ps.iter().map(|p| self.patron_a_rust(p)).collect();
                format!("{}({})", n, sub.join(", "))
            }
            Patron::Ignorar | Patron::Literal(_) => "_".to_string(),
        }
    }

    fn tipo_a_rust(&self, tipo: &Tipo) -> String {
        match tipo {
            Tipo::Entero => "i64".to_string(),
            Tipo::Decimal => "f64".to_string(),
            Tipo::Texto => "String".to_string(),
            Tipo::Booleano => "bool".to_string(),
            Tipo::Nulo => "()".to_string(),
            Tipo::Clase(nombre) => nombre.clone(),
            Tipo::Arreglo(t) => format!("Vec<{}>", self.tipo_a_rust(t)),
            Tipo::Funcion(params, ret) => {
                let p: Vec<String> = params.iter().map(|t| self.tipo_a_rust(t)).collect();
                format!("fn({}) -> {}", p.join(", "), self.tipo_a_rust(ret))
            }
            Tipo::Resultado(ok, err) => format!("Result<{}, {}>", self.tipo_a_rust(ok), self.tipo_a_rust(err)),
            Tipo::Opcion(inner) => format!("Option<{}>", self.tipo_a_rust(inner)),
            Tipo::TraitObjeto(nombre) => format!("Box<dyn {}>", nombre),
            Tipo::Parametro(nombre) => nombre.clone(),
        }
    }

    // ============================================================
    // Helpers de salida
    // ============================================================

    /// Emite #[derive(...)] a partir de atributos @derive(Mostrar, Igual, ...)
    fn emit_derive_from_atributos(&mut self, atributos: &[Atributo]) {
        if let Some(derive_attr) = atributos.iter().find(|a| a.nombre == "derive") {
            let traits: Vec<&String> = derive_attr.argumentos.iter().filter(|a| {
                matches!(a.as_str(), "Mostrar" | "Igual" | "Debug" | "Clone" | "Copiar")
            }).collect();
            if !traits.is_empty() {
                let rust_traits: Vec<String> = traits.iter().map(|t| {
                    match t.as_str() {
                        "Mostrar" => "Display".to_string(),
                        "Igual" => "PartialEq".to_string(),
                        "Debug" => "Debug".to_string(),
                        "Clone" => "Clone".to_string(),
                        "Copiar" => "Copy".to_string(),
                        _ => t.to_string(),
                    }
                }).collect();
                self.emit_line(&format!("#[derive({})]", rust_traits.join(", ")));
            }
        }
    }

    #[allow(dead_code)]
    fn emit(&mut self, texto: &str) {
        self.output.push_str(texto);
    }

    fn emit_line(&mut self, texto: &str) {
        let indent = "    ".repeat(self.indent_level);
        self.output.push_str(&indent);
        self.output.push_str(texto);
        self.output.push('\n');
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::semantics::BorrowChecker;

    fn transpilar_source(source: &str) -> Result<String, Vec<ErrorForja>> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| e)?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| e)?;

        let mut checker = BorrowChecker::new();
        checker.analizar(&programa).map_err(|e| e)?;

        let mut transpiler = Transpiler::new();
        transpiler.transpilar(&programa)
    }

    #[test]
    fn test_transpilar_variable() {
        let result = transpilar_source("variable x = 5").unwrap();
        // 'variable' sin reasignación -> let (sin mut innecesario)
        assert!(result.contains("let x = 5;"));
        assert!(!result.contains("let mut x = 5;"));
    }

    #[test]
    fn test_transpilar_constante() {
        let result = transpilar_source("constante x = 10").unwrap();
        // 'constante' es inmutable -> let
        assert!(result.contains("let x = 10;"));
    }

    #[test]
    fn test_transpilar_escribir() {
        let result = transpilar_source("escribir(\"Hola mundo\")").unwrap();
        assert!(result.contains("println!"));
        assert!(result.contains("Hola mundo"));
    }

    #[test]
    fn test_transpilar_si_sino() {
        let source = "variable x = 5\nsi (x > 0) { variable y = 1 } sino { variable z = 2 }";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("if"));
        assert!(result.contains("else"));
    }

    #[test]
    fn test_transpilar_mientras() {
        let source = "variable x = 0\nmientras (x < 10) { x = x + 1 }";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("while"));
    }

    #[test]
    fn test_transpilar_repetir() {
        let source = "repetir (5) { escribir(\"hola\") }";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("for _ in 0..5"));
    }

    #[test]
    fn test_transpilar_para() {
        let source = "para (variable i = 0; i < 10; i = i + 1) { escribir(i) }";
        let result = transpilar_source(source).unwrap();
        // Debe optimizar a for i in 0..10
        assert!(result.contains("for i in 0..10") || result.contains("while"));
    }

    #[test]
    fn test_transpilar_clase() {
        let source = "clase Persona { nombre constructor(n) { este.nombre = n } }";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("struct Persona"));
        assert!(result.contains("impl Persona"));
        assert!(result.contains("fn nuevo"));
    }

    #[test]
    fn test_transpilar_instanciacion() {
        let source = "clase Persona { nombre constructor(n) { este.nombre = n } } variable p = nuevo Persona(\"Ana\")";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("Persona::nuevo"));
    }

    #[test]
    fn test_transpilar_referencia() {
        let source = "variable x = 5\nvariable y = &x";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("&x"));
    }

    #[test]
    fn test_transpilar_main_generado() {
        let source = "variable x = 5\nescribir(x)";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("fn main()"));
        // x no se reasigna, solo se lee -> let sin mut
        assert!(result.contains("let x = 5;"));
        assert!(!result.contains("let mut x = 5;"));
    }

    #[test]
    fn test_transpilar_variable_con_reasignacion() {
        let source = "variable x = 5\nx = 10";
        let result = transpilar_source(source).unwrap();
        // x se reasigna -> debe ser let mut
        assert!(result.contains("let mut x = 5;"));
    }

    #[test]
    fn test_transpilar_variable_reasignada_en_si() {
        let source = "variable x = 5\nsi (verdadero) { x = 10 }";
        let result = transpilar_source(source).unwrap();
        // x se reasigna dentro de un bloque si -> debe ser let mut
        assert!(result.contains("let mut x = 5;"));
    }

    #[test]
    fn test_transpilar_variable_reasignada_en_mientras() {
        let source = "variable x = 0\nmientras (x < 10) { x = x + 1 }";
        let result = transpilar_source(source).unwrap();
        // x se reasigna dentro del bucle -> debe ser let mut
        assert!(result.contains("let mut x = 0;"));
    }

    #[test]
    fn test_transpilar_variable_sin_uso() {
        let source = "variable x = 5\nvariable y = 10";
        let result = transpilar_source(source).unwrap();
        // Ninguna se reasigna -> let sin mut
        assert!(result.contains("let x = 5;"));
        assert!(result.contains("let y = 10;"));
        assert!(!result.contains("let mut"));
    }

    #[test]
    fn test_gui_appstate_sin_variables() {
        let source = "importar \"gui\"\nfuncion main() {\n    escribir(\"hola\")\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("struct AppState {"));
        assert!(result.contains("_placeholder: (),"));
        assert!(result.contains("fn app_logic(data: &mut AppState)"));
    }

    #[test]
    fn test_gui_appstate_con_variables() {
        let source = "importar \"gui\"\nfuncion main() {\n    variable usuario = \"admin\"\n    variable contrasena = \"secreta\"\n    escribir(\"Usuario: \" + usuario)\n    escribir(\"Contrasena: \" + contrasena)\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("struct AppState {"));
        assert!(result.contains("    usuario: String,"));
        assert!(result.contains("    contrasena: String,"));
        assert!(!result.contains("_placeholder"));
    }

    #[test]
    fn test_gui_appstate_con_tipos_mixtos() {
        let source = "importar \"gui\"\nfuncion main() {\n    variable edad: Entero = 25\n    variable nombre: Texto = \"Ana\"\n    variable activo: Booleano = verdadero\n    escribir(nombre)\n    escribir(edad)\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("    edad: i32,"));
        assert!(result.contains("    nombre: String,"));
        assert!(result.contains("    activo: bool,"));
    }

    #[test]
    fn test_gui_appstate_variables_en_main_referencia_data() {
        let source = "importar \"gui\"\nfuncion main() {\n    variable mensaje = \"Hola Mundo\"\n    escribir(mensaje)\n}";
        let result = transpilar_source(source).unwrap();
        // Dentro de app_logic, 'mensaje' debe referenciarse como data.mensaje
        assert!(result.contains("data.mensaje"));
    }

    #[test]
    fn test_gui_boton_con_callback() {
        let source = "importar \"gui\"\nfuncion al_saludar() { escribir(\"Hola!\") }\nfuncion main() {\n    boton(\"Saludar\", &al_saludar)\n}";
        let result = transpilar_source(source).unwrap();
        // Debe generar text_button con callback que invoca al_saludar()
        assert!(result.contains("al_saludar();"));
        // No debe tener el hardcode d.contador
        assert!(!result.contains("d.contador"));
        // Debe mantener el texto del boton
        assert!(result.contains("Saludar"));
    }

    #[test]
    fn test_gui_boton_sin_callback() {
        let source = "importar \"gui\"\nfuncion main() {\n    boton(\"Cerrar\")\n}";
        let result = transpilar_source(source).unwrap();
        // Debe generar println! como callback
        assert!(result.contains("println!"));
        assert!(result.contains("Cerrar"));
        // No debe tener referencia a funcion
        assert!(!result.contains("();"));
    }

    #[test]
    fn test_gui_boton_con_callback_sin_referencia() {
        // Prueba: boton("Texto", &fn) SIN espacio entre & y nombre
        let source = "importar \"gui\"\nfuncion validar() { escribir(\"ok\") }\nfuncion main() {\n    boton(\"Validar\", &validar)\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("validar();"));
        assert!(!result.contains("d.contador"));
    }

    #[test]
    fn test_gui_boton_y_etiqueta_mezclados() {
        let source = "importar \"gui\"\nfuncion accion() {}\nfuncion main() {\n    etiqueta(\"Titulo\")\n    boton(\"Click\", &accion)\n    boton(\"Otro\")\n}";
        let result = transpilar_source(source).unwrap();
        // Debe tener 3 widgets generados
        assert!(result.contains("label("));
        assert!(result.contains("accion();"));
        assert!(result.contains("println!"));
    }

    #[test]
    fn test_gui_columna_basica() {
        let source = "importar \"gui\"\nfuncion main() {\n    columna(escribir(\"Arriba\"), boton(\"Click\"))\n}";
        let result = transpilar_source(source).unwrap();
        // Debe generar flex(Axis::Vertical, (...))
        assert!(result.contains("flex(Axis::Vertical, ("));
        assert!(result.contains("label("));
        assert!(result.contains("text_button("));
        // No debe tener el wrapper plano view::flex extra
        assert!(!result.contains("view::flex(Axis::Vertical, (\n        view::flex(Axis::Vertical, ("));
        // Debe tener cierre del flex
        assert!(result.contains(")),"));
    }

    #[test]
    fn test_gui_columna_fila_anidado() {
        let source = "importar \"gui\"\nfuncion main() {\n    columna(escribir(\"Arriba\"), boton(\"Click\"), fila(escribir(\"Izq\"), escribir(\"Der\")))\n}";
        let result = transpilar_source(source).unwrap();
        // Debe generar flex vertical con label, boton, y flex horizontal anidado
        assert!(result.contains("flex(Axis::Vertical, ("));
        assert!(result.contains("flex(Axis::Horizontal, ("));
        assert!(result.contains("label(String::from(\"Arriba\"))"));
        assert!(result.contains("text_button(String::from(\"Click\")"));
        assert!(result.contains("label(String::from(\"Izq\"))"));
        assert!(result.contains("label(String::from(\"Der\"))"));
        // Verificar orden: label, boton, flex horizontal
        let pos_arriba = result.find("Arriba").unwrap();
        let pos_click = result.find("Click").unwrap();
        let pos_izq = result.find("Izq").unwrap();
        assert!(pos_arriba < pos_click, "Arriba debe ir antes que Click");
        assert!(pos_click < pos_izq, "Click debe ir antes que Izq");
    }

    #[test]
    fn test_gui_fila_basica() {
        let source = "importar \"gui\"\nfuncion main() {\n    fila(escribir(\"A\"), escribir(\"B\"))\n}";
        let result = transpilar_source(source).unwrap();
        // Debe generar flex horizontal
        assert!(result.contains("flex(Axis::Horizontal, ("));
        assert!(result.contains("label(String::from(\"A\"))"));
        assert!(result.contains("label(String::from(\"B\"))"));
    }

    #[test]
    fn test_gui_widgets_plano_sin_columna_siguen_funcionando() {
        let source = "importar \"gui\"\nfuncion main() {\n    etiqueta(\"Titulo\")\n    boton(\"Click\")\n}";
        let result = transpilar_source(source).unwrap();
        // Sin columna/fila, debe seguir emitiendo el flex wrapper antiguo
        assert!(result.contains("view::flex(Axis::Vertical, ("));
        assert!(result.contains("label("));
        assert!(result.contains("text_button("));
    }

    #[test]
    fn test_gui_entrada_texto() {
        let source = "importar \"gui\"\nfuncion main() {\n    entrada_texto(\"Nombre\")\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("view::text_input("));
        assert!(result.contains("Nombre"));
    }

    #[test]
    fn test_gui_barra_progreso() {
        let source = "importar \"gui\"\nfuncion main() {\n    barra_progreso(0.5)\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("view::progress_bar(0.5"));
    }

    #[test]
    fn test_gui_slider() {
        let source = "importar \"gui\"\nfuncion main() {\n    deslizante(50, 0, 100)\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("view::slider(50, 0, 100"));
    }

    #[test]
    fn test_gui_checkbox() {
        let source = "importar \"gui\"\nfuncion main() {\n    casilla(\"Aceptar terminos\")\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("view::checkbox("));
    }

    #[test]
    fn test_gui_todos_widgets_juntos() {
        let source = "importar \"gui\"\nfuncion main() {\n    escribir(\"Config\")\n    entrada_texto(\"Nombre\")\n    barra_progreso(0.5)\n    casilla(\"Aceptar\")\n    deslizante(50, 0, 100)\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("view::label("));
        assert!(result.contains("view::text_input("));
        assert!(result.contains("view::progress_bar(0.5"));
        assert!(result.contains("view::checkbox("));
        assert!(result.contains("view::slider(50, 0, 100"));
    }
}
