// Forja Debug Adapter Protocol (DAP) Server
#![allow(dead_code, non_snake_case)]
// Implementa el Debug Adapter Protocol de VSCode sobre JSON-RPC stdin/stdout.
//
// Arquitectura:
//   VSCode Debug UI ←→ forja-dap.exe (stdin/stdout JSON-RPC) ←→ Debugger ←→ ForjaFast

use std::io::{self, BufRead, Read, Write};
use std::sync::mpsc;

// ─── Tipos DAP básicos (solo los que necesitamos) ──────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct Fuente {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ruta: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct DAPBreakpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<usize>,
    linea: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DAPPilaFrame {
    id: usize,
    name: String,
    linea: usize,
    column: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    fuente: Option<Fuente>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DAPScope {
    name: String,
    variablesReference: usize,
    expensive: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DAPVariable {
    name: String,
    valor: String,
    #[serde(rename = "type")]
    tipo: String,
    variablesReference: usize,
}

#[derive(serde::Serialize)]
struct Capabilities {
    supportsConfigurationDoneRequest: bool,
    supportsFunctionBreakpoints: bool,
    supportsConditionalBreakpoints: bool,
    supportsEvaluateForHovers: bool,
    supportTerminateDebuggee: bool,
    supportsSetVariable: bool,
}

// ─── Mensajes JSON-RPC ────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct RPCPeticion {
    #[serde(default)]
    seq: i64,
    command: String,
    #[serde(default)]
    arguments: serde_json::Value,
    #[serde(default)]
    r#type: String,
}

#[derive(serde::Serialize)]
struct RPCRespuesta {
    seq: i64,
    r#type: String,
    peticion_seq: i64,
    command: String,
    exito: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(serde::Serialize)]
struct RPCErrorRespuesta {
    seq: i64,
    r#type: String,
    peticion_seq: i64,
    command: String,
    exito: bool,
    message: String,
}

#[derive(serde::Serialize)]
struct RPCEvento {
    seq: i64,
    r#type: String,
    evento: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<serde_json::Value>,
}

// ─── Estado del servidor DAP ──────────────────────────────────────────

enum DepuracionCommand {
    Continue,
    Next,
    StepIn,
    StepOut,
    Pause,
    Disconnect,
}

struct DAPState {
    debugger: forja::debugger::Debugger,
    next_seq: i64,
    breakpoint_counter: usize,
    paused: bool,
    terminated: bool,
    cmd_rx: mpsc::Receiver<DepuracionCommand>,
    cmd_tx: mpsc::Sender<DepuracionCommand>,
    disconnect_requested: bool,
    fuente_archivo: Option<String>,
}

impl DAPState {
    fn nuevo() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        DAPState {
            debugger: forja::debugger::Debugger::nuevo(),
            next_seq: 1,
            breakpoint_counter: 0,
            paused: false,
            terminated: false,
            cmd_rx,
            cmd_tx,
            disconnect_requested: false,
            fuente_archivo: None,
        }
    }

    fn next_seq(&mut self) -> i64 {
        let s = self.next_seq;
        self.next_seq += 1;
        s
    }
}

// ─── Main ─────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .any(|a| a == "--version" || a == "-v" || a == "version")
    {
        println!("forja-dap v{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    forja::selfrun::shadow_copia();

    let mut state = DAPState::nuevo();
    let stdin = io::stdin();
    let _stdout = io::stdout();

    // Hilo para leer comandos del usuario (stdin DAP)
    let _cmd_tx = state.cmd_tx.clone();

    // Loop principal: recibe requests DAP y las procesa
    for linea in stdin.lock().lines() {
        let linea = match linea {
            Ok(l) => l,
            Err(_) => break,
        };

        // DAP usa Content-Length headers como LSP
        if linea.starts_with("Content-Length: ") {
            let longitud: usize = linea
                .strip_prefix("Content-Length: ")
                .unwrap_or_default()
                .trim()
                .parse()
                .unwrap_or(0);
            // Leer blank line
            let mut blank = String::new();
            stdin.lock().read_line(&mut blank).ok();
            // Leer contenido
            let mut content = vec![0u8; longitud];
            if io::stdin().read_exact(&mut content).is_err() {
                break;
            }
            let mensaje_texto = String::from_utf8_lossy(&content);
            if let Ok(req) = serde_json::from_str::<RPCPeticion>(&mensaje_texto) {
                let seq = req.seq;
                let cmd = req.command.clone();
                let args = req.arguments;

                match cmd.as_str() {
                    "initialize" => manejar_initialize(&mut state, seq),
                    "launch" => manejar_launch(&mut state, seq, args),
                    "setBreakpoints" => manejar_fijar_breakpoints(&mut state, seq, args),
                    "setExceptionBreakpoints" => {
                        manejar_fijar_exception_breakpoints(&mut state, seq)
                    }
                    "configurationDone" => manejar_configuration_done(&mut state, seq),
                    "continue" => manejar_continue(&mut state, seq),
                    "next" => manejar_siguiente(&mut state, seq),
                    "stepIn" => manejar_step_in(&mut state, seq),
                    "stepOut" => manejar_step_out(&mut state, seq),
                    "pause" => manejar_pause(&mut state, seq),
                    "stackTrace" => manejar_pila_traza(&mut state, seq, args),
                    "scopes" => manejar_scopes(&mut state, seq, args),
                    "variables" => manejar_variables(&mut state, seq, args),
                    "evaluate" => manejar_evaluate(&mut state, seq, args),
                    "disconnect" => manejar_disconnect(&mut state, seq),
                    _ => {
                        // Command not recognized - respond error
                        let resp = RPCErrorRespuesta {
                            seq: state.next_seq(),
                            r#type: "response".into(),
                            peticion_seq: seq,
                            command: cmd.clone(),
                            exito: false,
                            message: format!("comando '{}' no implementado", cmd),
                        };
                        send_json(&resp);
                    }
                }
            }
        }

        // Check if disconnected
        if state.disconnect_requested {
            break;
        }
    }
}

// ─── Handlers ─────────────────────────────────────────────────────────

fn manejar_initialize(state: &mut DAPState, seq: i64) {
    let caps = Capabilities {
        supportsConfigurationDoneRequest: true,
        supportsFunctionBreakpoints: false,
        supportsConditionalBreakpoints: true,
        supportsEvaluateForHovers: true,
        supportTerminateDebuggee: true,
        supportsSetVariable: false,
    };
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "initialize".into(),
        exito: true,
        body: Some(serde_json::to_value(&caps).unwrap_or_default()),
        message: None,
    };
    send_json(&resp);

    // Enviar InitializedEvent
    let evt = RPCEvento {
        seq: state.next_seq(),
        r#type: "event".into(),
        evento: "initialized".into(),
        body: None,
    };
    send_json(&evt);
}

fn manejar_launch(state: &mut DAPState, seq: i64, args: serde_json::Value) {
    // Extraer programa a ejecutar de args
    let mut program = String::new();
    let mut fuente_ruta = None;

    if let Some(prog) = args.get("program").and_then(|v| v.as_str()) {
        program = prog.to_string();
    }
    if let Some(src) = args.get("__sessionId").and_then(|v| v.as_str()) {
        fuente_ruta = Some(src.to_string());
    }

    // Si no hay program en args, intentar leer de source file
    if program.is_empty() {
        program = if let Some(ref ruta) = fuente_ruta {
            std::fs::read_to_string(ruta).unwrap_or_default()
        } else {
            String::new()
        };
    }

    // Compilar y cargar bytecode
    let bytecode = match forja::compilar_pipeline_completa(&program) {
        Ok((bc, _contratos)) => bc,
        Err(e) => {
            let resp = RPCErrorRespuesta {
                seq: state.next_seq(),
                r#type: "response".into(),
                peticion_seq: seq,
                command: "launch".into(),
                exito: false,
                message: format!("Error de compilación: {}", e),
            };
            send_json(&resp);
            return;
        }
    };

    // recuperar nombres reales de variables desde el AST para que el
    // panel de variables y la evaluación de expresiones muestren identificadores
    // legibles en lugar de `var_N`/`global_N`.
    if let Ok(programa) = forja::analizar(&program) {
        state.debugger.cargar_nombres_desde_ast(&programa);
    }

    state.debugger.cargar_bytecode(bytecode);
    state.fuente_archivo = fuente_ruta;

    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "launch".into(),
        exito: true,
        body: None,
        message: None,
    };
    send_json(&resp);
}

fn manejar_fijar_breakpoints(state: &mut DAPState, seq: i64, args: serde_json::Value) {
    // Extraer breakpoints del argumento
    let mut breakpoints_resp: Vec<DAPBreakpoint> = Vec::new();

    if let Some(bps) = args.get("breakpoints").and_then(|v| v.as_array()) {
        for bp in bps {
            if let Some(linea) = bp.get("line").and_then(|v| v.as_u64()) {
                let linea_usize = linea as usize;
                // soportar breakpoints condicionales. El campo `condition`
                // es opcional; si está presente y no vacío, el breakpoint solo
                // se activa cuando la expresión evalúa a verdadero.
                let condicion = bp
                    .get("condition")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                state
                    .debugger
                    .fijar_breakpoint_condicional(linea_usize, condicion);
                state.breakpoint_counter += 1;
                breakpoints_resp.push(DAPBreakpoint {
                    id: Some(state.breakpoint_counter),
                    linea: linea_usize,
                    verified: Some(true),
                    message: None,
                });
            }
        }
    }

    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "setBreakpoints".into(),
        exito: true,
        body: Some(serde_json::json!({
            "breakpoints": breakpoints_resp
        })),
        message: None,
    };
    send_json(&resp);
}

fn manejar_fijar_exception_breakpoints(state: &mut DAPState, seq: i64) {
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "setExceptionBreakpoints".into(),
        exito: true,
        body: None,
        message: None,
    };
    send_json(&resp);
}

fn manejar_configuration_done(state: &mut DAPState, seq: i64) {
    // Responder primero
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "configurationDone".into(),
        exito: true,
        body: None,
        message: None,
    };
    send_json(&resp);

    // Iniciar ejecución
    ejecutar_programa(state);
}

fn ejecutar_programa(state: &mut DAPState) {
    // El loop nunca iteraba más de una vez porque todas las ramas
    // hacen return. Se reemplaza por un bloque simple.
    {
        // Verificar comandos pendientes
        if let Ok(DepuracionCommand::Disconnect) = state.cmd_rx.try_recv() {
            state.disconnect_requested = true;
            return;
        }

        if state.debugger.vm.ip >= state.debugger.vm.bytecode.len() {
            // Programa esta_terminado
            let evt = RPCEvento {
                seq: state.next_seq(),
                r#type: "event".into(),
                evento: "terminated".into(),
                body: None,
            };
            send_json(&evt);
            state.terminated = true;
            return;
        }

        // Ejecutar un paso
        match state.debugger.ejecutar_hasta_evento() {
            Ok(evento) => match evento {
                forja::debugger::DepuracionEvento::Breakpoint { line } => {
                    let evt = RPCEvento {
                        seq: state.next_seq(),
                        r#type: "event".into(),
                        evento: "stopped".into(),
                        body: Some(serde_json::json!({
                            "reason": "breakpoint",
                            "threadId": 1,
                            "description": format!("Breakpoint en línea {}", linea),
                        })),
                    };
                    send_json(&evt); // Esperar otro comando del usuario
                }
                forja::debugger::DepuracionEvento::StepCompletado { line } => {
                    let evt = RPCEvento {
                        seq: state.next_seq(),
                        r#type: "event".into(),
                        evento: "stopped".into(),
                        body: Some(serde_json::json!({
                            "reason": "step",
                            "threadId": 1,
                            "description": format!("Pausado en línea {}", linea),
                        })),
                    };
                    send_json(&evt);
                }
                forja::debugger::DepuracionEvento::Pausado { line } => {
                    let evt = RPCEvento {
                        seq: state.next_seq(),
                        r#type: "event".into(),
                        evento: "stopped".into(),
                        body: Some(serde_json::json!({
                            "reason": "pause",
                            "threadId": 1,
                            "description": format!("Pausado en línea {}", linea),
                        })),
                    };
                    send_json(&evt);
                }
                forja::debugger::DepuracionEvento::Terminado => {
                    let evt = RPCEvento {
                        seq: state.next_seq(),
                        r#type: "event".into(),
                        evento: "terminated".into(),
                        body: None,
                    };
                    send_json(&evt);
                    state.terminated = true;
                }
            },
            Err(e) => {
                // Error de ejecución
                let evt = RPCEvento {
                    seq: state.next_seq(),
                    r#type: "event".into(),
                    evento: "stopped".into(),
                    body: Some(serde_json::json!({
                        "reason": "exception",
                        "threadId": 1,
                        "description": format!("Error: {}", e),
                    })),
                };
                send_json(&evt);
            }
        }
    }
}

fn manejar_continue(state: &mut DAPState, seq: i64) {
    // Responder inmediatamente
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "continue".into(),
        exito: true,
        body: Some(serde_json::json!({
            "allThreadsContinued": true
        })),
        message: None,
    };
    send_json(&resp);

    // Reanudar ejecución
    ejecutar_programa(state);
}

fn manejar_siguiente(state: &mut DAPState, seq: i64) {
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "next".into(),
        exito: true,
        body: None,
        message: None,
    };
    send_json(&resp);

    // Step over
    if state.debugger.step_over().is_err() {
        let evt = RPCEvento {
            seq: state.next_seq(),
            r#type: "event".into(),
            evento: "terminated".into(),
            body: None,
        };
        send_json(&evt);
        state.terminated = true;
        return;
    }

    // Enviar stopped event si step_over retornó inmediatamente
    let evt = RPCEvento {
        seq: state.next_seq(),
        r#type: "event".into(),
        evento: "stopped".into(),
        body: Some(serde_json::json!({
            "reason": "step",
            "threadId": 1,
            "description": format!("Step completado en línea {}", state.debugger.current_line),
        })),
    };
    send_json(&evt);
}

fn manejar_step_in(state: &mut DAPState, seq: i64) {
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "stepIn".into(),
        exito: true,
        body: None,
        message: None,
    };
    send_json(&resp);

    if state.debugger.step_into().is_err() {
        let evt = RPCEvento {
            seq: state.next_seq(),
            r#type: "event".into(),
            evento: "terminated".into(),
            body: None,
        };
        send_json(&evt);
        state.terminated = true;
        return;
    }

    let evt = RPCEvento {
        seq: state.next_seq(),
        r#type: "event".into(),
        evento: "stopped".into(),
        body: Some(serde_json::json!({
            "reason": "step",
            "threadId": 1,
            "description": format!("Step into completado en línea {}", state.debugger.current_line),
        })),
    };
    send_json(&evt);
}

fn manejar_step_out(state: &mut DAPState, seq: i64) {
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "stepOut".into(),
        exito: true,
        body: None,
        message: None,
    };
    send_json(&resp);

    if state.debugger.step_out().is_err() {
        let evt = RPCEvento {
            seq: state.next_seq(),
            r#type: "event".into(),
            evento: "terminated".into(),
            body: None,
        };
        send_json(&evt);
        state.terminated = true;
        return;
    }

    let evt = RPCEvento {
        seq: state.next_seq(),
        r#type: "event".into(),
        evento: "stopped".into(),
        body: Some(serde_json::json!({
            "reason": "step",
            "threadId": 1,
            "description": format!("Step out completado en línea {}", state.debugger.current_line),
        })),
    };
    send_json(&evt);
}

fn manejar_pause(state: &mut DAPState, seq: i64) {
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "pause".into(),
        exito: true,
        body: None,
        message: None,
    };
    send_json(&resp);
}

fn manejar_pila_traza(state: &mut DAPState, seq: i64, _args: serde_json::Value) {
    let frames = state.debugger.obtener_pila_traza();
    let pila_frames: Vec<DAPPilaFrame> = frames
        .iter()
        .map(|f| DAPPilaFrame {
            id: f.id,
            name: f.name.clone(),
            linea: f.linea,
            column: 1,
            fuente: state.fuente_archivo.as_ref().map(|ruta| Fuente {
                name: Some(ruta.rsplit('/').next().unwrap_or(ruta).to_string()),
                ruta: Some(ruta.clone()),
            }),
        })
        .collect();

    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "stackTrace".into(),
        exito: true,
        body: Some(serde_json::json!({
            "stackFrames": pila_frames,
            "totalFrames": pila_frames.len(),
        })),
        message: None,
    };
    send_json(&resp);
}

fn manejar_scopes(state: &mut DAPState, seq: i64, _args: serde_json::Value) {
    let scopes = vec![
        DAPScope {
            name: "Locales".into(),
            variablesReference: 1,
            expensive: false,
        },
        DAPScope {
            name: "Globales".into(),
            variablesReference: 2,
            expensive: true,
        },
    ];

    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "scopes".into(),
        exito: true,
        body: Some(serde_json::json!({
            "scopes": scopes
        })),
        message: None,
    };
    send_json(&resp);
}

fn manejar_variables(state: &mut DAPState, seq: i64, args: serde_json::Value) {
    let vars_ref = args
        .get("variablesReference")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let variables: Vec<DAPVariable> = if vars_ref == 1 {
        // Variables locales del frame actual
        let frames = state.debugger.obtener_pila_traza();
        frames
            .first()
            .map(|f| {
                f.vars
                    .iter()
                    .map(|v| DAPVariable {
                        name: v.name.clone(),
                        valor: v.valor.clone(),
                        tipo: v.tipo.clone(),
                        variablesReference: 0,
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else if vars_ref == 2 {
        // Variables globales
        state
            .debugger
            .obtener_variables_globales()
            .iter()
            .map(|v| DAPVariable {
                name: v.name.clone(),
                valor: v.valor.clone(),
                tipo: v.tipo.clone(),
                variablesReference: 0,
            })
            .collect()
    } else {
        Vec::new()
    };

    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "variables".into(),
        exito: true,
        body: Some(serde_json::json!({
            "variables": variables
        })),
        message: None,
    };
    send_json(&resp);
}

fn manejar_evaluate(state: &mut DAPState, seq: i64, args: serde_json::Value) {
    let expr = args
        .get("expression")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let resultado = state.debugger.evaluar(expr);

    match resultado {
        Ok(var) => {
            let resp = RPCRespuesta {
                seq: state.next_seq(),
                r#type: "response".into(),
                peticion_seq: seq,
                command: "evaluate".into(),
                exito: true,
                body: Some(serde_json::json!({
                    "result": var.valor,
                    "type": var.tipo,
                    "variablesReference": 0,
                })),
                message: None,
            };
            send_json(&resp);
        }
        Err(e) => {
            let resp = RPCRespuesta {
                seq: state.next_seq(),
                r#type: "response".into(),
                peticion_seq: seq,
                command: "evaluate".into(),
                exito: false,
                message: Some(e),
                body: None,
            };
            send_json(&resp);
        }
    }
}

fn manejar_disconnect(state: &mut DAPState, seq: i64) {
    let resp = RPCRespuesta {
        seq: state.next_seq(),
        r#type: "response".into(),
        peticion_seq: seq,
        command: "disconnect".into(),
        exito: true,
        body: None,
        message: None,
    };
    send_json(&resp);
    state.disconnect_requested = true;
}

// ─── Utilidad de envío JSON-RPC (stdout) ──────────────────────────────

fn send_json<T: serde::Serialize>(val: &T) {
    let json_str = serde_json::to_string(val).unwrap_or_default();
    let mut stdout = io::stdout();
    let _ = writeln!(
        stdout,
        "Content-Length: {}\r\n\r\n{}",
        json_str.len(),
        json_str
    );
    let _ = stdout.flush();
}
