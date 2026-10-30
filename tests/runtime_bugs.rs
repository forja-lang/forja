//! F8-T7/T8/T9: regresiones de runtime para callbacks, enums, hilos y canales.

fn ejecutar(fuente: &str) -> Vec<String> {
    forja::ejecutar_vm(fuente).expect("el programa debe ejecutarse")
}

#[test]
fn callbacks_bare_y_referencia() {
    let salida = ejecutar(
        r#"
funcion aplicar(f) { f() }
funcion aplicar_ref(f) { f() }
funcion saluda() { escribir("hola") }
aplicar(saluda)
aplicar_ref(&saluda)
"#,
    );
    assert_eq!(salida, vec!["hola", "hola"]);
}

#[test]
fn callback_con_parametros_y_retorno() {
    let salida = ejecutar(
        r#"
funcion doble(x) { retornar x * 2 }
funcion aplicar(f, x) { retornar f(x) }
escribir(aplicar(doble, 21))
"#,
    );
    assert_eq!(salida, vec!["42"]);
}

#[test]
fn enum_coincidir_distingue_variantes() {
    let salida = ejecutar(
        r#"
tipo Direccion = Norte | Sur | Este | Oeste
funcion describir(d) -> Texto {
    coincidir (d) {
        caso Norte -> { retornar "norte" }
        caso Sur -> { retornar "sur" }
        caso Este -> { retornar "este" }
        caso Oeste -> { retornar "oeste" }
    }
}
escribir(describir(Norte))
escribir(describir(Sur))
escribir(describir(Este))
escribir(describir(Oeste))
"#,
    );
    assert_eq!(salida, vec!["norte", "sur", "este", "oeste"]);
}

#[test]
fn hilo_llama_funcion_de_usuario() {
    let salida = ejecutar(
        r#"
funcion trabajo(x) { retornar x * 2 }
variable h = hilo {
    retornar trabajo(7)
}
escribir(h.unir())
"#,
    );
    assert_eq!(salida, vec!["14"]);
}

#[test]
fn canal_transfiere_arreglo_y_mapa() {
    let salida = ejecutar(
        r#"
variable tx, rx = canal()
tx.enviar([10, 20, 30])
variable arr = rx.recibir()
escribir(arr[1])
tx.enviar({"clave": 7})
variable m = rx.recibir()
escribir(m["clave"])
"#,
    );
    assert_eq!(salida, vec!["20", "7"]);
}

#[test]
fn hilo_devuelve_arreglo_al_padre() {
    let salida = ejecutar(
        r#"
variable h = hilo {
    retornar [4, 5, 6]
}
variable arr = h.unir()
escribir(arr[2])
"#,
    );
    assert_eq!(salida, vec!["6"]);
}

// ─── F11-T5: regresiones de quickening y de objetos en la ruta IR ───────────

/// El *quickening* infería tipos de operandos escaneando hacia atrás sin
/// respetar el efecto en la pila: en `escribir("x=" + f(2) + "")` tomaba como
/// operandos los argumentos de `f` y especializaba la concatenación como
/// `AddInt`, perdiendo el prefijo de texto.
#[test]
fn concatenacion_con_llamada_no_se_especializa_como_entero() {
    let salida = ejecutar(
        r#"
funcion mcd(mut a, mut b) -> Entero {
    mientras (b != 0) {
        variable temp = b
        b = a % b
        a = temp
    }
    retornar a
}
escribir("mcd=" + mcd(48, 18) + "")
escribir("(" + mcd(21, 6) + ")")
"#,
    );
    assert_eq!(salida, vec!["mcd=6", "(3)"]);
}

/// Los slots locales se reutilizan en cada función: un tipo inferido en una
/// función no debe aplicarse en la siguiente. Antes, `LoadIdx(4)` de un método
/// podía convertirse en `LoadIdxEntero` porque en otra función el slot 4 tenía
/// un entero, y `self` se leía como número.
#[test]
fn tipo_de_slot_no_se_filtra_entre_funciones() {
    let salida = ejecutar(
        r#"
clase Punto {
    x
    y
    constructor() {
        self.x = 0
        self.y = 0
    }
    funcion init(px, py) {
        self.x = px
        self.y = py
    }
    funcion mostrar() {
        escribir("(" + self.x + ", " + self.y + ")")
    }
}
constante PI = 3
funcion main() {
    escribir("PI = " + PI + "")
    variable p = nuevo Punto()
    p.init(10, 20)
    p.mostrar()
}
main()
"#,
    );
    assert_eq!(salida, vec!["PI = 3", "(10, 20)"]);
}

/// Una variante de enum guardada en un campo desde un constructor trivial debe
/// poder leerse y matchearse en un método. El conversor materializaba el objeto
/// variante en una variable *con nombre*; `optimizar_indices` la renumeraba sin
/// conocer los slots temporales del IR y pisaba `self`.
#[test]
fn variante_en_campo_desde_constructor_trivial() {
    let salida = ejecutar(
        r#"
tipo Estado = Rojo | Verde
clase Semáforo {
    e
    constructor() { self.e = Rojo }
    funcion describir() {
        coincidir (self.e) {
            caso Rojo -> { escribir("rojo") }
            caso Verde -> { escribir("verde") }
        }
    }
}
variable s = nuevo Semáforo()
s.describir()
"#,
    );
    assert_eq!(salida, vec!["rojo"]);
}

/// `coincidir` sobre un tipo de variante guardado en un campo y comparación de
/// texto dentro de un bucle (el caso de `validar_mac` de `std/red`): comparar
/// texto no debe especializarse a `DiferenteInt`.
#[test]
fn comparacion_de_texto_en_bucle_no_se_especializa_a_entero() {
    let salida = ejecutar(
        r#"
funcion es_mac(valor: Texto) -> Booleano {
    si (longitud(valor) != 2) { retornar falso }
    variable separador = ":"
    variable pos = 0
    mientras (pos < 2) {
        si (pos == 1) {
            si (subcadena(valor, pos, 1) != separador) { retornar falso }
        }
        pos = pos + 1
    }
    retornar verdadero
}
escribir(es_mac("0:"))
escribir(es_mac("0-"))
"#,
    );
    assert_eq!(salida, vec!["verdadero", "falso"]);
}

// ─── F11-T5: paridad de los backends y fusions del quickening ───────────────

/// Ejecuta el programa por la ruta canónica (IR) y por el backend AST, y exige
/// que ambas coincidan con la salida esperada.
fn ejecutar_ambos(fuente: &str) -> Vec<String> {
    use forja::bytecode::Opcode;
    use forja::vm_fast::ForjaFast;
    use std::path::Path;

    fn correr(bc: Vec<Opcode>) -> Vec<String> {
        let mut vm = ForjaFast::new();
        vm.set_max_inst(2_000_000);
        vm.cargar_bytecode(bc);
        vm.ejecutar().expect("la VM debe ejecutar sin error");
        vm.obtener_output().to_vec()
    }

    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (bc_ir, _) =
        forja::compilar_pipeline_completa_desde_ir(fuente, raiz).expect("ruta IR estricta");
    let (bc_ast, _) =
        forja::compilar_pipeline_completa_desde_ast(fuente, raiz).expect("backend AST");
    let salida_ir = correr(bc_ir);
    let salida_ast = correr(bc_ast);
    assert_eq!(
        salida_ast, salida_ir,
        "los backends deben coincidir (AST vs IR)"
    );
    salida_ir
}

/// Un método de usuario que se llama como un builtin (`obtener`) se despachaba
/// al builtin, que consume otra cantidad de valores de la pila y se comía el
/// texto que había debajo: `"pref " + c.obtener()` imprimía `nulonulo`.
#[test]
fn metodo_con_nombre_de_builtin_no_despacha_al_builtin() {
    let salida = ejecutar_ambos(
        r#"clase C {
    v
    constructor() { self.v = 42 }
    funcion obtener() { retornar self.v }
}
variable c = nuevo C()
escribir("pref " + c.obtener() + "")
escribir("fin")"#,
    );
    assert_eq!(salida, vec!["pref 42", "fin"]);
}

/// El string-builder (`LoadIdx + Add + StoreIdx → StrAppend`) acumulaba en un
/// buffer que sólo se materializaba en `Return`, así que acumular texto dentro
/// de un bucle perdía lo concatenado.
#[test]
fn acumular_texto_en_bucle_conserva_lo_concatenado() {
    let salida = ejecutar_ambos(
        r#"variable esp = ""
variable i = 0
mientras (i < 3) {
    esp = esp + "ab"
    i = i + 1
}
escribir(esp)
escribir(longitud(esp))"#,
    );
    assert_eq!(salida, vec!["ababab", "6"]);
}

/// La llamada indirecta (`CallClosure`) parcheaba el opcode a `Call` dejando el
/// destinatario en la pila: cada vuelta del bucle desalineaba el stack y
/// `res.empujar(callback(x))` sólo apilaba el primer elemento.
#[test]
fn llamada_indirecta_como_argumento_de_metodo() {
    let salida = ejecutar_ambos(
        r#"funcion mapa(arr, callback) {
    variable res = []
    variable i = 0
    mientras (i < longitud(arr)) {
        res.empujar(callback(arr[i]))
        i = i + 1
    }
    retornar res
}
funcion doble(x) { retornar x * 2 }
escribir(mapa([1, 2, 3], doble))"#,
    );
    assert_eq!(salida, vec!["[2,4,6]"]);
}

/// `LoadAddInt`/`LoadAddFloat` se fusionaban desde un `Add` sin tipo y asumían
/// enteros/flotantes: `variable error = tolerancia + 1` con `tolerancia`
/// decimal devolvía Nulo.
#[test]
fn suma_fusionada_respeta_tipos_no_enteros() {
    let salida = ejecutar_ambos(
        r#"funcion inicial(tolerancia) {
    variable error = tolerancia + 1
    retornar error
}
escribir(inicial(0.0001))
escribir("pref " + 1.5 + "")"#,
    );
    assert_eq!(salida, vec!["1.0001", "pref 1.5"]);
}

/// `postprocesar_globales` reescribía por nombre en todo el bytecode: un
/// parámetro que se llamaba como una variable global leía el global.
#[test]
fn parametro_que_sombra_una_variable_global() {
    let salida = ejecutar_ambos(
        r#"funcion mcd(mut a, mut b) {
    mientras (b > 0) {
        variable temp = b
        b = a - ((a / b) * b)
        a = temp
    }
    retornar a
}
variable a = mcd(12, 8)
variable b = mcd(100, 35)
escribir("a=" + a + "")
escribir("b=" + b + "")"#,
    );
    assert_eq!(salida, vec!["a=4", "b=5"]);
}
