// Forja Debugger — Modo paso a paso con breakpoints
// Envuelve ForjaFast y controla la ejecución opcode por opcode,
// permitiendo pausa en breakpoints, step over/into/out y
// extracción de variables del stack/frames.

use crate::bytecode::Opcode;
use crate::vm_fast::{obtener_small_entero_fast, ErrFast, ForjaFast, FrmFast, ValorFast};
use std::collections::HashMap;

/// Estado del debugger
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepuracionState {
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
pub struct FrameDepuracion {
    pub id: usize,
    pub name: String,                  // nombre de la función
    pub linea: usize,                  // línea actual
    pub ip: usize,                     // instruction pointer actual
    pub vars: Vec<VariableDepuracion>, // variables locales
}

/// Información de una variable para DAP
#[derive(Debug, Clone)]
pub struct VariableDepuracion {
    pub name: String,
    pub valor: String,
    pub tipo: String,
    pub referencia: Option<usize>, // para variables compuestas (array/objeto)
}

/// Wrapper de depuración sobre ForjaFast
pub struct Debugger {
    /// La VM subyacente
    pub vm: ForjaFast,
    /// Estado actual del debugger
    pub state: DepuracionState,
    /// Breakpoints activos: mapean número de línea → condición opcional.
    /// `None` = breakpoint incondicional; `Some(expr)` = pausa solo si la
    /// expresión evalúa a verdadero.
    pub breakpoints: HashMap<usize, Option<String>>,
    /// Cache de mapeo línea → IP (construido al cargar bytecode)
    pub linea_a_ip: HashMap<usize, Vec<usize>>,
    /// Cache inverso IP → línea (para lookup rápido)
    pub ip_a_linea: HashMap<usize, usize>,
    /// Línea actual de ejecución
    pub linea_actual: usize,
    /// Línea donde estábamos cuando se inició StepOut
    pub marco_destino_salir: usize,
    /// Cantidad de frames al iniciar StepOver
    pub cantidad_marcos_paso_a_paso: usize,
    /// Últimas líneas encontradas (para detección de cambio de línea)
    pub linea_ultimo_reporte: usize,
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
        Self::nuevo()
    }
}

impl Debugger {
    pub fn nuevo() -> Self {
        Debugger {
            vm: ForjaFast::nuevo(),
            state: DepuracionState::Running,
            breakpoints: HashMap::new(),
            linea_a_ip: HashMap::new(),
            ip_a_linea: HashMap::new(),
            linea_actual: 0,
            marco_destino_salir: 0,
            cantidad_marcos_paso_a_paso: 0,
            linea_ultimo_reporte: 0,
            next_id: 0,
            nombres_locales: Vec::new(),
            nombres_globales: Vec::new(),
            nombres_funciones: HashMap::new(),
        }
    }

    /// Cargar bytecode en la VM y construir el mapeo línea → IP
    pub fn cargar_bytecode(&mut self, bc: Vec<Opcode>) {
        // Construir mapeos línea ↔ IP
        self.linea_a_ip.clear();
        self.ip_a_linea.clear();
        for (ip, op) in bc.iter().enumerate() {
            if let Opcode::FijarLinea(linea) = op {
                self.linea_a_ip.entry(*linea).or_default().push(ip);
                self.ip_a_linea.insert(ip, *linea);
            }
        }
        self.linea_actual = 0;
        self.linea_ultimo_reporte = 0;
        self.state = DepuracionState::Running;
        self.vm.cargar_bytecode(bc);
    }

    /// Activar/desactivar breakpoint en línea (incondicional)
    pub fn toggle_breakpoint(&mut self, linea: usize, active: bool) {
        if active {
            self.breakpoints.insert(linea, None);
        } else {
            self.breakpoints.remove(&linea);
        }
    }

    /// Poner breakpoint incondicional en línea
    pub fn fijar_breakpoint(&mut self, linea: usize) {
        self.breakpoints.insert(linea, None);
    }

    /// Poner breakpoint condicional en línea.
    /// `condicion` vacía equivale a un breakpoint incondicional.
    pub fn fijar_breakpoint_condicional(&mut self, linea: usize, condicion: Option<String>) {
        let cond = condicion.filter(|c| !c.trim().is_empty());
        self.breakpoints.insert(linea, cond);
    }

    /// Quitar breakpoint de línea
    pub fn limpiar_breakpoint(&mut self, linea: usize) {
        self.breakpoints.remove(&linea);
    }

    /// Verificar si estamos en un breakpoint (incondicional o con condición
    /// que evalúa a verdadero en el estado actual de la VM).
    pub fn en_breakpoint(&mut self) -> bool {
        let cond = self.breakpoints.get(&self.linea_actual).cloned();
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
            Ok(var) => var.valor == "verdadero",
            Err(_) => false,
        }
    }

    /// Continuar ejecución libre (hasta breakpoint o fin)
    pub fn continuar(&mut self) -> Result<DepuracionEvento, ErrFast> {
        self.state = DepuracionState::Running;
        self.ejecutar_hasta_evento()
    }

    /// Step Over: ejecutar hasta la próxima línea en el mismo frame depth
    pub fn step_over(&mut self) -> Result<DepuracionEvento, ErrFast> {
        self.state = DepuracionState::StepOver;
        self.cantidad_marcos_paso_a_paso = self.vm.cantidad_marcos;
        self.ejecutar_hasta_evento()
    }

    /// Step Into: ejecutar hasta la próxima línea (puede entrar en funciones)
    pub fn step_into(&mut self) -> Result<DepuracionEvento, ErrFast> {
        self.state = DepuracionState::StepInto;
        self.ejecutar_hasta_evento()
    }

    /// Step Out: ejecutar hasta que la función actual retorne
    pub fn step_out(&mut self) -> Result<DepuracionEvento, ErrFast> {
        self.state = DepuracionState::StepOut;
        self.marco_destino_salir = self.vm.cantidad_marcos;
        self.ejecutar_hasta_evento()
    }

    /// Ejecutar la VM hasta que ocurra un evento (breakpoint, step complete, fin, error)
    pub fn ejecutar_hasta_evento(&mut self) -> Result<DepuracionEvento, ErrFast> {
        loop {
            if self.vm.ip >= self.vm.bytecode.len() {
                self.state = DepuracionState::Paused;
                return Ok(DepuracionEvento::Terminado);
            }
            if self.vm.ejecutadas > self.vm.max_inst {
                self.state = DepuracionState::Paused;
                return Err(ErrFast::Limite);
            }
            self.vm.ejecutadas += 1;

            let op = self.vm.bytecode[self.vm.ip].clone();

            // Procesar SetLine: actualizar línea actual
            if let Opcode::FijarLinea(linea) = &op {
                self.linea_actual = *linea;
                self.vm.ip += 1;

                // Verificar si debemos pausar
                match self.state {
                    DepuracionState::Running => {
                        if self.en_breakpoint() {
                            self.state = DepuracionState::Paused;
                            return Ok(DepuracionEvento::Breakpoint { linea: *linea });
                        }
                    }
                    DepuracionState::StepInto => {
                        if *linea != self.linea_ultimo_reporte {
                            self.state = DepuracionState::Paused;
                            self.linea_ultimo_reporte = *linea;
                            return Ok(DepuracionEvento::StepCompletado { linea: *linea });
                        }
                    }
                    DepuracionState::StepOver => {
                        if *linea != self.linea_ultimo_reporte
                            && self.vm.cantidad_marcos <= self.cantidad_marcos_paso_a_paso
                        {
                            self.state = DepuracionState::Paused;
                            self.linea_ultimo_reporte = *linea;
                            return Ok(DepuracionEvento::StepCompletado { linea: *linea });
                        }
                    }
                    DepuracionState::StepOut => {
                        if *linea != self.linea_ultimo_reporte
                            && self.vm.cantidad_marcos < self.marco_destino_salir
                        {
                            self.state = DepuracionState::Paused;
                            self.linea_ultimo_reporte = *linea;
                            return Ok(DepuracionEvento::StepCompletado { linea: *linea });
                        }
                    }
                    DepuracionState::Paused => {
                        return Ok(DepuracionEvento::Pausado { linea: *linea });
                    }
                }
                continue;
            }

            // Ejecutar el opcode
            self.ejecutar_un_paso(&op)?;

            // Detectar salida de funciones para StepOut
            if self.state == DepuracionState::StepOut
                && self.vm.cantidad_marcos < self.marco_destino_salir
            {
                self.state = DepuracionState::Paused;
                return Ok(DepuracionEvento::StepCompletado {
                    linea: self.linea_actual,
                });
            }

            // Si el state es Running, check breakpoints después de SetLine
            if self.state == DepuracionState::Running && self.linea_actual > 0 {
                if let Some(&linea) = self.ip_a_linea.get(&self.vm.ip) {
                    if linea != self.linea_ultimo_reporte && self.breakpoint_activo_en(linea) {
                        self.linea_actual = linea;
                        self.state = DepuracionState::Paused;
                        self.linea_ultimo_reporte = linea;
                        return Ok(DepuracionEvento::Breakpoint { linea });
                    }
                }
            }
        }
    }

    /// Ejecuta un solo opcode (equivalente a una iteración del loop de ForjaFast::ejecutar)
    fn ejecutar_un_paso(&mut self, op: &Opcode) -> Result<(), ErrFast> {
        match op {
            Opcode::EmpujarEntero(n) => {
                self.vm.empujar_valor(obtener_small_entero_fast(*n));
                self.vm.ip += 1;
            }
            Opcode::EmpujarDecimal(d) => {
                self.vm.empujar_valor(ValorFast::flotante(*d));
                self.vm.ip += 1;
            }
            Opcode::EmpujarTexto(s) => {
                let idx = self.vm.reservar_texto(s.clone());
                self.vm.empujar_valor(ValorFast::texto(idx));
                self.vm.ip += 1;
            }
            Opcode::EmpujarBooleano(b) => {
                self.vm.empujar_valor(ValorFast::booleano(*b));
                self.vm.ip += 1;
            }
            Opcode::EmpujarNulo => {
                self.vm.empujar_valor(ValorFast::nulo());
                self.vm.ip += 1;
            }
            Opcode::Descartar => {
                self.vm.desapilar_valor()?;
                self.vm.ip += 1;
            }
            Opcode::Duplicar => {
                let v = *self.vm.peek_valor(0);
                self.vm.empujar_valor(v);
                self.vm.ip += 1;
            }
            Opcode::CargarIndice(idx) => {
                self.vm.empujar_valor(self.vm.cargar_local(*idx));
                self.vm.ip += 1;
            }
            Opcode::GuardarIndice(idx) => {
                let val = self.vm.desapilar_valor()?;
                self.vm.almacen_local(*idx, val);
                self.vm.ip += 1;
            }
            Opcode::DeclararIndice(idx, _mutable) => {
                // Ensure local slot exists
                self.vm.almacen_local(*idx, self.vm.cargar_local(*idx));
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
                let cond = self.vm.desapilar_valor()?;
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
                ejecutar_llamada_depuracion(self, nombre, n)?;
            }
            Opcode::Retornar => {
                ejecutar_return_depuracion(self)?;
            }
            Opcode::DefinirFuncion(_, _) => {
                self.vm.ip += 1;
            }
            Opcode::Imprimir => {
                let val = self.vm.desapilar_valor()?;
                let s = self.vm.mostrar_valor(&val);
                self.vm.salida.lock().unwrap().push(s);
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
                let v = self.vm.desapilar_valor()?;
                self.vm
                    .empujar_valor(ValorFast::booleano(!v.es_verdadero()));
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
    pub fn obtener_pila_traza(&self) -> Vec<FrameDepuracion> {
        let mut frames = Vec::new();
        let frame_cantidad = self.vm.cantidad_marcos;

        if frame_cantidad > 0 {
            // Frame actual (0 = tope del buffer, último push)
            let _f = &self.vm.frame_buffer[frame_cantidad - 1];
            let name = format!("fn_{}", frame_cantidad - 1);
            let vars = self.obtener_variables_locales(frame_cantidad - 1);
            frames.push(FrameDepuracion {
                id: frame_cantidad - 1,
                name,
                linea: self.linea_actual,
                ip: self.vm.ip,
                vars,
            });

            // Frames padres
            for depth in (0..frame_cantidad - 1).rev() {
                let pf = &self.vm.frame_buffer[depth];
                let name = format!("fn_{}", depth);
                let ret_linea = self.ip_a_linea.get(&pf.ip_ret).copied().unwrap_or(0);
                let f_vars = self.obtener_variables_locales(depth);
                frames.push(FrameDepuracion {
                    id: depth,
                    name,
                    linea: ret_linea,
                    ip: pf.ip_ret,
                    vars: f_vars,
                });
            }
        } else {
            // Ámbito global
            let vars = self.obtener_variables_globales();
            frames.push(FrameDepuracion {
                id: 0,
                name: "global".to_string(),
                linea: self.linea_actual,
                ip: self.vm.ip,
                vars,
            });
        }
        frames
    }

    /// Verificar si hay un breakpoint activo en la línea dada, evaluando su
    /// condición (si la tiene) contra el estado actual de la VM.
    pub fn breakpoint_activo_en(&mut self, linea: usize) -> bool {
        let cond = self.breakpoints.get(&linea).cloned();
        match cond {
            None => false,
            Some(None) => true,
            Some(Some(cond)) => self.evaluar_condicion(&cond),
        }
    }

    /// Obtener variables locales de un frame dado, usando nombres reales
    /// cuando estén disponibles (poblados desde el AST).
    pub fn obtener_variables_locales(&self, frame_idx: usize) -> Vec<VariableDepuracion> {
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
            vars.push(VariableDepuracion {
                name,
                valor: valor_str,
                tipo,
                referencia: None,
            });
        }
        vars
    }

    /// Obtener variables globales (ámbito global), usando nombres reales
    /// cuando estén disponibles.
    pub fn obtener_variables_globales(&self) -> Vec<VariableDepuracion> {
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
            vars.push(VariableDepuracion {
                name,
                valor: valor_str,
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
    pub fn evaluar(&mut self, expr: &str) -> Result<VariableDepuracion, String> {
        let tokens = tokenizar_expr(expr)?;
        let mut parser = ExprParser::nuevo(&tokens);
        let ast = parser.parsear_expresion()?;
        if !parser.en_fin() {
            return Err(format!("expresión inválida: '{}'", expr));
        }
        let val = self.evaluar_ast(&ast)?;
        let (valor_str, tipo) = self.formatear_valor(val);
        Ok(VariableDepuracion {
            name: expr.trim().to_string(),
            valor: valor_str,
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
            ExprAst::Entero(n) => Ok(obtener_small_entero_fast(*n)),
            ExprAst::Decimal(d) => Ok(ValorFast::flotante(*d)),
            ExprAst::Texto(s) => {
                let idx = self.vm.reservar_texto(s.clone().into());
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
                            Ok(obtener_small_entero_fast(-v.a_entero()))
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
// Mini-evaluador de expresiones para el debugger
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
            let inicio = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[inicio..i].iter().collect();
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
            let inicio = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let s: String = chars[inicio..i].iter().collect();
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
            let entrecomillar = c;
            i += 1;
            let mut s = String::new();
            while i < chars.len() && chars[i] != entrecomillar {
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
    fn nuevo(tokens: &'a [ExprToken]) -> Self {
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

    fn parsear_expresion(&mut self) -> Result<ExprAst, String> {
        self.parsear_o()
    }

    fn parsear_o(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parsear_y()?;
        while matches!(self.peek(), Some(ExprToken::O)) {
            self.avanzar();
            let der = self.parsear_y()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: ExprOpBinaria::O,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parsear_y(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parsear_igualdad()?;
        while matches!(self.peek(), Some(ExprToken::Y)) {
            self.avanzar();
            let der = self.parsear_igualdad()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: ExprOpBinaria::Y,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parsear_igualdad(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parsear_relacional()?;
        loop {
            let op = match self.peek() {
                Some(ExprToken::Igual) => ExprOpBinaria::Igual,
                Some(ExprToken::Distinto) => ExprOpBinaria::Distinto,
                _ => break,
            };
            self.avanzar();
            let der = self.parsear_relacional()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: op,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parsear_relacional(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parsear_aditivo()?;
        loop {
            let op = match self.peek() {
                Some(ExprToken::Menor) => ExprOpBinaria::Menor,
                Some(ExprToken::Mayor) => ExprOpBinaria::Mayor,
                Some(ExprToken::MenorIgual) => ExprOpBinaria::MenorIgual,
                Some(ExprToken::MayorIgual) => ExprOpBinaria::MayorIgual,
                _ => break,
            };
            self.avanzar();
            let der = self.parsear_aditivo()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: op,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parsear_aditivo(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parsear_multiplicativo()?;
        loop {
            let op = match self.peek() {
                Some(ExprToken::Suma) => ExprOpBinaria::Suma,
                Some(ExprToken::Resta) => ExprOpBinaria::Resta,
                _ => break,
            };
            self.avanzar();
            let der = self.parsear_multiplicativo()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: op,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parsear_multiplicativo(&mut self) -> Result<ExprAst, String> {
        let mut izq = self.parsear_unario()?;
        loop {
            let op = match self.peek() {
                Some(ExprToken::Mul) => ExprOpBinaria::Mul,
                Some(ExprToken::Div) => ExprOpBinaria::Div,
                _ => break,
            };
            self.avanzar();
            let der = self.parsear_unario()?;
            izq = ExprAst::Binaria {
                izquierda: Box::new(izq),
                operador: op,
                derecha: Box::new(der),
            };
        }
        Ok(izq)
    }

    fn parsear_unario(&mut self) -> Result<ExprAst, String> {
        match self.peek() {
            Some(ExprToken::Resta) => {
                self.avanzar();
                let e = self.parsear_unario()?;
                Ok(ExprAst::Unaria {
                    op: ExprOpUnaria::Neg,
                    expr: Box::new(e),
                })
            }
            Some(ExprToken::No) => {
                self.avanzar();
                let e = self.parsear_unario()?;
                Ok(ExprAst::Unaria {
                    op: ExprOpUnaria::No,
                    expr: Box::new(e),
                })
            }
            _ => self.parsear_primario(),
        }
    }

    fn parsear_primario(&mut self) -> Result<ExprAst, String> {
        match self.avanzar() {
            Some(ExprToken::Entero(n)) => Ok(ExprAst::Entero(n)),
            Some(ExprToken::Decimal(d)) => Ok(ExprAst::Decimal(d)),
            Some(ExprToken::Texto(s)) => Ok(ExprAst::Texto(s)),
            Some(ExprToken::Verdadero) => Ok(ExprAst::Booleano(true)),
            Some(ExprToken::Falso) => Ok(ExprAst::Booleano(false)),
            Some(ExprToken::Nulo) => Ok(ExprAst::Nulo),
            Some(ExprToken::Ident(s)) => Ok(ExprAst::Ident(s)),
            Some(ExprToken::ParenIzq) => {
                let e = self.parsear_expresion()?;
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
                obtener_small_entero_fast(a.a_entero() + b.a_entero())
            } else {
                ValorFast::flotante(num(a, a_int) + num(b, b_int))
            }
        }
        ExprOpBinaria::Resta => {
            if a_int && b_int {
                obtener_small_entero_fast(a.a_entero() - b.a_entero())
            } else {
                ValorFast::flotante(num(a, a_int) - num(b, b_int))
            }
        }
        ExprOpBinaria::Mul => {
            if a_int && b_int {
                obtener_small_entero_fast(a.a_entero() * b.a_entero())
            } else {
                ValorFast::flotante(num(a, a_int) * num(b, b_int))
            }
        }
        ExprOpBinaria::Div => {
            if a_int && b_int && b.a_entero() != 0 {
                obtener_small_entero_fast(a.a_entero() / b.a_entero())
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
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int {
        dbg.vm
            .empujar_valor(obtener_small_entero_fast(a.a_entero() + b.a_entero()));
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
        dbg.vm.empujar_valor(ValorFast::flotante(af + bf));
    }
    Ok(())
}

fn ejecutar_sub(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int {
        dbg.vm
            .empujar_valor(obtener_small_entero_fast(a.a_entero() - b.a_entero()));
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
        dbg.vm.empujar_valor(ValorFast::flotante(af - bf));
    }
    Ok(())
}

fn ejecutar_mul(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int {
        dbg.vm
            .empujar_valor(obtener_small_entero_fast(a.a_entero() * b.a_entero()));
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
        dbg.vm.empujar_valor(ValorFast::flotante(af * bf));
    }
    Ok(())
}

fn ejecutar_div(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int && b.a_entero() != 0 {
        dbg.vm
            .empujar_valor(obtener_small_entero_fast(a.a_entero() / b.a_entero()));
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
        dbg.vm.empujar_valor(ValorFast::flotante(af / bf));
    }
    Ok(())
}

fn ejecutar_igual(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
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
    dbg.vm.empujar_valor(ValorFast::booleano(igual));
    Ok(())
}

fn ejecutar_menor(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
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
    dbg.vm.empujar_valor(ValorFast::booleano(menor));
    Ok(())
}

fn ejecutar_mayor(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
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
    dbg.vm.empujar_valor(ValorFast::booleano(mayor));
    Ok(())
}

fn ejecutar_y(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
    dbg.vm
        .empujar_valor(ValorFast::booleano(a.es_verdadero() && b.es_verdadero()));
    Ok(())
}

fn ejecutar_o(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.desapilar_valor()?;
    let a = dbg.vm.desapilar_valor()?;
    dbg.vm
        .empujar_valor(ValorFast::booleano(a.es_verdadero() || b.es_verdadero()));
    Ok(())
}

/// Ejecutar Call en modo debug.
///
/// Usa el mismo mecanismo que ForjaFast::ejecutar para Call:
/// 1. Lookup de función via `lookup_funcion_entrada` (soporta hot-reload)
/// 2. Creación de FrmFast con ip_ret, funcion_version
/// 3. Push de nuevo frame_locals Vec para el nuevo ámbito
/// 4. Paso de argumentos en los índices 0, 1, 2...
fn ejecutar_llamada_depuracion(
    dbg: &mut Debugger,
    nombre: &str,
    nargs: usize,
) -> Result<(), ErrFast> {
    let sym = dbg.vm.runtime.tabla_simbolos.internar(nombre);
    if let Some(entrada) = dbg.vm.lookup_funcion_entrada(sym) {
        // Sincronizar cache stack → stack real
        dbg.vm.flush_pila();

        let max_frames = dbg.vm.frame_buffer.len();
        if dbg.vm.cantidad_marcos >= max_frames {
            return Err(ErrFast::StackUnder(
                "Stack overflow: demasiadas llamadas anidadas".into(),
            ));
        }

        // Guardar frame actual
        dbg.vm.frame_buffer[dbg.vm.cantidad_marcos] = FrmFast {
            ip_ret: dbg.vm.ip, // ip ya incrementado por Call handler
            funcion_version: entrada.version,
            funcion_simbolo: sym,
        };
        dbg.vm.cantidad_marcos += 1;

        // Pop args del stack de valores y revesarlos (orden normal)
        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
        for _ in 0..nargs {
            args.push(dbg.vm.desapilar_valor()?);
        }
        args.reverse();

        // Crear nuevo frame con variables locales
        let vars_tamano = entrada.tamano_variables.max(nargs);
        let mut locals = vec![ValorFast::nulo(); vars_tamano];
        for (i, arg) in args.into_iter().enumerate() {
            locals[i] = arg;
        }
        dbg.vm.frame_locals.push(locals);

        // Saltar al código de la función
        dbg.vm.ip = entrada.ip;

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
/// - cantidad_marcos -= 1
/// - flush_pila (sincroniza cache)
/// - pop frame_locals (libera vars de la función que termina)
/// - restaura ip desde el frame
fn ejecutar_return_depuracion(dbg: &mut Debugger) -> Result<(), ErrFast> {
    if dbg.vm.cantidad_marcos == 0 {
        return Err(ErrFast::StackUnder("Return sin frame activo".into()));
    }
    dbg.vm.cantidad_marcos -= 1;
    let frame = dbg.vm.frame_buffer[dbg.vm.cantidad_marcos];
    // Sincronizar cache
    dbg.vm.flush_pila();
    // Liberar vars de la función que termina
    dbg.vm.frame_locals.pop();
    dbg.vm.ip = frame.ip_ret;
    Ok(())
}

// ======================================================================
// Eventos del debugger
// ======================================================================

#[derive(Debug, Clone)]
pub enum DepuracionEvento {
    /// Se alcanzó un breakpoint en la línea
    Breakpoint { linea: usize },
    /// Step (over/into/out) completado en la línea
    StepCompletado { linea: usize },
    /// Pausado manualmente en línea
    Pausado { linea: usize },
    /// Programa esta_terminado
    Terminado,
}
