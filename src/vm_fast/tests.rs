// Tests de la VM canónica `ForjaFast`.
//
// Extraído de `vm_fast.rs` (F1-T7) para reducir el tamaño del monolito.
// Al ser un submódulo de `vm_fast`, los tests conservan acceso a los items
// privados del módulo padre (campos de `ForjaFast`, helpers internos, etc.).

use super::*;

/// Compila un programa con el pipeline local del crate (lexer → parser →
/// type checker → bytecode + optimizaciones), replicando
/// `forja::compilar_pipeline`. Usa los módulos del crate actual para que el
/// tipo `bytecode::Opcode` coincida con el que consume la VM (válido tanto
/// en la lib como en el binario, que declara sus propios módulos).
fn compilar_programa(source: &str) -> Vec<bytecode::Opcode> {
    use bytecode::GeneradorBytecode;

    let mut lexer = crate::lexer::Lexer::new(source);
    let tokens = lexer.tokenize().expect("el programa debe tokenizar");
    let mut parser = crate::parser::Parser::new(tokens);
    let programa = parser.parse().expect("el programa debe parsear");

    let mut type_checker = crate::semantics::TypeChecker::new();
    type_checker
        .analizar(&programa)
        .expect("el programa debe pasar el type checker");
    let tipos_inferidos = type_checker.obtener_tipos_inferidos();
    let funciones_overload = type_checker.obtener_funciones();

    let mut gen = GeneradorBytecode::new();
    gen.set_tipos_inferidos(tipos_inferidos);
    gen.set_funciones_overload(funciones_overload);
    let bytecode = gen
        .generar(&programa)
        .expect("el programa debe generar bytecode");

    let globales: Vec<(String, bool)> = programa
        .declaraciones
        .iter()
        .filter_map(|d| match d {
            crate::ast::Declaracion::Variable {
                nombre, mutable, ..
            } => Some((nombre.clone(), *mutable)),
            _ => None,
        })
        .collect();
    let bytecode = bytecode::postprocesar_globales(bytecode, &globales);

    // Punto único de optimización de bytecode (F4-T3).
    crate::optimizer_pipeline::optimizar_bytecode(bytecode)
}

/// Compila, carga y ejecuta un programa con fast-math configurado
/// (`None` = modo automático). Retorna (output, fast_math_activo).
fn ejecutar_con_fast_math(source: &str, fast_math: Option<bool>) -> (Vec<String>, bool) {
    let bytecode = compilar_programa(source);
    let mut vm = ForjaFast::new();
    if let Some(b) = fast_math {
        vm.set_fast_math(b);
    }
    vm.cargar_bytecode(bytecode);
    let activo = vm.fast_math();
    vm.ejecutar().expect("el programa debe ejecutar");
    (vm.obtener_output().to_vec(), activo)
}

#[test]
fn fast_math_manual_omite_check_division_por_cero_float() {
    let src = "variable x: Decimal = 1.0\nvariable y: Decimal = 0.0\nescribir(x / y)\n";

    // Sin fast-math: x / 0.0 → nulo (comportamiento estándar)
    let (out, activo) = ejecutar_con_fast_math(src, Some(false));
    assert!(!activo);
    assert_eq!(out[0], "nulo");

    // Con fast-math manual: x / 0.0 → inf (IEEE 754, sin branches)
    let (out, activo) = ejecutar_con_fast_math(src, Some(true));
    assert!(activo);
    assert_eq!(out[0], "inf");
}

#[test]
fn fast_math_automatico_solo_con_tipos_decimal() {
    // Programa 100% Decimal → fast-math ON automáticamente
    let src = "variable a: Decimal = 2.5\nvariable b: Decimal = 1.5\nescribir(a * b)\n";
    let (out, activo) = ejecutar_con_fast_math(src, None);
    assert!(activo, "programa 100% Decimal debe activar fast-math");
    assert_eq!(out[0], "3.75");

    // Con Entero → fast-math OFF
    let src = "variable i: Entero = 1\nvariable j: Entero = 2\nescribir(i + j)\n";
    let (out, activo) = ejecutar_con_fast_math(src, None);
    assert!(!activo);
    assert_eq!(out[0], "3");

    // Con Texto → fast-math OFF
    let src = "variable s: Texto = \"hola\"\nescribir(s)\n";
    let (out, activo) = ejecutar_con_fast_math(src, None);
    assert!(!activo);
    assert_eq!(out[0], "hola");
}

#[test]
fn fast_math_division_entera_por_cero_sigue_protegida() {
    // La división entera conserva el check incluso con fast-math manual
    // (wrapping_div panica con divisor 0 en Rust).
    let src = "variable a: Entero = 5\nvariable b: Entero = 0\nescribir(a / b)\n";
    let (out, activo) = ejecutar_con_fast_math(src, Some(true));
    assert!(activo);
    assert_eq!(out[0], "nulo");
}

/*  */
#[test]
fn fast_math_automatico_no_cambia_resultados() {
    // Regresión: el modo automático NO puede cambiar el resultado de un
    // programa. Incluye valores enteros circulando en contexto float
    // (i % 2, literales) — el auto solo omite el check de división por
    // cero, nunca los checks de tipo.
    let src = concat!(
        "variable N = 50.0\n",
        "variable suma = 0.0\n",
        "variable i = 0.0\n",
        "mientras (i < N) {\n",
        "    variable signo = 1.0 - 2.0 * (i % 2)\n",
        "    variable divisor = 1.0 + 2.0 * i\n",
        "    suma = suma + signo / divisor\n",
        "    i = i + 1\n",
        "}\n",
        "escribir(suma)\n",
    );
    let (out_sin, activo) = ejecutar_con_fast_math(src, Some(false));
    assert!(!activo);
    let (out_auto, activo_auto) = ejecutar_con_fast_math(src, None);
    assert!(activo_auto, "programa 100% Decimal debe activar fast-math");
    assert_eq!(
        out_auto, out_sin,
        "el modo automático debe producir el mismo output que sin fast-math"
    );
}

#[test]
fn fast_math_automatico_omite_check_division_por_cero() {
    // El único cambio semántico del modo automático: x / 0.0 → inf en
    // vez de nulo (sin branches). Los tipos se siguen verificando.
    let src = "variable x: Decimal = 1.0\nvariable y: Decimal = 0.0\nescribir(x / y)\n";
    let (out, activo) = ejecutar_con_fast_math(src, None);
    assert!(activo);
    assert_eq!(out[0], "inf");
}

// Programa de diagnóstico: aproximación de pi por la serie de Leibniz (50 términos).
const LB50_SRC: &str = "variable suma = 0.0
variable divisor = 1.0
variable i = 0
mientras (i < 50) {
    suma = suma + 1.0 / divisor
    divisor = divisor + 4.0
    i = i + 1
}
variable pi = suma * 4.0
escribir(pi)
";

// Programa de diagnóstico: variante de la serie de Leibniz con términos alternados.
const LEIBNIZ_TEST_SRC: &str = "constante N = 100

variable suma = 0.0
variable divisor_pos = 1.0
variable divisor_neg = 3.0
variable i = 0

mientras (i < 50) {
    suma = suma + 1.0 / divisor_pos
    suma = suma - 1.0 / divisor_neg
    divisor_pos = divisor_pos + 4.0
    divisor_neg = divisor_neg + 4.0
    i = i + 1
}

variable pi = suma * 4.0
escribir(pi)
";

#[test]
fn dump_bytecode_leibniz_variants() {
    use std::io::Write;
    for (nombre, contenido) in [("lb50", LB50_SRC), ("leibniz_test", LEIBNIZ_TEST_SRC)] {
        let bc = compilar_programa(contenido);
        let mut out = String::new();
        for (i, op) in bc.iter().enumerate() {
            out.push_str(&format!("{:3} {:?}\n", i, op));
        }
        let destino = format!("{}/scratch/{}_bc.txt", env!("CARGO_MANIFEST_DIR"), nombre);
        if let Some(dir) = std::path::Path::new(&destino).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut f = std::fs::File::create(&destino).expect("crear dump");
        f.write_all(out.as_bytes()).expect("escribir");
    }
}

#[test]
fn dump_bytecode_post_quickening() {
    use std::io::Write;
    for (nombre, contenido) in [("lb50", LB50_SRC), ("leibniz_test", LEIBNIZ_TEST_SRC)] {
        let bc = compilar_programa(contenido);
        let mut vm = ForjaFast::new();
        vm.show_bytecode = true;
        vm.cargar_bytecode(bc);
        let mut out = String::new();
        for (i, op) in vm.bytecode.iter().enumerate() {
            out.push_str(&format!("{:3} {:?}\n", i, op));
        }
        let destino = format!(
            "{}/scratch/{}_final_bc.txt",
            env!("CARGO_MANIFEST_DIR"),
            nombre
        );
        if let Some(dir) = std::path::Path::new(&destino).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut f = std::fs::File::create(&destino).expect("crear dump");
        f.write_all(out.as_bytes()).expect("escribir");
    }
}

#[test]
fn trace_lb50_vs_leibniz_test() {
    for (nombre, contenido) in [("lb50", LB50_SRC), ("leibniz_test", LEIBNIZ_TEST_SRC)] {
        let bc = compilar_programa(contenido);
        let mut vm = ForjaFast::new();
        vm.show_bytecode = true;
        vm.cargar_bytecode(bc);
        eprintln!("===== {} =====", nombre);
        let r = vm.ejecutar();
        eprintln!(
            "===== {} result: {:?} output: {:?} =====",
            nombre,
            r,
            vm.obtener_output()
        );
    }
}

#[test]
fn errores_estrictos_division_por_cero() {
    // F2-T7: con `errores_estrictos` activo, dividir entre un cero calculado
    // en runtime lanza `ErrFast::DivCero` en lugar de devolver nulo.
    let src = "funcion cero() {\n    retornar 0\n}\n\nfuncion dividir(a, b) {\n    retornar a / b\n}\n\nvariable r = dividir(10, cero())\nescribir(r)\n";
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.set_errores_estrictos(true);
    vm.cargar_bytecode(bc);
    let r = vm.ejecutar();
    assert!(
        matches!(r, Err(ErrFast::DivCero)),
        "se esperaba ErrFast::DivCero, se obtuvo {:?}",
        r
    );
}

#[test]
fn errores_estrictos_division_por_cero_desactivado_devuelve_nulo() {
    // Sin modo estricto, el comportamiento por defecto se conserva: nulo.
    let src = "funcion cero() {\n    retornar 0\n}\n\nfuncion dividir(a, b) {\n    retornar a / b\n}\n\nvariable r = dividir(10, cero())\nescribir(r)\n";
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.cargar_bytecode(bc);
    let r = vm.ejecutar();
    assert!(r.is_ok(), "sin modo estricto no debe fallar: {:?}", r);
    assert_eq!(vm.obtener_output(), vec!["nulo".to_string()]);
}

#[test]
fn errores_estrictos_indice_fuera_de_rango() {
    // F2-T7: con `errores_estrictos` activo, acceder a un índice fuera de
    // rango lanza `ErrFast::IdxOut` en lugar de devolver nulo.
    let src = "variable arr = [1, 2, 3]\nvariable i = 99\nescribir(arr[i])\n";
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.set_errores_estrictos(true);
    vm.cargar_bytecode(bc);
    let r = vm.ejecutar();
    assert!(
        matches!(r, Err(ErrFast::IdxOut(_))),
        "se esperaba ErrFast::IdxOut, se obtuvo {:?}",
        r
    );
}

#[test]
fn fast_math_override_manual_persiste_despues_de_reset() {
    let bytecode = compilar_programa("escribir(1)\n");
    let mut vm = ForjaFast::new();
    vm.set_fast_math(true);
    vm.cargar_bytecode(bytecode);
    assert!(vm.fast_math());
    vm.reset();
    assert!(
        vm.fast_math(),
        "el override manual debe persistir tras reset()"
    );
}

#[test]
fn pgo_recolecta_perfil_de_ejecucion() {
    let src = r#"
funcion fib(n) {
    si (n <= 1) { retornar n }
    variable a = 0
    variable b = 1
    variable i = 2
    mientras (i <= n) {
        variable t = a + b
        a = b
        b = t
        i = i + 1
    }
    retornar b
}
variable r = fib(20)
"#;
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.set_max_inst(1_000_000_000);
    vm.cargar_bytecode(bc);
    vm.habilitar_pgo();
    vm.ejecutar().expect("el programa debe ejecutar");

    let perfil = vm.finalizar_pgo().expect("debe recolectar un perfil");
    // fib(20) se llama 1 vez + el while itera 18 veces → hay branches y loops
    assert!(
        !perfil.function_hotness.is_empty(),
        "debe registrar llamadas a funciones"
    );
    assert!(
        !perfil.branch_counts.is_empty(),
        "debe registrar branches (si/mientras)"
    );
    assert!(
        !perfil.loop_iterations.is_empty(),
        "debe registrar iteraciones de loop (back-edge del mientras)"
    );
    assert!(
        !perfil.hot_ips.is_empty(),
        "el instrumenter debe muestrear IPs calientes"
    );
}

#[test]
fn pgo_aplica_perfil_pre_especializando_ips_calientes() {
    let src = "variable s = 0\nvariable i = 0\nmientras (i < 10000) {\n    s = s + i\n    i = i + 1\n}\nescribir(s)\n";
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.set_max_inst(1_000_000_000);
    vm.cargar_bytecode(bc);
    vm.habilitar_pgo();
    vm.ejecutar().expect("el programa debe ejecutar");
    let perfil = vm.finalizar_pgo().expect("perfil recolectado");

    // Nueva VM: aplicar el perfil pre-especializa los IPs calientes
    let bc2 = compilar_programa(src);
    let mut vm2 = ForjaFast::new();
    vm2.set_max_inst(1_000_000_000);
    vm2.cargar_bytecode(bc2);
    let len_before = vm2.contador_especializacion.len();
    vm2.aplicar_pgo(&perfil);
    assert_eq!(vm2.contador_especializacion.len(), len_before);
    // Algunos IPs calientes deben quedar al umbral (listos para especializar)
    let preespecializados = vm2
        .contador_especializacion
        .iter()
        .filter(|&&c| c == vm2.umbral_especializacion)
        .count();
    assert!(
        preespecializados > 0,
        "los IPs calientes deben quedar pre-especializados"
    );
    vm2.ejecutar()
        .expect("el programa debe ejecutar con perfil aplicado");
    assert!(!vm2.obtener_output().is_empty(), "debe producir salida");
}

#[test]
fn try_operador_desempaqueta_resultado() {
    // Regresión: el DCE eliminaba funciones llamadas solo vía `?` porque
    // el recolector de usos ignoraba Expresion::Try.
    // F1-T4 (S6): el `?` debe estar dentro de una función que devuelva
    // Resultado/Opcion, por lo que envolvemos el desempaquetado.
    let src = r#"
funcion dividir_seguro(a, b) -> Resultado<Entero, Texto> {
    si (b == 0) { retornar Error("division por cero") }
    retornar Ok(a / b)
}

funcion calcular() -> Resultado<Entero, Texto> {
    variable r = dividir_seguro(10, 2)?
    retornar Ok(r)
}

variable res = calcular()
coincidir (res) {
    caso Ok(n) -> escribir(n)
    caso Error(m) -> escribir(m)
}
"#;
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.set_max_inst(1_000_000_000);
    vm.cargar_bytecode(bc);
    vm.ejecutar().expect("el programa debe ejecutar");
    let out = vm.obtener_output();
    assert_eq!(out, &["5"], "Ok(10/2)? debe desempaquetar a 5");
}

#[test]
fn try_operador_propaga_error_y_corta_la_funcion() {
    // Caso CRÍTICO: `?` sobre un `Error` debe PROPAGAR el error
    // (devolverlo como valor de retorno de `llamador`) y NO ejecutar el
    // código posterior a la línea del `?` dentro de la función.
    let src = r#"
funcion fallar() -> Resultado<Entero, Texto> {
    retornar Error("x")
}

funcion llamador() -> Resultado<Entero, Texto> {
    variable v = fallar()?
    escribir("NO DEBE EJECUTARSE")
    retornar Ok(v + 1)
}

variable res = llamador()
coincidir (res) {
    caso Ok(n) -> escribir("ok:${n}")
    caso Error(m) -> escribir("err:${m}")
}
"#;
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.set_max_inst(1_000_000_000);
    vm.cargar_bytecode(bc);
    vm.ejecutar().expect("el programa debe ejecutar");
    let out = vm.obtener_output();
    assert_eq!(
        out,
        &["err:x"],
        "el `?` debe propagar el Error y saltarse el resto de la función"
    );
}

#[test]
fn try_operador_propaga_ninguno_de_opcion() {
    // Caso análogo para `Opcion`: `?` sobre `Ninguno` propaga el `none` y
    // no ejecuta el código posterior dentro de la función.
    let src = r#"
funcion buscar(objetivo) -> Opcion<Entero> {
    si (objetivo == 0) { retornar Ninguno() }
    retornar Algo(objetivo)
}

funcion obtener(objetivo) -> Opcion<Entero> {
    variable v = buscar(objetivo)?
    escribir("NO DEBE EJECUTARSE")
    retornar Algo(v + 100)
}

variable res = obtener(0)
coincidir (res) {
    caso Alguno(n) -> escribir("algo:${n}")
    caso Ninguno -> escribir("nada")
}
"#;
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.set_max_inst(1_000_000_000);
    vm.cargar_bytecode(bc);
    vm.ejecutar().expect("el programa debe ejecutar");
    let out = vm.obtener_output();
    assert_eq!(
        out,
        &["nada"],
        "el `?` debe propagar Ninguno y saltarse el resto de la función"
    );
}

#[test]
fn match_enum_literal_en_subpatron() {
    // Regresión: literales en subpatrones de match (caso Heroe("Iron", poder))
    // y construcción de variantes de enum con datos.
    let src = r#"
tipo Personaje = Heroe(Texto, Entero) | Villano(Texto, Entero)
funcion clasificar(p) -> Texto {
    coincidir (p) {
        caso Heroe("Iron", poder) -> retornar "iron_${poder}"
        caso Heroe(nombre, poder) -> retornar "heroe_${nombre}_${poder}"
        caso Villano("Joker", maldad) -> retornar "joker_${maldad}"
        caso Villano(nombre, maldad) -> retornar "villano_${nombre}_${maldad}"
    }
}
funcion main() {
    escribir(clasificar(Heroe("Iron", 90)))
    escribir(clasificar(Heroe("Thor", 85)))
    escribir(clasificar(Villano("Joker", 70)))
    escribir(clasificar(Villano("Lex", 60)))
}
main()
"#;
    let bc = compilar_programa(src);
    let mut vm = ForjaFast::new();
    vm.set_max_inst(1_000_000_000);
    vm.cargar_bytecode(bc);
    vm.ejecutar().expect("el programa debe ejecutar");
    let out = vm.obtener_output();
    assert_eq!(
        out,
        &["iron_90", "heroe_Thor_85", "joker_70", "villano_Lex_60"],
        "los literales en subpatrones deben seleccionar el brazo correcto"
    );
}
