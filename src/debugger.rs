// Forja Debugger — Modo paso a paso con breakpoints
// Envuelve ForjaFast y controla la ejecución opcode por opcode,
// permitiendo pausa en breakpoints, step over/into/out y
// extracción de variables del stack/frames.

use crate::bytecode::Opcode;
use crate::vm_fast::{get_small_int_fast, ErrFast, ForjaFast, FrmFast, ValorFast};
use std::collections::HashMap;

/// Estado del debugger
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugState {
    /// Corriendo libremente hasta breakpoint
    Running,
    /// Pausado en una línea (esperando comando del usuario)
    Paused,
    /// Step Over — ejecutar hasta la próxima línea en el mismo frame
    StepOver,
    /// Step Into — ejecutar hasta la próxima línea (puede entrar a funciones)
    StepInto,
    /// Step Out — ejecutar hasta retornar de la función actual
    StepOut,
}

/// Información de un frame del call stack para DAP
#[derive(Debug, Clone)]
pub struct FrameDebug {
    pub id: usize,
    pub name: String,        // nombre de la función
    pub line: usize,         // línea actual
    pub ip: usize,           // instruction pointer actual
    pub vars: Vec<VarDebug>, // variables locales
}

/// Información de una variable para DAP
#[derive(Debug, Clone)]
pub struct VarDebug {
    pub name: String,
    pub value: String,
    pub tipo: String,
    pub referencia: Option<usize>, // para variables compuestas (array/objeto)
}

/// Wrapper de depuración sobre ForjaFast
pub struct Debugger {
    /// La VM subyacente
    pub vm: ForjaFast,
    /// Estado actual del debugger
    pub state: DebugState,
    /// Breakpoints activos: mapean número de línea → condición opcional.
    /// `None` = breakpoint incondicional; `Some(expr)` = pausa solo si la
    /// expresión evalúa a verdadero.
    pub breakpoints: HashMap<usize, Option<String>>,
    /// Cache de mapeo línea → IP (construido al cargar bytecode)
    pub line_to_ip: HashMap<usize, Vec<usize>>,
    /// Cache inverso IP → línea (para lookup rápido)
    pub ip_to_line: HashMap<usize, usize>,
    /// Línea actual de ejecución
    pub current_line: usize,
    /// Línea donde estábamos cuando se inició StepOut
    pub step_out_target_frame: usize,
    /// Cantidad de frames al iniciar StepOver
    pub step_over_frame_count: usize,
    /// Últimas líneas encontradas (para detección de cambio de línea)
    pub last_report_line: usize,
    /// ID de próxima petición (para callbacks)
    pub next_id: u32,
    /// Nombres reales de variables locales por frame (índice → nombre).
    /// Se puebla desde el AST en `cargar_nombres_desde_ast`.
    pub nombres_locales: Vec<Vec<String>>,
    /// Nombres reales de variables globales (índice → nombre).
    pub nombres_globales: Vec<String>,
    /// Nombres de funciones por IP de entrada (ip → nombre).
    pub nombres_funciones: HashMap<usize, String>,
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

impl Debugger {
    pub fn new() -> Self {
        Debugger {
            vm: ForjaFast::new(),
            state: DebugState::Running,
            breakpoints: HashMap::new(),
            line_to_ip: HashMap::new(),
            ip_to_line: HashMap::new(),
            current_line: 0,
            step_out_target_frame: 0,
            step_over_frame_count: 0,
            last_report_line: 0,
            next_id: 0,
            nombres_locales: Vec::new(),
            nombres_globales: Vec::new(),
            nombres_funciones: HashMap::new(),
        }
    }

    /// Cargar bytecode en la VM y construir el mapeo línea → IP
    pub fn cargar_bytecode(&mut self, bc: Vec<Opcode>) {
        // Construir mapeos línea ↔ IP
        self.line_to_ip.clear();
        self.ip_to_line.clear();
        for (ip, op) in bc.iter().enumerate() {
            if let Opcode::FijarLinea(line) = op {
                self.line_to_ip.entry(*line).or_default().push(ip);
                self.ip_to_line.insert(ip, *line);
            }
        }
        self.current_line = 0;
        self.last_report_line = 0;
        self.state = DebugState::Running;
        self.vm.cargar_bytecode(bc);
    }

    /// Activar/desactivar breakpoint en línea (incondicional)
    pub fn toggle_breakpoint(&mut self, line: usize, active: bool) {
        if active {
            self.breakpoints.insert(line, None);
        } else {
            self.breakpoints.remove(&line);
        }
    }

    /// Poner breakpoint incondicional en línea
    pub fn set_breakpoint(&mut self, line: usize) {
        self.breakpoints.insert(line, None);
    }

    /// Poner breakpoint condicional en línea.
    /// `condicion` vacía equivale a un breakpoint incondicional.
    pub fn set_breakpoint_condicional(&mut self, line: usize, condicion: Option<String>) {
        let cond = condicion.filter(|c| !c.trim().is_empty());
        self.breakpoints.insert(line, cond);
    }

    /// Quitar breakpoint de línea
    pub fn clear_breakpoint(&mut self, line: usize) {
        self.breakpoints.remove(&line);
    }

    /// Verificar si estamos en un breakpoint (incondicional o con condición
    /// que evalúa a verdadero en el estado actual de la VM).
    pub fn en_breakpoint(&mut self) -> bool {
        let cond = self.breakpoints.get(&self.current_line).cloned();
        match cond {
            None => false,
            Some(None) => true,
            Some(Some(cond)) => self.evaluar_condicion(&cond),
        }
    }

    /// Evaluar una condición de breakpoint contra el estado actual de la VM.
    /// Si la expresión no puede evaluarse, se considera falsa (no pausa).
    pub fn evaluar_condicion(&mut self, cond: &str) -> bool {
        match self.evaluar(cond) {
            Ok(var) => var.value == "verdadero",
            Err(_) => false,
        }
    }

    /// Continuar ejecución libre (hasta breakpoint o fin)
    pub fn continuar(&mut self) -> Result<DebugEvent, ErrFast> {
        self.state = DebugState::Running;
        self.ejecutar_hasta_evento()
    }

    /// Step Over: ejecutar hasta la próxima línea en el mismo frame depth
    pub fn step_over(&mut self) -> Result<DebugEvent, ErrFast> {
        self.state = DebugState::StepOver;
        self.step_over_frame_count = self.vm.frame_count;
        self.ejecutar_hasta_evento()
    }

    /// Step Into: ejecutar hasta la próxima línea (puede entrar en funciones)
    pub fn step_into(&mut self) -> Result<DebugEvent, ErrFast> {
        self.state = DebugState::StepInto;
        self.ejecutar_hasta_evento()
    }

    /// Step Out: ejecutar hasta que la función actual retorne
    pub fn step_out(&mut self) -> Result<DebugEvent, ErrFast> {
        self.state = DebugState::StepOut;
        self.step_out_target_frame = self.vm.frame_count;
        self.ejecutar_hasta_evento()
    }

    /// Ejecutar la VM hasta que ocurra un evento (breakpoint, step complete, fin, error)
    pub fn ejecutar_hasta_evento(&mut self) -> Result<DebugEvent, ErrFast> {
        loop {
            if self.vm.ip >= self.vm.bytecode.len() {
                self.state = DebugState::Paused;
                return Ok(DebugEvent::Terminado);
            }
            if self.vm.ejecutadas > self.vm.max_inst {
                self.state = DebugState::Paused;
                return Err(ErrFast::Limite);
            }
            self.vm.ejecutadas += 1;

            let op = self.vm.bytecode[self.vm.ip].clone();

            // Procesar SetLine: actualizar línea actual
            if let Opcode::FijarLinea(line) = &op {
                self.current_line = *line;
                self.vm.ip += 1;

                // Verificar si debemos pausar
                match self.state {
                    DebugState::Running => {
                        if self.en_breakpoint() {
                            self.state = DebugState::Paused;
                            return Ok(DebugEvent::Breakpoint { line: *line });
                        }
                    }
                    DebugState::StepInto => {
                        if *line != self.last_report_line {
                            self.state = DebugState::Paused;
                            self.last_report_line = *line;
                            return Ok(DebugEvent::StepCompletado { line: *line });
                        }
                    }
                    DebugState::StepOver => {
                        if *line != self.last_report_line
                            && self.vm.frame_count <= self.step_over_frame_count
                        {
                            self.state = DebugState::Paused;
                            self.last_report_line = *line;
                            return Ok(DebugEvent::StepCompletado { line: *line });
                        }
                    }
                    DebugState::StepOut => {
                        if *line != self.last_report_line
                            && self.vm.frame_count < self.step_out_target_frame
                        {
                            self.state = DebugState::Paused;
                            self.last_report_line = *line;
                            return Ok(DebugEvent::StepCompletado { line: *line });
                        }
                    }
                    DebugState::Paused => {
                        return Ok(DebugEvent::Pausado { line: *line });
                    }
                }
                continue;
            }

            // Ejecutar el opcode
            self.ejecutar_un_paso(&op)?;

            // Detectar salida de funciones para StepOut
            if self.state == DebugState::StepOut && self.vm.frame_count < self.step_out_target_frame
            {
                self.state = DebugState::Paused;
                return Ok(DebugEvent::StepCompletado {
                    line: self.current_line,
                });
            }

            // Si el state es Running, check breakpoints después de SetLine
            if self.state == DebugState::Running && self.current_line > 0 {
                if let Some(&line) = self.ip_to_line.get(&self.vm.ip) {
                    if line != self.last_report_line && self.breakpoint_activo_en(line) {
                        self.current_line = line;
                        self.state = DebugState::Paused;
                        self.last_report_line = line;
                        return Ok(DebugEvent::Breakpoint { line });
                    }
                }
            }
        }
    }

    /// Ejecuta un solo opcode (equivalente a una iteración del loop de ForjaFast::ejecutar)
    fn ejecutar_un_paso(&mut self, op: &Opcode) -> Result<(), ErrFast> {
        match op {
            Opcode::PushEntero(n) => {
                self.vm.push_valor(get_small_int_fast(*n));
                self.vm.ip += 1;
            }
            Opcode::PushDecimal(d) => {
                self.vm.push_valor(ValorFast::flotante(*d));
                self.vm.ip += 1;
            }
            Opcode::PushTexto(s) => {
                let idx = self.vm.alloc_str(s.clone());
                self.vm.push_valor(ValorFast::texto(idx));
                self.vm.ip += 1;
            }
            Opcode::PushBooleano(b) => {
                self.vm.push_valor(ValorFast::booleano(*b));
                self.vm.ip += 1;
            }
            Opcode::PushNulo => {
                self.vm.push_valor(ValorFast::nulo());
                self.vm.ip += 1;
            }
            Opcode::Descartar => {
                self.vm.pop_valor()?;
                self.vm.ip += 1;
            }
            Opcode::Duplicar => {
                let v = *self.vm.peek_valor(0);
                self.vm.push_valor(v);
                self.vm.ip += 1;
            }
            Opcode::CargarIndice(idx) => {
                self.vm.push_valor(self.vm.load_local(*idx));
                self.vm.ip += 1;
            }
            Opcode::GuardarIndice(idx) => {
                let val = self.vm.pop_valor()?;
                self.vm.store_local(*idx, val);
                self.vm.ip += 1;
            }
            Opcode::DeclararIndice(idx, _mutable) => {
                // Ensure local slot exists
                self.vm.store_local(*idx, self.vm.load_local(*idx));
                self.vm.ip += 1;
            }
            Opcode::Sumar => {
                self.vm.ip += 1;
                ejecutar_add(self)?;
            }
            Opcode::Restar => {
                self.vm.ip += 1;
                ejecutar_sub(self)?;
            }
            Opcode::Multiplicar => {
                self.vm.ip += 1;
                ejecutar_mul(self)?;
            }
            Opcode::Dividir => {
                self.vm.ip += 1;
                ejecutar_div(self)?;
            }
            Opcode::Salto(target) => {
                self.vm.ip = *target;
            }
            Opcode::SaltoSiFalso(target) => {
                let cond = self.vm.pop_valor()?;
                if !cond.es_verdadero() {
                    self.vm.ip = *target;
                } else {
                    self.vm.ip += 1;
                }
            }
            Opcode::Etiqueta(_) => {
                self.vm.ip += 1;
            }
            Opcode::Detener => {
                self.vm.ip = self.vm.bytecode.len();
            }
            Opcode::Llamar(nombre, nargs) => {
                self.vm.ip += 1;
                let n = *nargs;
                ejecutar_call_debug(self, nombre, n)?;
            }
            Opcode::Retornar => {
                ejecutar_return_debug(self)?;
            }
            Opcode::DefinirFuncion(_, _) => {
                self.vm.ip += 1;
            }
            Opcode::Imprimir => {
                let val = self.vm.pop_valor()?;
                let s = self.vm.mostrar_valor(&val);
                self.vm.output.lock().unwrap().push(s);
                self.vm.ip += 1;
            }
            Opcode::Igual => {
                self.vm.ip += 1;
                ejecutar_igual(self)?;
            }
            Opcode::Menor => {
                self.vm.ip += 1;
                ejecutar_menor(self)?;
            }
            Opcode::Mayor => {
                self.vm.ip += 1;
                ejecutar_mayor(self)?;
            }
            Opcode::Y => {
                self.vm.ip += 1;
                ejecutar_y(self)?;
            }
            Opcode::O => {
                self.vm.ip += 1;
                ejecutar_o(self)?;
            }
            Opcode::No => {
                let v = self.vm.pop_valor()?;
                self.vm.push_valor(ValorFast::booleano(!v.es_verdadero()));
                self.vm.ip += 1;
            }

            // Opcodes no manejados explícitamente: avanzan IP como no-op
            // En una implementación completa, deben implementarse todos.
            _ => {
                self.vm.ip += 1;
            }
        }
        Ok(())
    }

    /// Obtener el call stack actual
    pub fn get_stack_trace(&self) -> Vec<FrameDebug> {
        let mut frames = Vec::new();
        let frame_count = self.vm.frame_count;

        if frame_count > 0 {
            // Frame actual (0 = tope del buffer, último push)
            let _f = &self.vm.frame_buffer[frame_count - 1];
            let name = format!("fn_{}", frame_count - 1);
            let vars = self.obtener_variables_locales(frame_count - 1);
            frames.push(FrameDebug {
                id: frame_count - 1,
                name,
                line: self.current_line,
                ip: self.vm.ip,
                vars,
            });

            // Frames padres
            for depth in (0..frame_count - 1).rev() {
                let pf = &self.vm.frame_buffer[depth];
                let name = format!("fn_{}", depth);
                let ret_line = self.ip_to_line.get(&pf.ip_ret).copied().unwrap_or(0);
                let f_vars = self.obtener_variables_locales(depth);
                frames.push(FrameDebug {
                    id: depth,
                    name,
                    line: ret_line,
                    ip: pf.ip_ret,
                    vars: f_vars,
                });
            }
        } else {
            // Ámbito global
            let vars = self.obtener_variables_globales();
            frames.push(FrameDebug {
                id: 0,
                name: "global".to_string(),
                line: self.current_line,
                ip: self.vm.ip,
                vars,
            });
        }
        frames
    }

    /// Verificar si hay un breakpoint activo en la línea dada, evaluando su
    /// condición (si la tiene) contra el estado actual de la VM.
    pub fn breakpoint_activo_en(&mut self, line: usize) -> bool {
        let cond = self.breakpoints.get(&line).cloned();
        match cond {
            None => false,
            Some(None) => true,
            Some(Some(cond)) => self.evaluar_condicion(&cond),
        }
    }

    /// Obtener variables locales de un frame dado, usando nombres reales
    /// cuando estén disponibles (poblados desde el AST).
    pub fn obtener_variables_locales(&self, frame_idx: usize) -> Vec<VarDebug> {
        let mut vars = Vec::new();
        if frame_idx >= self.vm.frame_locals.len() {
            return vars;
        }
        let local_frame = &self.vm.frame_locals[frame_idx];
        let nombres = self.nombres_locales.get(frame_idx);

        for (i, val) in local_frame.iter().enumerate() {
            let name = nombres
                .and_then(|ns| ns.get(i))
                .filter(|n| !n.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("var_{}", i));
            let (valor_str, tipo) = self.formatear_valor(*val);
            vars.push(VarDebug {
                name,
                value: valor_str,
                tipo,
                referencia: None,
            });
        }
        vars
    }

    /// Obtener variables globales (ámbito global), usando nombres reales
    /// cuando estén disponibles.
    pub fn obtener_variables_globales(&self) -> Vec<VarDebug> {
        let mut vars = Vec::new();
        // En stack-based execution, las globales están en global_var_persist
        for (i, val) in self.vm.global_var_persist.iter().enumerate() {
            let name = self
                .nombres_globales
                .get(i)
                .filter(|n| !n.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("global_{}", i));
            let (valor_str, tipo) = self.formatear_valor(*val);
            vars.push(VarDebug {
                name,
                value: valor_str,
                tipo,
                referencia: None,
            });
        }
        vars
    }

    /// Formatear un valor para display
    pub fn formatear_valor(&self, val: ValorFast) -> (String, String) {
        if val.es_entero() {
            (format!("{}", val.a_entero()), "entero".to_string())
        } else if val.es_flotante() {
            (format!("{}", val.a_flotante()), "decimal".to_string())
        } else if val.es_booleano() {
            if val.es_verdadero() {
                ("verdadero".to_string(), "booleano".to_string())
            } else {
                ("falso".to_string(), "booleano".to_string())
            }
        } else {
            let s = self.vm.mostrar_valor(&val);
            if s.len() > 100 {
                (format!("{}...", &s[..100]), "texto".to_string())
            } else {
                (s, "texto".to_string())
            }
        }
    }

    /// Evaluar una expresión en el contexto actual de la VM.
    ///
    /// Soporta: literales (entero, decimal, texto, booleano, nulo),
    /// identificadores (variables locales/globales), operadores aritméticos
    /// (`+ - * /`), comparación (`== != < > <= >=`), lógicos (`y`/`&&`,
    /// `o`/`||`, `no`/`!`) y agrupación con paréntesis.
    pub fn evaluar(&mut self, expr: &str) -> Result<VarDebug, String> {
        let tokens = tokenizar_expr(expr)?;
        let mut parser = ExprParser::new(&tokens);
        let ast = parser.parse_expr()?;
        if !parser.en_fin() {
            return Err(format!("expresión inválida: '{}'", expr));
        }
        let val = self.evaluar_ast(&ast)?;
        let (valor_str, tipo) = self.formatear_valor(val);
        Ok(VarDebug {
            name: expr.trim().to_string(),
            value: valor_str,
            tipo,
            referencia: None,
        })
    }

    /// Resolver el valor de un identificador en el estado actual de la VM.
    /// Busca primero en el frame local actual y luego en las globales.
    fn resolver_identificador(&self, nombre: &str) -> Result<ValorFast, String> {
        // Buscar en el frame local actual (tope de frame_locals)
        if let Some(locals) = self.vm.frame_locals.last() {
            let nombres = self.nombres_locales.last();
            for (i, val) in locals.iter().enumerate() {
                let n = nombres
                    .and_then(|ns| ns.get(i))
                    .filter(|n| !n.is_empty())
                    .map(|s| s.as_str());
                if n == Some(nombre) {
                    return Ok(*val);
                }
            }
        }
        // Buscar en globales
        for (i, val) in self.vm.global_var_persist.iter().enumerate() {
            if self.nombres_globales.get(i).map(|s| s.as_str()) == Some(nombre) {
                return Ok(*val);
            }
        }
        Err(format!("variable '{}' no encontrada", nombre))
    }

    /// Evaluar un AST de expresión contra el estado actual de la VM.
    fn evaluar_ast(&mut self, ast: &ExprAst) -> Result<ValorFast, String> {
        match ast {
            ExprAst::Entero(n) => Ok(get_small_int_fast(*n)),
            ExprAst::Decimal(d) => Ok(ValorFast::flotante(*d)),
            ExprAst::Texto(s) => {
                let idx = self.vm.alloc_str(s.clone().into());
                Ok(ValorFast::texto(idx))
            }
            ExprAst::Booleano(b) => Ok(ValorFast::booleano(*b)),
            ExprAst::Nulo => Ok(ValorFast::nulo()),
            ExprAst::Ident(nombre) => self.resolver_identificador(nombre),
            ExprAst::Unaria { op, expr } => {
                let v = self.evaluar_ast(expr)?;
                match op {
                    ExprOpUnaria::Neg => {
                        if v.es_entero() {
                            Ok(get_small_int_fast(-v.a_entero()))
                        } else {
                            Ok(ValorFast::flotante(-v.a_flotante()))
                        }
                    }
                    ExprOpUnaria::No => Ok(ValorFast::booleano(!v.es_verdadero())),
                }
            }
            ExprAst::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let a = self.evaluar_ast(izquierda)?;
                let b = self.evaluar_ast(derecha)?;
                Ok(aplicar_binaria(operador, a, b))
            }
        }
    }

    /// Poblar los nombres reales de variables y funciones a partir del AST.
    ///
    /// El bytecode solo conserva índices (LoadIdx/StoreIdx/DeclareIdx), por lo
    /// que los nombres se recuperan del AST respetando el mismo orden de
    /// asignación de índices que `optimizar_indices`: parámetros primero, luego
    /// variables en orden de primera aparición (Load/Store/Declare).
    pub fn cargar_nombres_desde_ast(&mut self, programa: &crate::ast::Programa) {
        use crate::ast::Declaracion;

        self.nombres_globales.clear();
        self.nombres_locales.clear();
        self.nombres_funciones.clear();

        // Globales: variables de nivel superior en orden de declaración.
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Variable { nombre, .. } => {
                    self.nombres_globales.push(nombre.clone());
                }
                Declaracion::AsignacionMultiple { variables, .. } => {
                    for v in variables {
                        self.nombres_globales.push(v.clone());
                    }
                }
                Declaracion::Funcion {
                    nombre,
                    parametros,
                    cuerpo,
                    ..
                } => {
                    // Nombres locales de la función: params + vars del cuerpo.
                    let mut locales: Vec<String> =
                        parametros.iter().map(|p| p.nombre.clone()).collect();
                    recolectar_nombres_locales(cuerpo, &mut locales);
                    self.nombres_locales.push(locales);
                    // Registrar nombre de función (el IP se resuelve aparte).
                    let _ = nombre;
                }
                _ => {}
            }
        }
    }
}

/// Recolectar nombres de variables locales de un cuerpo en orden de aparición.
fn recolectar_nombres_locales(cuerpo: &[crate::ast::Declaracion], out: &mut Vec<String>) {
    use crate::ast::Declaracion;
    for decl in cuerpo {
        match decl {
            Declaracion::Variable { nombre, .. } => {
                if !out.contains(nombre) {
                    out.push(nombre.clone());
                }
            }
            Declaracion::AsignacionMultiple { variables, .. } => {
                for v in variables {
                    if !out.contains(v) {
                        out.push(v.clone());
                    }
                }
            }
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
                recolectar_nombres_locales(bloque_verdadero, out);
                if let Some(f) = bloque_falso {
                    recolectar_nombres_locales(f, out);
                }
            }
            Declaracion::Mientras { bloque, .. } => recolectar_nombres_locales(bloque, out),
            Declaracion::Para {
                inicializacion,
                bloque,
                ..
            } => {
                if let Some(init) = inicializacion {
                    recolectar_nombres_locales(std::slice::from_ref(init.as_ref()), out);
                }
                recolectar_nombres_locales(bloque, out);
            }
            Declaracion::Repetir { bloque, .. } => recolectar_nombres_locales(bloque, out),
            Declaracion::Cuando { cuerpo, .. } => recolectar_nombres_locales(cuerpo, out),
            _ => {}
        }
    }
}

// ======================================================================
// Mini-evaluador de expresiones para el debugger (F2-T6)
// ======================================================================

/// Operadores unarios soportados por el evaluador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExprOpUnaria {
    Neg,
    No,
}

/// Operadores binarios soportados por el evaluador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExprOpBinaria {
    Suma,
    Resta,
    Mul,
    Div,
    Igual,
    Distinto,
    Menor,
    Mayor,
    MenorIgual,
    MayorIgual,
    Y,
    O,
}

/// AST mínimo de expresiones para el evaluador del debugger.
#[derive(Debug, Clone)]
enum ExprAst {
    Entero(i64),
    Decimal(f64),
    Texto(String),
    Booleano(bool),
    Nulo,
    Ident(String),
    Unaria {
        op: ExprOpUnaria,
        expr: Box<ExprAst>,
    },
    Binaria {
        izquierda: Box<ExprAst>,
        operador: ExprOpBinaria,
        derecha: Box<ExprAst>,
    },
}

/// Token del mini-lexer de expresiones.
#[derive(Debug, Clone, PartialEq)]
enum ExprToken {
    Entero(i64),
    Decimal(f64),
    Texto(String),
    Ident(String),
    Verdadero,
    Falso,
    Nulo,
    Suma,
    Resta,
    Mul,
    Div,
    Igual,
    Distinto,
    Menor,
    Mayor,
    MenorIgual,
    MayorIgual,
    Y,
    O,
    No,
    ParenIzq,
    ParenDer,
}

/// Tokenizar una expresión de debug.
fn tokenizar_expr(expr: &str) -> Result<Vec<ExprToken>, String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            if s.contains('.') {
                let d: f64 = s
                    .parse()
                    .map_err(|_| format!("decimal inválido: '{}'", s))?;
                tokens.push(ExprToken::Decimal(d));
            } else {
                let n: i64 = s.parse().map_err(|_| format!("entero inválido: '{}'", s))?;
                tokens.push(ExprToken::Entero(n));
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            match s.as_str() {
                "verdadero" => tokens.push(ExprToken::Verdadero),
                "falso" => tokens.push(ExprToken::Falso),
                "nulo" => tokens.push(ExprToken::Nulo),
                "y" => tokens.push(ExprToken::Y),
                "o" => tokens.push(ExprToken::O),
                "no" => tokens.push(ExprToken::No),
                _ => tokens.push(ExprToken::Ident(s)),
            }
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut s = String::new();
            while i < chars.len() && chars[i] != quote {
                s.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() {
                return Err("texto sin cerrar".to_string());
            }
            i += 1; // consumir comilla de cierre
            tokens.push(ExprToken::Texto(s));
            continue;
        }
        // Operadores (con soporte de dos caracteres)
        let next = chars.get(i + 1).copied();
        match c {
            '+' => {
                tokens.push(ExprToken::Suma);
                i += 1;
            }
            '-' => {
                tokens.push(ExprToken::Resta);
                i += 1;
            }
            '*' => {
                tokens.push(ExprToken::Mul);
                i += 1;
            }
            '/' => {
                tokens.push(ExprToken::Div);
                i += 1;
            }
            '(' => {
                tokens.push(ExprToken::ParenIzq);
                i += 1;
            }
            ')' => {
                tokens.push(ExprToken::ParenDer);
                i += 1;
            }
            '=' if next == Some('=') => {
                tokens.push(ExprToken::Igual);
                i += 2;
            }
            '!' if next == Some('=') => {
                tokens.push(ExprToken::Distinto);
                i += 2;
            }
            '!' => {
                tokens.push(ExprToken::No);
                i += 1;
            }
            '<' if next == Some('=') => {
                tokens.push(ExprToken::MenorIgual);
                i += 2;
            }
            '<' => {
                tokens.push(ExprToken::Menor);
                i += 1;
            }
            '>' if next == Some('=') => {
                tokens.push(ExprToken::MayorIgual);
                i += 2;
            }
            '>' => {
                tokens.push(ExprToken::Mayor);
                i += 1;
            }
            '&' if next == Some('&') => {
                tokens.push(ExprToken::Y);
                i += 2;
            }
            '|' if next == Some('|') => {
                tokens.push(ExprToken::O);
                i += 2;
            }
            _ => return Err(format!("carácter inesperado: '{}'", c)),
        }
    }
    Ok(tokens)
}

/// Parser descendente recursivo para el mini-evaluador.
struct ExprParser<'a> {
    tokens: &'a [ExprToken],
    pos: usize,
}

impl<'a> ExprParser<'a> {
    fn new(tokens: &'a [ExprToken]) -> Self {
        ExprParser { tokens, pos: 0 }
    }

    fn en_fin(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&ExprToken> {
        self.tokens.get(self.pos)
    }

    fn avanzar(&mut self) -> Option<ExprToken> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_expr(&mut self) -> Result<ExprAst, String> {
        self.parse_o()
    }

    fn parse_o(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parse_y()?;
        while matches!(self.peek(), Some(ExprToken::O)) {
            self.avanzar();
            let der = self.parse_y()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: ExprOpBinaria::O,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parse_y(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parse_igualdad()?;
        while matches!(self.peek(), Some(ExprToken::Y)) {
            self.avanzar();
            let der = self.parse_igualdad()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: ExprOpBinaria::Y,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parse_igualdad(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parse_relacional()?;
        loop {
            let op = match self.peek() {
                Some(ExprToken::Igual) => ExprOpBinaria::Igual,
                Some(ExprToken::Distinto) => ExprOpBinaria::Distinto,
                _ => break,
            };
            self.avanzar();
            let der = self.parse_relacional()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: op,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parse_relacional(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parse_aditivo()?;
        loop {
            let op = match self.peek() {
                Some(ExprToken::Menor) => ExprOpBinaria::Menor,
                Some(ExprToken::Mayor) => ExprOpBinaria::Mayor,
                Some(ExprToken::MenorIgual) => ExprOpBinaria::MenorIgual,
                Some(ExprToken::MayorIgual) => ExprOpBinaria::MayorIgual,
                _ => break,
            };
            self.avanzar();
            let der = self.parse_aditivo()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: op,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parse_aditivo(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parse_multiplicativo()?;
        loop {
            let op = match self.peek() {
                Some(ExprToken::Suma) => ExprOpBinaria::Suma,
                Some(ExprToken::Resta) => ExprOpBinaria::Resta,
                _ => break,
            };
            self.avanzar();
            let der = self.parse_multiplicativo()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: op,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parse_multiplicativo(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parse_unario()?;
        loop {
            let op = match self.peek() {
                Some(ExprToken::Mul) => ExprOpBinaria::Mul,
                Some(ExprToken::Div) => ExprOpBinaria::Div,
                _ => break,
            };
            self.avanzar();
            let der = self.parse_unario()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: op,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parse_unario(&mut self) -> Result<ExprAst, String> {
        match self.peek() {
            Some(ExprToken::Resta) => {
                self.avanzar();
                let e = self.parse_unario()?;
                Ok(ExprAst::Unaria {
                    op: ExprOpUnaria::Neg,
                    expr: Box::new(e),
                })
            }
            Some(ExprToken::No) => {
                self.avanzar();
                let e = self.parse_unario()?;
                Ok(ExprAst::Unaria {
                    op: ExprOpUnaria::No,
                    expr: Box::new(e),
                })
            }
            _ => self.parse_primario(),
        }
    }

    fn parse_primario(&mut self) -> Result<ExprAst, String> {
        match self.avanzar() {
            Some(ExprToken::Entero(n)) => Ok(ExprAst::Entero(n)),
            Some(ExprToken::Decimal(d)) => Ok(ExprAst::Decimal(d)),
            Some(ExprToken::Texto(s)) => Ok(ExprAst::Texto(s)),
            Some(ExprToken::Verdadero) => Ok(ExprAst::Booleano(true)),
            Some(ExprToken::Falso) => Ok(ExprAst::Booleano(false)),
            Some(ExprToken::Nulo) => Ok(ExprAst::Nulo),
            Some(ExprToken::Ident(s)) => Ok(ExprAst::Ident(s)),
            Some(ExprToken::ParenIzq) => {
                let e = self.parse_expr()?;
                match self.avanzar() {
                    Some(ExprToken::ParenDer) => Ok(e),
                    _ => Err("se esperaba ')'".to_string()),
                }
            }
            other => Err(format!("token inesperado: {:?}", other)),
        }
    }
}

/// Aplicar un operador binario a dos valores.
fn aplicar_binaria(op: &ExprOpBinaria, a: ValorFast, b: ValorFast) -> ValorFast {
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    match op {
        ExprOpBinaria::Suma => {
            if a_int && b_int {
                get_small_int_fast(a.a_entero() + b.a_entero())
            } else {
                ValorFast::flotante(num(a, a_int) + num(b, b_int))
            }
        }
        ExprOpBinaria::Resta => {
            if a_int && b_int {
                get_small_int_fast(a.a_entero() - b.a_entero())
            } else {
                ValorFast::flotante(num(a, a_int) - num(b, b_int))
            }
        }
        ExprOpBinaria::Mul => {
            if a_int && b_int {
                get_small_int_fast(a.a_entero() * b.a_entero())
            } else {
                ValorFast::flotante(num(a, a_int) * num(b, b_int))
            }
        }
        ExprOpBinaria::Div => {
            if a_int && b_int && b.a_entero() != 0 {
                get_small_int_fast(a.a_entero() / b.a_entero())
            } else {
                ValorFast::flotante(num(a, a_int) / num(b, b_int))
            }
        }
        ExprOpBinaria::Igual => ValorFast::booleano(comparar_igual(a, b)),
        ExprOpBinaria::Distinto => ValorFast::booleano(!comparar_igual(a, b)),
        ExprOpBinaria::Menor => ValorFast::booleano(num(a, a_int) < num(b, b_int)),
        ExprOpBinaria::Mayor => ValorFast::booleano(num(a, a_int) > num(b, b_int)),
        ExprOpBinaria::MenorIgual => ValorFast::booleano(num(a, a_int) <= num(b, b_int)),
        ExprOpBinaria::MayorIgual => ValorFast::booleano(num(a, a_int) >= num(b, b_int)),
        ExprOpBinaria::Y => ValorFast::booleano(a.es_verdadero() && b.es_verdadero()),
        ExprOpBinaria::O => ValorFast::booleano(a.es_verdadero() || b.es_verdadero()),
    }
}

/// Convertir un valor a f64 para operaciones numéricas mixtas.
fn num(v: ValorFast, es_int: bool) -> f64 {
    if es_int {
        v.a_entero() as f64
    } else {
        v.a_flotante()
    }
}

/// Comparar dos valores por igualdad (numérica o textual).
fn comparar_igual(a: ValorFast, b: ValorFast) -> bool {
    if a.es_entero() && b.es_entero() {
        a.a_entero() == b.a_entero()
    } else if a.es_booleano() && b.es_booleano() {
        a.es_verdadero() == b.es_verdadero()
    } else if a.es_nulo() && b.es_nulo() {
        true
    } else {
        let af = if a.es_entero() {
            a.a_entero() as f64
        } else {
            a.a_flotante()
        };
        let bf = if b.es_entero() {
            b.a_entero() as f64
        } else {
            b.a_flotante()
        };
        (af - bf).abs() < 1e-12
    }
}

// ======================================================================
// Funciones auxiliares de ejecución de codigos para debug
// ======================================================================

fn ejecutar_add(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int {
        dbg.vm
            .push_valor(get_small_int_fast(a.a_entero() + b.a_entero()));
    } else {
        let af = if a_int {
            a.a_entero() as f64
        } else {
            a.a_flotante()
        };
        let bf = if b_int {
            b.a_entero() as f64
        } else {
            b.a_flotante()
        };
        dbg.vm.push_valor(ValorFast::flotante(af + bf));
    }
    Ok(())
}

fn ejecutar_sub(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int {
        dbg.vm
            .push_valor(get_small_int_fast(a.a_entero() - b.a_entero()));
    } else {
        let af = if a_int {
            a.a_entero() as f64
        } else {
            a.a_flotante()
        };
        let bf = if b_int {
            b.a_entero() as f64
        } else {
            b.a_flotante()
        };
        dbg.vm.push_valor(ValorFast::flotante(af - bf));
    }
    Ok(())
}

fn ejecutar_mul(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int {
        dbg.vm
            .push_valor(get_small_int_fast(a.a_entero() * b.a_entero()));
    } else {
        let af = if a_int {
            a.a_entero() as f64
        } else {
            a.a_flotante()
        };
        let bf = if b_int {
            b.a_entero() as f64
        } else {
            b.a_flotante()
        };
        dbg.vm.push_valor(ValorFast::flotante(af * bf));
    }
    Ok(())
}

fn ejecutar_div(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int && b.a_entero() != 0 {
        dbg.vm
            .push_valor(get_small_int_fast(a.a_entero() / b.a_entero()));
    } else {
        let af = if a_int {
            a.a_entero() as f64
        } else {
            a.a_flotante()
        };
        let bf = if b_int {
            b.a_entero() as f64
        } else {
            b.a_flotante()
        };
        dbg.vm.push_valor(ValorFast::flotante(af / bf));
    }
    Ok(())
}

fn ejecutar_igual(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    let igual = if a.es_entero() && b.es_entero() {
        a.a_entero() == b.a_entero()
    } else {
        let af = if a.es_entero() {
            a.a_entero() as f64
        } else {
            a.a_flotante()
        };
        let bf = if b.es_entero() {
            b.a_entero() as f64
        } else {
            b.a_flotante()
        };
        (af - bf).abs() < 1e-12
    };
    dbg.vm.push_valor(ValorFast::booleano(igual));
    Ok(())
}

fn ejecutar_menor(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    let menor = if a.es_entero() && b.es_entero() {
        a.a_entero() < b.a_entero()
    } else {
        let af = if a.es_entero() {
            a.a_entero() as f64
        } else {
            a.a_flotante()
        };
        let bf = if b.es_entero() {
            b.a_entero() as f64
        } else {
            b.a_flotante()
        };
        af < bf
    };
    dbg.vm.push_valor(ValorFast::booleano(menor));
    Ok(())
}

fn ejecutar_mayor(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    let mayor = if a.es_entero() && b.es_entero() {
        a.a_entero() > b.a_entero()
    } else {
        let af = if a.es_entero() {
            a.a_entero() as f64
        } else {
            a.a_flotante()
        };
        let bf = if b.es_entero() {
            b.a_entero() as f64
        } else {
            b.a_flotante()
        };
        af > bf
    };
    dbg.vm.push_valor(ValorFast::booleano(mayor));
    Ok(())
}

fn ejecutar_y(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    dbg.vm
        .push_valor(ValorFast::booleano(a.es_verdadero() && b.es_verdadero()));
    Ok(())
}

fn ejecutar_o(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    dbg.vm
        .push_valor(ValorFast::booleano(a.es_verdadero() || b.es_verdadero()));
    Ok(())
}

/// Ejecutar Call en modo debug.
///
/// Usa el mismo mecanismo que ForjaFast::ejecutar para Call:
/// 1. Lookup de función via `lookup_func_entry` (soporta hot-reload)
/// 2. Creación de FrmFast con ip_ret, func_version
/// 3. Push de nuevo frame_locals Vec para el nuevo ámbito
/// 4. Paso de argumentos en los índices 0, 1, 2...
fn ejecutar_call_debug(dbg: &mut Debugger, nombre: &str, nargs: usize) -> Result<(), ErrFast> {
    let sym = dbg.vm.tabla_simbolos.internar(nombre);
    if let Some(entry) = dbg.vm.lookup_func_entry(sym) {
        // Sincronizar cache stack → stack real
        dbg.vm.flush_stack();

        let max_frames = dbg.vm.frame_buffer.len();
        if dbg.vm.frame_count >= max_frames {
            return Err(ErrFast::StackUnder(
                "Stack overflow: demasiadas llamadas anidadas".into(),
            ));
        }

        // Guardar frame actual
        dbg.vm.frame_buffer[dbg.vm.frame_count] = FrmFast {
            ip_ret: dbg.vm.ip, // ip ya incrementado por Call handler
            func_version: entry.version,
            func_sym: sym,
        };
        dbg.vm.frame_count += 1;

        // Pop args del stack de valores y revesarlos (orden normal)
        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
        for _ in 0..nargs {
            args.push(dbg.vm.pop_valor()?);
        }
        args.reverse();

        // Crear nuevo frame con variables locales
        let vars_size = entry.vars_size.max(nargs);
        let mut locals = vec![ValorFast::nulo(); vars_size];
        for (i, arg) in args.into_iter().enumerate() {
            locals[i] = arg;
        }
        dbg.vm.frame_locals.push(locals);

        // Saltar al código de la función
        dbg.vm.ip = entry.ip;

        Ok(())
    } else {
        Err(ErrFast::FnNoDef(format!(
            "función '{}' no encontrada",
            nombre
        )))
    }
}

/// Ejecutar Return en modo debug.
///
/// Sigue exactamente el mismo mecanismo que ForjaFast::ejecutar:
/// - NO toca el stack de valores (el valor de retorno ya está ahí)
/// - frame_count -= 1
/// - flush_stack (sincroniza cache)
/// - pop frame_locals (libera vars de la función que termina)
/// - restaura ip desde el frame
fn ejecutar_return_debug(dbg: &mut Debugger) -> Result<(), ErrFast> {
    if dbg.vm.frame_count == 0 {
        return Err(ErrFast::StackUnder("Return sin frame activo".into()));
    }
    dbg.vm.frame_count -= 1;
    let frame = dbg.vm.frame_buffer[dbg.vm.frame_count];
    // Sincronizar cache
    dbg.vm.flush_stack();
    // Liberar vars de la función que termina
    dbg.vm.frame_locals.pop();
    dbg.vm.ip = frame.ip_ret;
    Ok(())
}

// ======================================================================
// Eventos del debugger
// ======================================================================

#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// Se alcanzó un breakpoint en la línea
    Breakpoint { line: usize },
    /// Step (over/into/out) completado en la línea
    StepCompletado { line: usize },
    /// Pausado manualmente en línea
    Pausado { line: usize },
    /// Programa esta_terminado
    Terminado,
}
