//! Cobertura del backend ASM (`src/compiler_asm.rs`) al 100 %.
//!
//! Estos tests son de caja blanca: construyen el AST y empujan al compilador
//! por cada rama de emisión, cosa que un programa Forja no siempre permite
//! (constructos no soportados, estados internos como `postcondiciones_activas`,
//! pool de registros agotado, etc.). La validez del código generado la cubren
//! `tests/asm_targets.rs` (batería por target + e2e real) y los tests del
//! módulo `tests`/`tests_targets` de `compiler_asm.rs`.

use super::*;

const TARGETS: [TargetArch; 3] = [
    TargetArch::X86_64Windows,
    TargetArch::X86_64Linux,
    TargetArch::AArch64,
];

// ──────────────────────────────────────────────────────────
// Detección de plataforma y tabla de targets
// ──────────────────────────────────────────────────────────

#[test]
fn detect_mapea_cada_plataforma() {
    assert_eq!(
        TargetArch::desde_plataforma("x86_64", "windows"),
        TargetArch::X86_64Windows
    );
    assert_eq!(
        TargetArch::desde_plataforma("x86_64", "linux"),
        TargetArch::X86_64Linux
    );
    assert_eq!(
        TargetArch::desde_plataforma("aarch64", "macos"),
        TargetArch::AArch64
    );
    assert_eq!(
        TargetArch::desde_plataforma("x86_64", "macos"),
        TargetArch::X86_64Linux
    );
    assert_eq!(
        TargetArch::desde_plataforma("riscv64", "linux"),
        TargetArch::X86_64Linux
    );
    assert_eq!(
        TargetArch::detect(),
        TargetArch::desde_plataforma(std::env::consts::ARCH, std::env::consts::OS)
    );
}

#[test]
fn tabla_de_targets_completa() {
    for t in TARGETS {
        assert!(!t.name().is_empty());
        assert!(!t.syntax_directive().is_empty() || t == TargetArch::AArch64);
        assert!(!t.section_rodata().is_empty());
        assert!(!t.ret_reg().is_empty());
        assert!(!t.ret_reg_32().is_empty());
        assert!(!t.tmp_reg().is_empty());
        assert!(!t.tmp2_reg().is_empty());
        assert!(!t.fp_reg().is_empty());
        assert!(!t.sp_reg().is_empty());
        assert!(!t.float_reg().is_empty());
        assert!(t.shadow_space() >= 0);
        assert_eq!(t.extern_directive("puts"), format!(".extern puts"));
        assert!(t.globl_directive("main").contains("main"));
        assert!(t.asciz_directive("x").contains("\"x\""));
    }

    // `from_str` acepta alias y normaliza separadores.
    let casos = [
        ("x86_64_windows", TargetArch::X86_64Windows),
        ("x86_64-windows", TargetArch::X86_64Windows),
        ("win64", TargetArch::X86_64Windows),
        ("windows", TargetArch::X86_64Windows),
        ("x86_64_linux", TargetArch::X86_64Linux),
        ("x86_64-linux", TargetArch::X86_64Linux),
        ("linux64", TargetArch::X86_64Linux),
        ("linux", TargetArch::X86_64Linux),
        ("x86_64", TargetArch::X86_64Linux),
        ("aarch64", TargetArch::AArch64),
        ("arm64", TargetArch::AArch64),
        ("arm", TargetArch::AArch64),
    ];
    for (texto, esperado) in casos {
        assert_eq!(
            TargetArch::desde_str(texto),
            Some(esperado),
            "from_str({texto})"
        );
        let mayusculas = texto.to_uppercase();
        assert_eq!(
            TargetArch::desde_str(&mayusculas),
            Some(esperado),
            "from_str({mayusculas})"
        );
    }
    assert_eq!(TargetArch::desde_str("mips"), None);
}

// ──────────────────────────────────────────────────────────
// Emisores de instrucciones
// ──────────────────────────────────────────────────────────

#[test]
fn emisores_x86_usan_eax_para_32_bits() {
    // `ldr_reg_mem` tiene una rama dedicada para el registro de 32 bits.
    let asm = TargetArch::X86_64Linux.ldr_reg_mem("eax", "rbp", 8);
    assert!(asm.contains("mov eax, [rbp - 8]"), "{asm}");
    let asm64 = TargetArch::X86_64Windows.ldr_reg_mem("rax", "rbp", 8);
    assert!(asm64.contains("mov rax, [rbp - 8]"), "{asm64}");
}

#[test]
fn to_8bit_reg_convierte_todos_los_registros() {
    let esperados = [
        ("rax", "al"),
        ("eax", "al"),
        ("rbx", "bl"),
        ("ebx", "bl"),
        ("rcx", "cl"),
        ("ecx", "cl"),
        ("rdx", "dl"),
        ("edx", "dl"),
        ("rsi", "sil"),
        ("esi", "sil"),
        ("rdi", "dil"),
        ("edi", "dil"),
        ("rbp", "bpl"),
        ("ebp", "bpl"),
        ("rsp", "spl"),
        ("esp", "spl"),
        ("r8", "r8b"),
        ("r8d", "r8b"),
        ("r9", "r9b"),
        ("r9d", "r9b"),
        ("r10", "r10b"),
        ("r10d", "r10b"),
        ("r11", "r11b"),
        ("r11d", "r11b"),
        ("r12", "r12b"),
        ("r12d", "r12b"),
        ("r13", "r13b"),
        ("r13d", "r13b"),
        ("r14", "r14b"),
        ("r14d", "r14b"),
        ("r15", "r15b"),
        ("r15d", "r15b"),
    ];
    for (reg, esperado) in esperados {
        assert_eq!(TargetArch::to_8bit_reg(reg), esperado, "to_8bit_reg({reg})");
    }
    // Registros no estándar: fallback `{reg}l`.
    assert_eq!(TargetArch::to_8bit_reg("x19"), "x19l");
}

#[test]
fn emisores_por_target_emiten_formas_distintas() {
    for t in TARGETS {
        let a = t.arg_regs()[0];
        let b = t.arg_regs()[1];
        let fp = t.fp_reg();
        let sp = t.sp_reg();

        assert!(!t.mov_reg_imm(a, 0).is_empty());
        assert!(!t.mov_reg_imm(a, -1).is_empty());
        assert!(t.push_reg(a).contains(a));
        assert!(t.pop_reg(a).contains(a));
        assert!(!t.push_fp_lr().is_empty());
        assert!(!t.pop_fp_lr().is_empty());
        assert!(t.set_fp_from_sp().contains(fp));
        assert!(t.sub_sp(16).contains("16"));
        assert!(t.add_sp(16).contains("16"));
        assert!(t.mov_sp_fp().contains(fp));
        assert!(t.str_reg_mem(a, fp, 8).contains(a));
        assert!(t.ldr_reg_mem(a, fp, 8).contains(a));
        assert!(t.str_field(fp, 8, a).contains(a));
        assert!(t.ldr_field(a, fp, 8).contains(a));
        // Escalas de 8 y 16 bytes: `log2` debe dar 3 y 4 (no 0).
        assert!(t.str_mem_index(fp, b, 8, a).contains(a));
        assert!(t.str_mem_index(fp, b, 16, a).contains(a));
        assert!(t.add_reg_reg(a, b).contains(b));
        assert!(t.sub_reg_reg(a, b).contains(b));
        assert!(t.mul_reg_reg(a, b).contains(b));
        assert!(!t.div_reg(b).is_empty());
        assert!(t.neg_reg(a).contains(a));
        assert!(t.xor_reg_reg(a, a).contains(a));
        assert!(t.cmp_reg_reg(a, b).contains(b));
        assert!(t.test_reg(a).contains(a));
        for set in [
            t.set_g(a),
            t.set_l(a),
            t.set_ge(a),
            t.set_le(a),
            t.set_e(a),
            t.set_ne(a),
        ] {
            assert!(!set.is_empty());
        }
        assert!(t.movzx(a, a).contains(a));
        assert!(t.call("printf").contains("printf"));
        assert!(!t.ret().is_empty());
        assert!(t.jump(".Lx").contains(".Lx"));
        assert!(t.jump_if_zero(".Lx").contains(".Lx"));
        assert!(t.jump_if_ge(".Lx").contains(".Lx"));
        assert!(t.lea_label(a, ".Lx").contains(".Lx"));
        assert!(t.movsd_mem_fp(fp, 8, t.float_reg()).contains(t.float_reg()));
        assert!(t.movsd_fp_mem(t.float_reg(), fp, 8).contains(t.float_reg()));
        assert!(t.movsd_label(t.float_reg(), ".Lf").contains(".Lf"));
        let qword = t.mov_qword_ptr_imm(fp, 8, 42);
        assert!(qword.contains(fp), "{t:?}: {qword}");
        assert!(qword.contains("42"), "{t:?}: {qword}");
        assert!(!t.gen_syscall_write(a, b).is_empty());
        assert!(!t.section_rodata().is_empty());
        assert!(t.fp_reg() != sp || t == TargetArch::AArch64);
    }
}

#[test]
fn log2_todos_los_grados() {
    assert_eq!(log2(0), 0);
    assert_eq!(log2(1), 0);
    assert_eq!(log2(2), 1);
    assert_eq!(log2(3), 1);
    assert_eq!(log2(4), 2);
    assert_eq!(log2(8), 3);
    assert_eq!(log2(16), 4);
    assert_eq!(log2(1024), 10);
}

// ──────────────────────────────────────────────────────────
// Pool de registros
// ──────────────────────────────────────────────────────────

#[test]
fn pool_de_registros_se_agota_y_se_libera() {
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        let total = c.reg_names.len();
        let mut alocados = Vec::new();
        while let Some(reg) = c.alloc_reg() {
            alocados.push(reg);
        }
        assert_eq!(alocados.len(), total, "{t:?}: el pool debe agotarse");
        assert!(c.alloc_reg().is_none(), "{t:?}: pool agotado → None");

        for reg in &alocados {
            c.free_reg(reg);
        }
        assert_eq!(c.alloc_reg(), Some(alocados[0]));

        // Liberar un registro que no está en el pool no hace nada.
        c.free_reg("registro_inexistente");
        // Liberar una variable sin registro asociado no hace nada.
        c.free_var_reg("variable_sin_registro");
    }
}

#[test]
fn alloc_var_reg_persistente_y_agotamiento() {
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        let mut asignados = Vec::new();
        for i in 0..c.reg_names.len() {
            let nombre = format!("v{i}");
            let reg = c.alloc_var_reg(&nombre).expect("registro disponible");
            assert_eq!(c.var_reg_map.get(&nombre), Some(&reg));
            assert_eq!(c.reg_var_map.get(reg), Some(&nombre));
            asignados.push(nombre);
        }
        assert!(c.alloc_var_reg("sobra").is_none(), "{t:?}: pool agotado");

        for nombre in &asignados {
            c.free_var_reg(nombre);
        }
        assert!(c.var_reg_map.is_empty());
        assert!(c.reg_var_map.is_empty());
        assert!(c.reg_pool.iter().all(|usado| !usado));
    }
}

// ──────────────────────────────────────────────────────────
// Tipos
// ──────────────────────────────────────────────────────────

#[test]
fn tipos_forja_a_asm_cubre_todas_las_variantes() {
    let c = CompilerAsm::new();
    let casos = [
        (None, TipoAsm::Entero),
        (Some(Tipo::Entero), TipoAsm::Entero),
        (Some(Tipo::Decimal), TipoAsm::Decimal),
        (Some(Tipo::Texto), TipoAsm::Texto),
        (Some(Tipo::Booleano), TipoAsm::Booleano),
        (Some(Tipo::Exacto), TipoAsm::Texto),
        (Some(Tipo::Clase("C".into())), TipoAsm::Clase("C".into())),
        (Some(Tipo::Nulo), TipoAsm::Entero),
        (Some(Tipo::Arreglo(Box::new(Tipo::Entero))), TipoAsm::Texto),
        (
            Some(Tipo::Funcion(vec![Tipo::Entero], Box::new(Tipo::Entero))),
            TipoAsm::Texto,
        ),
        (
            Some(Tipo::Resultado(
                Box::new(Tipo::Entero),
                Box::new(Tipo::Texto),
            )),
            TipoAsm::Texto,
        ),
        (Some(Tipo::Opcion(Box::new(Tipo::Entero))), TipoAsm::Texto),
        (
            Some(Tipo::RasgoObjeto("R".into())),
            TipoAsm::Clase("R".into()),
        ),
        (Some(Tipo::Parametro("T".into())), TipoAsm::Texto),
    ];
    for (forja, esperado) in casos {
        assert_eq!(c.tipo_forja_a_asm(&forja), esperado, "{forja:?}");
    }

    // Tamaño en bytes de cada tipo de máquina.
    assert_eq!(c.tipo_asm_size(&TipoAsm::Booleano), 4);
    for tipo in [
        TipoAsm::Entero,
        TipoAsm::Decimal,
        TipoAsm::Texto,
        TipoAsm::Clase("C".into()),
    ] {
        assert_eq!(c.tipo_asm_size(&tipo), 8, "{tipo:?}");
    }
}

#[test]
fn tipo_parametro_a_asm_cubre_todas_las_variantes() {
    let c = CompilerAsm::new();
    let param = |tipo: Option<Tipo>| Parametro {
        nombre: "p".into(),
        prestado: false,
        mutable: false,
        tipo,
    };
    let casos = [
        (None, TipoAsm::Entero),
        (Some(Tipo::Entero), TipoAsm::Entero),
        (Some(Tipo::Decimal), TipoAsm::Decimal),
        (Some(Tipo::Texto), TipoAsm::Texto),
        (Some(Tipo::Booleano), TipoAsm::Booleano),
        (Some(Tipo::Exacto), TipoAsm::Texto),
        (Some(Tipo::Clase("C".into())), TipoAsm::Clase("C".into())),
        // Resto de tipos → Entero
        (Some(Tipo::Nulo), TipoAsm::Entero),
        (Some(Tipo::Opcion(Box::new(Tipo::Entero))), TipoAsm::Entero),
    ];
    for (tipo, esperado) in casos {
        assert_eq!(
            c.tipo_parametro_a_asm(&param(tipo.clone())),
            esperado,
            "{tipo:?}"
        );
    }
}

#[test]
fn inferir_tipo_campo_e_inferir_tipo_de_expr() {
    let c = CompilerAsm::new();
    let campo = |tipo: Option<Tipo>| VariableClase {
        nombre: "campo".into(),
        tipo,
    };
    let casos = [
        (None, TipoAsm::Entero),
        (Some(Tipo::Entero), TipoAsm::Entero),
        (Some(Tipo::Decimal), TipoAsm::Decimal),
        (Some(Tipo::Texto), TipoAsm::Texto),
        (Some(Tipo::Booleano), TipoAsm::Booleano),
        (Some(Tipo::Clase("C".into())), TipoAsm::Clase("C".into())),
        (Some(Tipo::Nulo), TipoAsm::Entero),
    ];
    for (tipo, esperado) in casos {
        assert_eq!(
            c.inferir_tipo_campo_asm(&campo(tipo.clone())),
            esperado,
            "{tipo:?}"
        );
    }

    assert_eq!(
        c.inferir_tipo_de_expr(&Expresion::LiteralNumero(1)),
        Some(Tipo::Entero)
    );
    assert_eq!(
        c.inferir_tipo_de_expr(&Expresion::LiteralDecimal(1.0)),
        Some(Tipo::Decimal)
    );
    assert_eq!(
        c.inferir_tipo_de_expr(&Expresion::LiteralTexto("x".into())),
        Some(Tipo::Texto)
    );
    assert_eq!(
        c.inferir_tipo_de_expr(&Expresion::LiteralBooleano(true)),
        Some(Tipo::Booleano)
    );
    assert_eq!(
        c.inferir_tipo_de_expr(&Expresion::LiteralExacto(1, 0)),
        Some(Tipo::Exacto)
    );
    assert_eq!(
        c.inferir_tipo_de_expr(&Expresion::LiteralNulo),
        Some(Tipo::Nulo)
    );
    assert_eq!(
        c.inferir_tipo_de_expr(&Expresion::Nada),
        None,
        "las expresiones no literales no tienen tipo inferible"
    );
}

// ──────────────────────────────────────────────────────────
// Constructores de AST y utilidades compartidas
// ──────────────────────────────────────────────────────────

fn id(nombre: &str) -> Expresion {
    Expresion::Identificador {
        nombre: nombre.into(),
        linea: 0,
        columna: 0,
    }
}

fn num(n: i64) -> Expresion {
    Expresion::LiteralNumero(n)
}

fn txt(s: &str) -> Expresion {
    Expresion::LiteralTexto(s.into())
}

fn bin(izq: Expresion, operador: Operador, der: Expresion) -> Expresion {
    Expresion::Binaria {
        izquierda: Box::new(izq),
        operador,
        derecha: Box::new(der),
    }
}

fn llamada(nombre: &str, argumentos: Vec<Expresion>) -> Expresion {
    Expresion::LlamadaFuncion {
        nombre: nombre.into(),
        argumentos,
    }
}

fn decl_variable(nombre: &str, tipo: Option<Tipo>, valor: Option<Expresion>) -> Declaracion {
    Declaracion::Variable {
        mutable: true,
        nombre: nombre.into(),
        tipo,
        valor,
        linea: 0,
        columna: 0,
    }
}

fn decl_asignacion(nombre: &str, valor: Expresion) -> Declaracion {
    Declaracion::Asignacion {
        nombre: nombre.into(),
        valor: Box::new(valor),
        linea: 0,
        columna: 0,
    }
}

fn retornar(valor: Option<Expresion>) -> Declaracion {
    Declaracion::Retornar { valor }
}

fn param(nombre: &str, tipo: Option<Tipo>) -> Parametro {
    Parametro {
        nombre: nombre.into(),
        prestado: false,
        mutable: false,
        tipo,
    }
}

fn decl_funcion(nombre: &str, parametros: Vec<Parametro>, cuerpo: Vec<Declaracion>) -> Declaracion {
    Declaracion::Funcion {
        nombre: nombre.into(),
        parametros_tipo: vec![],
        parametros,
        tipo_retorno: None,
        cuerpo,
        externa: false,
        asincrona: false,
        enlace_nombre: None,
        atributos: vec![],
        doc: None,
        precondiciones: vec![],
        postcondiciones: vec![],
    }
}

fn decl_clase(nombre: &str, campos: Vec<VariableClase>, metodos: Vec<Metodo>) -> Declaracion {
    Declaracion::Clase {
        nombre: nombre.into(),
        parametros_tipo: vec![],
        campos,
        metodos,
        atributos: vec![],
        invariantes: vec![],
    }
}

fn campo(nombre: &str, tipo: Option<Tipo>) -> VariableClase {
    VariableClase {
        nombre: nombre.into(),
        tipo,
    }
}

fn metodo(nombre: &str, parametros: Vec<Parametro>, cuerpo: Vec<Declaracion>) -> Metodo {
    Metodo {
        nombre: nombre.into(),
        parametros,
        tipo_retorno: None,
        cuerpo,
        precondiciones: vec![],
        postcondiciones: vec![],
    }
}

/// Compilador con una variable de cada `TipoAsm` disponible.
fn con_variables(target: TargetArch) -> CompilerAsm {
    let mut c = CompilerAsm::with_target(target);
    for (i, (nombre, tipo)) in [
        ("ent", TipoAsm::Entero),
        ("dec", TipoAsm::Decimal),
        ("txt", TipoAsm::Texto),
        ("bool", TipoAsm::Booleano),
        ("obj", TipoAsm::Clase("C".into())),
    ]
    .into_iter()
    .enumerate()
    {
        c.variables.insert(
            nombre.into(),
            StackVar {
                offset: 8 * (i as i32 + 1),
                tipo,
            },
        );
    }
    c
}

/// Registra la clase `C` con un campo `x` para pruebas de acceso a miembros.
fn registrar_clase_c(c: &mut CompilerAsm) {
    c.clases.insert(
        "C".into(),
        ClaseAsmInfo {
            campos: vec![("x".into(), TipoAsm::Entero)],
            size: 8,
            offsets: HashMap::from([("x".to_string(), 0)]),
            metodos: vec![],
        },
    );
}

/// Ocupa todos los registros calle-saved para forzar el camino de stack.
fn llenar_pool(c: &mut CompilerAsm) {
    while c.alloc_reg().is_some() {}
}

mod bordes;
mod declaraciones;
mod escritura;
mod expresiones;
mod inlining;
mod inlining_negativo;
