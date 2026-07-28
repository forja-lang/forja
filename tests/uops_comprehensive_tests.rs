use forja::bytecode::{self, Opcode};
use forja::uops::{expandir_a_uops, optimizar_uops, remapear_saltos_uops, Uop};
use std::sync::Arc;
use std::collections::HashMap;

fn expandir(opcodes: Vec<Opcode>) -> Vec<Uop> {
    expandir_a_uops(&opcodes)
}

// ============================================================
// Expansion Tests
// ============================================================

#[test]
fn test_uops_expand_push_entero() {
    let uops = expandir(vec![Opcode::PushEntero(42)]);
    assert_eq!(uops[0], Uop::PushEntero(42));
}

#[test]
fn test_uops_expand_push_decimal() {
    let uops = expandir(vec![Opcode::PushDecimal(3.14)]);
    assert_eq!(uops[0], Uop::PushDecimal(3.14));
}

#[test]
fn test_uops_expand_push_texto() {
    let uops = expandir(vec![Opcode::PushTexto(Arc::from("hi"))]);
    assert_eq!(uops[0], Uop::PushTexto(Arc::from("hi")));
}

#[test]
fn test_uops_expand_push_booleano_true() {
    let uops = expandir(vec![Opcode::PushBooleano(true)]);
    assert_eq!(uops[0], Uop::PushBooleano(true));
}

#[test]
fn test_uops_expand_push_nulo() {
    let uops = expandir(vec![Opcode::PushNulo]);
    assert_eq!(uops[0], Uop::PushNulo);
}

#[test]
fn test_uops_expand_add() {
    let uops = expandir(vec![Opcode::Add]);
    assert_eq!(uops[0], Uop::Add);
}

#[test]
fn test_uops_expand_sub() {
    let uops = expandir(vec![Opcode::Sub]);
    assert_eq!(uops[0], Uop::Sub);
}

#[test]
fn test_uops_expand_mul() {
    let uops = expandir(vec![Opcode::Mul]);
    assert_eq!(uops[0], Uop::Mul);
}

#[test]
fn test_uops_expand_div() {
    let uops = expandir(vec![Opcode::Div]);
    assert_eq!(uops[0], Uop::Div);
}

#[test]
fn test_uops_expand_compare() {
    let uops = expandir(vec![Opcode::Mayor]);
    assert_eq!(uops[0], Uop::Mayor);
}

#[test]
fn test_uops_expand_jump() {
    let uops = expandir(vec![Opcode::Jump(42)]);
    assert_eq!(uops[0], Uop::Jump(42));
}

#[test]
fn test_uops_expand_jump_si_falso() {
    let uops = expandir(vec![Opcode::JumpSiFalso(99)]);
    assert_eq!(uops[0], Uop::JumpSiFalso(99));
}

#[test]
fn test_uops_expand_print() {
    let uops = expandir(vec![Opcode::Print]);
    assert_eq!(uops[0], Uop::Print);
}

#[test]
fn test_uops_expand_halt() {
    let uops = expandir(vec![Opcode::Halt]);
    assert_eq!(uops[0], Uop::Halt);
}

#[test]
fn test_uops_expand_call() {
    let uops = expandir(vec![Opcode::Call(Arc::from("f"), 2)]);
    assert_eq!(uops[0], Uop::Call("f".to_string(), 2));
}

#[test]
fn test_uops_expand_return() {
    let uops = expandir(vec![Opcode::Return]);
    assert_eq!(uops[0], Uop::Return);
}

#[test]
fn test_uops_expand_array_new() {
    let uops = expandir(vec![Opcode::ArrayNew(3)]);
    assert_eq!(uops[0], Uop::ArrayNew(3));
}

#[test]
fn test_uops_expand_array_get() {
    let uops = expandir(vec![Opcode::ArrayGet]);
    assert_eq!(uops[0], Uop::ArrayGet);
}

#[test]
fn test_uops_expand_array_set() {
    let uops = expandir(vec![Opcode::ArraySet]);
    assert_eq!(uops[0], Uop::ArraySet);
}

#[test]
fn test_uops_expand_map_new() {
    let uops = expandir(vec![Opcode::MapNew(2)]);
    assert_eq!(uops[0], Uop::MapNew(2));
}

#[test]
fn test_uops_expand_map_get() {
    let uops = expandir(vec![Opcode::MapGet]);
    assert_eq!(uops[0], Uop::MapGet);
}

#[test]
fn test_uops_expand_map_set() {
    let uops = expandir(vec![Opcode::MapSet]);
    assert_eq!(uops[0], Uop::MapSet);
}

#[test]
fn test_uops_expand_dup() {
    let uops = expandir(vec![Opcode::Dup]);
    assert_eq!(uops[0], Uop::Dup);
}

#[test]
fn test_uops_expand_pop() {
    let uops = expandir(vec![Opcode::Pop]);
    assert_eq!(uops[0], Uop::Pop);
}

#[test]
fn test_uops_expand_multiples() {
    let uops = expandir(vec![
        Opcode::PushEntero(1),
        Opcode::PushEntero(2),
        Opcode::AddInt,
        Opcode::Print,
        Opcode::Halt,
    ]);
    assert_eq!(uops.len(), 5);
    assert_eq!(uops[0], Uop::PushEntero(1));
    assert_eq!(uops[4], Uop::Halt);
}

#[test]
fn test_uops_expand_declare_idx() {
    let uops = expandir(vec![Opcode::DeclareIdx(3, true)]);
    // DeclareIdx with mutable flag should expand
    assert_eq!(uops[0], Uop::DeclareInit(3));
}

#[test]
fn test_uops_expand_declare_idx_global() {
    let uops = expandir(vec![Opcode::DeclareIdxGlobal(42, true)]);
    assert_eq!(uops[0], Uop::DeclareInit(42));
}

#[test]
fn test_uops_expand_fusionados_declare_entero() {
    let uops = expandir(vec![Opcode::DeclareEnteroOp(0, 42)]);
    assert!(uops.len() >= 1);
}

#[test]
fn test_uops_expand_declare_booleano_op() {
    let uops = expandir(vec![Opcode::DeclareBooleanoOp(1, true)]);
    assert!(uops.len() >= 1);
}

#[test]
fn test_uops_expand_add_int() {
    let uops = expandir(vec![Opcode::AddInt]);
    assert_eq!(uops[0], Uop::AddInt);
}

#[test]
fn test_uops_expand_add_float() {
    let uops = expandir(vec![Opcode::AddFloat]);
    assert_eq!(uops[0], Uop::AddFloat);
}

// ============================================================
// Optimize Uops
// ============================================================

#[test]
fn test_uops_optimize_no_change() {
    let uops = vec![Uop::PushEntero(1), Uop::Print, Uop::Halt];
    let optimized = optimizar_uops(&uops);
    assert_eq!(optimized.len(), 3);
}

#[test]
fn test_uops_optimize_push_pop_elimination() {
    let uops = vec![Uop::PushEntero(5), Uop::Pop, Uop::Halt];
    let optimized = optimizar_uops(&uops);
    assert_eq!(optimized.len(), 1);
    assert_eq!(optimized[0], Uop::Halt);
}

// ============================================================
// Remapear saltos
// ============================================================

#[test]
fn test_uops_remap_jumps() {
    let mut uops = vec![
        Uop::Jump(5),
        Uop::JumpSiFalso(10),
        Uop::Halt,
    ];
    let bc: Vec<Opcode> = vec![];
    remapear_saltos_uops(&mut uops, &bc);
    assert_eq!(uops.len(), 3);
}
