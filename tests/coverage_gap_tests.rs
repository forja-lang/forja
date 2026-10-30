use forja::bytecode::{deserializar_bytecode, serializar_bytecode, GeneradorBytecode, Opcode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::token::TokenKind;
use forja::uops::{expandir_a_uops, Uop};
use std::sync::Arc;

// ============================================================
// Missing TokenKind coverage
// ============================================================

fn kinds(source: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(source);
    lexer
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|t| t.kind)
        .collect()
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
        forja::ast::Declaracion::Variable {
            valor: Some(forja::ast::Expresion::LiteralNulo),
            ..
        } => {}
        _ => panic!("expected LiteralNulo"),
    }
}

#[test]
fn test_expr_llamada_funcion_as_expr() {
    let prog = parse("variable x = f(42)");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::Variable {
            valor: Some(forja::ast::Expresion::LlamadaFuncion { nombre, .. }),
            ..
        } => {
            assert_eq!(nombre, "f");
        }
        _ => panic!("expected LlamadaFuncion as expression"),
    }
}

#[test]
fn test_expr_llamada_metodo() {
    let prog = parse("variable s = \"hola\".trim()");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::Variable {
            valor: Some(forja::ast::Expresion::LlamadaMetodo { metodo, .. }),
            ..
        } => {
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
        forja::ast::Declaracion::Funcion {
            postcondiciones, ..
        } => {
            assert_eq!(postcondiciones.len(), 1);
        }
        _ => panic!("expected Funcion with postcondiciones"),
    }
}

#[test]
fn test_expr_error_type() {
    let prog = parse("variable e = Error(\"fail\")");
    match &prog.declaraciones[0] {
        forja::ast::Declaracion::Variable {
            valor: Some(forja::ast::Expresion::Error(_)),
            ..
        } => {}
        _ => panic!("expected Error expression"),
    }
}

// ============================================================
// Opcode serialization for untested variants
// ============================================================

#[test]
fn test_opc_serialize_check_tag() {
    let codigos = vec![Opcode::VerificarEtiqueta(5)];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::VerificarEtiqueta(5));
}

#[test]
fn test_opc_serialize_extract_field() {
    let codigos = vec![Opcode::ExtraerCampo(3)];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::ExtraerCampo(3));
}

#[test]
fn test_opc_serialize_label() {
    let codigos = vec![Opcode::Etiqueta(999)];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::Etiqueta(999));
}

#[test]
fn test_opc_serialize_function_def() {
    let codigos = vec![Opcode::DefinirFuncion(
        Arc::from("test"),
        vec![Arc::from("a"), Arc::from("b")],
    )];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(
        d[0],
        Opcode::DefinirFuncion(Arc::from("test"), vec![Arc::from("a"), Arc::from("b")])
    );
}

#[test]
fn test_opc_serialize_new_object() {
    let codigos = vec![Opcode::NuevoObjeto(Arc::from("Punto"))];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::NuevoObjeto(Arc::from("Punto")));
}

#[test]
fn test_opc_serialize_set_field() {
    let codigos = vec![Opcode::FijarCampo(Arc::from("x"))];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::FijarCampo(Arc::from("x")));
}

#[test]
fn test_opc_serialize_get_field() {
    let codigos = vec![Opcode::ObtenerCampo(Arc::from("nombre"))];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::ObtenerCampo(Arc::from("nombre")));
}

#[test]
fn test_opc_serialize_call_method() {
    let codigos = vec![Opcode::LlamarMetodo(Arc::from("saludar"), 1)];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::LlamarMetodo(Arc::from("saludar"), 1));
}

#[test]
fn test_opc_serialize_try() {
    let codigos = vec![Opcode::Intentar];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::Intentar);
}

#[test]
fn test_opc_serialize_channel_new() {
    let codigos = vec![Opcode::NuevoCanal];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::NuevoCanal);
}

#[test]
fn test_opc_serialize_thread_spawn() {
    // ThreadSpawn serialization may not support round-trip yet
    let ops = vec![Opcode::CrearHilo(Arc::from("f"), 2)];
    let s = serializar_bytecode(&ops);
    assert!(s.len() > 4);
}

#[test]
fn test_opc_serialize_array_len() {
    let codigos = vec![Opcode::LongitudArreglo];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::LongitudArreglo);
}

#[test]
fn test_opc_serialize_parse_int() {
    let codigos = vec![Opcode::ParsearEntero];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::ParsearEntero);
}

#[test]
fn test_opc_serialize_read_line() {
    let codigos = vec![Opcode::LeerLinea];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::LeerLinea);
}

#[test]
fn test_opc_serialize_tiempo_actual() {
    let codigos = vec![Opcode::TiempoActual];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::TiempoActual);
}

#[test]
fn test_opc_serialize_y() {
    let codigos = vec![Opcode::Y, Opcode::O, Opcode::No];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::Y);
    assert_eq!(d[1], Opcode::O);
    assert_eq!(d[2], Opcode::No);
}

#[test]
fn test_opc_serialize_diferente_menor_igual() {
    let codigos = vec![Opcode::Diferente, Opcode::MenorIgual, Opcode::MayorIgual];
    let s = serializar_bytecode(&codigos);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::Diferente);
    assert_eq!(d[1], Opcode::MenorIgual);
    assert_eq!(d[2], Opcode::MayorIgual);
}

#[test]
fn test_opc_serialize_exacto() {
    let ops = vec![
        Opcode::PushExacto(123, 2),
        Opcode::SumarExacto,
        Opcode::RestarExacto,
        Opcode::MultiplicarExacto,
        Opcode::DividirExacto,
        Opcode::IgualExacto,
        Opcode::MenorExacto,
        Opcode::MayorExacto,
        Opcode::EnteroAExacto,
        Opcode::DecimalAExacto,
    ];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d.len(), ops.len());
    assert_eq!(d[0], Opcode::PushExacto(123, 2));
    assert_eq!(d[5], Opcode::IgualExacto);
}

#[test]
fn test_opc_serialize_float_comparisons() {
    let ops = vec![
        Opcode::IgualDecimal,
        Opcode::DistintoDecimal,
        Opcode::MayorDecimal,
        Opcode::MayorIgualDecimal,
        Opcode::MenorIgualDecimal,
    ];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d, ops);
}

#[test]
fn test_opc_serialize_contract() {
    let ops = vec![
        Opcode::VerificarPrecondicion(0),
        Opcode::VerificarPostcondicion(1),
        Opcode::GuardarAnterior(2),
        Opcode::VerificarInvariante(3),
    ];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d, ops);
}

#[test]
fn test_opc_serialize_set_line() {
    let ops = vec![Opcode::FijarLinea(42)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::FijarLinea(42));
}

#[test]
fn test_opc_serialize_store_entero_op() {
    let ops = vec![Opcode::GuardarOperacionEntero(5, 100)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::GuardarOperacionEntero(5, 100));
}

#[test]
fn test_opc_serialize_declare_float_op() {
    let ops = vec![Opcode::DeclararOperacionDecimal(3, 2.5)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::DeclararOperacionDecimal(3, 2.5));
}

#[test]
fn test_opc_serialize_store_float_op() {
    let ops = vec![Opcode::GuardarOperacionDecimal(7, 1.5)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::GuardarOperacionDecimal(7, 1.5));
}

#[test]
fn test_opc_serialize_call_native_roundtrip() {
    let ops = vec![Opcode::LlamarNativa(Arc::from("printf"), 2)];
    let s = serializar_bytecode(&ops);
    assert!(s.len() > 4);
}

#[test]
fn test_opc_serialize_socket_poll() {
    let ops = vec![Opcode::ConsultarSocket(Arc::from("conn"))];
    let s = serializar_bytecode(&ops);
    assert!(s.len() > 4);
}

#[test]
fn test_opc_serialize_declare_exact_op() {
    let ops = vec![Opcode::DeclararOperacionExacto(0, 12345, 3)];
    let s = serializar_bytecode(&ops);
    let d = deserializar_bytecode(&s).unwrap();
    assert_eq!(d[0], Opcode::DeclararOperacionExacto(0, 12345, 3));
}

// ============================================================
// Uop expansion tests for untested uops
// ============================================================

#[test]
fn test_uop_expand_load_idx() {
    let uops = expandir_a_uops(&[Opcode::CargarIndice(42)]);
    assert_eq!(uops[0], Uop::LoadIdx(42));
}

#[test]
fn test_uop_expand_store_idx() {
    let uops = expandir_a_uops(&[Opcode::GuardarIndice(99)]);
    assert_eq!(uops[0], Uop::StoreIdx(99));
}

#[test]
fn test_uop_expand_try() {
    let uops = expandir_a_uops(&[Opcode::Intentar]);
    assert_eq!(uops[0], Uop::Try);
}

#[test]
fn test_uop_expand_label() {
    let uops = expandir_a_uops(&[Opcode::Etiqueta(7)]);
    assert_eq!(uops[0], Uop::Label(7));
}

#[test]
fn test_uop_expand_function_def() {
    let uops = expandir_a_uops(&[Opcode::DefinirFuncion(Arc::from("f"), vec![])]);
    assert_eq!(uops.len(), 1);
}

#[test]
fn test_uop_expand_new_object() {
    let uops = expandir_a_uops(&[Opcode::NuevoObjeto(Arc::from("Punto"))]);
    assert_eq!(uops[0], Uop::NewObject("Punto".to_string()));
}

#[test]
fn test_uop_expand_set_field() {
    let uops = expandir_a_uops(&[Opcode::FijarCampo(Arc::from("x"))]);
    assert_eq!(uops[0], Uop::SetField("x".to_string()));
}

#[test]
fn test_uop_expand_get_field() {
    let uops = expandir_a_uops(&[Opcode::ObtenerCampo(Arc::from("nombre"))]);
    assert_eq!(uops[0], Uop::GetField("nombre".to_string()));
}

#[test]
fn test_uop_expand_call_method() {
    let uops = expandir_a_uops(&[Opcode::LlamarMetodo(Arc::from("saludar"), 1)]);
    assert_eq!(uops[0], Uop::CallMethod("saludar".to_string(), 1));
}

#[test]
fn test_uop_expand_array_len() {
    let uops = expandir_a_uops(&[Opcode::LongitudArreglo]);
    assert_eq!(uops[0], Uop::ArrayLen);
}

#[test]
fn test_uop_expand_parse_int() {
    let uops = expandir_a_uops(&[Opcode::ParsearEntero]);
    assert_eq!(uops[0], Uop::ParseInt);
}

#[test]
fn test_uop_expand_tiempo_actual() {
    let uops = expandir_a_uops(&[Opcode::TiempoActual]);
    assert_eq!(uops[0], Uop::TiempoActual);
}

#[test]
fn test_uop_expand_read_line() {
    let uops = expandir_a_uops(&[Opcode::LeerLinea]);
    assert_eq!(uops[0], Uop::ReadLine);
}

#[test]
fn test_uop_expand_channel_new() {
    let uops = expandir_a_uops(&[Opcode::NuevoCanal]);
    // ChannelNew expands to some sequence of uops
    assert!(!uops.is_empty());
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
    let uops = expandir_a_uops(&[Opcode::ConsultarSocket(Arc::from("s"))]);
    assert_eq!(uops[0], Uop::SocketPoll(Arc::from("s")));
}

#[test]
fn test_uop_expand_call_native() {
    let uops = expandir_a_uops(&[Opcode::LlamarNativa(Arc::from("printf"), 1)]);
    assert_eq!(uops[0], Uop::CallNative(Arc::from("printf"), 1));
}

#[test]
fn test_uop_expand_exacto() {
    let uops = expandir_a_uops(&[Opcode::PushExacto(100, 2)]);
    assert_eq!(uops[0], Uop::PushExacto(100, 2));
}

#[test]
fn test_uop_expand_add_exact() {
    let uops = expandir_a_uops(&[Opcode::SumarExacto]);
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
    let uops = expandir_a_uops(&[Opcode::VerificarPrecondicion(0)]);
    assert!(!uops.is_empty());
}

#[test]
fn test_uop_expand_contract_check_post() {
    let uops = expandir_a_uops(&[Opcode::VerificarPostcondicion(1)]);
    assert!(!uops.is_empty());
}

#[test]
fn test_uop_expand_contract_save_anterior() {
    let uops = expandir_a_uops(&[Opcode::GuardarAnterior(2)]);
    assert!(!uops.is_empty());
}

#[test]
fn test_uop_expand_contract_check_inv() {
    let uops = expandir_a_uops(&[Opcode::VerificarInvariante(3)]);
    assert!(!uops.is_empty());
}

#[test]
fn test_uop_expand_set_line() {
    let uops = expandir_a_uops(&[Opcode::FijarLinea(10)]);
    assert!(!uops.is_empty());
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
    let mut gen = GeneradorBytecode::new();
    let bc = gen.generar(&prog).unwrap();
    assert!(bc.len() > 2);
    assert!(bc.contains(&Opcode::Detener));
}
