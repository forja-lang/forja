// Forja GUI Nativa — construye widgets xilem directamente desde el AST
#![allow(dead_code)]

use std::collections::HashMap;
use crate::ast::*;
use forja_gui_rt::*;
use forja_gui_rt::view::{self, Axis};

#[derive(Debug, Clone)]
pub enum ValorGUI {
    Texto(String),
    Entero(i64),
    Nulo,
}

impl ValorGUI {
    fn to_string(&self) -> String {
        match self {
            ValorGUI::Texto(s) => s.clone(),
            ValorGUI::Entero(n) => n.to_string(),
            ValorGUI::Nulo => "nulo".to_string(),
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
    Label { texto: String, es_variable: bool },
    Button { texto: String, callback: String },
    TextInput { variable: String },
    ProgressBar { variable: String },
    Slider { variable: String },
    Checkbox { variable: String },
    Spacer,
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
                "escribir" | "etiqueta" | "label" => {
                    if let Some(arg) = argumentos.first() {
                        match arg {
                            Expresion::Identificador(v) =>
                                Some(Layout::Label { texto: v.clone(), es_variable: true }),
                            Expresion::LiteralTexto(s) =>
                                Some(Layout::Label { texto: s.clone(), es_variable: false }),
                            _ => Some(Layout::Spacer),
                        }
                    } else { Some(Layout::Spacer) }
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
                    Some(Layout::TextInput { variable })
                }
                "barra_progreso" | "gui_barra_progreso" | "progress_bar" => {
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
                    Some(Layout::Slider { variable })
                }
                "casilla" | "gui_casilla" | "checkbox" | "check" => {
                    let variable = argumentos.first()
                        .map(|a| match a {
                            Expresion::LiteralTexto(s) => s.clone(),
                            Expresion::Identificador(s) => s.clone(),
                            _ => String::new(),
                        }).unwrap_or_default();
                    Some(Layout::Checkbox { variable })
                }
                "columna" | "gui_columna" => Some(Layout::Column(procesar_args(argumentos))),
                "fila" | "gui_fila" => Some(Layout::Row(procesar_args(argumentos))),
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
        Layout::Label { texto, es_variable } => {
            let txt = if *es_variable { data.leer(texto).to_string() } else { texto.clone() };
            Box::new(view::label(txt))
        }
        Layout::Button { texto, callback } => {
            let cb = callback.clone();
            let t = texto.clone();
            let prog = _prog.to_vec();
            Box::new(view::text_button(t, move |data: &mut AppStateNativo| {
                ejecutar_callback_y_actualizar(&cb, data, &prog);
            }))
        }
        Layout::TextInput { variable } => {
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
        Layout::Slider { variable } => {
            let var_name = variable.clone();
            let val = data.leer(variable).to_string();
            let num: f64 = val.parse().unwrap_or(0.0);
            Box::new(view::slider(num, 0.0, 100.0, move |data: &mut AppStateNativo, new_val: f64| {
                data.escribir(&var_name, ValorGUI::Entero(new_val as i64));
            }))
        }
        Layout::Checkbox { variable } => {
            let var_name = variable.clone();
            let txt = variable.clone();
            let checked = data.leer(variable).to_string() == "verdadero";
            Box::new(view::checkbox(txt, checked, move |data: &mut AppStateNativo, new_checked: bool| {
                data.escribir(&var_name, ValorGUI::Texto(if new_checked { "verdadero".to_string() } else { "falso".to_string() }));
            }))
        }
        Layout::Spacer => Box::new(view::label("")),
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
