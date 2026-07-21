#![allow(dead_code)]
use crate::ast::*;
use crate::error::ErrorForja;
use std::collections::HashMap;

/// Profundidad máxima de recursión para la transpilación.
/// Previene stack overflow al recorrer ASTs con expresiones muy anidadas.
const MAX_AST_PROFUNDIDAD: u32 = 10000;

/// Analiza si una variable es realmente mutable (se reasigna) dentro de un conjunto de declaraciones.
/// Busca declaraciones `Asignacion` o `AsignacionIndex` que modifiquen la variable,
/// respetando límites de ámbito (no cruza fronteras de función).
fn es_variable_mutable(nombre: &str, declaraciones: &[Declaracion], _ambito_actual: usize) -> bool {
    for decl in declaraciones {
        match decl {
            Declaracion::Asignacion { nombre: var, .. } if var == nombre => return true,
            Declaracion::AsignacionIndex { nombre: var, .. } if var == nombre => return true,
            // Buscar en ámbitos anidados (bloques si, mientras, para, repetir)
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
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
            Declaracion::Cuando { cuerpo, .. } => {
                if es_variable_mutable(nombre, cuerpo, _ambito_actual + 1) {
                    return true;
                }
            }
            Declaracion::Para {
                bloque, incremento, ..
            } => {
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

fn existe_variable_resultado(declaraciones: &[Declaracion]) -> bool {
    for decl in declaraciones {
        match decl {
            Declaracion::Variable { nombre, .. } if nombre == "resultado" => return true,
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
                if existe_variable_resultado(bloque_verdadero) {
                    return true;
                }
                if let Some(bf) = bloque_falso {
                    if existe_variable_resultado(bf) {
                        return true;
                    }
                }
            }
            Declaracion::Mientras { bloque, .. } => {
                if existe_variable_resultado(bloque) {
                    return true;
                }
            }
            Declaracion::Cuando { cuerpo, .. } => {
                if existe_variable_resultado(cuerpo) {
                    return true;
                }
            }
            Declaracion::Para { bloque, .. } => {
                if existe_variable_resultado(bloque) {
                    return true;
                }
            }
            Declaracion::Repetir { bloque, .. } => {
                if existe_variable_resultado(bloque) {
                    return true;
                }
            }
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
    /// Postcondiciones de la función actual (para transformar retornar)
    postcondiciones_actuales: Vec<Contrato>,
    /// Si la función actual tiene postcondiciones activas
    modo_postcondiciones: bool,
    /// Si la función/método actual tiene un parámetro o variable local "resultado"
    tiene_resultado_var: bool,
    /// Profundidad actual de recursión al transpilar expresiones.
    /// Previene stack overflow en ASTs con expresiones muy anidadas.
    profundidad_expresion: u32,
}

struct ClaseInfo {
    #[allow(dead_code)]
    campos: Vec<(String, String)>, // (nombre_campo, tipo)
    #[allow(dead_code)]
    metodos: Vec<String>, // nombres de métodos
    /// Mapa campo -> tipo inferido desde constructor
    tipos_campos: HashMap<String, String>,
    /// Invariantes de clase (Design by Contract)
    invariantes: Vec<Contrato>,
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
            saltar_main: false,
            postcondiciones_actuales: Vec::new(),
            modo_postcondiciones: false,
            tiene_resultado_var: false,
            profundidad_expresion: 0,
        }
    }

    /// Indica si el programa usa el paquete GUI
    pub fn usa_gui(&self) -> bool {
        self.declaraciones_globales
            .iter()
            .any(|d| matches!(d, Declaracion::Importar(ruta) if ruta == "gui"))
    }

    /// Exporta un programa Forja a código Rust (opcional, Forja ya ejecuta directo con VM)
    pub fn transpilar(&mut self, programa: &Programa) -> Result<String, Vec<ErrorForja>> {
        // Almacenar declaraciones globales para análisis de mutabilidad
        self.declaraciones_globales = programa.declaraciones.clone();

        // Primera pasada: recolectar clases
        self.recolectar_clases(&programa.declaraciones);

        // Segunda pasada: generar código
        self.emit_line(
            "// Código exportado desde Forja (fa) — https://github.com/forja-lang/forja",
        );
        self.emit_line(
            "// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust",
        );
        self.emit_line("");

        // En Windows, las apps GUI deben usar el subsistema "windows" para no mostrar consola
        if self.usa_gui() {
            self.emit_line(
                "#![cfg_attr(target_os = \"windows\", windows_subsystem = \"windows\")]",
            );
            self.emit_line("");
        }

        // Detectar si hay concurrencia para añadir imports
        let tiene_concurrencia = self.detectar_concurrencia(&programa.declaraciones);
        if tiene_concurrencia {
            self.emit_line("use std::thread;");
            self.emit_line("use std::sync::mpsc;");
            self.emit_line("");
        }

        // Detectar si se usa el paquete GUI para emitir código que usa forja_gui_rt
        if self.usa_gui() {
            self.emit_line("// ─── GUI: Forja GUI Runtime (xilem precompilado) ───");
            self.emit_line("use forja_gui_rt::*;");
            self.emit_line("");
        }

        // Recolectar funciones externas
        self.funciones_externas = programa
            .declaraciones
            .iter()
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
                if let Declaracion::Funcion {
                    nombre,
                    parametros,
                    tipo_retorno,
                    externa: true,
                    ..
                } = decl
                {
                    let params_str: Vec<String> = parametros
                        .iter()
                        .map(|p| {
                            format!(
                                "{}: {}",
                                p.nombre,
                                self.tipo_a_rust(p.tipo.as_ref().unwrap_or(&Tipo::Entero))
                            )
                        })
                        .collect();
                    let ret = tipo_retorno
                        .as_ref()
                        .map(|t| self.tipo_a_rust(t))
                        .unwrap_or_else(|| "()".to_string());
                    self.emit_line(&format!(
                        "fn {}({}) -> {};",
                        nombre,
                        params_str.join(", "),
                        ret
                    ));
                }
            }
            self.dedent();
            self.emit_line("}");
            self.emit_line("");
        }

        // Detectar si hay función main o clases para generar el fn main()
        let tiene_main = programa
            .declaraciones
            .iter()
            .any(|d| matches!(d, Declaracion::Funcion { nombre, .. } if nombre == "main"));
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

        // Generar rasgos e implementaciones después de las funciones
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Rasgo { .. } | Declaracion::Implementacion { .. } => {
                    self.transpilar_declaracion(decl);
                    self.emit_line("");
                }
                _ => {}
            }
        }

        // Si hay GUI: generar el AST completo como datos estáticos para el runtime
        if self.usa_gui() {
            // Generar el programa completo como datos estáticos de Rust
            // El runtime (forja_gui_rt) se encarga de todo: tema, estado, eventos, layout, bucle
            let ast_code = self.generar_ast_programa(&programa.declaraciones);
            self.emit_line("// ─── Programa Forja como datos estáticos para el runtime ───");
            self.emit_line("use forja::ast::*;");
            self.emit_line("");
            self.emit_line("static PROGRAMA: Programa = Programa {");
            self.emit_line("    declaraciones: vec![");
            self.indent();
            for line in ast_code.lines() {
                self.emit_line(line);
            }
            self.dedent();
            self.emit_line("    ],");
            self.emit_line("};");
            self.emit_line("");

            // main() delega completamente al runtime
            // Soporta --load-state=<json> para hot reload
            self.emit_line("fn main() -> Result<(), String> {");
            self.emit_line("    let args: Vec<String> = std::env::args().collect();");
            self.emit_line("    let load_state = args.iter()");
            self.emit_line("        .find_map(|a| a.strip_prefix(\"--load-state=\"))");
            self.emit_line("        .map(|s| s.to_string());");
            self.emit_line("");
            self.emit_line(
                "    forja_gui_rt::build_and_run(&PROGRAMA, load_state.as_deref(), None, true)",
            );
            self.emit_line("}");
            self.emit_line("");

            // Android entry point
            self.emit_line("#[cfg(target_os = \"android\")]");
            self.emit_line("#[no_mangle]");
            self.emit_line(
                "fn android_main(app: winit::platform::android::activity::AndroidApp) {",
            );
            self.emit_line(
                "    forja_gui_rt::build_and_run_android(&PROGRAMA, None, None, true, app);",
            );
            self.emit_line("}");
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

    /// Recolecta expresiones Layout recorriendo el AST recursivamente.
    /// Similar a `recolectar_widgets` pero genera `Layout::xxx` en lugar de widgets Xilem.
    fn recolectar_layouts(
        &mut self,
        declaraciones: &[Declaracion],
        layouts: &mut Vec<Vec<String>>,
    ) {
        for decl in declaraciones {
            match decl {
                Declaracion::LlamadaFuncion { nombre, argumentos } => {
                    let expr = Expresion::LlamadaFuncion {
                        nombre: nombre.clone(),
                        argumentos: argumentos.clone(),
                    };
                    let layout_code = self.generar_layout_code(&expr);
                    layouts.push(vec![layout_code]);
                }
                Declaracion::Expresion(expr) => {
                    let layout_code = self.generar_layout_code(expr);
                    layouts.push(vec![layout_code]);
                }
                Declaracion::Funcion { cuerpo, .. } => {
                    self.recolectar_layouts(cuerpo, layouts);
                }
                Declaracion::Si {
                    bloque_verdadero,
                    bloque_falso,
                    ..
                } => {
                    self.recolectar_layouts(bloque_verdadero, layouts);
                    if let Some(bf) = bloque_falso {
                        self.recolectar_layouts(bf, layouts);
                    }
                }
                Declaracion::Mientras { bloque, .. } => {
                    self.recolectar_layouts(bloque, layouts);
                }
                Declaracion::Cuando { cuerpo, .. } => {
                    self.recolectar_layouts(cuerpo, layouts);
                }
                Declaracion::Para { bloque, .. } => {
                    self.recolectar_layouts(bloque, layouts);
                }
                Declaracion::Repetir { bloque, .. } => {
                    self.recolectar_layouts(bloque, layouts);
                }
                _ => {}
            }
        }
    }

    /// Genera código Rust que construye un valor `Layout::xxx` directamente,
    /// delegando al runtime de forja_gui_rt para la conversión a widgets reales.
    fn generar_layout_code(&mut self, expr: &Expresion) -> String {
        match expr {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let nombre_lower = nombre.to_lowercase();
                match nombre_lower.as_str() {
                    // Contenedores básicos (se generan como Layout recursivo)
                    "columna" | "gui_columna" => {
                        let children: Vec<String> = argumentos.iter()
                            .map(|a| self.generar_layout_code(a))
                            .collect();
                        format!(
                            "Layout::Column {{ children: vec![{}], gap: 8.0, alignment: String::from(\"start\") }}",
                            children.join(", ")
                        )
                    }
                    "fila" | "gui_fila" => {
                        let children: Vec<String> = argumentos.iter()
                            .map(|a| self.generar_layout_code(a))
                            .collect();
                        format!(
                            "Layout::Row {{ children: vec![{}], gap: 8.0, alignment: String::from(\"start\") }}",
                            children.join(", ")
                        )
                    }
                    "pila" | "gui_pila" | "zstack" => {
                        let children: Vec<String> = argumentos.iter()
                            .map(|a| self.generar_layout_code(a))
                            .collect();
                        format!("Layout::ZStack(vec![{}])", children.join(", "))
                    }
                    "desplazable" | "gui_desplazable" | "scroll" => {
                        if let Some(arg) = argumentos.first() {
                            format!("Layout::Portal(Box::new({}))", self.generar_layout_code(arg))
                        } else {
                            "Layout::Spacer(0.0)".to_string()
                        }
                    }
                    // Labels / Texto
                    "escribir" | "etiqueta" | "gui_etiqueta" | "text" | "label" => {
                        if let Some(arg) = argumentos.first() {
                            match arg {
                                Expresion::Identificador { nombre: var, .. } => {
                                    format!("Layout::Label {{ texto: String::from(\"{}\"), es_variable: true }}", var)
                                }
                                Expresion::LiteralTexto(s) => {
                                    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                                    format!("Layout::Label {{ texto: String::from(\"{}\"), es_variable: false }}", escaped)
                                }
                                _ => {
                                    "Layout::Label { texto: String::from(\"\"), es_variable: false }".to_string()
                                }
                            }
                        } else {
                            "Layout::Label { texto: String::from(\"\"), es_variable: false }".to_string()
                        }
                    }
                    "etiqueta_dinamica" | "varlabel" => {
                        if let Some(Expresion::LiteralTexto(var_name)) = argumentos.first() {
                            format!("Layout::VariableLabel {{ variable: String::from(\"{}\") }}", var_name)
                        } else {
                            "Layout::VariableLabel { variable: String::from(\"\") }".to_string()
                        }
                    }
                    "texto_grande" | "heading" | "title" => {
                        if let Some(arg) = argumentos.first() {
                            match arg {
                                Expresion::LiteralTexto(s) => {
                                    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                                    format!("Layout::Title(String::from(\"{}\"))", escaped)
                                }
                                _ => "Layout::Title(String::from(\"\"))".to_string()
                            }
                        } else {
                            "Layout::Title(String::from(\"\"))".to_string()
                        }
                    }
                    // Botones (callback como string para que el runtime lo maneje)
                    "boton" | "gui_boton" | "btn" | "button" => {
                        let texto = match argumentos.first() {
                            Some(Expresion::LiteralTexto(s)) => s.clone(),
                            _ => String::new(),
                        };
                        let callback = if argumentos.len() >= 2 {
                            // El callback puede ser Identificador("funcion") o Referencia { expr: Identificador("funcion") }
                            let arg = &argumentos[1];
                            match arg {
                                Expresion::Identificador { nombre: cb, .. } => cb.clone(),
                                Expresion::Referencia { expr, .. } => {
                                    if let Expresion::Identificador { nombre: cb, .. } = expr.as_ref() {
                                        cb.clone()
                                    } else {
                                        String::new()
                                    }
                                }
                                _ => String::new(),
                            }
                        } else {
                            String::new()
                        };
                        let escaped_texto = texto.replace('\\', "\\\\").replace('"', "\\\"");
                        format!(
                            "Layout::Button {{ texto: String::from(\"{}\"), callback: String::from(\"{}\") }}",
                            escaped_texto, callback
                        )
                    }
                    // Interactivos básicos
                    "entrada_texto" | "gui_entrada_texto" | "input" => {
                        let var_name = match argumentos.first() {
                            Some(Expresion::LiteralTexto(v)) => v.clone(),
                            _ => String::new(),
                        };
                        format!(
                            "Layout::TextInput {{ variable: String::from(\"{}\"), multiline: false, placeholder: String::new() }}",
                            var_name
                        )
                    }
                    "area_texto" | "textarea" => {
                        let var_name = match argumentos.first() {
                            Some(Expresion::LiteralTexto(v)) => v.clone(),
                            _ => String::new(),
                        };
                        format!(
                            "Layout::TextInput {{ variable: String::from(\"{}\"), multiline: true, placeholder: String::new() }}",
                            var_name
                        )
                    }
                    "barra_progreso" | "gui_barra_progreso" | "progress" => {
                        let var_name = match argumentos.first() {
                            Some(Expresion::LiteralTexto(v)) => v.clone(),
                            _ => String::new(),
                        };
                        format!("Layout::ProgressBar {{ variable: String::from(\"{}\") }}", var_name)
                    }
                    "deslizante" | "gui_deslizante" | "slider" => {
                        let var_name = match argumentos.first() {
                            Some(Expresion::LiteralTexto(v)) => v.clone(),
                            _ => String::new(),
                        };
                        let min = if argumentos.len() >= 2 {
                            Self::extraer_f64_valor(&argumentos[1])
                        } else { 0.0 };
                        let max = if argumentos.len() >= 3 {
                            Self::extraer_f64_valor(&argumentos[2])
                        } else { 100.0 };
                        format!(
                            "Layout::Slider {{ variable: String::from(\"{}\"), min: {}, max: {} }}",
                            var_name, min, max
                        )
                    }
                    "casilla" | "checkbox" | "gui_casilla" | "check" => {
                        let var_name = match argumentos.first() {
                            Some(Expresion::LiteralTexto(v)) => v.clone(),
                            _ => String::new(),
                        };
                        format!("Layout::Checkbox {{ variable: String::from(\"{}\") }}", var_name)
                    }
                    "texto_enriquecido" | "prose" => {
                        let texto = match argumentos.first() {
                            Some(Expresion::LiteralTexto(s)) => s.clone(),
                            _ => String::new(),
                        };
                        let escaped = texto.replace('\\', "\\\\").replace('"', "\\\"");
                        format!("Layout::Prose(String::from(\"{}\"))", escaped)
                    }
                    "cargando" | "spinner" => {
                        "Layout::Spinner".to_string()
                    }
                    "separador" | "divider" => {
                        "Layout::Separator".to_string()
                    }
                    "espacio" | "spacer" => {
                        if let Some(arg) = argumentos.first() {
                            let size = Self::extraer_f64_valor(arg);
                            format!("Layout::Spacer({})", size)
                        } else {
                            "Layout::Spacer(8.0)".to_string()
                        }
                    }
                    // Material You widgets
                    "tema_material" | "material_theme" => {
                        // ThemeProvider: child + seed color
                        let seed = if argumentos.len() >= 2 {
                            match &argumentos[1] {
                                Expresion::LiteralTexto(s) => s.clone(),
                                _ => "#6750A4".to_string(),
                            }
                        } else {
                            "#6750A4".to_string()
                        };
                        let child_code = if let Some(arg) = argumentos.first() {
                            self.generar_layout_code(arg)
                        } else {
                            "Layout::Spacer(0.0)".to_string()
                        };
                        format!("Layout::ThemeProvider {{ child: Box::new({}), theme: String::from(\"{}\") }}", child_code, seed)
                    }
                    "sombra" | "elevated_box" => {
                        let child_code = if let Some(arg) = argumentos.first() {
                            self.generar_layout_code(arg)
                        } else {
                            "Layout::Spacer(0.0)".to_string()
                        };
                        let nivel = if argumentos.len() >= 2 {
                            Self::extraer_entero_valor(&argumentos[1])
                        } else { 1u8 };
                        format!(
                            "Layout::ElevatedBox {{ child: Box::new({}), level: {}, shape_family: String::from(\"small\") }}",
                            child_code, nivel
                        )
                    }
                    "relleno" | "padding" => {
                        let child_code = if let Some(arg) = argumentos.first() {
                            self.generar_layout_code(arg)
                        } else {
                            "Layout::Spacer(0.0)".to_string()
                        };
                        let cantidad = if argumentos.len() >= 2 {
                            Self::extraer_f64_valor(&argumentos[1])
                        } else { 16.0 };
                        format!(
                            "Layout::Padding {{ child: Box::new({}), amount: {} }}",
                            child_code, cantidad
                        )
                    }
                    "expandido" | "expanded" => {
                        let child_code = if let Some(arg) = argumentos.first() {
                            self.generar_layout_code(arg)
                        } else {
                            "Layout::Spacer(0.0)".to_string()
                        };
                        format!("Layout::Expanded {{ child: Box::new({}) }}", child_code)
                    }
                    "centrado" | "centered" => {
                        let child_code = if let Some(arg) = argumentos.first() {
                            self.generar_layout_code(arg)
                        } else {
                            "Layout::Spacer(0.0)".to_string()
                        };
                        format!("Layout::Centered {{ child: Box::new({}) }}", child_code)
                    }
                    // ===== INDICADORES =====
                    "barra_progreso_linear" | "progress_bar_linear" | "lineal_progreso" => {
                        let _progress = argumentos.first().map_or(0.5, |a| Self::extraer_f64_valor(a));
                        format!("Layout::LinearProgress {{ variable: String::from(\"\"), indeterminado: false }}")
                    }
                    "barra_progreso_indeterminada" | "indeterminado" | "indeterminate" => {
                        "Layout::LinearProgress { variable: String::from(\"\"), indeterminado: true }".to_string()
                    }
                    "circulo_progreso" | "circular_progress" | "cargando_circular" => {
                        let size = argumentos.first().map_or(48.0, |a| Self::extraer_f64_valor(a));
                        format!("Layout::CircularProgress {{ variable: String::from(\"\"), size: {}, indeterminado: true }}", size)
                    }
                    "distintivo" | "badge" | "insignia" => {
                        let texto = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() {
                            self.esc_ast_string(s)
                        } else { String::new() };
                        format!("Layout::Badge {{ child: Box::new(Layout::Spacer(0.0)), valor: Some(String::from(\"{}\")), dot: false }}", texto)
                    }
                    "distintivo_punto" | "badge_dot" | "punto" => {
                        "Layout::Badge { child: Box::new(Layout::Spacer(0.0)), valor: None, dot: true }".to_string()
                    }
                    "esqueleto" | "skeleton" | "placeholder_carga" => {
                        let ancho = argumentos.first().map_or(200.0, |a| Self::extraer_f64_valor(a));
                        let alto = argumentos.get(1).map_or(20.0, |a| Self::extraer_f64_valor(a));
                        format!("Layout::Skeleton {{ ancho: {}, alto: {}, tipo: String::from(\"rect\") }}", ancho, alto)
                    }
                    "esqueleto_tarjeta" | "skeleton_card" | "tarjeta_placeholder" => {
                        "Layout::Skeleton { ancho: 300.0, alto: 200.0, tipo: String::from(\"card\") }".to_string()
                    }
                    "esqueleto_linea" | "skeleton_line" | "linea_placeholder" => {
                        "Layout::Skeleton { ancho: 200.0, alto: 16.0, tipo: String::from(\"line\") }".to_string()
                    }
                    "estado_vacio" | "empty_state" | "sin_datos" => {
                        let mensaje = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { "Sin datos".to_string() };
                        let icono = if argumentos.len() > 1 { if let Expresion::LiteralTexto(s) = &argumentos[1] { self.esc_ast_string(s) } else { "inbox".to_string() } } else { "inbox".to_string() };
                        format!("Layout::EmptyState {{ icono: String::from(\"{}\"), mensaje: String::from(\"{}\"), accion_texto: None, accion_cb: None }}", icono, mensaje)
                    }
                    "estado_error" | "error_state" | "error" => {
                        let mensaje = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { "Error".to_string() };
                        format!("Layout::ErrorState {{ mensaje: String::from(\"{}\"), on_retry: None }}", mensaje)
                    }

                    // ===== AVATARES =====
                    "avatar" | "avatar_text" | "avatar_texto" => {
                        let texto = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        format!("Layout::Avatar {{ texto: String::from(\"{}\"), variant: AvatarVariant::Text, tamano: 40.0 }}", texto)
                    }
                    "avatar_icono" | "avatar_icon" => {
                        let icono = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        format!("Layout::Avatar {{ texto: String::from(\"{}\"), variant: AvatarVariant::Icon, tamano: 40.0 }}", icono)
                    }
                    "avatar_imagen" | "avatar_image" | "avatar_img" => {
                        let url = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        format!("Layout::Avatar {{ texto: String::from(\"{}\"), variant: AvatarVariant::Image, tamano: 40.0 }}", url)
                    }
                    "grupo_avatar" | "avatar_group" | "grupo_avatares" => {
                        let hijos: Vec<String> = argumentos.iter().map(|a| {
                            match a {
                                Expresion::LiteralTexto(s) => format!("String::from(\"{}\")", self.esc_ast_string(s)),
                                _ => String::from("String::from(\"?\")"),
                            }
                        }).collect();
                        format!("Layout::AvatarGroup {{ avatares: vec![{}], max: 3 }}", hijos.join(", "))
                    }

                    // ===== NAVEGACION =====
                    "navegador" | "navigator" | "navegacion" => {
                        let tipo = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { s.as_str() } else { "bottom" };
                        let hijos: Vec<String> = if argumentos.len() > 1 {
                            argumentos[1..].iter().map(|a| self.generar_layout_code(a)).collect()
                        } else { vec![] };
                        format!("Layout::Navigator {{ screens: vec![{}], current_var: String::from(\"screen_active\"), history_var: String::from(\"nav_history\"), nav_type: NavigatorType::{}, anim: NavigatorAnim::Fade }}",
                            hijos.join(", "),
                            match tipo { "rail" => "Rail", "tabs" => "Tabs", "drawer" => "Drawer", _ => "BottomBar" })
                    }
                    "barra_navegacion" | "navigation_bar" | "nav_bar" | "barra_inferior" | "bottom_bar" => {
                        let items: Vec<String> = argumentos.iter().map(|a| self.generar_layout_code(a)).collect();
                        format!("Layout::NavigationBar {{ items: vec![{}], seleccion: 0, on_change: String::new() }}", items.join(", "))
                    }
                    "item_navegacion" | "nav_item" | "item_nav" => {
                        let icono = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        let texto = if argumentos.len() > 1 { if let Expresion::LiteralTexto(s) = &argumentos[1] { self.esc_ast_string(s) } else { String::new() } } else { String::new() };
                        format!("Layout::NavItem {{ icono: String::from(\"{}\"), texto: String::from(\"{}\"), selected: false, on_click: None }}", icono, texto)
                    }
                    "riel_navegacion" | "navigation_rail" | "nav_rail" | "barra_lateral" => {
                        let items: Vec<String> = argumentos.iter().map(|a| self.generar_layout_code(a)).collect();
                        format!("Layout::NavigationRail {{ items: vec![{}], seleccion: 0, on_change: String::new(), extended: false }}", items.join(", "))
                    }
                    "cajon_navegacion" | "navigation_drawer" | "drawer" | "cajon_lateral" | "sidebar" => {
                        let items: Vec<String> = argumentos.iter().map(|a| self.generar_layout_code(a)).collect();
                        format!("Layout::NavigationDrawer {{ items: vec![{}], seleccion: 0, on_change: String::new(), modal: false, visible: String::from(\"drawer_visible\") }}", items.join(", "))
                    }
                    "cajon_modal" | "modal_drawer" | "drawer_modal" => {
                        let items: Vec<String> = argumentos.iter().map(|a| self.generar_layout_code(a)).collect();
                        format!("Layout::NavigationDrawer {{ items: vec![{}], seleccion: 0, on_change: String::new(), modal: true, visible: String::from(\"drawer_visible\") }}", items.join(", "))
                    }
                    "barra_superior" | "top_app_bar" | "barra_titulo" | "app_bar" => {
                        let titulo = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        format!("Layout::TopAppBar {{ titulo: String::from(\"{}\"), acciones: vec![], menu_visible: false, variant: TopAppBarVariant::Small }}", titulo)
                    }
                    "barra_superior_mediana" | "medium_top_bar" => {
                        let titulo = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        format!("Layout::TopAppBar {{ titulo: String::from(\"{}\"), acciones: vec![], menu_visible: false, variant: TopAppBarVariant::Medium }}", titulo)
                    }
                    "barra_superior_grande" | "large_top_bar" => {
                        let titulo = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        format!("Layout::TopAppBar {{ titulo: String::from(\"{}\"), acciones: vec![], menu_visible: false, variant: TopAppBarVariant::Large }}", titulo)
                    }
                    "pestanas" | "tabs_widget" | "tabs" => {
                        let items: Vec<String> = argumentos.iter().map(|a| {
                            match a {
                                Expresion::LiteralTexto(s) => format!("String::from(\"{}\")", self.esc_ast_string(s)),
                                _ => String::from("String::new()"),
                            }
                        }).collect();
                        format!("Layout::Tabs {{ tabs: vec![{}], seleccion: 0, on_change: String::from(\"tab_change\"), scrollable: false }}", items.join(", "))
                    }
                    "barra_busqueda" | "search_bar" | "buscador" => {
                        let placeholder = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { "Buscar...".to_string() };
                        format!("Layout::SearchBar {{ placeholder: String::from(\"{}\"), on_search: String::new(), variable: String::from(\"busqueda\") }}", placeholder)
                    }

                    // ===== FEEDBACK =====
                    "dialogo_alerta" | "dialog_alert" | "alerta" => {
                        let titulo = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        let mensaje = if argumentos.len() > 1 { if let Expresion::LiteralTexto(s) = &argumentos[1] { self.esc_ast_string(s) } else { String::new() } } else { String::new() };
                        format!("Layout::DialogAlert {{ titulo: String::from(\"{}\"), mensaje: String::from(\"{}\"), confirmar_texto: String::from(\"OK\"), cancelar_texto: String::from(\"Cancelar\"), on_confirm: String::new(), on_cancel: String::new() }}", titulo, mensaje)
                    }
                    "dialogo_confirmacion" | "dialog_confirm" | "confirmacion" => {
                        let titulo = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        let mensaje = if argumentos.len() > 1 { if let Expresion::LiteralTexto(s) = &argumentos[1] { self.esc_ast_string(s) } else { String::new() } } else { String::new() };
                        format!("Layout::DialogAlert {{ titulo: String::from(\"{}\"), mensaje: String::from(\"{}\"), confirmar_texto: String::from(\"Confirmar\"), cancelar_texto: String::from(\"Cancelar\"), on_confirm: String::new(), on_cancel: String::new() }}", titulo, mensaje)
                    }
                    "hoja_inferior" | "bottom_sheet" | "sheet" | "panel_inferior" => {
                        let visible = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { "sheet_visible".to_string() };
                        let hijos: Vec<String> = if argumentos.len() > 1 {
                            argumentos[1..].iter().map(|a| self.generar_layout_code(a)).collect()
                        } else { vec![] };
                        format!("Layout::BottomSheet {{ child: Box::new(Layout::Column {{ children: vec![{}], gap: 8.0, alignment: String::from(\"start\") }}), variant: SheetVariant::Standard, visible: String::from(\"{}\"), on_dismiss: None }}",
                            hijos.join(", "), visible)
                    }
                    "snackbar" | "notification" | "notificacion" => {
                        let mensaje = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        format!("Layout::Snackbar {{ mensaje: String::from(\"{}\"), accion_texto: None, accion_callback: None, duracion: 4000.0, visible: String::from(\"snack_visible\") }}", mensaje)
                    }
                    "informacion" | "tooltip" | "info" => {
                        let texto = if let Some(Expresion::LiteralTexto(s)) = argumentos.first() { self.esc_ast_string(s) } else { String::new() };
                        let hijo = if argumentos.len() > 1 { self.generar_layout_code(&argumentos[1]) } else { "Layout::Spacer(0.0)".to_string() };
                        format!("Layout::Tooltip {{ child: Box::new({}), texto: String::from(\"{}\") }}", hijo, texto)
                    }

                    // Si no es una función de widget conocida, usar fallback
                    _ => {
                        // Intentar transpilar como expresión genérica
                        let s = self.transpilar_expresion(expr);
                        format!("Layout::Label {{ texto: String::from(\"(widget: {})\"), es_variable: false }}", s.replace('"', "'"))
                    }
                }
            }
            Expresion::LiteralTexto(s) => {
                let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                format!(
                    "Layout::Label {{ texto: String::from(\"{}\"), es_variable: false }}",
                    escaped
                )
            }
            Expresion::LiteralNumero(n) => {
                format!(
                    "Layout::Label {{ texto: String::from(\"{}\"), es_variable: false }}",
                    n
                )
            }
            Expresion::LiteralDecimal(f) => {
                format!(
                    "Layout::Label {{ texto: String::from(\"{}\"), es_variable: false }}",
                    f
                )
            }
            Expresion::Identificador { nombre: var, .. } => {
                format!(
                    "Layout::Label {{ texto: String::from(\"{}\"), es_variable: true }}",
                    var
                )
            }
            _ => {
                let s = self.transpilar_expresion(expr);
                format!(
                    "Layout::Label {{ texto: String::from(\"(expr: {})\"), es_variable: false }}",
                    s.replace('"', "'")
                )
            }
        }
    }

    /// Extrae un valor f64 de una expresión literal
    fn extraer_f64_valor(expr: &Expresion) -> f64 {
        match expr {
            Expresion::LiteralNumero(n) => *n as f64,
            Expresion::LiteralDecimal(f) => *f,
            Expresion::LiteralExacto(coeff, scale) => *coeff as f64 * 10_f64.powi(-(*scale as i32)),
            _ => 0.0,
        }
    }

    /// Extrae un valor entero (u8) de una expresión literal
    fn extraer_entero_valor(expr: &Expresion) -> u8 {
        match expr {
            Expresion::LiteralNumero(n) => *n as u8,
            Expresion::LiteralDecimal(f) => *f as u8,
            _ => 1,
        }
    }

    /// Detecta si el programa usa concurrencia (hilo, canal, enviar, recibir, unir, seleccionar)
    fn detectar_concurrencia(&self, declaraciones: &[Declaracion]) -> bool {
        for decl in declaraciones {
            match decl {
                Declaracion::Expresion(Expresion::Seleccionar { .. }) => return true,
                Declaracion::Expresion(Expresion::Hilo { .. }) => return true,
                Declaracion::Expresion(Expresion::CanalNuevo) => return true,
                Declaracion::Variable {
                    valor: Some(val), ..
                } => {
                    if self.expr_tiene_concurrencia(val) {
                        return true;
                    }
                }
                Declaracion::AsignacionMultiple { valor, .. } => {
                    if self.expr_tiene_concurrencia(valor) {
                        return true;
                    }
                }
                Declaracion::LlamadaFuncion { nombre, .. } => {
                    if nombre.contains("enviar")
                        || nombre.contains("recibir")
                        || nombre.contains("unir")
                    {
                        return true;
                    }
                }
                Declaracion::Si {
                    bloque_verdadero,
                    bloque_falso,
                    ..
                } => {
                    if self.detectar_concurrencia(bloque_verdadero) {
                        return true;
                    }
                    if let Some(bf) = bloque_falso {
                        if self.detectar_concurrencia(bf) {
                            return true;
                        }
                    }
                }
                Declaracion::Mientras { bloque, .. } => {
                    if self.detectar_concurrencia(bloque) {
                        return true;
                    }
                }
                Declaracion::Cuando { cuerpo, .. } => {
                    if self.detectar_concurrencia(cuerpo) {
                        return true;
                    }
                }
                Declaracion::Para { bloque, .. } => {
                    if self.detectar_concurrencia(bloque) {
                        return true;
                    }
                }
                Declaracion::Repetir { bloque, .. } => {
                    if self.detectar_concurrencia(bloque) {
                        return true;
                    }
                }
                Declaracion::Funcion { cuerpo, .. } => {
                    if self.detectar_concurrencia(cuerpo) {
                        return true;
                    }
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
            if let Declaracion::Clase {
                nombre,
                campos,
                metodos,
                invariantes,
                ..
            } = decl
            {
                let mut tipos_campos: HashMap<String, String> = HashMap::new();

                // Escanear constructores para inferir tipos de campos
                for metodo in metodos {
                    if metodo.nombre == "nuevo" {
                        for decl_cuerpo in &metodo.cuerpo {
                            if let Declaracion::AsignacionMiembro {
                                objeto,
                                miembro,
                                valor,
                                ..
                            } = decl_cuerpo
                            {
                                // este.campo = expr → inferir tipo
                                if let Expresion::Identificador {
                                    nombre: ref nombre_self,
                                    ..
                                } = objeto.as_ref()
                                {
                                    if nombre_self == "self" {
                                        let tipo_inferido =
                                            self.inferir_tipo_expr(valor, &metodo.parametros);
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
                        let tipo = tipos_campos
                            .get(&c.nombre)
                            .cloned()
                            .unwrap_or_else(|| "String".to_string());
                        (c.nombre.clone(), tipo)
                    })
                    .collect();

                let metodos_info: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();

                self.clases.insert(
                    nombre.clone(),
                    ClaseInfo {
                        campos: campos_info,
                        metodos: metodos_info,
                        tipos_campos,
                        invariantes: invariantes.clone(),
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
            Expresion::LiteralExacto(_, _) => "f64".to_string(),
            Expresion::LiteralNulo => "()".to_string(),
            Expresion::Identificador { nombre, .. } => {
                // Buscar si el identificador es un parámetro con tipo conocido
                for p in params {
                    if p.nombre == *nombre {
                        if let Some(ref tipo) = p.tipo {
                            return match tipo {
                                Tipo::Entero => "i64".to_string(),
                                Tipo::Decimal => "f64".to_string(),
                                Tipo::Texto => {
                                    if p.prestado {
                                        "&str".to_string()
                                    } else {
                                        "String".to_string()
                                    }
                                }
                                Tipo::Booleano => "bool".to_string(),
                                Tipo::Nulo => "()".to_string(),
                                Tipo::Exacto => "f64".to_string(),
                                Tipo::Clase(n) => n.clone(),
                                Tipo::Arreglo(t) => format!("Vec<{}>", self.tipo_a_rust(t)),
                                Tipo::Funcion(_, _) => "fn".to_string(),
                                Tipo::Resultado(_, _) => "Result<...>".to_string(),
                                Tipo::Opcion(_) => "Option<...>".to_string(),
                                Tipo::RasgoObjeto(n) => format!("Box<dyn {}>", n),
                                Tipo::Parametro(n) => n.clone(),
                            };
                        }
                    }
                }
                // Si es un literal conocido (verdadero/falso)
                match nombre.as_str() {
                    "verdadero" | "falso" => "bool".to_string(),
                    _ => "String".to_string(), // default
                }
            }
            Expresion::Binaria { izquierda, .. } => {
                // Para expresiones como a + b, inferir del lado izquierdo
                self.inferir_tipo_expr(izquierda, params)
            }
            Expresion::Unaria { expr: e, .. } => self.inferir_tipo_expr(e, params),
            _ => "String".to_string(),
        }
    }

    fn generar_clases(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Clase {
                nombre,
                parametros_tipo,
                campos,
                metodos,
                atributos,
                ..
            } = decl
            {
                // Generar parámetros genéricos si existen
                let gen_params_str = if parametros_tipo.is_empty() {
                    String::new()
                } else {
                    let gen_names: Vec<String> =
                        parametros_tipo.iter().map(|p| p.nombre.clone()).collect();
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
                    let gen_names: Vec<String> =
                        parametros_tipo.iter().map(|p| p.nombre.clone()).collect();
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

            self.emit_line(&format!("fn nuevo({}) -> Self {{", params.join(", ")));
            self.indent();

            // Generar inicialización de campos basada en el cuerpo del constructor
            // Busca patrones: este.campo = param → Self { campo: param }
            let campos_inicializar: Vec<(String, String)> = metodo
                .cuerpo
                .iter()
                .filter_map(|decl| {
                    if let Declaracion::AsignacionMiembro {
                        objeto,
                        miembro,
                        valor,
                        ..
                    } = decl
                    {
                        if let Expresion::Identificador {
                            nombre: ref nombre_self,
                            ..
                        } = objeto.as_ref()
                        {
                            if nombre_self == "self" {
                                // El valor puede ser un identificador (param) o una expresión
                                let val_str = match valor.as_ref() {
                                    Expresion::Identificador { nombre: id, .. } => id.clone(),
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
                self.emit_line(&format!("{} {{", nombre_clase));
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
            let tiene_self = metodo
                .parametros
                .first()
                .map_or(false, |p| p.nombre == "self");
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

            // ─── Invariantes de clase (al inicio del método) ─────────
            let invariantes = self
                .clases
                .get(nombre_clase)
                .map(|info| info.invariantes.clone())
                .unwrap_or_default();
            if !invariantes.is_empty() {
                self.emit_line("// ═══ Invariantes ═══");
                for c in &invariantes {
                    let cond = self.transpilar_expresion(&c.condicion);
                    let msg = c
                        .mensaje
                        .clone()
                        .unwrap_or_else(|| "Invariante falló".to_string());
                    self.emit_line(&format!("debug_assert!({}, \"{}\");", cond, msg));
                }
            }

            // ─── Precondiciones del método ───────────────────────────
            if !metodo.precondiciones.is_empty() {
                self.emit_line("// ═══ Precondiciones ═══");
                for c in &metodo.precondiciones {
                    let cond = self.transpilar_expresion(&c.condicion);
                    let msg = c
                        .mensaje
                        .clone()
                        .unwrap_or_else(|| "Precondición falló".to_string());
                    self.emit_line(&format!("debug_assert!({}, \"{}\");", cond, msg));
                }
            }

            // ─── Snapshots para anterior() en postcondiciones ─────────
            let mut vars_anterior: Vec<String> = Vec::new();
            for c in &metodo.postcondiciones {
                self.recolectar_vars_anterior(&c.condicion, &mut vars_anterior);
            }
            for var in &vars_anterior {
                self.emit_line(&format!("let _anterior_{} = {}.clone();", var, var));
            }

            // ─── Guardar estado de postcondiciones ────────────────────
            let prev_postcondiciones = std::mem::take(&mut self.postcondiciones_actuales);
            let prev_modo = self.modo_postcondiciones;
            if !metodo.postcondiciones.is_empty() {
                self.postcondiciones_actuales = metodo.postcondiciones.clone();
                self.modo_postcondiciones = true;
            }
            let prev_tiene_resultado = self.tiene_resultado_var;
            self.tiene_resultado_var = metodo.parametros.iter().any(|p| p.nombre == "resultado")
                || existe_variable_resultado(&metodo.cuerpo);

            for decl in &metodo.cuerpo {
                self.transpilar_declaracion(decl);
            }

            self.tiene_resultado_var = prev_tiene_resultado;

            // ─── Invariantes al final del método ──────────────────────
            if !invariantes.is_empty() {
                self.emit_line("// ═══ Invariantes ═══");
                for c in &invariantes {
                    let cond = self.transpilar_expresion(&c.condicion);
                    let msg = c
                        .mensaje
                        .clone()
                        .unwrap_or_else(|| "Invariante falló".to_string());
                    self.emit_line(&format!("debug_assert!({}, \"{}\");", cond, msg));
                }
            }

            // Restaurar estado de postcondiciones
            self.postcondiciones_actuales = prev_postcondiciones;
            self.modo_postcondiciones = prev_modo;

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
                    if param.prestado {
                        "&str".to_string()
                    } else {
                        "String".to_string()
                    }
                }
                Tipo::Booleano => "bool".to_string(),
                Tipo::Nulo => "()".to_string(),
                Tipo::Exacto => "f64".to_string(),
                Tipo::Clase(nombre) => nombre.clone(),
                Tipo::Arreglo(_) => "Vec<...>".to_string(),
                Tipo::Funcion(_, _) => "fn".to_string(),
                Tipo::Resultado(_, _) => "Result<...>".to_string(),
                Tipo::Opcion(_) => "Option<...>".to_string(),
                Tipo::RasgoObjeto(nombre) => format!("Box<dyn {}>", nombre),
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
            if let Declaracion::Retornar {
                valor: Some(val), ..
            } = decl
            {
                // Inferir tipo del valor retornado
                let tipo = match val {
                    Expresion::LiteralNumero(_) => Some(Tipo::Entero),
                    Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
                    Expresion::LiteralTexto(_) => Some(Tipo::Texto),
                    Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
                    Expresion::LiteralNulo => Some(Tipo::Nulo),
                    Expresion::Identificador { nombre, .. } => {
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
            if let Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } = decl
            {
                if let Some(t) = self.inferir_tipo_retorno(bloque_verdadero) {
                    return Some(t);
                }
                if let Some(bf) = bloque_falso {
                    if let Some(t) = self.inferir_tipo_retorno(bf) {
                        return Some(t);
                    }
                }
            }
            if let Declaracion::Mientras { bloque, .. } = decl {
                if let Some(t) = self.inferir_tipo_retorno(bloque) {
                    return Some(t);
                }
            }
            if let Declaracion::Cuando { cuerpo, .. } = decl {
                if let Some(t) = self.inferir_tipo_retorno(cuerpo) {
                    return Some(t);
                }
            }
        }
        None
    }

    /// Escanea el cuerpo de una función para inferir tipos de parámetros no anotados.
    /// Retorna un mapa: nombre_del_parametro -> tipo_rust_inferido
    fn inferir_tipos_desde_cuerpo(
        &self,
        cuerpo: &[Declaracion],
        params: &[Parametro],
    ) -> std::collections::HashMap<String, String> {
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

    fn analizar_declaracion_para_tipos(
        &self,
        decl: &Declaracion,
        tipos: &mut std::collections::HashMap<String, String>,
    ) {
        match decl {
            Declaracion::Variable {
                valor: Some(val), ..
            } => {
                self.analizar_expr_para_tipos(val, tipos);
            }
            Declaracion::Asignacion { valor, .. } => {
                self.analizar_expr_para_tipos(valor, tipos);
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                self.analizar_expr_para_tipos(condicion, tipos);
                for d in bloque_verdadero {
                    self.analizar_declaracion_para_tipos(d, tipos);
                }
                if let Some(bf) = bloque_falso {
                    for d in bf {
                        self.analizar_declaracion_para_tipos(d, tipos);
                    }
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                self.analizar_expr_para_tipos(condicion, tipos);
                for d in bloque {
                    self.analizar_declaracion_para_tipos(d, tipos);
                }
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                self.analizar_expr_para_tipos(condicion, tipos);
                for d in cuerpo {
                    self.analizar_declaracion_para_tipos(d, tipos);
                }
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(init) = inicializacion {
                    self.analizar_declaracion_para_tipos(init, tipos);
                }
                if let Some(cond) = condicion {
                    self.analizar_expr_para_tipos(cond, tipos);
                }
                if let Some(inc) = incremento {
                    self.analizar_declaracion_para_tipos(inc, tipos);
                }
                for d in bloque {
                    self.analizar_declaracion_para_tipos(d, tipos);
                }
            }
            Declaracion::Repetir { cantidad, bloque } => {
                self.analizar_expr_para_tipos(cantidad, tipos);
                for d in bloque {
                    self.analizar_declaracion_para_tipos(d, tipos);
                }
            }
            Declaracion::Retornar {
                valor: Some(val), ..
            } => {
                self.analizar_expr_para_tipos(val, tipos);
            }
            Declaracion::Expresion(expr) => {
                self.analizar_expr_para_tipos(expr, tipos);
            }
            Declaracion::LlamadaFuncion { argumentos, .. } => {
                for arg in argumentos {
                    self.analizar_expr_para_tipos(arg, tipos);
                }
            }
            _ => {}
        }
    }

    fn analizar_expr_para_tipos(
        &self,
        expr: &Expresion,
        tipos: &mut std::collections::HashMap<String, String>,
    ) {
        match expr {
            Expresion::Identificador { nombre, .. } => {
                // Si el parámetro se usa con literales numéricos, es Entero
                if !tipos.contains_key(nombre) {
                    tipos.insert(nombre.clone(), "i64".to_string());
                }
            }
            Expresion::Binaria {
                izquierda,
                derecha,
                operador,
            } => {
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
                for arg in argumentos {
                    self.analizar_expr_para_tipos(arg, tipos);
                }
            }
            Expresion::Arreglo(elementos) => {
                for e in elementos {
                    self.analizar_expr_para_tipos(e, tipos);
                }
            }
            Expresion::Grupo(expr) => self.analizar_expr_para_tipos(expr, tipos),
            _ => {}
        }
    }

    fn asignar_tipo_si_parametro(
        &self,
        expr: &Expresion,
        tipos: &mut std::collections::HashMap<String, String>,
        tipo: &str,
    ) {
        if let Expresion::Identificador { nombre, .. } = expr {
            tipos.insert(nombre.clone(), tipo.to_string());
        }
    }

    // ============================================================
    // Transpilación de declaraciones
    // ============================================================

    fn transpilar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                tipo,
                valor,
                ..
            } => {
                // Analizar si la variable es realmente mutable (se reasigna en el código)
                let realmente_mutable =
                    *mutable && es_variable_mutable(nombre, &self.declaraciones_globales, 0);
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

            Declaracion::Asignacion { nombre, valor, .. } => {
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{} = {};", nombre, val_str));
            }

            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                ..
            } => {
                let obj_str = self.transpilar_expresion(objeto);
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{}.{} = {};", obj_str, miembro, val_str));
            }

            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                ..
            } => {
                let idx_str = self.transpilar_expresion(indice);
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{}[{}] = {};", nombre, idx_str, val_str));
            }

            Declaracion::Funcion {
                nombre,
                parametros_tipo,
                parametros,
                tipo_retorno,
                cuerpo,
                atributos,
                doc,
                precondiciones,
                postcondiciones,
                ..
            } => {
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
                let inferred_ret = tipo_retorno
                    .clone()
                    .or_else(|| self.inferir_tipo_retorno(cuerpo));

                // Generar parámetros de tipo genérico <T, U> si existen
                let gen_params_str = if parametros_tipo.is_empty() {
                    String::new()
                } else {
                    let gen_names: Vec<String> =
                        parametros_tipo.iter().map(|p| p.nombre.clone()).collect();
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
                            tipos_inferidos
                                .get(&p.nombre)
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

                self.emit_line(&format!(
                    "fn {}{}({}){} {{",
                    nombre,
                    gen_params_str,
                    params.join(", "),
                    ret_str
                ));
                self.indent();

                // ─── Precondiciones ───────────────────────────────────────
                if !precondiciones.is_empty() {
                    self.emit_line("// ═══ Precondiciones ═══");
                    for c in precondiciones {
                        let cond = self.transpilar_expresion(&c.condicion);
                        let msg = c
                            .mensaje
                            .clone()
                            .unwrap_or_else(|| "Precondición falló".to_string());
                        self.emit_line(&format!("debug_assert!({}, \"{}\");", cond, msg));
                    }
                }

                // ─── Snapshots para anterior() en postcondiciones ─────────
                let mut vars_anterior: Vec<String> = Vec::new();
                for c in postcondiciones {
                    self.recolectar_vars_anterior(&c.condicion, &mut vars_anterior);
                }
                for var in &vars_anterior {
                    self.emit_line(&format!("let _anterior_{} = {}.clone();", var, var));
                }

                // ─── Guardar estado de postcondiciones ────────────────────
                let prev_postcondiciones = std::mem::take(&mut self.postcondiciones_actuales);
                let prev_modo = self.modo_postcondiciones;
                if !postcondiciones.is_empty() {
                    self.postcondiciones_actuales = postcondiciones.clone();
                    self.modo_postcondiciones = true;
                }

                // Guardar contexto actual y poner el cuerpo de la función como ámbito de búsqueda
                // para que las variables locales también sean analizadas por es_variable_mutable
                let declaraciones_previas = std::mem::take(&mut self.declaraciones_globales);
                self.declaraciones_globales = cuerpo.clone();

                let prev_tiene_resultado = self.tiene_resultado_var;
                self.tiene_resultado_var = parametros.iter().any(|p| p.nombre == "resultado")
                    || existe_variable_resultado(cuerpo);

                for d in cuerpo {
                    self.transpilar_declaracion(d);
                }

                self.tiene_resultado_var = prev_tiene_resultado;

                // Restaurar contexto anterior
                self.declaraciones_globales = declaraciones_previas;
                self.postcondiciones_actuales = prev_postcondiciones;
                self.modo_postcondiciones = prev_modo;

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::Rasgo { nombre, metodos } => {
                self.emit_line(&format!("trait {} {{", nombre));
                self.indent();
                for metodo in metodos {
                    let params: Vec<String> = metodo
                        .parametros
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
                            s.push_str(&self.inferir_tipo_parametro(p));
                            s
                        })
                        .collect();
                    let ret = match &metodo.tipo_retorno {
                        Some(t) => format!(" -> {}", self.tipo_a_rust(t)),
                        None => String::new(),
                    };
                    self.emit_line(&format!(
                        "fn {}({}){};",
                        metodo.nombre,
                        params.join(", "),
                        ret
                    ));
                }
                self.dedent();
                self.emit_line("}");
                self.emit_line("");
            }

            Declaracion::Implementacion {
                rasgo_nombre,
                clase_nombre,
                metodos,
            } => {
                self.emit_line(&format!("impl {} for {} {{", rasgo_nombre, clase_nombre));
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

            Declaracion::Enum {
                nombre,
                variantes,
                atributos,
            } => {
                // Generar #[derive(...)] desde @derive(Mostrar, Igual, ...)
                self.emit_derive_from_atributos(atributos);
                let vars: Vec<String> = variantes
                    .iter()
                    .map(|v| {
                        let tipos: Vec<String> =
                            v.tipos.iter().map(|t| self.tipo_a_rust(t)).collect();
                        if tipos.is_empty() {
                            v.nombre.clone()
                        } else {
                            format!("{}({})", v.nombre, tipos.join(", "))
                        }
                    })
                    .collect();
                self.emit_line(&format!("enum {} {{", nombre));
                self.indent();
                for v in &vars {
                    self.emit_line(&format!("{},", v));
                }
                self.dedent();
                self.emit_line("}");
                self.emit_line("");
            }

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
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

            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                let cond_str = self.transpilar_expresion(condicion);
                self.emit_line(&format!("if {} {{", cond_str));
                self.indent();

                for d in cuerpo {
                    self.transpilar_declaracion(d);
                }

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                // Forja: para (i = 0; i < N; i = i + 1) { ... }
                // Rust: for i in 0..N { ... }
                //
                // Si es el patrón estándar (i = 0; i < N; i = i + 1), optimizamos a range
                // De lo contrario, usamos while

                if let Some(cond) = condicion {
                    if let Expresion::Binaria {
                        izquierda,
                        operador: Operador::Menor,
                        derecha,
                    } = cond.as_ref()
                    {
                        if let Expresion::Identificador {
                            nombre: ref var_name,
                            ..
                        } = izquierda.as_ref()
                        {
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

            Declaracion::Retornar { valor, .. } => {
                if self.modo_postcondiciones {
                    let postconds = self.postcondiciones_actuales.clone();
                    if let Some(val) = valor {
                        let val_str = self.transpilar_expresion(val);
                        self.emit_line(&format!("let _return_value = {};", val_str));
                        // Emitir postcondiciones
                        for c in &postconds {
                            let cond = self.transpilar_expresion(&c.condicion);
                            let msg = c
                                .mensaje
                                .clone()
                                .unwrap_or_else(|| "Postcondición falló".to_string());
                            self.emit_line(&format!("debug_assert!({}, \"{}\");", cond, msg));
                        }
                        self.emit_line("return _return_value;");
                    } else {
                        // Sin valor de retorno: emitir postcondiciones y retornar
                        for c in &postconds {
                            let cond = self.transpilar_expresion(&c.condicion);
                            let msg = c
                                .mensaje
                                .clone()
                                .unwrap_or_else(|| "Postcondición falló".to_string());
                            self.emit_line(&format!("debug_assert!({}, \"{}\");", cond, msg));
                        }
                        self.emit_line("return;");
                    }
                } else {
                    if let Some(val) = valor {
                        let val_str = self.transpilar_expresion(val);
                        self.emit_line(&format!("return {};", val_str));
                    } else {
                        self.emit_line("return;");
                    }
                }
            }

            Declaracion::Romper => {
                self.emit_line("break;");
            }
            Declaracion::Continuar => {
                self.emit_line("continue;");
            }

            Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor,
            } => {
                let valor_str = self.transpilar_expresion(valor);
                if *mutable {
                    self.emit_line(&format!(
                        "let mut ({}) = {};",
                        variables.join(", "),
                        valor_str
                    ));
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
        // Verificar profundidad para prevenir stack overflow
        self.profundidad_expresion += 1;
        if self.profundidad_expresion > MAX_AST_PROFUNDIDAD {
            self.profundidad_expresion -= 1;
            self.errors.push(ErrorForja::new(
                crate::error::ErrorTipo::ErrorSintactico,
                0,
                0,
                "Expresión demasiado grande: se excedió la profundidad máxima en transpilación.",
                "Simplificá la expresión dividiéndola en partes más pequeñas.",
            ));
            return String::from("/* expresión demasiado grande */ 0");
        }
        let result = self.transpilar_expresion_inner(expr);
        self.profundidad_expresion -= 1;
        result
    }

    /// Implementación interna de transpilar_expresion (sin control de profundidad).
    fn transpilar_expresion_inner(&mut self, expr: &Expresion) -> String {
        match expr {
            Expresion::LiteralNumero(n) => n.to_string(),
            Expresion::LiteralDecimal(d) => d.to_string(),
            Expresion::LiteralExacto(coeff, scale) => {
                format!("({} as f64) * 10f64.powi({})", coeff, -(*scale as i32))
            }
            Expresion::LiteralTexto(s) => {
                let mut escaped = String::new();
                for c in s.chars() {
                    match c {
                        '\\' => escaped.push_str("\\\\"),
                        '"' => escaped.push_str("\\\""),
                        '\n' => escaped.push_str("\\n"),
                        '\r' => escaped.push_str("\\r"),
                        '\t' => escaped.push_str("\\t"),
                        '\0' => escaped.push_str("\\0"),
                        _ if c.is_control() => {
                            escaped.push_str(&format!("\\u{{{:x}}}", c as u32));
                        }
                        _ => escaped.push(c),
                    }
                }
                format!("String::from(\"{}\")", escaped)
            }
            Expresion::LiteralBooleano(b) => b.to_string(),
            Expresion::LiteralNulo => "()".to_string(),

            Expresion::Identificador { nombre, .. } => {
                if nombre == "self" {
                    "self".to_string()
                } else if nombre == "verdadero" {
                    "true".to_string()
                } else if nombre == "falso" {
                    "false".to_string()
                } else {
                    nombre.clone()
                }
            }

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                // Detectar concatenación de strings:
                // Si ALGÚN operando es un literal de texto, usamos format!("{}{}", ...)
                // en vez de String + String (que no compila si el otro operando es String).
                if let Operador::Suma = operador {
                    let es_texto_izq = matches!(izquierda.as_ref(), Expresion::LiteralTexto(_));
                    let es_texto_der = matches!(derecha.as_ref(), Expresion::LiteralTexto(_));

                    if es_texto_izq || es_texto_der {
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

            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                let cond = self.transpilar_expresion(condicion);
                let v = self.transpilar_expresion(si_verdadero);
                let f = self.transpilar_expresion(si_falso);
                format!("(if {} {{ {} }} else {{ {} }})", cond, v, f)
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
                let entries: Vec<String> = pares
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "({}, {})",
                            self.transpilar_expresion(k),
                            self.transpilar_expresion(v)
                        )
                    })
                    .collect();
                format!("std::collections::HashMap::from([{}])", entries.join(", "))
            }

            Expresion::Coincidir { expr, brazos } => {
                let expr_str = self.transpilar_expresion(expr);
                let mut result = format!("match {} {{\n", expr_str);
                for brazo in brazos {
                    let patron_str = self.patron_a_rust(&brazo.patron);
                    result.push_str(&format!("    {} => {{\n", patron_str));
                    // Save current output state and redirect to capture body
                    let saved_output = std::mem::take(&mut self.output);
                    let saved_indent = self.indent_level;
                    self.indent_level += 2;
                    for decl in &brazo.cuerpo {
                        self.transpilar_declaracion(decl);
                    }
                    result.push_str(&self.output);
                    // Restore state
                    self.output = saved_output;
                    self.indent_level = saved_indent;
                    result.push_str("    },\n");
                }
                result.push_str("}");
                result
            }

            Expresion::Closure { parametros, cuerpo } => {
                let params: Vec<String> = parametros
                    .iter()
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
            Expresion::CanalNuevo => "mpsc::channel()".to_string(),
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
                        arms.push(format!(
                            "    recv({}) -> {} => {{\n        {}\n    }},",
                            canal_str, var, cuerpo_str
                        ));
                    } else if brazo.timeout_ms > 0 {
                        // tiempo ms { ... } -> default con Duration
                        arms.push(format!("    default(std::time::Duration::from_millis({})) => {{\n        {}\n    }},", brazo.timeout_ms, cuerpo_str));
                    } else {
                        // otro { ... } -> default (sin timeout)
                        arms.push(format!(
                            "    default => {{\n        {}\n    }},",
                            cuerpo_str
                        ));
                    }
                }
                format!("crossbeam::select!{{\n{}\n}}", arms.join("\n"))
            }
            Expresion::Asignacion { variable, valor } => {
                let val_str = self.transpilar_expresion(valor);
                format!("{{ let __tmp = {}; {} = __tmp; __tmp }}", val_str, variable)
            }
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                let obj_str = self.transpilar_expresion(objeto);
                let val_str = self.transpilar_expresion(valor);
                format!(
                    "{{ let __tmp = {}; {}.{} = __tmp; __tmp }}",
                    val_str, obj_str, campo
                )
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
            Expresion::Algo(expr) => {
                format!("Some({})", self.transpilar_expresion(expr))
            }
            Expresion::Resultado => {
                if self.tiene_resultado_var {
                    "resultado".to_string()
                } else {
                    "_return_value".to_string()
                }
            }
            Expresion::Anterior(expr) => {
                if let Expresion::Identificador { nombre: var, .. } = expr.as_ref() {
                    format!("_anterior_{}", var)
                } else {
                    // para anterior(este.campo) usar el valor actual como fallback
                    self.transpilar_expresion(expr)
                }
            }
        }
    }

    fn patron_a_rust(&mut self, patron: &Patron) -> String {
        match patron {
            Patron::Variable(n) => n.clone(),
            Patron::Constructor(n, ps) => {
                let sub: Vec<String> = ps.iter().map(|p| self.patron_a_rust(p)).collect();
                format!("{}({})", n, sub.join(", "))
            }
            Patron::Ignorar => "_".to_string(),
            Patron::Literal(lit) => self.transpilar_expresion(lit),
        }
    }

    fn tipo_a_rust(&self, tipo: &Tipo) -> String {
        match tipo {
            Tipo::Entero => "i64".to_string(),
            Tipo::Decimal => "f64".to_string(),
            Tipo::Texto => "String".to_string(),
            Tipo::Booleano => "bool".to_string(),
            Tipo::Nulo => "()".to_string(),
            // Se usa f64 con pérdida de precisión para Exacto/BigDecimal (no hay dependencia externa)
            Tipo::Exacto => "f64".to_string(),
            Tipo::Clase(nombre) => nombre.clone(),
            Tipo::Arreglo(t) => format!("Vec<{}>", self.tipo_a_rust(t)),
            Tipo::Funcion(params, ret) => {
                let p: Vec<String> = params.iter().map(|t| self.tipo_a_rust(t)).collect();
                format!("fn({}) -> {}", p.join(", "), self.tipo_a_rust(ret))
            }
            Tipo::Resultado(ok, err) => format!(
                "Result<{}, {}>",
                self.tipo_a_rust(ok),
                self.tipo_a_rust(err)
            ),
            Tipo::Opcion(inner) => format!("Option<{}>", self.tipo_a_rust(inner)),
            Tipo::RasgoObjeto(nombre) => format!("Box<dyn {}>", nombre),
            Tipo::Parametro(nombre) => nombre.clone(),
        }
    }

    /// Recorre recursivamente una expresión buscando `Anterior(Identificador)` para
    /// generar las variables snapshot necesarias en Design by Contract.
    fn recolectar_vars_anterior(&self, expr: &Expresion, vars: &mut Vec<String>) {
        match expr {
            Expresion::Anterior(inner) => {
                if let Expresion::Identificador { nombre: var, .. } = inner.as_ref() {
                    if !vars.contains(var) {
                        vars.push(var.clone());
                    }
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                self.recolectar_vars_anterior(izquierda, vars);
                self.recolectar_vars_anterior(derecha, vars);
            }
            Expresion::Unaria { expr: e, .. } => {
                self.recolectar_vars_anterior(e, vars);
            }
            Expresion::LlamadaFuncion { argumentos, .. } => {
                for arg in argumentos {
                    self.recolectar_vars_anterior(arg, vars);
                }
            }
            Expresion::AccesoMiembro { objeto, .. } => {
                self.recolectar_vars_anterior(objeto, vars);
            }
            Expresion::Grupo(inner) => {
                self.recolectar_vars_anterior(inner, vars);
            }
            Expresion::Index { objeto, indice } => {
                self.recolectar_vars_anterior(objeto, vars);
                self.recolectar_vars_anterior(indice, vars);
            }
            Expresion::Arreglo(elementos) => {
                for e in elementos {
                    self.recolectar_vars_anterior(e, vars);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares {
                    self.recolectar_vars_anterior(k, vars);
                    self.recolectar_vars_anterior(v, vars);
                }
            }
            Expresion::Referencia { expr: e, .. } => {
                self.recolectar_vars_anterior(e, vars);
            }
            Expresion::Ok(inner) => self.recolectar_vars_anterior(inner, vars),
            Expresion::Error(inner) => self.recolectar_vars_anterior(inner, vars),
            Expresion::Algo(inner) => self.recolectar_vars_anterior(inner, vars),
            Expresion::Try(inner) => self.recolectar_vars_anterior(inner, vars),
            _ => {}
        }
    }

    // ============================================================
    // Helpers de salida
    // ============================================================

    /// Emite #[derive(...)] a partir de atributos @derive(Mostrar, Igual, ...)
    fn emit_derive_from_atributos(&mut self, atributos: &[Atributo]) {
        if let Some(derive_attr) = atributos.iter().find(|a| a.nombre == "derive") {
            let rasgos: Vec<&String> = derive_attr
                .argumentos
                .iter()
                .filter(|a| {
                    matches!(
                        a.as_str(),
                        "Mostrar" | "Igual" | "Debug" | "Clone" | "Copiar"
                    )
                })
                .collect();
            if !rasgos.is_empty() {
                let rust_traits: Vec<String> = rasgos
                    .iter()
                    .map(|t| match t.as_str() {
                        "Mostrar" => "Display".to_string(),
                        "Igual" => "PartialEq".to_string(),
                        "Debug" => "Debug".to_string(),
                        "Clone" => "Clone".to_string(),
                        "Copiar" => "Copy".to_string(),
                        _ => t.to_string(),
                    })
                    .collect();
                self.emit_line(&format!("#[derive({})]", rust_traits.join(", ")));
            }
        }
    }

    // ============================================================
    // Generacion de AST como codigo Rust (para delegar al runtime)
    // ============================================================

    /// Genera codigo Rust que reconstruye el `Programa` completo como datos estaticos.
    fn generar_ast_programa(&self, declaraciones: &[Declaracion]) -> String {
        let mut parts = Vec::new();
        for decl in declaraciones {
            parts.push(self.generar_ast_declaracion(decl));
        }
        parts.join(",\n")
    }

    /// Genera codigo Rust que reconstruye una `Declaracion`.
    fn generar_ast_declaracion(&self, decl: &Declaracion) -> String {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                tipo,
                valor,
                linea,
                columna,
            } => {
                let tipo_str = match tipo {
                    Some(t) => format!("Some({})", self.tipo_a_ast(t)),
                    None => "None".to_string(),
                };
                let valor_str = match valor {
                    Some(v) => format!("Some({})", self.generar_ast_expresion(v)),
                    None => "None".to_string(),
                };
                format!("Declaracion::Variable {{ mutable: {}, nombre: String::from(\"{}\"), tipo: {}, valor: {}, linea: {}, columna: {} }}",
                    mutable, self.esc_ast_string(nombre), tipo_str, valor_str, linea, columna)
            }
            Declaracion::Asignacion {
                nombre,
                valor,
                linea,
                columna,
            } => {
                format!("Declaracion::Asignacion {{ nombre: String::from(\"{}\"), valor: Box::new({}), linea: {}, columna: {} }}",
                    self.esc_ast_string(nombre), self.generar_ast_expresion(valor), linea, columna)
            }
            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                linea,
                columna,
            } => {
                format!("Declaracion::AsignacionMiembro {{ objeto: Box::new({}), miembro: String::from(\"{}\"), valor: Box::new({}), linea: {}, columna: {} }}",
                    self.generar_ast_expresion(objeto), self.esc_ast_string(miembro), self.generar_ast_expresion(valor), linea, columna)
            }
            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                linea,
                columna,
            } => {
                format!("Declaracion::AsignacionIndex {{ nombre: String::from(\"{}\"), indice: Box::new({}), valor: Box::new({}), linea: {}, columna: {} }}",
                    self.esc_ast_string(nombre), self.generar_ast_expresion(indice), self.generar_ast_expresion(valor), linea, columna)
            }
            Declaracion::Funcion {
                nombre,
                parametros_tipo,
                parametros,
                tipo_retorno,
                cuerpo,
                externa,
                enlace_nombre,
                atributos: _,
                doc,
                precondiciones: _,
                postcondiciones: _,
            } => {
                let params_tipo_str: Vec<String> = parametros_tipo
                    .iter()
                    .map(|p| {
                        format!(
                            "ParametroTipo {{ nombre: String::from(\"{}\"), rasgos: vec![] }}",
                            self.esc_ast_string(&p.nombre)
                        )
                    })
                    .collect();
                let params_str: Vec<String> = parametros.iter().map(|p| {
                    let tipo_str = match &p.tipo {
                        Some(t) => format!("Some({})", self.tipo_a_ast(t)),
                        None => "None".to_string(),
                    };
                    format!("Parametro {{ nombre: String::from(\"{}\"), tipo: {}, prestado: {}, mutable: {} }}",
                        self.esc_ast_string(&p.nombre), tipo_str, p.prestado, p.mutable)
                }).collect();
                let ret_str = match tipo_retorno {
                    Some(t) => format!("Some({})", self.tipo_a_ast(t)),
                    None => "None".to_string(),
                };
                let cuerpo_str: Vec<String> = cuerpo
                    .iter()
                    .map(|d| self.generar_ast_declaracion(d))
                    .collect();
                let enlace_str = match enlace_nombre {
                    Some(s) => format!("Some(String::from(\"{}\"))", self.esc_ast_string(s)),
                    None => "None".to_string(),
                };
                let docs_str = match doc {
                    Some(s) => format!("Some(String::from(\"{}\"))", self.esc_ast_string(s)),
                    None => "None".to_string(),
                };
                format!("Declaracion::Funcion {{ nombre: String::from(\"{}\"), parametros_tipo: vec![{}], parametros: vec![{}], tipo_retorno: {}, cuerpo: vec![{}], externa: {}, enlace_nombre: {}, atributos: vec![], doc: {}, precondiciones: vec![], postcondiciones: vec![] }}",
                    self.esc_ast_string(nombre), params_tipo_str.join(", "), params_str.join(", "), ret_str, cuerpo_str.join(", "), externa, enlace_str, docs_str)
            }
            Declaracion::Expresion(expr) => {
                format!(
                    "Declaracion::Expresion({})",
                    self.generar_ast_expresion(expr)
                )
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                let args_str: Vec<String> = argumentos
                    .iter()
                    .map(|a| self.generar_ast_expresion(a))
                    .collect();
                format!("Declaracion::LlamadaFuncion {{ nombre: String::from(\"{}\"), argumentos: vec![{}] }}",
                    self.esc_ast_string(nombre), args_str.join(", "))
            }
            Declaracion::Retornar { valor } => match valor {
                Some(v) => format!(
                    "Declaracion::Retornar {{ valor: Some({}) }}",
                    self.generar_ast_expresion(v)
                ),
                None => "Declaracion::Retornar { valor: None }".to_string(),
            },
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let bv: Vec<String> = bloque_verdadero
                    .iter()
                    .map(|d| self.generar_ast_declaracion(d))
                    .collect();
                let bf_str = match bloque_falso {
                    Some(bf) => {
                        let items: Vec<String> =
                            bf.iter().map(|d| self.generar_ast_declaracion(d)).collect();
                        format!("Some(vec![{}])", items.join(", "))
                    }
                    None => "None".to_string(),
                };
                format!("Declaracion::Si {{ condicion: Box::new({}), bloque_verdadero: vec![{}], bloque_falso: {} }}",
                    self.generar_ast_expresion(condicion), bv.join(", "), bf_str)
            }
            Declaracion::Mientras { condicion, bloque } => {
                let blk: Vec<String> = bloque
                    .iter()
                    .map(|d| self.generar_ast_declaracion(d))
                    .collect();
                format!(
                    "Declaracion::Mientras {{ condicion: Box::new({}), bloque: vec![{}] }}",
                    self.generar_ast_expresion(condicion),
                    blk.join(", ")
                )
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                let init_str = match inicializacion {
                    Some(d) => format!("Some(Box::new({}))", self.generar_ast_declaracion(d)),
                    None => "None".to_string(),
                };
                let cond_str = match condicion {
                    Some(e) => format!("Some(Box::new({}))", self.generar_ast_expresion(e)),
                    None => "None".to_string(),
                };
                let inc_str = match incremento {
                    Some(d) => format!("Some(Box::new({}))", self.generar_ast_declaracion(d)),
                    None => "None".to_string(),
                };
                let blk: Vec<String> = bloque
                    .iter()
                    .map(|d| self.generar_ast_declaracion(d))
                    .collect();
                format!("Declaracion::Para {{ inicializacion: {}, condicion: {}, incremento: {}, bloque: vec![{}] }}",
                    init_str, cond_str, inc_str, blk.join(", "))
            }
            Declaracion::Repetir { cantidad, bloque } => {
                let blk: Vec<String> = bloque
                    .iter()
                    .map(|d| self.generar_ast_declaracion(d))
                    .collect();
                format!(
                    "Declaracion::Repetir {{ cantidad: Box::new({}), bloque: vec![{}] }}",
                    self.generar_ast_expresion(cantidad),
                    blk.join(", ")
                )
            }
            Declaracion::Cuando {
                condicion,
                cuerpo,
                linea,
                columna,
            } => {
                let blk: Vec<String> = cuerpo
                    .iter()
                    .map(|d| self.generar_ast_declaracion(d))
                    .collect();
                format!("Declaracion::Cuando {{ condicion: Box::new({}), cuerpo: vec![{}], linea: {}, columna: {} }}",
                    self.generar_ast_expresion(condicion), blk.join(", "), linea, columna)
            }
            Declaracion::Importar(ruta) => {
                format!(
                    "Declaracion::Importar(String::from(\"{}\"))",
                    self.esc_ast_string(ruta)
                )
            }
            Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor,
            } => {
                let vars_str: Vec<String> = variables
                    .iter()
                    .map(|v| format!("String::from(\"{}\")", self.esc_ast_string(v)))
                    .collect();
                format!("Declaracion::AsignacionMultiple {{ variables: vec![{}], mutable: {}, valor: Box::new({}) }}",
                    vars_str.join(", "), mutable, self.generar_ast_expresion(valor))
            }
            Declaracion::AccesoMiembro { objeto, miembro } => {
                format!("Declaracion::AccesoMiembro {{ objeto: Box::new({}), miembro: String::from(\"{}\") }}",
                    self.generar_ast_expresion(objeto), self.esc_ast_string(miembro))
            }
            _ => {
                format!("// Declaracion omitida (generacion directa)")
            }
        }
    }

    /// Genera codigo Rust que reconstruye una `Expresion`.
    fn generar_ast_expresion(&self, expr: &Expresion) -> String {
        match expr {
            Expresion::LiteralNumero(n) => format!("Expresion::LiteralNumero({})", n),
            Expresion::LiteralDecimal(f) => format!("Expresion::LiteralDecimal({})", f),
            Expresion::LiteralTexto(s) => format!(
                "Expresion::LiteralTexto(String::from(\"{}\"))",
                self.esc_ast_string(s)
            ),
            Expresion::LiteralBooleano(b) => format!("Expresion::LiteralBooleano({})", b),
            Expresion::LiteralNulo => "Expresion::LiteralNulo".to_string(),
            Expresion::LiteralExacto(coeff, scale) => {
                format!("Expresion::LiteralExacto({}, {})", coeff, scale)
            }
            Expresion::Identificador { nombre, .. } => format!(
                "Expresion::Identificador(String::from(\"{}\"))",
                self.esc_ast_string(nombre)
            ),
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                format!("Expresion::Binaria {{ izquierda: Box::new({}), operador: {}, derecha: Box::new({}) }}",
                    self.generar_ast_expresion(izquierda), self.operador_a_ast(operador), self.generar_ast_expresion(derecha))
            }
            Expresion::Unaria { operador, expr: e } => {
                format!(
                    "Expresion::Unaria {{ operador: {}, expr: Box::new({}) }}",
                    match operador {
                        OperadorUnario::Negar => "OperadorUnario::Negar",
                        OperadorUnario::No => "OperadorUnario::No",
                    },
                    self.generar_ast_expresion(e)
                )
            }
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                format!(
                    "Expresion::Ternario {{ condicion: Box::new({}), si_verdadero: Box::new({}), si_falso: Box::new({}) }}",
                    self.generar_ast_expresion(condicion),
                    self.generar_ast_expresion(si_verdadero),
                    self.generar_ast_expresion(si_falso)
                )
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos
                    .iter()
                    .map(|a| self.generar_ast_expresion(a))
                    .collect();
                format!("Expresion::LlamadaFuncion {{ nombre: String::from(\"{}\"), argumentos: vec![{}] }}",
                    self.esc_ast_string(nombre), args.join(", "))
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                format!("Expresion::AccesoMiembro {{ objeto: Box::new({}), miembro: String::from(\"{}\") }}",
                    self.generar_ast_expresion(objeto), self.esc_ast_string(miembro))
            }
            Expresion::Instanciacion { clase, argumentos } => {
                let args: Vec<String> = argumentos
                    .iter()
                    .map(|a| self.generar_ast_expresion(a))
                    .collect();
                format!("Expresion::Instanciacion {{ clase: String::from(\"{}\"), argumentos: vec![{}] }}",
                    self.esc_ast_string(clase), args.join(", "))
            }
            Expresion::Referencia { expr: e, mutable } => {
                format!(
                    "Expresion::Referencia {{ expr: Box::new({}), mutable: {} }}",
                    self.generar_ast_expresion(e),
                    mutable
                )
            }
            Expresion::Arreglo(elementos) => {
                let elems: Vec<String> = elementos
                    .iter()
                    .map(|e| self.generar_ast_expresion(e))
                    .collect();
                format!("Expresion::Arreglo(vec![{}])", elems.join(", "))
            }
            Expresion::Mapa(pares) => {
                let entries: Vec<String> = pares
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "({}, {})",
                            self.generar_ast_expresion(k),
                            self.generar_ast_expresion(v)
                        )
                    })
                    .collect();
                format!("Expresion::Mapa(vec![{}])", entries.join(", "))
            }
            Expresion::Index { objeto, indice } => {
                format!(
                    "Expresion::Index {{ objeto: Box::new({}), indice: Box::new({}) }}",
                    self.generar_ast_expresion(objeto),
                    self.generar_ast_expresion(indice)
                )
            }
            Expresion::Grupo(inner) => format!(
                "Expresion::Grupo(Box::new({}))",
                self.generar_ast_expresion(inner)
            ),
            Expresion::Hilo { cuerpo } => {
                let blk: Vec<String> = cuerpo
                    .iter()
                    .map(|d| self.generar_ast_declaracion(d))
                    .collect();
                format!("Expresion::Hilo {{ cuerpo: vec![{}] }}", blk.join(", "))
            }
            Expresion::CanalNuevo => "Expresion::CanalNuevo".to_string(),
            Expresion::Try(inner) => format!(
                "Expresion::Try(Box::new({}))",
                self.generar_ast_expresion(inner)
            ),
            Expresion::Asignacion { variable, valor } => {
                format!("Expresion::Asignacion {{ variable: String::from(\"{}\"), valor: Box::new({}) }}",
                    self.esc_ast_string(variable), self.generar_ast_expresion(valor))
            }
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                format!("Expresion::AsignacionCampo {{ objeto: Box::new({}), campo: String::from(\"{}\"), valor: Box::new({}) }}",
                    self.generar_ast_expresion(objeto), self.esc_ast_string(campo), self.generar_ast_expresion(valor))
            }
            Expresion::ArraySet { array, valor } => {
                format!(
                    "Expresion::ArraySet {{ array: Box::new({}), valor: Box::new({}) }}",
                    self.generar_ast_expresion(array),
                    self.generar_ast_expresion(valor)
                )
            }
            Expresion::Ok(inner) => format!(
                "Expresion::Ok(Box::new({}))",
                self.generar_ast_expresion(inner)
            ),
            Expresion::Error(inner) => format!(
                "Expresion::Error(Box::new({}))",
                self.generar_ast_expresion(inner)
            ),
            Expresion::Algo(inner) => format!(
                "Expresion::Algo(Box::new({}))",
                self.generar_ast_expresion(inner)
            ),
            Expresion::Resultado => "Expresion::Resultado".to_string(),
            Expresion::Anterior(inner) => format!(
                "Expresion::Anterior(Box::new({}))",
                self.generar_ast_expresion(inner)
            ),
            Expresion::Coincidir { expr: e, brazos } => {
                let brazos_str: Vec<String> = brazos
                    .iter()
                    .map(|b| {
                        let patron_str = self.patron_a_ast(&b.patron);
                        let cuerpo_str: Vec<String> = b
                            .cuerpo
                            .iter()
                            .map(|d| self.generar_ast_declaracion(d))
                            .collect();
                        format!(
                            "BrazoMatch {{ patron: {}, cuerpo: vec![{}] }}",
                            patron_str,
                            cuerpo_str.join(", ")
                        )
                    })
                    .collect();
                format!(
                    "Expresion::Coincidir {{ expr: Box::new({}), brazos: vec![{}] }}",
                    self.generar_ast_expresion(e),
                    brazos_str.join(", ")
                )
            }
            Expresion::Closure { parametros, cuerpo } => {
                let params_str: Vec<String> = parametros.iter().map(|p| {
                    let tipo_str = match &p.tipo {
                        Some(t) => format!("Some({})", self.tipo_a_ast(t)),
                        None => "None".to_string(),
                    };
                    format!("Parametro {{ nombre: String::from(\"{}\"), tipo: {}, prestado: {}, mutable: {} }}",
                        self.esc_ast_string(&p.nombre), tipo_str, p.prestado, p.mutable)
                }).collect();
                let cuerpo_str: Vec<String> = cuerpo
                    .iter()
                    .map(|d| self.generar_ast_declaracion(d))
                    .collect();
                format!(
                    "Expresion::Closure {{ parametros: vec![{}], cuerpo: vec![{}] }}",
                    params_str.join(", "),
                    cuerpo_str.join(", ")
                )
            }
            Expresion::Seleccionar { brazos } => {
                let brazos_str: Vec<String> = brazos.iter().map(|b| {
                    let recp_str = match &b.recepcion {
                        Some((var, expr)) => format!("Some((String::from(\"{}\"), {}))", var, self.generar_ast_expresion(expr)),
                        None => "None".to_string(),
                    };
                    let cuerpo_str: Vec<String> = b.cuerpo.iter().map(|d| self.generar_ast_declaracion(d)).collect();
                    format!("BrazoSeleccionar {{ recepcion: {}, cuerpo: vec![{}], timeout_ms: {} }}",
                        recp_str, cuerpo_str.join(", "), b.timeout_ms)
                }).collect();
                format!(
                    "Expresion::Seleccionar {{ brazos: vec![{}] }}",
                    brazos_str.join(", ")
                )
            }
        }
    }

    fn operador_a_ast(&self, op: &Operador) -> String {
        match op {
            Operador::Suma => "Operador::Suma",
            Operador::Resta => "Operador::Resta",
            Operador::Multiplicacion => "Operador::Multiplicacion",
            Operador::Division => "Operador::Division",
            Operador::Modulo => "Operador::Modulo",
            Operador::Mayor => "Operador::Mayor",
            Operador::Menor => "Operador::Menor",
            Operador::MayorIgual => "Operador::MayorIgual",
            Operador::MenorIgual => "Operador::MenorIgual",
            Operador::IgualIgual => "Operador::IgualIgual",
            Operador::Diferente => "Operador::Diferente",
            Operador::Y => "Operador::Y",
            Operador::O => "Operador::O",
        }
        .to_string()
    }

    fn tipo_a_ast(&self, tipo: &Tipo) -> String {
        match tipo {
            Tipo::Entero => "Tipo::Entero".to_string(),
            Tipo::Decimal => "Tipo::Decimal".to_string(),
            Tipo::Texto => "Tipo::Texto".to_string(),
            Tipo::Booleano => "Tipo::Booleano".to_string(),
            Tipo::Nulo => "Tipo::Nulo".to_string(),
            Tipo::Exacto => "Tipo::Exacto".to_string(),
            Tipo::Clase(n) => format!("Tipo::Clase(String::from(\"{}\"))", self.esc_ast_string(n)),
            Tipo::Arreglo(t) => format!("Tipo::Arreglo(Box::new({}))", self.tipo_a_ast(t)),
            Tipo::Funcion(params, ret) => {
                let p: Vec<String> = params.iter().map(|t| self.tipo_a_ast(t)).collect();
                format!(
                    "Tipo::Funcion(vec![{}], Box::new({}))",
                    p.join(", "),
                    self.tipo_a_ast(ret)
                )
            }
            Tipo::Resultado(ok, err) => format!(
                "Tipo::Resultado(Box::new({}), Box::new({}))",
                self.tipo_a_ast(ok),
                self.tipo_a_ast(err)
            ),
            Tipo::Opcion(inner) => format!("Tipo::Opcion(Box::new({}))", self.tipo_a_ast(inner)),
            Tipo::RasgoObjeto(n) => format!(
                "Tipo::RasgoObjeto(String::from(\"{}\"))",
                self.esc_ast_string(n)
            ),
            Tipo::Parametro(n) => format!(
                "Tipo::Parametro(String::from(\"{}\"))",
                self.esc_ast_string(n)
            ),
        }
    }

    fn patron_a_ast(&self, patron: &Patron) -> String {
        match patron {
            Patron::Variable(n) => format!(
                "Patron::Variable(String::from(\"{}\"))",
                self.esc_ast_string(n)
            ),
            Patron::Constructor(n, ps) => {
                let sub: Vec<String> = ps.iter().map(|p| self.patron_a_ast(p)).collect();
                format!(
                    "Patron::Constructor(String::from(\"{}\"), vec![{}])",
                    self.esc_ast_string(n),
                    sub.join(", ")
                )
            }
            Patron::Ignorar => "Patron::Ignorar".to_string(),
            Patron::Literal(lit) => format!("Patron::Literal({})", self.generar_ast_expresion(lit)),
        }
    }

    fn esc_ast_string(&self, s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
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
    fn test_gui_genera_programa_estatico() {
        let source = "importar \"gui\"\nfuncion main() {\n    escribir(\"hola\")\n}";
        let result = transpilar_source(source).unwrap();
        // Ahora genera el programa como datos estaticos en vez de inline Xilem
        assert!(result.contains("static PROGRAMA: Programa"));
        assert!(result.contains("use forja::ast::*;"));
        // main() usa build_and_run del runtime
        assert!(result.contains("build_and_run"));
        assert!(result.contains("PROGRAMA"));
    }

    #[test]
    fn test_gui_genera_ast_widgets() {
        let source = "importar \"gui\"\nfuncion main() {\n    variable usuario = \"admin\"\n    variable contrasena = \"secreta\"\n    escribir(usuario)\n    escribir(contrasena)\n}";
        let result = transpilar_source(source).unwrap();
        // Debe generar el AST con declaraciones de variables y llamadas a funcion
        assert!(result.contains("Declaracion::Variable"));
        assert!(result.contains("Declaracion::LlamadaFuncion"));
        // Debe contener los valores literales
        assert!(result.contains("admin"));
        assert!(result.contains("secreta"));
    }

    #[test]
    fn test_gui_boton_con_callback() {
        let source = "importar \"gui\"\nfuncion al_saludar() { escribir(\"Hola!\") }\nfuncion main() {\n    boton(\"Saludar\", &al_saludar)\n}";
        let result = transpilar_source(source).unwrap();
        // Ahora genera el AST en vez de Layout::Button directamente
        assert!(result.contains("LlamadaFuncion"));
        assert!(result.contains("boton"));
        assert!(result.contains("al_saludar"));
        assert!(result.contains("Saludar"));
    }

    #[test]
    fn test_gui_boton_sin_callback() {
        let source = "importar \"gui\"\nfuncion main() {\n    boton(\"Cerrar\")\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("Cerrar"));
        assert!(result.contains("LlamadaFuncion"));
    }

    #[test]
    fn test_gui_columna_basica() {
        let source = "importar \"gui\"\nfuncion main() {\n    columna(escribir(\"Arriba\"), boton(\"Click\"))\n}";
        let result = transpilar_source(source).unwrap();
        // Ahora genera el AST, no Layout::Column directamente
        assert!(result.contains("columna"));
        assert!(result.contains("Arriba"));
        assert!(result.contains("Click"));
    }

    #[test]
    fn test_gui_entrada_texto() {
        let source = "importar \"gui\"\nfuncion main() {\n    entrada_texto(\"Nombre\")\n}";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("entrada_texto"));
        assert!(result.contains("Nombre"));
    }

    #[test]
    fn test_gui_multiple_widgets() {
        let source = "importar \"gui\"\nfuncion main() {\n    escribir(\"Config\")\n    entrada_texto(\"Nombre\")\n    boton(\"Click\")\n}";
        let result = transpilar_source(source).unwrap();
        // Verificar que aparecen todos los nombres de widgets en el AST
        assert!(result.contains("Config"));
        assert!(result.contains("Nombre"));
        assert!(result.contains("Click"));
    }
}
