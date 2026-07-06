// Forja GUI Nativa — construye widgets xilem directamente desde el AST
#![allow(dead_code)]

use std::collections::HashMap;
use crate::ast::*;
use forja_gui_rt::*;
use forja_gui_rt::view::{self, Axis};
use forja_gui_rt::Length;

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
    pub variables: HashMap<String, ValorGUI>,
}

impl AppStateNativo {
    pub fn new() -> Self {
        AppStateNativo { variables: HashMap::new() }
    }
    pub fn leer(&self, nombre: &str) -> ValorGUI {
        self.variables.get(nombre).cloned().unwrap_or(ValorGUI::Nulo)
    }
    pub fn escribir(&mut self, nombre: &str, valor: ValorGUI) {
        self.variables.insert(nombre.to_string(), valor);
    }
}

impl Default for AppStateNativo {
    fn default() -> Self { Self::new() }
}

// ─── Layout (representación intermedia) ───────────────────────────

enum Layout {
    Column(Vec<Layout>),
    Row(Vec<Layout>),
    ZStack(Vec<Layout>),
    Portal(Box<Layout>),
    Label { texto: String, es_variable: bool },
    VariableLabel { variable: String },
    Button { texto: String, callback: String },
    TextInput { variable: String, multiline: bool },
    ProgressBar { variable: String },
    Slider { variable: String, min: f64, max: f64 },
    Checkbox { variable: String },
    Prose(String),
    Spinner,
    Separator,
    Spacer(f64),
}

// ─── AST → Layout ─────────────────────────────────────────────────

/// Extrae el layout del AST
fn extraer_layout(decls: &[Declaracion]) -> Layout {
    for decl in decls {
        if let Declaracion::Funcion { nombre, cuerpo, .. } = decl {
            if nombre == "main" {
                for d in cuerpo {
                    if let Declaracion::LlamadaFuncion { nombre, argumentos } = d {
                        match nombre.as_str() {
                            "columna" | "gui_columna" =>
                                return Layout::Column(procesar_args(argumentos)),
                            "fila" | "gui_fila" =>
                                return Layout::Row(procesar_args(argumentos)),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    Layout::Column(vec![])
}

fn procesar_args(args: &[Expresion]) -> Vec<Layout> {
    args.iter().filter_map(expr_a_layout).collect()
}

fn expr_a_layout(expr: &Expresion) -> Option<Layout> {
    match expr {
        Expresion::LlamadaFuncion { nombre, argumentos } => {
            match nombre.as_str() {
                "escribir" | "etiqueta" | "label" | "text" => {
                    if let Some(arg) = argumentos.first() {
                        match arg {
                            Expresion::Identificador(v) =>
                                Some(Layout::Label { texto: v.clone(), es_variable: true }),
                            Expresion::LiteralTexto(s) =>
                                Some(Layout::Label { texto: s.clone(), es_variable: false }),
                            _ => Some(Layout::Spacer(0.0)),
                        }
                    } else { Some(Layout::Spacer(0.0)) }
                }
                "etiqueta_dinamica" | "varlabel" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::Identificador(s) => s.clone(),
                            Expresion::LiteralTexto(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::VariableLabel { variable })
                }
                "boton" | "button" | "btn" => {
                    let texto = argumentos.first()
                        .map(|a| match a { Expresion::LiteralTexto(s) => s.clone(), _ => String::new() })
                        .unwrap_or_default();
                    let callback = argumentos.get(1)
                        .map(|a| match a {
                            Expresion::Referencia { expr, .. } => match expr.as_ref() {
                                Expresion::Identificador(n) => n.clone(), _ => String::new()
                            }, _ => String::new()
                        }).unwrap_or_default();
                    Some(Layout::Button { texto, callback })
                }
                "entrada_texto" | "text_input" | "input" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::TextInput { variable, multiline: false })
                }
                "area_texto" | "textarea" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::TextInput { variable, multiline: true })
                }
                "barra_progreso" | "gui_barra_progreso" | "progress_bar" | "progress" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::ProgressBar { variable })
                }
                "deslizante" | "gui_deslizante" | "slider" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s) => s.clone(),
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
                            Expresion::Identificador(s) => s.clone(),
                            _ => String::new(),
                        }).or_else(|| {
                            argumentos.first().map(|a| match a {
                                Expresion::LiteralTexto(s) => s.clone(),
                                Expresion::Identificador(s) => s.clone(),
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
                "columna" | "gui_columna" => Some(Layout::Column(procesar_args(argumentos))),
                "fila" | "gui_fila" => Some(Layout::Row(procesar_args(argumentos))),
                "pila" | "gui_pila" | "zstack" => Some(Layout::ZStack(procesar_args(argumentos))),
                "desplazable" | "gui_desplazable" | "scroll" => {
                    argumentos.first().and_then(|a| expr_a_layout(a))
                        .map(|child| Layout::Portal(Box::new(child)))
                }
                _ => None,
            }
        }
        Expresion::LiteralTexto(s) =>
            Some(Layout::Label { texto: s.clone(), es_variable: false }),
        _ => None,
    }
}

// ─── Layout → xilem widgets ───────────────────────────────────────

/// Convierte Layout a xilem usando AnyWidgetView para type erasure
fn layout_a_view<'a>(
    layout: &'a Layout,
    data: &'a mut AppStateNativo,
    _prog: &'a [Declaracion],
) -> Box<AnyWidgetView<AppStateNativo>> {
    match layout {
        Layout::Column(hijos) => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in hijos {
                widgets.push(layout_a_view(h, data, _prog));
            }
            Box::new(view::flex(Axis::Vertical, (widgets,)))
        }
        Layout::Row(hijos) => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in hijos {
                widgets.push(layout_a_view(h, data, _prog));
            }
            Box::new(view::flex(Axis::Horizontal, (widgets,)))
        }
        Layout::ZStack(hijos) => {
            let mut widgets: Vec<Box<AnyWidgetView<AppStateNativo>>> = Vec::new();
            for h in hijos {
                widgets.push(layout_a_view(h, data, _prog));
            }
            Box::new(view::zstack((widgets,)))
        }
        Layout::Portal(child) => {
            let inner = layout_a_view(child, data, _prog);
            Box::new(view::portal(inner))
        }
        Layout::Label { texto, es_variable } => {
            let txt = if *es_variable { data.leer(texto).to_string() } else { texto.clone() };
            Box::new(view::label(txt))
        }
        Layout::VariableLabel { variable } => {
            let txt = data.leer(variable).to_string();
            Box::new(view::variable_label(txt))
        }
        Layout::Button { texto, callback } => {
            let cb = callback.clone();
            let t = texto.clone();
            let prog = _prog.to_vec();
            Box::new(view::text_button(t, move |data: &mut AppStateNativo| {
                ejecutar_callback_y_actualizar(&cb, data, &prog);
            }))
        }
        Layout::TextInput { variable, multiline: _ } => {
            let val = data.leer(variable).to_string();
            let var_name = variable.clone();
            Box::new(view::text_input(val, move |data: &mut AppStateNativo, new_val: String| {
                data.escribir(&var_name, ValorGUI::Texto(new_val));
            }))
        }
        Layout::ProgressBar { variable } => {
            let val = data.leer(variable).to_string();
            let num: f64 = val.parse().unwrap_or(0.0);
            Box::new(view::progress_bar(Some(num)))
        }
        Layout::Slider { variable, min, max } => {
            let var_name = variable.clone();
            let val = data.leer(variable).to_f64();
            let mn = *min;
            let mx = *max;
            Box::new(view::slider(val, mn, mx, move |data: &mut AppStateNativo, new_val: f64| {
                data.escribir(&var_name, ValorGUI::Decimal(new_val));
            }))
        }
        Layout::Checkbox { variable } => {
            let var_name = variable.clone();
            let txt = variable.clone();
            let checked = data.leer(variable).to_bool();
            Box::new(view::checkbox(txt, checked, move |data: &mut AppStateNativo, new_checked: bool| {
                data.escribir(&var_name, ValorGUI::Booleano(new_checked));
            }))
        }
        Layout::Prose(texto) => {
            Box::new(view::prose(texto.clone()))
        }
        Layout::Spinner => {
            Box::new(view::spinner())
        }
        Layout::Separator => {
            Box::new(view::sized_box(view::label(String::new())).height(Length::px(1.0)))
        }
        Layout::Spacer(tamano) => {
            let t = *tamano;
            Box::new(view::sized_box(view::label(String::new())).width(Length::px(t)).height(Length::px(t)))
        }
    }
}

// ─── Punto de entrada ─────────────────────────────────────────────

pub fn build_and_run(programa: &Programa) -> Result<(), String> {
    let mut state = AppStateNativo::new();
    inicializar_estado(&programa.declaraciones, &mut state);
    let layout = extraer_layout(&programa.declaraciones);
    let prog = programa.declaraciones.clone();

    println!("  🪟 Lanzando ventana GUI nativa...");

    let app = Xilem::new_simple(
        state,
        move |data: &mut AppStateNativo| -> Box<AnyWidgetView<AppStateNativo>> {
            layout_a_view(&layout, data, &prog)
        },
        WindowOptions::new("Forja GUI".to_string()),
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
            Declaracion::Retornar { valor } => {
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
        Expresion::LiteralNulo => ValorGUI::Nulo,
        Expresion::Identificador(v) => {
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
    state.variables.insert("resultado".to_string(), resultado);
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
                            _ => ValorGUI::Texto(String::new()),
                        };
                        state.variables.insert(nombre.clone(), v);
                    }
                }
                return;
            }
        }
    }
}
