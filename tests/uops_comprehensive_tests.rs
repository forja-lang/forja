use forja::bytecode::Opcode;
use forja::uops::{expandir_a_uops, optimizar_uops, remapear_saltos_uops, Uop};
use std::sync::Arc;

fn expandir(codigos: Vec<Opcode>) -> Vec<Uop> {
    expandir_a_uops(&codigos)
}

// ============================================================
// Expansion Tests
// ============================================================

#[test]
fn test_uops_expandir_empujar_entero() {
    let uops = expandir(vec![Opcode::EmpujarEntero(42)]);
    assert_eq!(uops[0], Uop::EmpujarEntero(42));
}

#[test]
fn test_uops_expandir_empujar_decimal() {
    let uops = expandir(vec![Opcode::EmpujarDecimal(2.5)]);
    assert_eq!(uops[0], Uop::EmpujarDecimal(2.5));
}

#[test]
fn test_uops_expandir_empujar_texto() {
    let uops = expandir(vec![Opcode::EmpujarTexto(Arc::from("hi"))]);
    assert_eq!(uops[0], Uop::EmpujarTexto(Arc::from("hi")));
}

#[test]
fn test_uops_expandir_empujar_booleano_true() {
    let uops = expandir(vec![Opcode::EmpujarBooleano(true)]);
    assert_eq!(uops[0], Uop::EmpujarBooleano(true));
}

#[test]
fn test_uops_expandir_empujar_nulo() {
    let uops = expandir(vec![Opcode::EmpujarNulo]);
    assert_eq!(uops[0], Uop::EmpujarNulo);
}

#[test]
fn test_uops_expand_add() {
    let uops = expandir(vec![Opcode::Sumar]);
    assert_eq!(uops[0], Uop::Add);
}

#[test]
fn test_uops_expand_sub() {
    let uops = expandir(vec![Opcode::Restar]);
    assert_eq!(uops[0], Uop::Sub);
}

#[test]
fn test_uops_expand_mul() {
    let uops = expandir(vec![Opcode::Multiplicar]);
    assert_eq!(uops[0], Uop::Mul);
}

#[test]
fn test_uops_expand_div() {
    let uops = expandir(vec![Opcode::Dividir]);
    assert_eq!(uops[0], Uop::Div);
}

#[test]
fn test_uops_expand_compare() {
    let uops = expandir(vec![Opcode::Mayor]);
    assert_eq!(uops[0], Uop::Mayor);
}

#[test]
fn test_uops_expand_jump() {
    let uops = expandir(vec![Opcode::Salto(42)]);
    assert_eq!(uops[0], Uop::Jump(42));
}

#[test]
fn test_uops_expand_jump_si_falso() {
    let uops = expandir(vec![Opcode::SaltoSiFalso(99)]);
    assert_eq!(uops[0], Uop::JumpSiFalso(99));
}

#[test]
fn test_uops_expand_print() {
    let uops = expandir(vec![Opcode::Imprimir]);
    assert_eq!(uops[0], Uop::Print);
}

#[test]
fn test_uops_expand_halt() {
    let uops = expandir(vec![Opcode::Detener]);
    assert_eq!(uops[0], Uop::Halt);
}

#[test]
fn test_uops_expand_call() {
    let uops = expandir(vec![Opcode::Llamar(Arc::from("f"), 2)]);
    assert_eq!(uops[0], Uop::Call("f".to_string(), 2));
}

#[test]
fn test_uops_expand_return() {
    let uops = expandir(vec![Opcode::Retornar]);
    assert_eq!(uops[0], Uop::Return);
}

#[test]
fn test_uops_expandir_arreglo_nuevo() {
    let uops = expandir(vec![Opcode::NuevoArreglo(3)]);
    assert_eq!(uops[0], Uop::ArregloNuevo(3));
}

#[test]
fn test_uops_expandir_arreglo_obtener() {
    let uops = expandir(vec![Opcode::ObtenerElemento]);
    assert_eq!(uops[0], Uop::ArregloObtener);
}

#[test]
fn test_uops_expandir_arreglo_fijar() {
    let uops = expandir(vec![Opcode::FijarElemento]);
    assert_eq!(uops[0], Uop::ArregloFijar);
}

#[test]
fn test_uops_expandir_mapa_nuevo() {
    let uops = expandir(vec![Opcode::NuevoMapa(2)]);
    assert_eq!(uops[0], Uop::MapaNuevo(2));
}

#[test]
fn test_uops_expandir_mapa_obtener() {
    let uops = expandir(vec![Opcode::ObtenerValorDeMapa]);
    assert_eq!(uops[0], Uop::MapaObtener);
}

#[test]
fn test_uops_expandir_mapa_fijar() {
    let uops = expandir(vec![Opcode::FijarValorDeMapa]);
    assert_eq!(uops[0], Uop::MapaFijar);
}

#[test]
fn test_uops_expand_dup() {
    let uops = expandir(vec![Opcode::Duplicar]);
    assert_eq!(uops[0], Uop::Dup);
}

#[test]
fn test_uops_expandir_desapilar() {
    let uops = expandir(vec![Opcode::Descartar]);
    assert_eq!(uops[0], Uop::Desapilar);
}

#[test]
fn test_uops_expand_multiples() {
    let uops = expandir(vec![
        Opcode::EmpujarEntero(1),
        Opcode::EmpujarEntero(2),
        Opcode::SumarEntero,
        Opcode::Imprimir,
        Opcode::Detener,
    ]);
    assert_eq!(uops.len(), 5);
    assert_eq!(uops[0], Uop::EmpujarEntero(1));
    assert_eq!(uops[4], Uop::Halt);
}

#[test]
fn test_uops_expand_declare_idx() {
    let uops = expandir(vec![Opcode::DeclararIndice(3, true)]);
    // DeclareIdx con bandera de mutabilidad debe expandirse
    assert_eq!(uops[0], Uop::DeclararInicializar(3));
}

#[test]
fn test_uops_expand_declare_idx_global() {
    let uops = expandir(vec![Opcode::DeclararIndiceGlobal(42, true)]);
    assert_eq!(uops[0], Uop::DeclararInicializar(42));
}

#[test]
fn test_uops_expand_fusionados_declare_entero() {
    let uops = expandir(vec![Opcode::DeclararOperacionEntero(0, 42)]);
    assert!(!uops.is_empty());
}

#[test]
fn test_uops_expand_declare_booleano_op() {
    let uops = expandir(vec![Opcode::DeclararOperacionBooleano(1, true)]);
    assert!(!uops.is_empty());
}

#[test]
fn test_uops_expand_add_int() {
    let uops = expandir(vec![Opcode::SumarEntero]);
    assert_eq!(uops[0], Uop::AddInt);
}

#[test]
fn test_uops_expand_add_float() {
    let uops = expandir(vec![Opcode::SumarDecimal]);
    assert_eq!(uops[0], Uop::AddFloat);
}

// ============================================================
// Optimize Uops
// ============================================================

#[test]
fn test_uops_optimizar_no_cambio() {
    let uops = vec![Uop::EmpujarEntero(1), Uop::Print, Uop::Halt];
    let optimized = optimizar_uops(&uops);
    assert_eq!(optimized.len(), 3);
}

#[test]
fn test_uops_optimizar_empujar_desapilar_eliminacion() {
    let uops = vec![Uop::EmpujarEntero(5), Uop::Desapilar, Uop::Halt];
    let optimized = optimizar_uops(&uops);
    assert_eq!(optimized.len(), 1);
    assert_eq!(optimized[0], Uop::Halt);
}

// ============================================================
// Remapear saltos
// ============================================================

#[test]
fn test_uops_remap_jumps() {
    let mut uops = vec![Uop::Jump(5), Uop::JumpSiFalso(10), Uop::Halt];
    let bc: Vec<Opcode> = vec![];
    remapear_saltos_uops(&mut uops, &bc);
    assert_eq!(uops.len(), 3);
}
