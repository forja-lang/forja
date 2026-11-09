//! Bytecode de Forja: opcodes, generador, contratos y serialización.

#![allow(rustdoc::broken_intra_doc_links)]

pub mod contratos;
pub mod gen;
pub mod metadata;
pub mod opcode;
pub mod postproceso;
pub mod serialize;
pub mod symbol_table;
pub mod uops;

pub use contratos::{ContratoBytecode, TipoContrato};
pub use gen::{GeneradorBytecode, ModuloBytecode};
pub use opcode::{Opcode, TipoIncorporada};
pub use postproceso::{
    deserializar_con_metadata, fusionar_direct_float_opcodes, fusionar_opcodes, optimizar_indices,
    postprocesar_globales, serializar_con_metadata,
};
pub use serialize::{deserializar_bytecode, sanitizar_para_serializacion, serializar_bytecode};
pub use symbol_table::IdSimbolo;

/// Límites del `Entero` de la VM (48 bits de payload).
pub const ENTERO_MIN: i64 = -(1 << 47);
pub const ENTERO_MAX: i64 = (1 << 47) - 1;
pub const RESULTADO_IDX: usize = usize::MAX;

#[inline(always)]
pub fn entero_representable(n: i64) -> bool {
    (ENTERO_MIN..=ENTERO_MAX).contains(&n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_syntax::error::ErrorForja;
    use forja_syntax::lexer::Lexer;
    use forja_syntax::parser::Parser;
    use forja_types::semantics::TypeChecker;
    use std::sync::Arc;

    fn generar_bytecode(fuente: &str) -> Result<Vec<Opcode>, Vec<ErrorForja>> {
        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse()?;
        let mut gen = GeneradorBytecode::new();
        gen.generar(&programa)
    }

    #[test]
    fn bytecode_variable() {
        let bc = generar_bytecode("variable x = 5").unwrap();
        assert_eq!(bc[0], Opcode::PushEntero(5));
        assert_eq!(bc[1], Opcode::Declarar(Arc::from("x"), true));
        assert_eq!(bc[2], Opcode::Detener);
    }

    #[test]
    fn bytecode_constante() {
        let bc = generar_bytecode("constante x = 10").unwrap();
        assert_eq!(bc[0], Opcode::PushEntero(10));
        assert_eq!(bc[1], Opcode::Declarar(Arc::from("x"), false));
        assert_eq!(bc[2], Opcode::Detener);
    }

    #[test]
    fn bytecode_aritmetica() {
        let bc = generar_bytecode("variable x = 2 + 3").unwrap();
        assert_eq!(bc[0], Opcode::PushEntero(2));
        assert_eq!(bc[1], Opcode::PushEntero(3));
        // Ahora se emite AddInt porque ambos literales se infieren como Entero
        assert_eq!(bc[2], Opcode::SumarEntero);
        assert_eq!(bc[3], Opcode::Declarar(Arc::from("x"), true));
        assert_eq!(bc[4], Opcode::Detener);
    }

    #[test]
    fn bytecode_escribir() {
        let bc = generar_bytecode("escribir(\"Hola\")").unwrap();
        assert_eq!(bc[0], Opcode::PushTexto(Arc::from("Hola")));
        assert_eq!(bc[1], Opcode::Imprimir);
        assert_eq!(bc[2], Opcode::Detener);
    }

    #[test]
    fn bytecode_si() {
        let bc = generar_bytecode("si (verdadero) { variable x = 1 }").unwrap();
        // debe contener: PushBooleano(true), JumpSiFalso, PushEntero(1), Declare("x"), Jump, Label, Halt
        assert!(bc.iter().any(|op| matches!(op, Opcode::PushBooleano(true))));
        assert!(bc.iter().any(|op| matches!(op, Opcode::SaltoSiFalso(_))));
        assert!(bc.iter().any(|op| matches!(op, Opcode::Declarar(_, _))));
    }

    #[test]
    fn bytecode_mientras() {
        let bc = generar_bytecode("mientras (verdadero) { }").unwrap();
        assert!(bc.iter().any(|op| matches!(op, Opcode::PushBooleano(true))));
        assert!(bc.iter().any(|op| matches!(op, Opcode::SaltoSiFalso(_))));
    }

    #[test]
    fn bytecode_repetir() {
        let bc = generar_bytecode("repetir (3) { }").unwrap();
        assert!(bc.iter().any(|op| matches!(op, Opcode::PushEntero(3))));
    }

    #[test]
    fn serializacion() {
        let bc = vec![
            Opcode::PushEntero(42),
            Opcode::Declarar(Arc::from("x"), true),
            Opcode::Detener,
        ];
        let serializado = serializar_bytecode(&bc);
        assert!(serializado.len() > 8);
        assert_eq!(&serializado[0..4], b"FBC\0");
    }

    // ============================================================
    // Tests de especialización de codigos en compile-time
    // ============================================================

    /// Helper que usa el pipeline completo (con TypeChecker + tipos inferidos)
    fn generar_bytecode_con_tipos(fuente: &str) -> Result<Vec<Opcode>, String> {
        use forja_syntax::lexer::Lexer;
        use forja_syntax::parser::Parser;
        use forja_types::semantics::TypeChecker;

        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

        let mut verificador = TypeChecker::new();
        verificador
            .analizar(&programa)
            .map_err(|e| format!("{}", e[0]))?;
        let tipos_inferidos = verificador.obtener_tipos_inferidos();

        let mut gen = GeneradorBytecode::new();
        gen.set_tipos_inferidos(tipos_inferidos);
        gen.generar(&programa)
            .map_err(|_| "Error generando bytecode".to_string())
    }

    #[test]
    fn especializacion_entero_suma() {
        // 5 + 3 debe emitir AddInt (ambos operandos son LiteralNumero → Entero)
        let bc = generar_bytecode_con_tipos("variable x = 5 + 3").unwrap();
        assert!(
            bc.iter().any(|i| matches!(i, Opcode::SumarEntero)),
            "Se esperaba AddInt en las instrucciones, pero se encontraron: {:?}",
            bc
        );
    }

    #[test]
    fn especializacion_decimal_suma() {
        // 2.5 + 3.7 debe emitir AddFloat (ambos operandos son LiteralDecimal → Decimal)
        let bc = generar_bytecode_con_tipos("variable x = 2.5 + 3.7").unwrap();
        assert!(
            bc.iter().any(|i| matches!(i, Opcode::SumarDecimal)),
            "Se esperaba AddFloat en las instrucciones, pero se encontraron: {:?}",
            bc
        );
    }

    #[test]
    fn especializacion_entero_multiplicacion() {
        // 4 * 2 debe emitir MulInt
        let bc = generar_bytecode_con_tipos("variable x = 4 * 2").unwrap();
        assert!(
            bc.iter().any(|i| matches!(i, Opcode::MultiplicarEntero)),
            "Se esperaba MulInt en las instrucciones, pero se encontraron: {:?}",
            bc
        );
    }

    #[test]
    fn especializacion_entero_comparacion() {
        // 5 < 3 debe emitir MenorInt
        let bc = generar_bytecode_con_tipos("variable x = 5 < 3").unwrap();
        assert!(
            bc.iter().any(|i| matches!(i, Opcode::MenorEntero)),
            "Se esperaba MenorInt en las instrucciones, pero se encontraron: {:?}",
            bc
        );
    }

    #[test]
    fn especializacion_con_variable() {
        // variable x = 5; variable y = x + 3 → x es Entero, 3 es Entero → AddInt
        let bc = generar_bytecode_con_tipos("variable x = 5\nvariable y = x + 3").unwrap();
        assert!(
            bc.iter().any(|i| matches!(i, Opcode::SumarEntero)),
            "Se esperaba AddInt (variable + literal), pero se encontraron: {:?}",
            bc
        );
    }

    #[test]
    fn fallback_generico_variables_sin_tipo() {
        // Sin type inference para variables (solo literales tienen tipo conocido),
        // el generador debe emitir codigos genéricos para operaciones entre variables.
        // Nota: los literales SIEMPRE se especializan (AddInt para 5 + 3).
        // El fallback genérico solo se usa cuando NO se puede inferir el tipo.
        let bc = generar_bytecode("variable x = \"hola\" + 5").unwrap();
        // "hola" es Texto, 5 es Entero → tipos diferentes → fallback a Add genérico
        assert!(
            bc.iter().any(|i| matches!(i, Opcode::Sumar)),
            "Se esperaba Add genérico (tipos incompatibles), pero se encontraron: {:?}",
            bc
        );
    }

    // ═════════════════════════════════════════════════════════════════════
    // Hot Reload — Fase 2
    // ═════════════════════════════════════════════════════════════════════

    fn generar_bytecode_para_modulo(
        fuente: &str,
        id_modulo: IdSimbolo,
    ) -> Result<ModuloBytecode, Vec<ErrorForja>> {
        use forja_syntax::lexer::Lexer;
        use forja_syntax::parser::Parser;
        use forja_types::semantics;

        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse()?;

        let mut verificador = semantics::TypeChecker::new();
        verificador.analizar(&programa)?;
        let tipos_inferidos = verificador.obtener_tipos_inferidos();

        let mut gen = GeneradorBytecode::new();
        gen.set_tipos_inferidos(tipos_inferidos);
        gen.generar_para_modulo(&programa, id_modulo)
    }

    #[test]
    fn generar_para_modulo_con_funciones() {
        let fuente = "\
fun saludar(nombre) {
    escribir(\"Hola, ${nombre}\")
}

fun sumar(a, b) {
    retornar a + b
}

fun version() {
    retornar 1
}
";
        let id_modulo = IdSimbolo(42);
        let bytecode_modulo = generar_bytecode_para_modulo(fuente, id_modulo).unwrap();

        // El ModuloBytecode debe tener el id_modulo correcto
        assert_eq!(bytecode_modulo.id_modulo, id_modulo);
        // Debe haber al menos un opcode
        assert!(
            !bytecode_modulo.codigos.is_empty(),
            "El bytecode no debe estar vacío"
        );
        // Debe tener 3 funciones registradas
        assert_eq!(
            bytecode_modulo.ids_simbolos_funcion.len(),
            3,
            "Se esperaban 3 funciones (saludar, sumar, version)"
        );
    }

    #[test]
    fn generar_para_modulo_con_globales() {
        let fuente = "\
variable contador = 0
variable nombre = \"test\"

fun incrementar() {
    contador = contador + 1
}

fun obtener() {
    retornar contador
}
";
        let id_modulo = IdSimbolo(7);
        let bytecode_modulo = generar_bytecode_para_modulo(fuente, id_modulo).unwrap();

        // Debe tener al menos una variable global (contador, nombre)
        assert!(
            !bytecode_modulo.global_var_indices.is_empty(),
            "Debe haber al menos una variable global registrada"
        );
        // Cada entrada debe ser (nombre -> mutable)
        for (_nombre, mutable) in &bytecode_modulo.global_var_indices {
            assert!(
                *mutable,
                "Las variables declaradas deben ser mutables, got {}",
                mutable
            );
        }
    }

    #[test]
    fn module_bytecode_desglosar() {
        let fuente = "\
fun foo(a, b) {
    escribir(a + b)
}

fun bar(x) {
    retornar x * 2
}
";
        let id_modulo = IdSimbolo(99);
        let bytecode_modulo = generar_bytecode_para_modulo(fuente, id_modulo).unwrap();

        let funciones = bytecode_modulo.desglosar();

        // Debe encontrar exactamente 2 funciones
        assert_eq!(
            funciones.len(),
            2,
            "desglosar() debe encontrar 2 funciones, encontró {}: {:?}",
            funciones.len(),
            funciones
                .iter()
                .map(|(n, _, _)| n.clone())
                .collect::<Vec<_>>()
        );

        // Verificar nombres
        let nombres: Vec<&str> = funciones.iter().map(|(n, _, _)| n.as_str()).collect();
        assert!(
            nombres.contains(&"foo"),
            "Debe contener 'foo', got {:?}",
            nombres
        );
        assert!(
            nombres.contains(&"bar"),
            "Debe contener 'bar', got {:?}",
            nombres
        );

        // foo tiene 2 parámetros → vars_size >= 2
        let (_name, _ip, vars_foo) = funciones.iter().find(|(n, _, _)| n == "foo").unwrap();
        assert!(
            *vars_foo >= 2,
            "foo debe tener vars_size >= 2, got {}",
            vars_foo
        );

        // bar tiene 1 parámetro → vars_size >= 1
        let (_name, _ip, vars_bar) = funciones.iter().find(|(n, _, _)| n == "bar").unwrap();
        assert!(
            *vars_bar >= 1,
            "bar debe tener vars_size >= 1, got {}",
            vars_bar
        );
    }

    #[test]
    fn declare_idx_global_serializacion_roundtrip() {
        let codigos = vec![
            Opcode::DeclararIndiceGlobal(0, true),   // idx 0, mutable
            Opcode::DeclararIndiceGlobal(5, false),  // idx 5, inmutable
            Opcode::DeclararIndiceGlobal(255, true), // idx 255, mutable
        ];

        let serialized = serializar_bytecode(&codigos);
        let deserialized = deserializar_bytecode(&serialized).unwrap();

        assert_eq!(deserialized.len(), 3, "Debe deserializar 3 codigos");
        assert_eq!(
            deserialized[0],
            Opcode::DeclararIndiceGlobal(0, true),
            "El primer opcode debe ser DeclareIdxGlobal(0, true)"
        );
        assert_eq!(
            deserialized[1],
            Opcode::DeclararIndiceGlobal(5, false),
            "El segundo opcode debe ser DeclareIdxGlobal(5, false)"
        );
        assert_eq!(
            deserialized[2],
            Opcode::DeclararIndiceGlobal(255, true),
            "El tercer opcode debe ser DeclareIdxGlobal(255, true)"
        );
    }

    #[test]
    fn opcodes_especializados_serializacion_roundtrip() {
        let codigos = vec![
            Opcode::SumarEntero,
            Opcode::RestarDecimal,
            Opcode::EmpujarSumarEntero(42),
            Opcode::DuplicarSumarEntero,
            Opcode::CargarIndiceEntero(10),
            Opcode::GuardarIndiceDecimal(20),
            Opcode::IgualEntero,
            Opcode::MenorDecimal,
            Opcode::Intentar,
            Opcode::ParsearEntero,
        ];

        let serialized = serializar_bytecode(&codigos);
        let deserialized = deserializar_bytecode(&serialized).unwrap();

        assert_eq!(
            deserialized.len(),
            codigos.len(),
            "La cantidad de codigos debe coincidir"
        );
        assert_eq!(deserialized[0], Opcode::SumarEntero);
        assert_eq!(deserialized[1], Opcode::RestarDecimal);
        assert_eq!(deserialized[2], Opcode::EmpujarSumarEntero(42));
        assert_eq!(deserialized[3], Opcode::DuplicarSumarEntero);
        assert_eq!(deserialized[4], Opcode::CargarIndiceEntero(10));
        assert_eq!(deserialized[5], Opcode::GuardarIndiceDecimal(20));
        assert_eq!(deserialized[6], Opcode::IgualEntero);
        assert_eq!(deserialized[7], Opcode::MenorDecimal);
        assert_eq!(deserialized[8], Opcode::Intentar);
        assert_eq!(deserialized[9], Opcode::ParsearEntero);
    }

    #[test]
    fn module_bytecode_serializacion() {
        let fuente = "\
variable global_x = 42

fun probar() {
    retornar global_x
}
";
        let id_modulo = IdSimbolo(100);
        let bytecode_modulo = generar_bytecode_para_modulo(fuente, id_modulo).unwrap();

        // Verificar que tiene DeclareIdxGlobal
        let tiene_declarar_indice = bytecode_modulo
            .codigos
            .iter()
            .any(|op| matches!(op, Opcode::DeclararIndiceGlobal(_, _)));
        assert!(
            tiene_declarar_indice,
            "El bytecode del módulo debe contener DeclareIdxGlobal"
        );

        // Serialización round-trip
        let serialized = serializar_bytecode(&bytecode_modulo.codigos);
        let deserialized = deserializar_bytecode(&serialized).unwrap();
        assert_eq!(bytecode_modulo.codigos.len(), deserialized.len(),
            "La cantidad de codigos debe coincidir tras round-trip. Original: {}, deserializado: {}",
            bytecode_modulo.codigos.len(), deserialized.len());

        // Verificar que los idx globales sobreviven
        for op in &deserialized {
            if let Opcode::DeclararIndiceGlobal(idx, mutable) = op {
                assert!(*idx == 0, "El único global debe ser idx=0, got {}", idx);
                assert!(*mutable, "global_x debe ser mutable, got {}", mutable);
            }
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // Tests de sobrecarga automática por tipo
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn sobrecarga_funciones_con_tipos_distintos() {
        // Dos funciones con mismo nombre pero diferentes tipos de parámetros.
        // TypeChecker debe recolectar ambos como overloads y el bytecode
        // debe resolver el nombre mangled correcto para cada llamada.
        let fuente = "\
funcion mcd(a: Texto, b: Texto) -> Texto {
    retornar a
}
funcion mcd(a, b) {
    retornar a + b
}
variable x = mcd(\"a\", \"b\")
variable y = mcd(10, 20)
";
        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();

        let mut verificador = TypeChecker::new();
        // Debe analizar sin errores: ambas funciones deben coexistir
        verificador.analizar(&programa).unwrap();
        let tipos_inferidos = verificador.obtener_tipos_inferidos();
        let funciones_overload = verificador.obtener_funciones();

        // Verificar que mcd tiene 2 overloads
        let overloads = funciones_overload.get("mcd");
        assert!(overloads.is_some(), "mcd debe estar en funciones_overload");
        assert_eq!(overloads.unwrap().len(), 2, "mcd debe tener 2 overloads");

        // Generar bytecode con overloads
        let mut gen = GeneradorBytecode::new();
        gen.set_tipos_inferidos(tipos_inferidos);
        gen.set_funciones_overload(funciones_overload);
        gen.generar(&programa).unwrap();

        let bytecode = gen.codigos.clone();

        // Verificar que las definiciones/funciones con nombre mangled existen
        let tiene_mcd_ov0 = bytecode.iter().any(|op| match op {
            Opcode::DefinirFuncion(n, _) | Opcode::Llamar(n, _) => n.as_ref() == "mcd$ov0",
            _ => false,
        });
        let tiene_mcd_ov1 = bytecode.iter().any(|op| match op {
            Opcode::DefinirFuncion(n, _) | Opcode::Llamar(n, _) => n.as_ref() == "mcd$ov1",
            _ => false,
        });

        assert!(
            tiene_mcd_ov0,
            "Se esperaba 'mcd$ov0' en FunctionDef o Call, bytecode: {:?}",
            bytecode
        );
        assert!(
            tiene_mcd_ov1,
            "Se esperaba 'mcd$ov1' en FunctionDef o Call, bytecode: {:?}",
            bytecode
        );
    }

    #[test]
    fn sobrecarga_sin_conflicto_con_nombre_sin_tipo() {
        // Una función sin tipo y otra con tipo deben resolverse correctamente
        // cuando se llaman con tipos que matchean exactamente.
        let fuente = "\
    funcion sumar(a: Texto, b: Texto) -> Texto {
        retornar a + b
    }
    funcion sumar(a, b) {
        retornar a + b
    }
    variable texto = sumar(\"hola \", \"mundo\")
    variable entero = sumar(3, 4)
    ";
        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();

        let mut verificador = TypeChecker::new();
        verificador.analizar(&programa).unwrap();
        let funciones_overload = verificador.obtener_funciones();

        let overloads = funciones_overload.get("sumar");
        assert!(overloads.is_some(), "sumar debe estar en overloads");
        assert_eq!(overloads.unwrap().len(), 2, "sumar debe tener 2 overloads");

        let mut gen = GeneradorBytecode::new();
        gen.set_tipos_inferidos(verificador.obtener_tipos_inferidos());
        gen.set_funciones_overload(funciones_overload);
        gen.generar(&programa).unwrap();

        let bytecode = gen.codigos.clone();
        let tiene_sumar_ov0 = bytecode.iter().any(|op| match op {
            Opcode::DefinirFuncion(n, _) | Opcode::Llamar(n, _) => n.as_ref() == "sumar$ov0",
            _ => false,
        });
        let tiene_sumar_ov1 = bytecode.iter().any(|op| match op {
            Opcode::DefinirFuncion(n, _) | Opcode::Llamar(n, _) => n.as_ref() == "sumar$ov1",
            _ => false,
        });

        assert!(
            tiene_sumar_ov0,
            "sumar$ov0 debe existir en bytecode, bytecode: {:?}",
            bytecode
        );
        assert!(
            tiene_sumar_ov1,
            "sumar$ov1 debe existir en bytecode, bytecode: {:?}",
            bytecode
        );
    }

    #[test]
    fn sobrecarga_funcion_unica_sin_mangling() {
        // Una sola función sin overloads debe mantener su nombre original
        // (sin sufijo $ov)
        let fuente = "\
    funcion saludar(nombre: Texto) {
        escribir(nombre)
    }
    saludar(\"Pepe\")
    ";
        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();

        let mut verificador = TypeChecker::new();
        verificador.analizar(&programa).unwrap();
        let funciones_overload = verificador.obtener_funciones();

        // Verificar que sigue teniendo solo 1 overload
        let overloads = funciones_overload.get("saludar");
        assert!(overloads.is_some());
        assert_eq!(overloads.unwrap().len(), 1);

        let mut gen = GeneradorBytecode::new();
        gen.set_tipos_inferidos(verificador.obtener_tipos_inferidos());
        gen.set_funciones_overload(funciones_overload);
        gen.generar(&programa).unwrap();

        let bytecode = gen.codigos.clone();

        // Debe llamarse "saludar", no "saludar$ov0"
        let tiene_saludar_ov0 = bytecode.iter().any(|op| match op {
            Opcode::DefinirFuncion(n, _) | Opcode::Llamar(n, _) => n.as_ref() == "saludar$ov0",
            _ => false,
        });
        let tiene_saludar_original = bytecode.iter().any(|op| match op {
            Opcode::DefinirFuncion(n, _) => n.as_ref() == "saludar",
            _ => false,
        });

        assert!(
            !tiene_saludar_ov0,
            "saludar no debe tener $ov0 porque no tiene overloads"
        );
        assert!(
            tiene_saludar_original,
            "saludar debe mantener su nombre original"
        );
    }
}
