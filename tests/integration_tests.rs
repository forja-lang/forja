use forja::bytecode::{fusionar_opcodes, optimizar_indices, GeneradorBytecode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::vm_fast::ForjaFast;

/// Helper: ejecuta código Forja en la VM y devuelve output
fn ejecutar(source: &str) -> Result<Vec<String>, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

    let mut parser = Parser::new(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    let mut gen = GeneradorBytecode::new();
    let bytecode = gen
        .generar(&programa)
        .map_err(|_| "Error generando bytecode".to_string())?;
    let bytecode = optimizar_indices(&bytecode);
    let bytecode = fusionar_opcodes(&bytecode);

    let mut vm = ForjaFast::new();
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;

    Ok(vm.obtener_output().to_vec())
}

// ============================================================
// Tests E2E — Pipeline completo: .fa → VM
// ============================================================

#[test]
fn test_e2e_cuando() {
    let out = ejecutar(
        "variable x = 35
         cuando (x > 30) {
             escribir(\"Caliente\")
         }",
    )
    .unwrap();
    assert_eq!(out, vec!["Caliente"]);
}

#[test]
fn test_e2e_hola_mundo() {
    let out = ejecutar("escribir(\"Hola, mundo!\")").unwrap();
    assert_eq!(out, vec!["Hola, mundo!"]);
}

#[test]
fn test_e2e_aritmetica() {
    let out = ejecutar(
        "variable x = 2 + 3
         escribir(x)",
    )
    .unwrap();
    assert_eq!(out, vec!["5"]);
}

#[test]
fn test_e2e_aritmetica_compleja() {
    let out = ejecutar(
        "variable x = (2 + 3) * 4
         escribir(x)",
    )
    .unwrap();
    assert_eq!(out, vec!["20"]);
}

#[test]
fn test_e2e_variable_mutable() {
    let out = ejecutar(
        "variable x = 5
         x = 10
         escribir(x)",
    )
    .unwrap();
    assert_eq!(out, vec!["10"]);
}

#[test]
fn test_e2e_si_verdadero() {
    let out = ejecutar("si (verdadero) { escribir(\"si\") } sino { escribir(\"no\") }").unwrap();
    assert_eq!(out, vec!["si"]);
}

#[test]
fn test_e2e_si_falso() {
    let out = ejecutar("si (falso) { escribir(\"si\") } sino { escribir(\"no\") }").unwrap();
    assert_eq!(out, vec!["no"]);
}

#[test]
fn test_e2e_mientras() {
    let out = ejecutar(
        "variable x = 0
         mientras (x < 3) {
             escribir(x)
             x = x + 1
         }",
    )
    .unwrap();
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn test_e2e_repetir() {
    let out = ejecutar("repetir (3) { escribir(\"hola\") }").unwrap();
    assert_eq!(out, vec!["hola", "hola", "hola"]);
}

#[test]
fn test_e2e_para() {
    let out = ejecutar("para (variable i = 0; i < 3; i = i + 1) { escribir(i) }").unwrap();
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn test_e2e_comparacion() {
    let out = ejecutar(
        "escribir(5 > 3)
         escribir(2 > 10)",
    )
    .unwrap();
    assert_eq!(out, vec!["verdadero", "falso"]);
}

#[test]
fn test_e2e_funcion_simple() {
    let out = ejecutar(
        "funcion suma(a, b) {
             retornar a + b
         }
         variable r = suma(3, 4)
         escribir(r)",
    )
    .unwrap();
    assert_eq!(out, vec!["7"]);
}

#[test]
fn test_e2e_funcion_saludo() {
    let out = ejecutar(
        "funcion saludar(nombre) {
             escribir(nombre)
         }
         saludar(\"Ana\")
         saludar(\"Pedro\")",
    )
    .unwrap();
    assert_eq!(out, vec!["Ana", "Pedro"]);
}

#[test]
fn test_e2e_clase_sin_constructor() {
    let out = ejecutar(
        "clase Punto { x y }
         variable p = nuevo Punto()
         escribir(p.x)",
    )
    .unwrap();
    assert_eq!(out, vec!["nulo"]);
}

#[test]
fn test_e2e_multiples_prints() {
    let out = ejecutar(
        "escribir(\"a\")
         escribir(\"b\")
         escribir(\"c\")",
    )
    .unwrap();
    assert_eq!(out, vec!["a", "b", "c"]);
}

#[test]
fn test_e2e_si_anidado() {
    let out = ejecutar(
        "variable nota = 85
         si (nota >= 90) {
             escribir(\"Excelente\")
         } sino {
             si (nota >= 70) {
                 escribir(\"Buen trabajo\")
             } sino {
                 escribir(\"Sigue intentando\")
             }
         }",
    )
    .unwrap();
    assert_eq!(out, vec!["Buen trabajo"]);
}

#[test]
fn test_e2e_concatenacion_texto() {
    let out = ejecutar(
        "variable saludo = \"Hola\"
         variable nombre = \"Mundo\"
         escribir(saludo + \" \" + nombre)",
    )
    .unwrap();
    assert_eq!(out, vec!["Hola Mundo"]);
}

#[test]
fn test_e2e_decimales() {
    let out = ejecutar(
        "variable pi = 3.14
         escribir(pi)",
    )
    .unwrap();
    assert_eq!(out, vec!["3.14"]);
}

// ============================================================
// String API Tests
// ============================================================

#[test]
fn test_string_length() {
    let out = ejecutar("escribir(\"hola\".longitud())").unwrap();
    assert_eq!(out, vec!["4"]);
}

#[test]
fn test_string_to_upper() {
    let out = ejecutar("escribir(\"hola\".a_mayusculas())").unwrap();
    assert_eq!(out, vec!["HOLA"]);
}

#[test]
fn test_string_to_lower() {
    let out = ejecutar("escribir(\"HOLA\".a_minusculas())").unwrap();
    assert_eq!(out, vec!["hola"]);
}

#[test]
fn test_string_trim() {
    let out = ejecutar("escribir(\"  hola  \".recortar())").unwrap();
    assert_eq!(out, vec!["hola"]);
}

#[test]
fn test_string_contains_true() {
    let out = ejecutar("escribir(\"hello world\".contiene(\"world\"))").unwrap();
    assert_eq!(out, vec!["verdadero"]);
}

#[test]
fn test_string_contains_false() {
    let out = ejecutar("escribir(\"hello world\".contiene(\"xyz\"))").unwrap();
    assert_eq!(out, vec!["falso"]);
}

#[test]
fn test_string_reverse() {
    let out = ejecutar("escribir(\"rust\".invertir())").unwrap();
    assert_eq!(out, vec!["tsur"]);
}

#[test]
fn test_string_starts_with() {
    let out = ejecutar("escribir(\"hola mundo\".empieza_con(\"hola\"))").unwrap();
    assert_eq!(out, vec!["verdadero"]);
    let out2 = ejecutar("escribir(\"hola mundo\".empieza_con(\"xyz\"))").unwrap();
    assert_eq!(out2, vec!["falso"]);
}

#[test]
fn test_string_ends_with() {
    let out = ejecutar("escribir(\"hola mundo\".termina_con(\"mundo\"))").unwrap();
    assert_eq!(out, vec!["verdadero"]);
    let out2 = ejecutar("escribir(\"hola mundo\".termina_con(\"xyz\"))").unwrap();
    assert_eq!(out2, vec!["falso"]);
}

#[test]
fn test_string_char_at() {
    let out = ejecutar("escribir(\"hola\".caracter_en(0))").unwrap();
    assert_eq!(out, vec!["h"]);
    let out2 = ejecutar("escribir(\"hola\".caracter_en(3))").unwrap();
    assert_eq!(out2, vec!["a"]);
}

#[test]
fn test_string_index_of() {
    let out = ejecutar("escribir(\"hello world\".indice_de(\"world\"))").unwrap();
    assert_eq!(out, vec!["6"]);
    let out2 = ejecutar("escribir(\"hello\".indice_de(\"xyz\"))").unwrap();
    assert_eq!(out2, vec!["-1"]);
}

#[test]
fn test_string_substr() {
    let out = ejecutar("escribir(\"hello world\".subcadena(6, 5))").unwrap();
    assert_eq!(out, vec!["world"]);
}

#[test]
fn test_string_replace() {
    let out = ejecutar("escribir(\"hola mundo\".reemplazar(\"mundo\", \"forja\"))").unwrap();
    assert_eq!(out, vec!["hola forja"]);
}

#[test]
fn test_string_parse_entero() {
    let out = ejecutar("escribir(\"42\".a_entero())").unwrap();
    assert_eq!(out, vec!["42"]);
}

#[test]
fn test_string_parse_flotante() {
    let out = ejecutar("escribir(\"3.14\".a_flotante())").unwrap();
    assert_eq!(out, vec!["3.14"]);
}

#[test]
fn test_array_join() {
    let out =
        ejecutar("variable arr = [\"a\", \"b\", \"c\"]\nescribir(arr.unir_elementos(\", \"))")
            .unwrap();
    assert_eq!(out, vec!["a, b, c"]);
}

// ============================================================
// Array Tests
// ============================================================

#[test]
fn test_array_literal() {
    let out = ejecutar("variable arr = [1, 2, 3]\nescribir(arr)").unwrap();
    assert_eq!(out, vec!["[1,2,3]"]);
}

#[test]
fn test_array_get() {
    let out = ejecutar("variable arr = [10, 20, 30]\nescribir(arr[1])").unwrap();
    assert_eq!(out, vec!["20"]);
}

#[test]
fn test_array_set() {
    let out = ejecutar(
        "variable arr = [1, 2, 3]
         arr[1] = 99
         escribir(arr[1])",
    )
    .unwrap();
    assert_eq!(out, vec!["99"]);
}

#[test]
fn test_array_out_of_bounds() {
    // Ahora retorna Nulo para índice fuera de rango
    let out = ejecutar("variable arr = [1, 2]\nescribir(arr[99])").unwrap();
    assert_eq!(out, vec!["nulo"]);
}

#[test]
fn test_array_empty() {
    let out = ejecutar("variable arr = []\nescribir(arr)").unwrap();
    assert_eq!(out, vec!["[]"]);
}

// ============================================================
// Mapa Tests
// ============================================================

#[test]
fn test_mapa_literal() {
    let out = ejecutar(
        "variable m = {\"nombre\": \"Ana\", \"edad\": 30}
         escribir(m[\"nombre\"])",
    )
    .unwrap();
    assert_eq!(out, vec!["Ana"]);
}

#[test]
fn test_mapa_get_edad() {
    let out = ejecutar(
        "variable m = {\"x\": 10, \"y\": 20}
         escribir(m[\"y\"])",
    )
    .unwrap();
    assert_eq!(out, vec!["20"]);
}

// ============================================================
// Formatter Integration Tests
// ============================================================

fn assert_format_roundtrip(source: &str) {
    let mut lexer1 = Lexer::new(source);
    let tokens1 = lexer1
        .tokenize()
        .unwrap_or_else(|e| panic!("Lexer error: {:?}", e));
    let mut parser1 = Parser::new(tokens1);
    let ast1 = parser1
        .parse()
        .unwrap_or_else(|e| panic!("Parser error: {:?}", e));

    let mut fmt = forja::formatter::Formatter::new();
    let formatted = fmt.formatear(&ast1);

    // Format second time to test stability (idempotency)
    let mut lexer2 = Lexer::new(&formatted);
    let tokens2 = lexer2
        .tokenize()
        .unwrap_or_else(|e| panic!("Lexer error on formatted: {:?}\nCode:\n{}", e, formatted));
    let mut parser2 = Parser::new(tokens2);
    let ast2 = parser2
        .parse()
        .unwrap_or_else(|e| panic!("Parser error on formatted: {:?}\nCode:\n{}", e, formatted));

    let mut fmt2 = forja::formatter::Formatter::new();
    let formatted2 = fmt2.formatear(&ast2);

    assert_eq!(
        formatted, formatted2,
        "Formatter output is not stable:\nFirst formatting:\n{}\nSecond formatting:\n{}",
        formatted, formatted2
    );
}

#[test]
fn test_fmt_simple_var_and_arith() {
    assert_format_roundtrip("variable x = 5\nconstante y = x * 2 + 10\nescribir(y)\n");
}

#[test]
fn test_fmt_si_sino() {
    assert_format_roundtrip(
        "si (x > 10) {\n    escribir(\"mayor\")\n} sino {\n    escribir(\"menor\")\n}\n",
    );
}

#[test]
fn test_fmt_canal_con_capacidad() {
    // F12-T2: `canal(n)` (backpressure) debe formatearse y re-parsearse igual.
    assert_format_roundtrip(
        "variable tx, rx = canal(4)
variable t2, r2 = canal()
",
    );
}

#[test]
fn test_fmt_mientras_para_repetir() {
    assert_format_roundtrip("mientras (x < 10) {\n    x = x + 1\n}\n");
    assert_format_roundtrip("para (variable i = 0; i < 5; i = i + 1) {\n    escribir(i)\n}\n");
    assert_format_roundtrip("repetir (5) {\n    escribir(\"hola\")\n}\n");
}

#[test]
fn test_fmt_clase_y_metodos() {
    assert_format_roundtrip(
        "clase Persona {\n    nombre\n    edad: Entero\n\n    constructor(nombre: Texto, edad: Entero) {\n        este.nombre = nombre\n        este.edad = edad\n    }\n\n    funcion saludar() {\n        escribir(\"Hola \" + este.nombre)\n    }\n}\n"
    );
}

#[test]
fn test_fmt_enum_y_match() {
    assert_format_roundtrip(
        "tipo Estado = Activo | Inactivo | Suspenso\n\ntipo Resultado = Ok(Entero) | Error(Texto)\n"
    );
    assert_format_roundtrip(
        "coincidir (x) {\n    caso Activo -> {\n        retornar 1\n    }\n    caso _ -> {\n        retornar 0\n    }\n}\n"
    );
}

#[test]
fn test_fmt_contratos() {
    assert_format_roundtrip(
        "funcion dividir(a: Entero, b: Entero) -> Entero\n    requiere b != 0, \"No dividir por cero\"\n    asegura resultado <= a\n{\n    retornar a / b\n}\n"
    );
}

#[test]
fn test_fmt_rasgos() {
    assert_format_roundtrip(
        "rasgo Volador {\n    funcion volar(altitud: Entero)\n}\n\nimplementa Volador para Avion {\n    funcion volar(altitud: Entero) {\n        escribir(\"Volando\")\n    }\n}\n"
    );
}

#[test]
fn test_fmt_multiple_assignment_and_hilo() {
    assert_format_roundtrip("variable tx, rx = canal()\nhilo {\n    tx.enviar(42)\n}\n");
}

#[test]
fn test_red_validaciones() {
    let code = r#"
importar std/red

variable ipv4_valida = validar_ipv4("192.168.1.1")
variable ipv6_valida = validar_ipv6("::1")
variable mac_valida = validar_mac("00:1A:2B:3C:4D:5E")

escribir(ipv4_valida)
escribir(ipv6_valida)
escribir(mac_valida)
"#;
    let root_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = forja::ejecutar_con_opciones_desde(code, root_dir, true, None).unwrap();
    assert_eq!(out, vec!["verdadero", "verdadero", "verdadero"]);
}

#[test]
fn test_quic_e2e() {
    let code = r#"
importar std/quic

variable res_conn = quic_conectar("ejemplo.com", 443)
coincidir (res_conn) {
    caso Ok(conn) -> {
        variable res_st = conn.abrir_stream()
        coincidir (res_st) {
            caso Ok(st) -> {
                variable res = st.recibir(1024)
                coincidir (res) {
                    caso Ok(datos) -> escribir(datos)
                    caso _ -> escribir("error")
                }
            }
            caso _ -> escribir("error stream")
        }
    }
    caso _ -> escribir("error conexion")
}
"#;
    let root_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = forja::ejecutar_con_opciones_desde(code, root_dir, true, None).unwrap();
    assert_eq!(out, vec!["HTTP/3 QUIC Stream OK"]);
}

#[test]
fn test_h3_e2e() {
    let code = r#"
importar std/cliente_h3

variable res = h3_obtener("https://cloudflare.com")
coincidir (res) {
    caso Ok(resp) -> {
        escribir(resp.version)
        escribir(resp.codigo)
    }
    caso _ -> escribir("error h3")
}
"#;
    let root_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = forja::ejecutar_con_opciones_desde(code, root_dir, true, None).unwrap();
    assert_eq!(out, vec!["HTTP/3.0", "200"]);
}

#[test]
fn test_mapa_sintaxis_punto() {
    let code = r#"
variable m = {"nombre": "Forja", "version": 1}
escribir(m.nombre)
escribir(m.version)
"#;
    let root_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = forja::ejecutar_con_opciones_desde(code, root_dir, true, None).unwrap();
    assert_eq!(out, vec!["Forja", "1"]);
}

#[test]
fn test_metodos_primitivos_oo() {
    let code = r#"
variable txt = "hola mundo"
escribir(txt.longitud())
escribir(txt.a_mayusculas())
escribir(txt.contiene("mundo"))

variable arr = [10, 20]
escribir(arr.longitud())
"#;
    let out = ejecutar(code).unwrap();
    assert_eq!(out, vec!["10", "HOLA MUNDO", "verdadero", "2"]);
}

#[test]
fn test_interpolacion_cadenas_llaves() {
    let code = r#"
variable nombre = "Mundo"
variable msg = "Hola ${nombre}!"
escribir(msg)
"#;
    let root_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = forja::ejecutar_con_opciones_desde(code, root_dir, true, None).unwrap();
    assert_eq!(out, vec!["Hola Mundo!"]);
}

/// F5-T7: `${...}` funciona igual en VM y JIT, y las llaves sin `$` son
/// literales (la sintaxis `{expr}` antigua ya no interpola).
#[test]
fn test_interpolacion_vm_jit_y_llaves_literales() {
    let code = r#"
variable nombre = "Forja"
variable a = "Hola ${nombre}"
variable b = "Hola {nombre}"
escribir(a)
escribir(b)
"#;
    let vm = forja::ejecutar_vm(code).expect("VM");
    let jit = forja::ejecutar_jit(code).expect("JIT");
    assert_eq!(vm, vec!["Hola Forja", "Hola {nombre}"]);
    assert_eq!(jit, vm);
}

#[test]
fn test_struct_literal_init() {
    let code = r#"
clase Persona {
    nombre
    edad
}

variable p = nuevo Persona { nombre: "Ana", edad: 25 }
escribir(p.nombre)
escribir(p.edad)
"#;
    let root_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = forja::ejecutar_con_opciones_desde(code, root_dir, true, None).unwrap();
    assert_eq!(out, vec!["Ana", "25"]);
}

// ─── F12-T1: closures como valor con captura de entorno ─────────────────────

/// Ejecuta por los motores disponibles y exige la misma salida: ruta canónica
/// (IR + VM), backend AST y la ruta JIT (`ejecutar_jit`, que orquesta
/// `jit_tiered` sobre la VM).
///
/// El ejecutor de uops (`ForjaFast::ejecutar_uops`) queda fuera: es experimental
/// y hoy ni siquiera devuelve valores de objetos (da Nulo), así que no es un
/// motor de ejecución utilizable.
fn ejecutar_todos_los_motores(fuente: &str) -> Vec<String> {
    fn correr_vm(bc: Vec<forja::bytecode::Opcode>) -> Vec<String> {
        let mut vm = ForjaFast::new();
        vm.set_max_inst(2_000_000);
        vm.cargar_bytecode(bc);
        vm.ejecutar().expect("vm");
        vm.obtener_output().to_vec()
    }

    let (bc_ir, _) = forja::compilar_desde_ir(fuente).expect("ruta IR");
    let salida_ir = correr_vm(bc_ir);
    // Backend AST por la ruta real (con `postprocesar_globales`): el helper
    // `ejecutar` de este archivo no lo aplica y deja los globales como locales.
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let (bc_ast, _) =
        forja::compilar_pipeline_completa_desde_ast(fuente, raiz).expect("backend AST");
    let salida_ast = correr_vm(bc_ast);
    let salida_jit = forja::ejecutar_jit(fuente).expect("ruta JIT");

    assert_eq!(salida_ast, salida_ir, "AST vs IR");
    assert_eq!(salida_jit, salida_ir, "JIT vs VM");
    salida_ir
}

/// F12-T1 (DoD): un closure anónimo asignado a una variable y llamado funciona,
/// y captura por valor las variables locales del ámbito donde se crea.
#[test]
fn f12_t1_closure_captura_por_valor() {
    let salida = ejecutar_todos_los_motores(
        r#"funcion h() -> Entero {
    variable base = 40
    variable f = func(x) { retornar x + base }
    retornar f(2)
}
escribir(h())"#,
    );
    assert_eq!(salida, vec!["42"]);
}

/// La captura es un **snapshot**: mutar la variable después de crear el closure
/// no cambia lo que el closure ve.
#[test]
fn f12_t1_la_captura_es_un_snapshot() {
    let salida = ejecutar_todos_los_motores(
        r#"funcion h() -> Entero {
    variable k = 1
    variable leer_k = func() { retornar k }
    k = 99
    retornar leer_k()
}
escribir(h())"#,
    );
    assert_eq!(salida, vec!["1"]);
}

/// Un closure puede escaparse de la función que lo crea: el entorno viaja con
/// el valor.
#[test]
fn f12_t1_closure_retornado_conserva_su_entorno() {
    let salida = ejecutar_todos_los_motores(
        r#"funcion sumador(base) {
    variable f = func(x) { retornar x + base }
    retornar f
}
variable mas_cien = sumador(100)
variable mas_uno = sumador(1)
escribir(mas_cien(10))
escribir(mas_uno(10))"#,
    );
    assert_eq!(salida, vec!["110", "11"]);
}

/// Closures anidados: el interno captura también del ámbito externo.
#[test]
fn f12_t1_closures_anidados() {
    let salida = ejecutar_todos_los_motores(
        r#"funcion h() -> Entero {
    variable a = 10
    variable exterior = func(b) {
        variable interior = func(c) { retornar a + b + c }
        retornar interior(100)
    }
    retornar exterior(1000)
}
escribir(h())"#,
    );
    assert_eq!(salida, vec!["1110"]);
}

/// Un closure con captura se puede pasar como callback a una función de usuario.
#[test]
fn f12_t1_callback_con_captura() {
    let salida = ejecutar_todos_los_motores(
        r#"funcion aplicar(f, v) { retornar f(v) }
funcion h() -> Entero {
    variable base = 7
    variable f = func(x) { retornar x + base }
    retornar aplicar(f, 35)
}
escribir(h())"#,
    );
    assert_eq!(salida, vec!["42"]);
}

/// Cada vuelta de un bucle crea un closure nuevo con el valor vigente.
#[test]
fn f12_t1_captura_en_bucle() {
    let salida = ejecutar_todos_los_motores(
        r#"funcion h() -> Entero {
    variable suma = 0
    variable i = 0
    mientras (i < 3) {
        variable por_i = func(x) { retornar x * i }
        suma = suma + por_i(10)
        i = i + 1
    }
    retornar suma
}
escribir(h())"#,
    );
    assert_eq!(salida, vec!["30"]);
}

/// Los globales **no** se capturan: el closure los lee en tiempo de llamada.
#[test]
fn f12_t1_los_globales_no_se_capturan() {
    let salida = ejecutar_todos_los_motores(
        r#"variable contador = 1
variable leer_contador = func() { retornar contador }
contador = 41
escribir(leer_contador())"#,
    );
    assert_eq!(salida, vec!["41"]);
}
