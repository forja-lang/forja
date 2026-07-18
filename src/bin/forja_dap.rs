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
struct Source {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct DAPBreakpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<usize>,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DAPStackFrame {
    id: usize,
    name: String,
    line: usize,
    column: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<Source>,
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
    value: String,
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
struct RPCRequest {
    #[serde(default)]
    seq: i64,
    command: String,
    #[serde(default)]
    arguments: serde_json::Value,
    #[serde(default)]
    r#type: String,
}

#[derive(serde::Serialize)]
struct RPCResponse {
    seq: i64,
    r#type: String,
    request_seq: i64,
    command: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(serde::Serialize)]
struct RPCErrorResponse {
    seq: i64,
    r#type: String,
    request_seq: i64,
    command: String,
    success: bool,
    message: String,
}

#[derive(serde::Serialize)]
struct RPCEvent {
    seq: i64,
    r#type: String,
    event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<serde_json::Value>,
}

// ─── Estado del servidor DAP ──────────────────────────────────────────

enum DebugCommand {
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
    cmd_rx: mpsc::Receiver<DebugCommand>,
    cmd_tx: mpsc::Sender<DebugCommand>,
    disconnect_requested: bool,
    source_file: Option<String>,
}

impl DAPState {
    fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        DAPState {
            debugger: forja::debugger::Debugger::new(),
            next_seq: 1,
            breakpoint_counter: 0,
            paused: false,
            terminated: false,
            cmd_rx,
            cmd_tx,
            disconnect_requested: false,
            source_file: None,
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

    forja::selfrun::shadow_copy();

    let mut state = DAPState::new();
    let stdin = io::stdin();
    let _stdout = io::stdout();

    // Hilo para leer comandos del usuario (stdin DAP)
    let _cmd_tx = state.cmd_tx.clone();

    // Loop principal: recibe requests DAP y las procesa
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        // DAP usa Content-Length headers como LSP
        if line.starts_with("Content-Length: ") {
            let len: usize = line["Content-Length: ".len()..].trim().parse().unwrap_or(0);
            // Leer blank line
            let mut blank = String::new();
            stdin.lock().read_line(&mut blank).ok();
            // Leer contenido
            let mut content = vec![0u8; len];
            if let Err(_) = io::stdin().read_exact(&mut content) {
                break;
            }
            let msg_str = String::from_utf8_lossy(&content);
            if let Ok(req) = serde_json::from_str::<RPCRequest>(&msg_str) {
                let seq = req.seq;
                let cmd = req.command.clone();
                let args = req.arguments;

                match cmd.as_str() {
                    "initialize" => handle_initialize(&mut state, seq),
                    "launch" => handle_launch(&mut state, seq, args),
                    "setBreakpoints" => handle_set_breakpoints(&mut state, seq, args),
                    "setExceptionBreakpoints" => handle_set_exception_breakpoints(&mut state, seq),
                    "configurationDone" => handle_configuration_done(&mut state, seq),
                    "continue" => handle_continue(&mut state, seq),
                    "next" => handle_next(&mut state, seq),
                    "stepIn" => handle_step_in(&mut state, seq),
                    "stepOut" => handle_step_out(&mut state, seq),
                    "pause" => handle_pause(&mut state, seq),
                    "stackTrace" => handle_stack_trace(&mut state, seq, args),
                    "scopes" => handle_scopes(&mut state, seq, args),
                    "variables" => handle_variables(&mut state, seq, args),
                    "evaluate" => handle_evaluate(&mut state, seq, args),
                    "disconnect" => handle_disconnect(&mut state, seq),
                    _ => {
                        // Command not recognized - respond error
                        let resp = RPCErrorResponse {
                            seq: state.next_seq(),
                            r#type: "response".into(),
                            request_seq: seq,
                            command: cmd.clone(),
                            success: false,
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

fn handle_initialize(state: &mut DAPState, seq: i64) {
    let caps = Capabilities {
        supportsConfigurationDoneRequest: true,
        supportsFunctionBreakpoints: false,
        supportsConditionalBreakpoints: false,
        supportsEvaluateForHovers: true,
        supportTerminateDebuggee: true,
        supportsSetVariable: false,
    };
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "initialize".into(),
        success: true,
        body: Some(serde_json::to_value(&caps).unwrap_or_default()),
        message: None,
    };
    send_json(&resp);

    // Enviar InitializedEvent
    let evt = RPCEvent {
        seq: state.next_seq(),
        r#type: "event".into(),
        event: "initialized".into(),
        body: None,
    };
    send_json(&evt);
}

fn handle_launch(state: &mut DAPState, seq: i64, args: serde_json::Value) {
    // Extraer programa a ejecutar de args
    let mut program = String::new();
    let mut source_path = None;

    if let Some(prog) = args.get("program").and_then(|v| v.as_str()) {
        program = prog.to_string();
    }
    if let Some(src) = args.get("__sessionId").and_then(|v| v.as_str()) {
        source_path = Some(src.to_string());
    }

    // Si no hay program en args, intentar leer de source file
    if program.is_empty() {
        program = if let Some(ref path) = source_path {
            std::fs::read_to_string(path).unwrap_or_default()
        } else {
            String::new()
        };
    }

    // Compilar y cargar bytecode
    let bytecode = match forja::compilar_pipeline_completa(&program) {
        Ok((bc, _contratos)) => bc,
        Err(e) => {
            let resp = RPCErrorResponse {
                seq: state.next_seq(),
                r#type: "response".into(),
                request_seq: seq,
                command: "launch".into(),
                success: false,
                message: format!("Error de compilación: {}", e),
            };
            send_json(&resp);
            return;
        }
    };

    state.debugger.cargar_bytecode(bytecode);
    state.source_file = source_path;

    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "launch".into(),
        success: true,
        body: None,
        message: None,
    };
    send_json(&resp);
}

fn handle_set_breakpoints(state: &mut DAPState, seq: i64, args: serde_json::Value) {
    // Extraer breakpoints del argumento
    let mut breakpoints_resp: Vec<DAPBreakpoint> = Vec::new();

    if let Some(bps) = args.get("breakpoints").and_then(|v| v.as_array()) {
        for bp in bps {
            if let Some(line) = bp.get("line").and_then(|v| v.as_u64()) {
                let line_usize = line as usize;
                state.debugger.set_breakpoint(line_usize);
                state.breakpoint_counter += 1;
                breakpoints_resp.push(DAPBreakpoint {
                    id: Some(state.breakpoint_counter),
                    line: line_usize,
                    verified: Some(true),
                    message: None,
                });
            }
        }
    }

    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "setBreakpoints".into(),
        success: true,
        body: Some(serde_json::json!({
            "breakpoints": breakpoints_resp
        })),
        message: None,
    };
    send_json(&resp);
}

fn handle_set_exception_breakpoints(state: &mut DAPState, seq: i64) {
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "setExceptionBreakpoints".into(),
        success: true,
        body: None,
        message: None,
    };
    send_json(&resp);
}

fn handle_configuration_done(state: &mut DAPState, seq: i64) {
    // Responder primero
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "configurationDone".into(),
        success: true,
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
        if let Ok(cmd) = state.cmd_rx.try_recv() {
            match cmd {
                DebugCommand::Disconnect => {
                    state.disconnect_requested = true;
                    return;
                }
                _ => {}
            }
        }

        if state.debugger.vm.ip >= state.debugger.vm.bytecode.len() {
            // Programa terminado
            let evt = RPCEvent {
                seq: state.next_seq(),
                r#type: "event".into(),
                event: "terminated".into(),
                body: None,
            };
            send_json(&evt);
            state.terminated = true;
            return;
        }

        // Ejecutar un paso
        match state.debugger.ejecutar_hasta_evento() {
            Ok(event) => match event {
                forja::debugger::DebugEvent::Breakpoint { line } => {
                    let evt = RPCEvent {
                        seq: state.next_seq(),
                        r#type: "event".into(),
                        event: "stopped".into(),
                        body: Some(serde_json::json!({
                            "reason": "breakpoint",
                            "threadId": 1,
                            "description": format!("Breakpoint en línea {}", line),
                        })),
                    };
                    send_json(&evt);
                    return; // Esperar otro comando del usuario
                }
                forja::debugger::DebugEvent::StepCompletado { line } => {
                    let evt = RPCEvent {
                        seq: state.next_seq(),
                        r#type: "event".into(),
                        event: "stopped".into(),
                        body: Some(serde_json::json!({
                            "reason": "step",
                            "threadId": 1,
                            "description": format!("Pausado en línea {}", line),
                        })),
                    };
                    send_json(&evt);
                    return;
                }
                forja::debugger::DebugEvent::Pausado { line } => {
                    let evt = RPCEvent {
                        seq: state.next_seq(),
                        r#type: "event".into(),
                        event: "stopped".into(),
                        body: Some(serde_json::json!({
                            "reason": "pause",
                            "threadId": 1,
                            "description": format!("Pausado en línea {}", line),
                        })),
                    };
                    send_json(&evt);
                    return;
                }
                forja::debugger::DebugEvent::Terminado => {
                    let evt = RPCEvent {
                        seq: state.next_seq(),
                        r#type: "event".into(),
                        event: "terminated".into(),
                        body: None,
                    };
                    send_json(&evt);
                    state.terminated = true;
                    return;
                }
            },
            Err(e) => {
                // Error de ejecución
                let evt = RPCEvent {
                    seq: state.next_seq(),
                    r#type: "event".into(),
                    event: "stopped".into(),
                    body: Some(serde_json::json!({
                        "reason": "exception",
                        "threadId": 1,
                        "description": format!("Error: {}", e),
                    })),
                };
                send_json(&evt);
                return;
            }
        }
    }
}

fn handle_continue(state: &mut DAPState, seq: i64) {
    // Responder inmediatamente
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "continue".into(),
        success: true,
        body: Some(serde_json::json!({
            "allThreadsContinued": true
        })),
        message: None,
    };
    send_json(&resp);

    // Reanudar ejecución
    ejecutar_programa(state);
}

fn handle_next(state: &mut DAPState, seq: i64) {
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "next".into(),
        success: true,
        body: None,
        message: None,
    };
    send_json(&resp);

    // Step over
    if let Err(_) = state.debugger.step_over() {
        let evt = RPCEvent {
            seq: state.next_seq(),
            r#type: "event".into(),
            event: "terminated".into(),
            body: None,
        };
        send_json(&evt);
        state.terminated = true;
        return;
    }

    // Enviar stopped event si step_over retornó inmediatamente
    let evt = RPCEvent {
        seq: state.next_seq(),
        r#type: "event".into(),
        event: "stopped".into(),
        body: Some(serde_json::json!({
            "reason": "step",
            "threadId": 1,
            "description": format!("Step completado en línea {}", state.debugger.current_line),
        })),
    };
    send_json(&evt);
}

fn handle_step_in(state: &mut DAPState, seq: i64) {
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "stepIn".into(),
        success: true,
        body: None,
        message: None,
    };
    send_json(&resp);

    if let Err(_) = state.debugger.step_into() {
        let evt = RPCEvent {
            seq: state.next_seq(),
            r#type: "event".into(),
            event: "terminated".into(),
            body: None,
        };
        send_json(&evt);
        state.terminated = true;
        return;
    }

    let evt = RPCEvent {
        seq: state.next_seq(),
        r#type: "event".into(),
        event: "stopped".into(),
        body: Some(serde_json::json!({
            "reason": "step",
            "threadId": 1,
            "description": format!("Step into completado en línea {}", state.debugger.current_line),
        })),
    };
    send_json(&evt);
}

fn handle_step_out(state: &mut DAPState, seq: i64) {
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "stepOut".into(),
        success: true,
        body: None,
        message: None,
    };
    send_json(&resp);

    if let Err(_) = state.debugger.step_out() {
        let evt = RPCEvent {
            seq: state.next_seq(),
            r#type: "event".into(),
            event: "terminated".into(),
            body: None,
        };
        send_json(&evt);
        state.terminated = true;
        return;
    }

    let evt = RPCEvent {
        seq: state.next_seq(),
        r#type: "event".into(),
        event: "stopped".into(),
        body: Some(serde_json::json!({
            "reason": "step",
            "threadId": 1,
            "description": format!("Step out completado en línea {}", state.debugger.current_line),
        })),
    };
    send_json(&evt);
}

fn handle_pause(state: &mut DAPState, seq: i64) {
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "pause".into(),
        success: true,
        body: None,
        message: None,
    };
    send_json(&resp);
}

fn handle_stack_trace(state: &mut DAPState, seq: i64, _args: serde_json::Value) {
    let frames = state.debugger.get_stack_trace();
    let stack_frames: Vec<DAPStackFrame> = frames
        .iter()
        .map(|f| DAPStackFrame {
            id: f.id,
            name: f.name.clone(),
            line: f.line,
            column: 1,
            source: state.source_file.as_ref().map(|path| Source {
                name: Some(path.rsplit('/').next().unwrap_or(path).to_string()),
                path: Some(path.clone()),
            }),
        })
        .collect();

    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "stackTrace".into(),
        success: true,
        body: Some(serde_json::json!({
            "stackFrames": stack_frames,
            "totalFrames": stack_frames.len(),
        })),
        message: None,
    };
    send_json(&resp);
}

fn handle_scopes(state: &mut DAPState, seq: i64, _args: serde_json::Value) {
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

    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "scopes".into(),
        success: true,
        body: Some(serde_json::json!({
            "scopes": scopes
        })),
        message: None,
    };
    send_json(&resp);
}

fn handle_variables(state: &mut DAPState, seq: i64, args: serde_json::Value) {
    let vars_ref = args
        .get("variablesReference")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let variables: Vec<DAPVariable> = if vars_ref == 1 {
        // Variables locales del frame actual
        let frames = state.debugger.get_stack_trace();
        frames
            .first()
            .map(|f| {
                f.vars
                    .iter()
                    .map(|v| DAPVariable {
                        name: v.name.clone(),
                        value: v.value.clone(),
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
                value: v.value.clone(),
                tipo: v.tipo.clone(),
                variablesReference: 0,
            })
            .collect()
    } else {
        Vec::new()
    };

    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "variables".into(),
        success: true,
        body: Some(serde_json::json!({
            "variables": variables
        })),
        message: None,
    };
    send_json(&resp);
}

fn handle_evaluate(state: &mut DAPState, seq: i64, args: serde_json::Value) {
    let expr = args
        .get("expression")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let result = state.debugger.evaluar(expr);

    match result {
        Ok(var) => {
            let resp = RPCResponse {
                seq: state.next_seq(),
                r#type: "response".into(),
                request_seq: seq,
                command: "evaluate".into(),
                success: true,
                body: Some(serde_json::json!({
                    "result": var.value,
                    "type": var.tipo,
                    "variablesReference": 0,
                })),
                message: None,
            };
            send_json(&resp);
        }
        Err(e) => {
            let resp = RPCResponse {
                seq: state.next_seq(),
                r#type: "response".into(),
                request_seq: seq,
                command: "evaluate".into(),
                success: false,
                message: Some(e),
                body: None,
            };
            send_json(&resp);
        }
    }
}

fn handle_disconnect(state: &mut DAPState, seq: i64) {
    let resp = RPCResponse {
        seq: state.next_seq(),
        r#type: "response".into(),
        request_seq: seq,
        command: "disconnect".into(),
        success: true,
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
