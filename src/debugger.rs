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
    /// Breakpoints activos: mapean número de línea → activo
    pub breakpoints: HashMap<usize, bool>,
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
        }
    }

    /// Cargar bytecode en la VM y construir el mapeo línea → IP
    pub fn cargar_bytecode(&mut self, bc: Vec<Opcode>) {
        // Construir mapeos línea ↔ IP
        self.line_to_ip.clear();
        self.ip_to_line.clear();
        for (ip, op) in bc.iter().enumerate() {
            if let Opcode::SetLine(line) = op {
                self.line_to_ip
                    .entry(*line)
                    .or_insert_with(Vec::new)
                    .push(ip);
                self.ip_to_line.insert(ip, *line);
            }
        }
        self.current_line = 0;
        self.last_report_line = 0;
        self.state = DebugState::Running;
        self.vm.cargar_bytecode(bc);
    }

    /// Activar/desactivar breakpoint en línea
    pub fn toggle_breakpoint(&mut self, line: usize, active: bool) {
        self.breakpoints.insert(line, active);
    }

    /// Poner breakpoint en línea
    pub fn set_breakpoint(&mut self, line: usize) {
        self.breakpoints.insert(line, true);
    }

    /// Quitar breakpoint de línea
    pub fn clear_breakpoint(&mut self, line: usize) {
        self.breakpoints.remove(&line);
    }

    /// Verificar si estamos en un breakpoint
    pub fn en_breakpoint(&self) -> bool {
        self.breakpoints
            .get(&self.current_line)
            .copied()
            .unwrap_or(false)
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
            if let Opcode::SetLine(line) = &op {
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
            if self.state == DebugState::StepOut {
                if self.vm.frame_count < self.step_out_target_frame {
                    self.state = DebugState::Paused;
                    return Ok(DebugEvent::StepCompletado {
                        line: self.current_line,
                    });
                }
            }

            // Si el state es Running, check breakpoints después de SetLine
            if self.state == DebugState::Running && self.en_breakpoint() && self.current_line > 0 {
                if let Some(&line) = self.ip_to_line.get(&self.vm.ip) {
                    if line != self.last_report_line
                        && self.breakpoints.get(&line).copied().unwrap_or(false)
                    {
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
            Opcode::Pop => {
                self.vm.pop_valor()?;
                self.vm.ip += 1;
            }
            Opcode::Dup => {
                let v = *self.vm.peek_valor(0);
                self.vm.push_valor(v);
                self.vm.ip += 1;
            }
            Opcode::LoadIdx(idx) => {
                let actual = self.vm.base_ptr + idx;
                if actual < self.vm.flat_vars.len() {
                    self.vm.push_valor(self.vm.flat_vars[actual]);
                } else {
                    self.vm.push_valor(ValorFast::nulo());
                }
                self.vm.ip += 1;
            }
            Opcode::StoreIdx(idx) => {
                let val = self.vm.pop_valor()?;
                let actual = self.vm.base_ptr + idx;
                if actual >= self.vm.flat_vars.len() {
                    self.vm.flat_vars.resize(actual + 1, ValorFast::nulo());
                }
                self.vm.flat_vars[actual] = val;
                self.vm.ip += 1;
            }
            Opcode::DeclareIdx(idx, _mutable) => {
                let actual = self.vm.base_ptr + idx;
                if actual >= self.vm.flat_vars.len() {
                    self.vm.flat_vars.resize(actual + 1, ValorFast::nulo());
                }
                self.vm.ip += 1;
            }
            Opcode::Add => {
                self.vm.ip += 1;
                ejecutar_add(self)?;
            }
            Opcode::Sub => {
                self.vm.ip += 1;
                ejecutar_sub(self)?;
            }
            Opcode::Mul => {
                self.vm.ip += 1;
                ejecutar_mul(self)?;
            }
            Opcode::Div => {
                self.vm.ip += 1;
                ejecutar_div(self)?;
            }
            Opcode::Jump(target) => {
                self.vm.ip = *target;
            }
            Opcode::JumpSiFalso(target) => {
                let cond = self.vm.pop_valor()?;
                if !cond.es_verdadero() {
                    self.vm.ip = *target;
                } else {
                    self.vm.ip += 1;
                }
            }
            Opcode::Label(_) => {
                self.vm.ip += 1;
            }
            Opcode::Halt => {
                self.vm.ip = self.vm.bytecode.len();
            }
            Opcode::Call(nombre, nargs) => {
                self.vm.ip += 1;
                let n = *nargs;
                ejecutar_call_debug(self, nombre, n)?;
            }
            Opcode::Return => {
                ejecutar_return_debug(self)?;
            }
            Opcode::FunctionDef(_, _) => {
                self.vm.ip += 1;
            }
            Opcode::Print => {
                let val = self.vm.pop_valor()?;
                let s = self.vm.mostrar_valor(&val);
                self.vm.output.push(s);
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

    /// Obtener variables locales de un frame dado
    pub fn obtener_variables_locales(&self, frame_idx: usize) -> Vec<VarDebug> {
        let mut vars = Vec::new();
        if frame_idx >= self.vm.frame_count {
            return vars;
        }
        let f = &self.vm.frame_buffer[frame_idx];
        let base = f.base_ptr_previo;
        let num_vars = f.num_vars;

        for i in 0..num_vars {
            let idx = base + i;
            let name = format!("var_{}", i);
            if idx < self.vm.flat_vars.len() {
                let val = self.vm.flat_vars[idx];
                let (valor_str, tipo) = self.formatear_valor(val);
                vars.push(VarDebug {
                    name,
                    value: valor_str,
                    tipo,
                    referencia: None,
                });
            }
        }
        vars
    }

    /// Obtener variables globales (ámbito global)
    pub fn obtener_variables_globales(&self) -> Vec<VarDebug> {
        let mut vars = Vec::new();
        for (i, val) in self.vm.flat_vars.iter().enumerate() {
            let (valor_str, tipo) = self.formatear_valor(*val);
            vars.push(VarDebug {
                name: format!("global_{}", i),
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

    /// Evaluar expresión simple (por ahora solo leer variables)
    pub fn evaluar(&self, _expr: &str) -> Result<VarDebug, String> {
        Ok(VarDebug {
            name: _expr.to_string(),
            value: "(expresión no soportada en debug básico)".to_string(),
            tipo: "desconocido".to_string(),
            referencia: None,
        })
    }
}

// ======================================================================
// Funciones auxiliares de ejecución de opcodes para debug
// ======================================================================

fn ejecutar_add(dbg: &mut Debugger) -> Result<(), ErrFast> {
    let b = dbg.vm.pop_valor()?;
    let a = dbg.vm.pop_valor()?;
    let a_int = a.es_entero();
    let b_int = b.es_entero();
    if a_int && b_int {
        dbg.vm
            .push_valor(get_small_int_fast((a.a_entero() + b.a_entero()) as i64));
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
            .push_valor(get_small_int_fast((a.a_entero() - b.a_entero()) as i64));
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
            .push_valor(get_small_int_fast((a.a_entero() * b.a_entero()) as i64));
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
            .push_valor(get_small_int_fast((a.a_entero() / b.a_entero()) as i64));
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
/// 2. Creación de FrmFast con ip_ret, base_ptr_previo, num_vars, func_version
/// 3. Expansión de flat_vars para el nuevo ámbito
/// 4. Paso de argumentos por flat_vars[base_ptr..base_ptr+nargs]
fn ejecutar_call_debug(dbg: &mut Debugger, nombre: &str, nargs: usize) -> Result<(), ErrFast> {
    let sym = dbg.vm.sym_table.intern(nombre);
    if let Some(entry) = dbg.vm.lookup_func_entry(sym) {
        // Sincronizar cache stack → stack real antes de manipular flat_vars
        dbg.vm.flush_stack();

        let max_frames = dbg.vm.frame_buffer.len();
        if dbg.vm.frame_count >= max_frames {
            return Err(ErrFast::StackUnder(
                "Stack overflow: demasiadas llamadas anidadas".into(),
            ));
        }

        // Guardar frame actual
        let num_vars_actual = dbg.vm.flat_vars.len() - dbg.vm.base_ptr;
        dbg.vm.frame_buffer[dbg.vm.frame_count] = FrmFast {
            ip_ret: dbg.vm.ip, // ip ya incrementado por Call handler
            base_ptr_previo: dbg.vm.base_ptr,
            num_vars: num_vars_actual,
            func_version: entry.version,
        };
        dbg.vm.frame_count += 1;

        // Nuevo base_ptr al final del flat_vars actual
        dbg.vm.base_ptr = dbg.vm.flat_vars.len();

        // Pop args del stack de valores y revesarlos (orden normal)
        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
        for _ in 0..nargs {
            args.push(dbg.vm.pop_valor()?);
        }
        args.reverse();

        // Reservar espacio para todas las variables de la función
        let vars_size = entry.vars_size.max(nargs);
        dbg.vm
            .flat_vars
            .resize(dbg.vm.base_ptr + vars_size, ValorFast::nulo());

        // Copiar args a flat_vars en índices 0, 1, 2...
        for (i, arg) in args.into_iter().enumerate() {
            dbg.vm.flat_vars[dbg.vm.base_ptr + i] = arg;
        }

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
/// - truncate flat_vars a base_ptr (libera vars de la función que termina)
/// - restaura base_ptr desde el frame
/// - restaura ip desde el frame
fn ejecutar_return_debug(dbg: &mut Debugger) -> Result<(), ErrFast> {
    if dbg.vm.frame_count == 0 {
        return Err(ErrFast::StackUnder("Return sin frame activo".into()));
    }
    dbg.vm.frame_count -= 1;
    let frame = dbg.vm.frame_buffer[dbg.vm.frame_count];
    // Sincronizar cache antes de truncar flat_vars
    dbg.vm.flush_stack();
    // Liberar vars de la función que termina (O(1))
    dbg.vm.flat_vars.truncate(dbg.vm.base_ptr);
    dbg.vm.base_ptr = frame.base_ptr_previo;
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
    /// Programa terminado
    Terminado,
}
