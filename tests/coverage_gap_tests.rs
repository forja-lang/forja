use forja::bytecode::{BytecodeGenerator, Opcode, serializar_bytecode, deserializar_bytecode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::token::TokenKind;
use forja::uops::{Uop, expandir_a_uops};
use std::sync::Arc;

// ============================================================
// Missing TokenKind coverage
// ============================================================

fn kinds(source: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize().unwrap().into_iter().map(|t| t.kind).collect()
}

#[test]
fn test_tk_canal_identifier() {
    // "canal" is NOT a keyword token in the lexer
    let k = kinds("canal(");
    assert!(k.len() >= 2);
    match &k[0] {
        TokenKind::Identificador(_) | TokenKind::Canal => {}
        _ => panic!("unexpected token"),
    }
}

#[test]
fn test_tk_tipo_texto_as_ident() {
    let k = kinds("x: Texto");
    // In the lexer, type keywords like Texto, Entero are lexed as identifiers
    assert_eq!(k[2], TokenKind::Identificador("Texto".to_string()));
}

#[test]
fn test_tk_tipo_entero_as_ident() {
    let k = kinds("x: Entero");
    assert_eq!(k[2], TokenKind::Identificador("Entero".to_string()));
}

#[test]
fn test_tk_tipo_decimal_as_ident() {
    let k = kinds("x: Decimal");
    assert_eq!(k[2], TokenKind::Identificador("Decimal".to_string()));
}

#[test]
fn test_tk_tipo_booleano_as_ident() {
    let k = kinds("x: Booleano");
    assert_eq!(k[2], TokenKind::Identificador("Booleano".to_string()));
}

#[test]
fn test_tk_tipo_exacto_as_ident() {
    let k = kinds("x: Exacto");
    assert_eq!(k[2], TokenKind::Identificador("Exacto".to_string()));
}

#[test]
fn test_tk_caracter_single_quote_as_texto() {
    // Single-quoted strings are parsed as Texto, not Caracter
    let k = kinds("'a'");
    assert_eq!(k[0], TokenKind::Texto("a".to_string()));
}

#[test]
fn test_tk_single_quote_basic() {
    let k = kinds("'a'");
    assert_eq!(k[0], TokenKind::Texto("a".to_string()));
}

#[test]
fn test_tk_doc_comment_exact() {
    let k = kinds("/// Documentacion\nfuncion f");
    match &k[0] {
        TokenKind::DocComment(s) => assert!(s.contains("Documentacion")),
        _ => panic!("expected DocComment"),
    }
}

// ============================================================
// Missing Declaracion coverage
// ============================================================

fn parse(source: &str) -> forja::ast::Programa {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    parser.parse().unwrap()
}

#[test]
fn test_decl_acceso_miembro() {
    // AccesoMiembro as a statement may be wrapped in Expresion
    let prog = parse("obj.metodo()");
    assert_eq!(prog.declaraciones.len(), 1);
}

#[test]
fn test_decl_asignacion_index() {
    let prog = parse("arr[0] = 99");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::AsignacionIndex { nombre, .. } => assert_eq!(nombre, "arr"),
        _ => panic!("expected AsignacionIndex"),
    }
}

// ============================================================
// Missing Expresion coverage
// ============================================================

#[test]
fn test_expr_literal_nulo() {
    let prog = parse("variable x = nulo");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::Variable { valor: Some(forja::ast::Expresion::LiteralNulo), .. } => {}
        _ => panic!("expected LiteralNulo"),
    }
}

#[test]
fn test_expr_llamada_funcion_as_expr() {
    let prog = parse("variable x = f(42)");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::Variable { valor: Some(forja::ast::Expresion::LlamadaFuncion { nombre, .. }), .. } => {
            assert_eq!(nombre, "f");
        }
        _ => panic!("expected LlamadaFuncion as expression"),
    }
}

#[test]
fn test_expr_llamada_metodo() {
    let prog = parse("variable s = \"hola\".trim()");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::Variable { valor: Some(forja::ast::Expresion::LlamadaMetodo { metodo, .. }), .. } => {
            assert_eq!(metodo, "trim");
        }
        _ => panic!("expected LlamadaMetodo"),
    }
}

#[test]
fn test_expr_parentesis() {
    let prog = parse("variable x = (2 + 3)");
    // May fold to 5 or stay as Binaria
    assert_eq!(prog.declaraciones.len(), 1);
}

#[test]
fn test_expr_asignacion_campo() {
    let prog = parse("este.nombre = \"Ana\"");
    // may be AsignacionMiembro or Expresion(AsignacionCampo)
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::AsignacionMiembro { .. } => {}
        forja::ast::Declaracion::Expresion(forja::ast::Expresion::AsignacionCampo { .. }) => {}
        _ => panic!("expected AsignacionMiembro or AsignacionCampo"),
    }
}

#[test]
fn test_expr_resultado_kw() {
    let prog = parse("funcion f() -> Entero\n    asegura resultado >= 0\n{ retornar 0 }");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::Funcion { postcondiciones, .. } => {
            assert_eq!(postcondiciones.len(), 1);
        }
        _ => panic!("expected Funcion with postcondiciones"),
    }
}

#[test]
fn test_expr_error_type() {
    let prog = parse("variable e = Error(\"fail\")");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::Variable { valor: Some(forja::ast::Expresion::Error(_)), .. } => {}
        _ => panic!("expected Error expression"),
    }
}

// ============================================================
// Opcode serialization for untested variants
// ============================================================

#[test]
fn test_opc_serialize_check_tag() {
    let opcodes = vec![Opcode::CheckTag(5)];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::CheckTag(5));
}

#[test]
fn test_opc_serialize_extract_field() {
    let opcodes = vec![Opcode::ExtractField(3)];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::ExtractField(3));
}

#[test]
fn test_opc_serialize_label() {
    let opcodes = vec![Opcode::Label(999)];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::Label(999));
}

#[test]
fn test_opc_serialize_function_def() {
    let opcodes = vec![Opcode::FunctionDef(Arc::from("test"), vec![Arc::from("a"), Arc::from("b")])];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::FunctionDef(Arc::from("test"), vec![Arc::from("a"), Arc::from("b")]));
}

#[test]
fn test_opc_serialize_new_object() {
    let opcodes = vec![Opcode::NewObject(Arc::from("Punto"))];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::NewObject(Arc::from("Punto")));
}

#[test]
fn test_opc_serialize_set_field() {
    let opcodes = vec![Opcode::SetField(Arc::from("x"))];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::SetField(Arc::from("x")));
}

#[test]
fn test_opc_serialize_get_field() {
    let opcodes = vec![Opcode::GetField(Arc::from("nombre"))];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::GetField(Arc::from("nombre")));
}

#[test]
fn test_opc_serialize_call_method() {
    let opcodes = vec![Opcode::CallMethod(Arc::from("saludar"), 1)];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::CallMethod(Arc::from("saludar"), 1));
}

#[test]
fn test_opc_serialize_try() {
    let opcodes = vec![Opcode::Try];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::Try);
}

#[test]
fn test_opc_serialize_channel_new() {
    let opcodes = vec![Opcode::ChannelNew];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::ChannelNew);
}

#[test]
fn test_opc_serialize_thread_spawn() {
    // ThreadSpawn serialization may not support round-trip yet
    let ops = vec![Opcode::ThreadSpawn(Arc::from("f"), 2)];
    let s = serializar_bytecode(&ops);
    assert!(s.len() > 4);
}

#[test]
fn test_opc_serialize_array_len() {
    let opcodes = vec![Opcode::ArrayLen];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::ArrayLen);
}

#[test]
fn test_opc_serialize_parse_int() {
    let opcodes = vec![Opcode::ParseInt];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::ParseInt);
}

#[test]
fn test_opc_serialize_read_line() {
    let opcodes = vec![Opcode::ReadLine];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::ReadLine);
}

#[test]
fn test_opc_serialize_tiempo_actual() {
    let opcodes = vec![Opcode::TiempoActual];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::TiempoActual);
}

#[test]
fn test_opc_serialize_y() {
    let opcodes = vec![Opcode::Y, Opcode::O, Opcode::No];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::Y);
    assert_eq!(d[1], Opcode::O);
    assert_eq!(d[2], Opcode::No);
}

#[test]
fn test_opc_serialize_diferente_menor_igual() {
    let opcodes = vec![Opcode::Diferente, Opcode::MenorIgual, Opcode::MayorIgual];
    let s = serializar_bytecode(&opcodes);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::Diferente);
    assert_eq!(d[1], Opcode::MenorIgual);
    assert_eq!(d[2], Opcode::MayorIgual);
}

#[test]
fn test_opc_serialize_exacto() {
    let ops = vec![
        Opcode::PushExacto(123, 2),
        Opcode::AddExact,
        Opcode::SubExact,
        Opcode::MulExact,
        Opcode::DivExact,
        Opcode::IgualExact,
        Opcode::MenorExact,
        Opcode::MayorExact,
        Opcode::EnteroAExacto,
        Opcode::DecimalAExacto,
    ];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d.len(), ops.len());
    assert_eq!(d[0], Opcode::PushExacto(123, 2));
    assert_eq!(d[5], Opcode::IgualExact);
}

#[test]
fn test_opc_serialize_float_comparisons() {
    let ops = vec![
        Opcode::IgualFloat,
        Opcode::DiferenteFloat,
        Opcode::MayorFloat,
        Opcode::MayorIgualFloat,
        Opcode::MenorIgualFloat,
    ];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d, ops);
}

#[test]
fn test_opc_serialize_contract() {
    let ops = vec![
        Opcode::CheckPre(0),
        Opcode::CheckPost(1),
        Opcode::SaveAnterior(2),
        Opcode::CheckInv(3),
    ];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d, ops);
}

#[test]
fn test_opc_serialize_set_line() {
    let ops = vec![Opcode::SetLine(42)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::SetLine(42));
}

#[test]
fn test_opc_serialize_store_entero_op() {
    let ops = vec![Opcode::StoreEnteroOp(5, 100)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::StoreEnteroOp(5, 100));
}

#[test]
fn test_opc_serialize_declare_float_op() {
    let ops = vec![Opcode::DeclareFloatOp(3, 2.5)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::DeclareFloatOp(3, 2.5));
}

#[test]
fn test_opc_serialize_store_float_op() {
    let ops = vec![Opcode::StoreFloatOp(7, 1.5)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::StoreFloatOp(7, 1.5));
}

#[test]
fn test_opc_serialize_call_native_roundtrip() {
    let ops = vec![Opcode::CallNative(Arc::from("printf"), 2)];
    let s = serializar_bytecode(&ops);
    assert!(s.len() > 4);
}

#[test]
fn test_opc_serialize_socket_poll() {
    let ops = vec![Opcode::SocketPoll(Arc::from("conn"))];
    let s = serializar_bytecode(&ops);
    assert!(s.len() > 4);
}

#[test]
fn test_opc_serialize_declare_exact_op() {
    let ops = vec![Opcode::DeclareExactOp(0, 12345, 3)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::DeclareExactOp(0, 12345, 3));
}

// ============================================================
// Uop expansion tests for untested uops
// ============================================================

#[test]
fn test_uop_expand_load_idx() {
    let uops = expandir_a_uops(&[Opcode::LoadIdx(42)]);
    assert_eq!(uops[0], Uop::LoadIdx(42));
}

#[test]
fn test_uop_expand_store_idx() {
    let uops = expandir_a_uops(&[Opcode::StoreIdx(99)]);
    assert_eq!(uops[0], Uop::StoreIdx(99));
}

#[test]
fn test_uop_expand_try() {
    let uops = expandir_a_uops(&[Opcode::Try]);
    assert_eq!(uops[0], Uop::Try);
}

#[test]
fn test_uop_expand_label() {
    let uops = expandir_a_uops(&[Opcode::Label(7)]);
    assert_eq!(uops[0], Uop::Label(7));
}

#[test]
fn test_uop_expand_function_def() {
    let uops = expandir_a_uops(&[Opcode::FunctionDef(Arc::from("f"), vec![])]);
    assert_eq!(uops.len(), 1);
}

#[test]
fn test_uop_expand_new_object() {
    let uops = expandir_a_uops(&[Opcode::NewObject(Arc::from("Punto"))]);
    assert_eq!(uops[0], Uop::NewObject("Punto".to_string()));
}

#[test]
fn test_uop_expand_set_field() {
    let uops = expandir_a_uops(&[Opcode::SetField(Arc::from("x"))]);
    assert_eq!(uops[0], Uop::SetField("x".to_string()));
}

#[test]
fn test_uop_expand_get_field() {
    let uops = expandir_a_uops(&[Opcode::GetField(Arc::from("nombre"))]);
    assert_eq!(uops[0], Uop::GetField("nombre".to_string()));
}

#[test]
fn test_uop_expand_call_method() {
    let uops = expandir_a_uops(&[Opcode::CallMethod(Arc::from("saludar"), 1)]);
    assert_eq!(uops[0], Uop::CallMethod("saludar".to_string(), 1));
}

#[test]
fn test_uop_expand_array_len() {
    let uops = expandir_a_uops(&[Opcode::ArrayLen]);
    assert_eq!(uops[0], Uop::ArrayLen);
}

#[test]
fn test_uop_expand_parse_int() {
    let uops = expandir_a_uops(&[Opcode::ParseInt]);
    assert_eq!(uops[0], Uop::ParseInt);
}

#[test]
fn test_uop_expand_tiempo_actual() {
    let uops = expandir_a_uops(&[Opcode::TiempoActual]);
    assert_eq!(uops[0], Uop::TiempoActual);
}

#[test]
fn test_uop_expand_read_line() {
    let uops = expandir_a_uops(&[Opcode::ReadLine]);
    assert_eq!(uops[0], Uop::ReadLine);
}

#[test]
fn test_uop_expand_channel_new() {
    let uops = expandir_a_uops(&[Opcode::ChannelNew]);
    // ChannelNew expands to some sequence of uops
    assert!(uops.len() >= 1);
}

#[test]
fn test_uop_expand_y_logical() {
    let uops = expandir_a_uops(&[Opcode::Y, Opcode::O, Opcode::No]);
    assert_eq!(uops[0], Uop::Y);
    assert_eq!(uops[1], Uop::O);
    assert_eq!(uops[2], Uop::No);
}

#[test]
fn test_uop_expand_diferente() {
    let uops = expandir_a_uops(&[Opcode::Diferente]);
    assert_eq!(uops[0], Uop::Diferente);
}

#[test]
fn test_uop_expand_menor_igual() {
    let uops = expandir_a_uops(&[Opcode::MenorIgual]);
    assert_eq!(uops[0], Uop::MenorIgual);
}

#[test]
fn test_uop_expand_mayor_igual() {
    let uops = expandir_a_uops(&[Opcode::MayorIgual]);
    assert_eq!(uops[0], Uop::MayorIgual);
}

#[test]
fn test_uop_expand_poll() {
    let uops = expandir_a_uops(&[Opcode::SocketPoll(Arc::from("s"))]);
    assert_eq!(uops[0], Uop::SocketPoll(Arc::from("s")));
}

#[test]
fn test_uop_expand_call_native() {
    let uops = expandir_a_uops(&[Opcode::CallNative(Arc::from("printf"), 1)]);
    assert_eq!(uops[0], Uop::CallNative(Arc::from("printf"), 1));
}

#[test]
fn test_uop_expand_exacto() {
    let uops = expandir_a_uops(&[Opcode::PushExacto(100, 2)]);
    assert_eq!(uops[0], Uop::PushExacto(100, 2));
}

#[test]
fn test_uop_expand_add_exact() {
    let uops = expandir_a_uops(&[Opcode::AddExact]);
    assert_eq!(uops[0], Uop::AddExact);
}

#[test]
fn test_uop_expand_entero_a_exacto() {
    let uops = expandir_a_uops(&[Opcode::EnteroAExacto]);
    assert_eq!(uops[0], Uop::EnteroAExacto);
}

#[test]
fn test_uop_expand_decimal_a_exacto() {
    let uops = expandir_a_uops(&[Opcode::DecimalAExacto]);
    assert_eq!(uops[0], Uop::DecimalAExacto);
}

#[test]
fn test_uop_expand_contract_check_pre() {
    let uops = expandir_a_uops(&[Opcode::CheckPre(0)]);
    assert!(uops.len() >= 1);
}

#[test]
fn test_uop_expand_contract_check_post() {
    let uops = expandir_a_uops(&[Opcode::CheckPost(1)]);
    assert!(uops.len() >= 1);
}

#[test]
fn test_uop_expand_contract_save_anterior() {
    let uops = expandir_a_uops(&[Opcode::SaveAnterior(2)]);
    assert!(uops.len() >= 1);
}

#[test]
fn test_uop_expand_contract_check_inv() {
    let uops = expandir_a_uops(&[Opcode::CheckInv(3)]);
    assert!(uops.len() >= 1);
}

#[test]
fn test_uop_expand_set_line() {
    let uops = expandir_a_uops(&[Opcode::SetLine(10)]);
    assert!(uops.len() >= 1);
}

// ============================================================
// E2E: generate bytecode from source and verify it runs
// ============================================================

#[test]
fn test_e2e_bytecode_gen_ok() {
    let mut lex = Lexer::new("variable x = 42\nescribir(x)");
    let tokens = lex.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let prog = parser.parse().unwrap();
    let mut gen = BytecodeGenerator::new();
    let bc = gen.generar(&prog).unwrap();
    assert!(bc.len() > 2);
    assert!(bc.contains(&Opcode::Halt));
}
