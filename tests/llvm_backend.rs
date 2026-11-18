//! El backend LLVM (lowering nuevo) debe generar LLVM IR válido y ejecutable:
//! aritmética, control de flujo, funciones, texto tipado, arreglos, clases y
//! globals. Cuando hay `clang` en PATH, además se compila y se ejecuta.

use std::path::PathBuf;
use std::process::Command;

fn ir(fuente: &str) -> String {
    forja::compilar_a_llvm(fuente).expect("LLVM IR")
}

fn hay_clang() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Compila el fuente a nativo con clang + el runtime de soporte y devuelve la
/// salida estándar. `None` si no hay clang.
fn ejecutar_nativo(fuente: &str) -> Option<String> {
    let (codigo, salida) = ejecutar_nativo_con_codigo(fuente)?;
    assert!(codigo == 0, "el binario salió con {codigo}: {salida}");
    Some(salida)
}

/// Igual que [`ejecutar_nativo`] pero devuelve el código de salida y no falla si
/// el programa aborta (contratos, errores de runtime).
fn ejecutar_nativo_con_codigo(fuente: &str) -> Option<(i32, String)> {
    if !hay_clang() {
        return None;
    }
    let directorio: PathBuf = std::env::temp_dir().join(format!(
        "forja_llvm_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&directorio).expect("dir temporal");
    let ll = directorio.join("main.ll");
    let rt = directorio.join("forja_rt.c");
    let exe = directorio.join(if cfg!(windows) { "main.exe" } else { "main" });
    std::fs::write(&ll, ir(fuente)).expect("escribir .ll");
    std::fs::write(&rt, forja::backend_llvm::RUNTIME_C).expect("escribir runtime");
    let salida = Command::new("clang")
        .args(["-O1", "-w"])
        .arg("-o")
        .arg(&exe)
        .arg(&ll)
        .arg(&rt)
        .output()
        .expect("clang");
    assert!(
        salida.status.success(),
        "clang falló:\n{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let corrida = Command::new(&exe).output().expect("ejecutar");
    let _ = std::fs::remove_dir_all(&directorio);
    let mut salida = String::from_utf8_lossy(&corrida.stdout).replace("\r\n", "\n");
    salida.push_str(&String::from_utf8_lossy(&corrida.stderr));
    Some((corrida.status.code().unwrap_or(-1), salida))
}

#[test]
fn llvm_genera_ir_de_funciones_y_aritmetica() {
    let fuente = r#"
funcion sumar(a: Entero, b: Entero) -> Entero {
    retornar a + b
}
variable x = sumar(2, 3)
escribir(x)
"#;
    let ir = ir(fuente);
    assert!(ir.contains("define"), "falta define:\n{ir}");
    assert!(ir.contains("add"), "falta add:\n{ir}");
    assert!(ir.contains("ret"), "falta ret:\n{ir}");
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "5");
    }
}

#[test]
fn llvm_genera_control_de_flujo() {
    let fuente = r#"
variable x = 3
si (x > 2) {
    escribir("mayor")
} sino {
    escribir("menor")
}
"#;
    let ir = ir(fuente);
    assert!(ir.contains("icmp"), "falta icmp:\n{ir}");
    assert!(ir.contains("br "), "falta branch:\n{ir}");
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "mayor");
    }
}

/// Bug §1 de `plans/BUGS-BACKEND-LLVM.md`: `Texto + Entero` fuera de `escribir`
/// sumaba la dirección del literal. Ahora concatena.
#[test]
fn texto_mas_entero_concatena_en_variables_y_retornos() {
    let fuente = r#"
funcion etiqueta(n: Entero) -> Texto {
    retornar "a" + n + "b"
}
variable s = "x = " + 5
escribir(s)
escribir(etiqueta(7))
"#;
    let ir = ir(fuente);
    assert!(ir.contains("@forja_concat"), "falta forja_concat:\n{ir}");
    assert!(!ir.contains("add i64"), "no debe sumar punteros:\n{ir}");
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida, "x = 5\na7b\n");
    }
}

/// Bug §2: variables de nivel superior sin `main` generaban IR inválido
/// (`store` a nivel de módulo).
#[test]
fn globales_sin_principal_generan_ir_valido() {
    // Bug §2 del plan: los `variable` de nivel superior generaban `store` a
    // nivel de módulo (IR inválido). Ahora viven en `@forja_init`; además, si
    // ninguna función las usa, son `alloca` locales (más rápidas que un global).
    let fuente = r#"
variable a = 5
variable b = a * 2
escribir("r = " + b)
"#;
    let ir_local = ir(fuente);
    assert!(
        ir_local.contains("define void @forja_init"),
        "falta forja_init:
{ir_local}"
    );
    assert!(
        ir_local.contains("= alloca i64") && ir_local.contains("%a."),
        "falta alloca de a:
{ir_local}"
    );
    // No hay instrucciones de nivel de módulo (`store` fuera de un define).
    for l in ir_local.lines() {
        let t = l.trim_start();
        if t.contains("store i64 5") {
            assert_ne!(
                l, t,
                "store a nivel de módulo:
{l}"
            );
        }
    }
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "r = 10");
    }

    // Si una función la usa, sí se emite como global real.
    let con_funcion = r#"
variable a = 5
funcion leer() -> Entero { retornar a }
escribir(leer())
"#;
    let ir_global = ir(con_funcion);
    assert!(
        ir_global.contains("@g_a = internal global i64 5")
            || ir_global.contains("@g_a = internal global i64 0"),
        "falta global @g_a:
{ir_global}"
    );
    if let Some(salida) = ejecutar_nativo(con_funcion) {
        assert_eq!(salida.trim(), "5");
    }
}

#[test]
fn arreglos_y_longitud() {
    let fuente = r#"
variable a = [1, 2, 3]
a[1] = 40
variable s = 0
variable i = 0
mientras (i < longitud(a)) {
    s = s + a[i]
    i = i + 1
}
escribir(s)
escribir(longitud(a))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida, "44\n3\n");
    }
}

#[test]
fn clases_con_constructor_campos_y_metodos() {
    let fuente = r#"
clase Caja {
    variable valor: Entero
    constructor(v: Entero) {
        este.valor = v
    }
    funcion doble() -> Entero {
        retornar este.valor * 2
    }
}
variable c = nuevo Caja(21)
escribir(c.doble())
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "42");
    }
}

#[test]
fn despacho_dinamico_por_rasgo() {
    // Un campo sin anotar guarda objetos de varias clases: el método se resuelve
    // por el tag de la clase en tiempo de ejecución.
    let fuente = r#"
rasgo Saludo {
    funcion saludar() -> Texto
}

clase Formal {
    constructor() {}
}
implementa Saludo para Formal {
    funcion saludar() -> Texto {
        retornar "buenas"
    }
}

clase Informal {
    constructor() {}
}
implementa Saludo para Informal {
    funcion saludar() -> Texto {
        retornar "hola"
    }
}

clase Persona {
    variable saludo
    constructor(s) {
        este.saludo = s
    }
    funcion hablar() -> Texto {
        retornar este.saludo.saludar()
    }
}

escribir(nuevo Persona(nuevo Formal()).hablar())
escribir(nuevo Persona(nuevo Informal()).hablar())
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "buenas\nhola");
    }
}

#[test]
fn arreglos_de_decimal() {
    // La casilla de un arreglo es un i64: un `Decimal` viaja bit-casteado y se
    // lee de vuelta como double (antes se truncaba a entero al guardarlo).
    let fuente = r#"
variable medias = [0, 2.5, 7.5]
escribir(medias[1])
variable i = 1
mientras (i < longitud(medias)) {
    escribir(medias[i])
    i = i + 1
}
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "2.5\n2.5\n7.5");
    }
}

#[test]
fn texto_en_aritmetica_da_nulo_como_la_vm() {
    // Mezclar un Texto con un número no es aritmética ni comparación: la VM
    // devuelve `nulo` y `falso` (antes se convertía el texto a número).
    let fuente = r#"
variable t = "" + 10
escribir(t - 1)
escribir(7 == t)
escribir(7 != t)
escribir(t == t)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida.trim(),
            "nulo
falso
verdadero
verdadero"
        );
    }
}

#[test]
fn precondiciones_abortan_con_el_mensaje_de_la_vm() {
    let fuente = r#"
funcion dividir(a: Entero, b: Entero) -> Entero
    requiere b != 0, "No dividir por cero"
{
    retornar a / b
}
escribir(dividir(10, 2))
escribir(dividir(10, 0))
escribir("no se llega")
"#;
    if let Some((codigo, salida)) = ejecutar_nativo_con_codigo(fuente) {
        assert!(
            salida.contains("Error en ejecución: contrato violado: precondición no se cumple: No dividir por cero"),
            "salida inesperada: {salida}"
        );
        assert!(
            !salida.contains("no se llega"),
            "siguió después del contrato"
        );
        assert_ne!(codigo, 0, "debería abortar");
    }
}

#[test]
fn casilla_con_numero_se_comporta_como_la_vm() {
    // La VM decide por el valor: `c.f[0]` guarda un entero, así que `longitud`
    // da 0, `[i]` da `nulo` y `escribir` imprime el número. Antes la ruta nativa
    // lo leía como `%ForjaArreglo` y hacía segfault.
    // shortcut: `nulo` y el entero 0 comparten representación en la casilla, así
    // que `c.f[0][0]` imprime 0 y no "nulo" (la VM sí los distingue). Upgrade:
    // valores con tag, como el `Value` de la VM.
    let fuente = r#"
clase C {
    f
    constructor() { este.f = [] }
}
funcion main() {
    variable c = nuevo C()
    c.f[0] = 1
    escribir("slot=" + c.f[0])
    escribir("long=" + longitud(c.f[0]))
    escribir("sub=" + c.f[0][0])
}
main()
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        let limpio = salida.replace("\r\n", "\n");
        assert_eq!(limpio.trim(), "slot=1\nlong=0\nsub=0");
    }
}

#[test]
fn indice_fuera_de_rango_no_aborta_y_la_casilla_crece() {
    // La VM no aborta por índice fuera de rango: lee la casilla que no está y
    // `a[3] = 9` alarga el arreglo con huecos.
    let fuente = r#"
funcion main() {
    variable a = [1, 2]
    escribir("oob=" + a[5])
    variable b = []
    b[3] = 9
    escribir("huecos=" + b[0] + "," + b[3] + " len=" + longitud(b))
    escribir("bien=" + a[1])
}
main()
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        let limpio = salida.replace("\r\n", "\n");
        assert_eq!(limpio.trim(), "oob=0\nhuecos=0,9 len=4\nbien=2");
    }
}

#[test]
fn payload_de_resultado_anotado() {
    // `funcion f() -> Resultado<Texto, Texto>`: el payload de `Ok`/`Error` se
    // enlaza con esos tipos (el preludio asume Entero/Texto).
    let fuente = r#"
funcion buscar(n) -> Resultado<Texto, Texto> {
    si (n > 0) {
        retornar Ok("encontrado")
    }
    retornar Error("no está")
}
variable r = buscar(1)
coincidir (r) {
    caso Ok(v) -> {
        escribir("ok: " + v)
    }
    caso Error(m) -> {
        escribir("error: " + m)
    }
}
coincidir (buscar(0)) {
    caso Ok(v) -> {
        escribir("ok: " + v)
    }
    caso Error(m) -> {
        escribir("error: " + m)
    }
}
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "ok: encontrado\nerror: no está");
    }
}

#[test]
fn metodo_booleano_por_despacho_dinamico() {
    // `unificar_retorno` trataba `Booleano` como `Entero`: un método booleano
    // llamado a través de un campo sin tipo estático se imprimía `1`.
    let fuente = r#"
rasgo Chequeo {
    funcion esta_bien() -> Booleano
}
clase Bien {
    constructor() {}
}
implementa Chequeo para Bien {
    funcion esta_bien() -> Booleano {
        retornar verdadero
    }
}
clase Mal {
    constructor() {}
}
implementa Chequeo para Mal {
    funcion esta_bien() -> Booleano {
        retornar falso
    }
}
clase Caja {
    chequeo
    constructor(c) {
        este.chequeo = c
    }
}
escribir(nuevo Caja(nuevo Bien()).chequeo.esta_bien())
escribir(nuevo Caja(nuevo Mal()).chequeo.esta_bien())
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "verdadero\nfalso");
    }
}

#[test]
fn derive_igual_compara_campos() {
    // `@derive(Igual)`: `==` compara campo por campo (antes comparaba punteros).
    let fuente = r#"
@derive(Igual)
clase Punto {
    x
    y
    constructor(a, b) {
        este.x = a
        este.y = b
    }
}
escribir(nuevo Punto(1, 2) == nuevo Punto(1, 2))
escribir(nuevo Punto(1, 2) == nuevo Punto(1, 3))
escribir(nuevo Punto(1, 2) != nuevo Punto(1, 3))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "verdadero\nfalso\nverdadero");
    }
}

#[test]
fn clase_generica_que_crea_otra_instanciacion() {
    // El cuerpo de un método especializado puede instanciar el genérico con
    // otros tipos (`intercambiar` devuelve `Par<Texto, Entero>`): la
    // monomorfización recorre esos cuerpos hasta el punto fijo.
    let fuente = r#"
clase Par<A, B> {
    primero
    segundo
    constructor(a: A, b: B) {
        este.primero = a
        este.segundo = b
    }
    funcion obtener_primero() -> A {
        retornar este.primero
    }
    funcion intercambiar() -> Par {
        retornar nuevo Par(este.segundo, este.primero)
    }
}
variable par = nuevo Par(1, "uno")
variable cambiado = par.intercambiar()
escribir(cambiado.obtener_primero())
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "uno");
    }
}

#[test]
fn generico_especializado_conserva_el_tipo_del_valor() {
    // La misma clase genérica usada con tipos distintos: cada especialización
    // lleva el tipo concreto del campo, del parámetro y del `-> T`. Antes el
    // `constructor(a: T)` y el `-> T` del `Caja<T>` genérico marcaban como
    // «dinámicos» los de `Caja_Texto`/`Caja_Booleano` (IR inválido: i64 pasado
    // donde va `ptr`) y `obtener()` se imprimía por bits (`1` vs `verdadero`).
    let fuente = r#"
clase Caja<T> {
    contenido
    constructor(valor: T) {
        este.contenido = valor
    }
    funcion obtener() -> T {
        retornar este.contenido
    }
}
variable b = nuevo Caja(verdadero)
variable t = nuevo Caja("hola")
escribir("b: " + b.obtener())
escribir("t: " + t.obtener())
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "b: verdadero\nt: hola");
    }
}

#[test]
fn derive_mostrar_formatea_la_clase() {
    // `@derive(Mostrar)`: `escribir(obj)` y la concatenación dan
    // `Clase { campo: valor, … }`, como la VM (antes salía la dirección).
    let fuente = r#"
@derive(Mostrar)
clase Punto {
    x
    y: Decimal
    constructor(a, b) {
        este.x = a
        este.y = b
    }
}
variable p = nuevo Punto(1, 2.5)
escribir(p)
escribir("p = " + p)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida.trim(),
            "Punto { x: 1, y: 2.5 }\np = Punto { x: 1, y: 2.5 }"
        );
    }
}

#[test]
fn indexar_un_texto() {
    // `s[i]` da un texto de un carácter (nulo si se pasa), como la VM.
    let fuente = r#"
variable s = "hola"
escribir(s[0])
escribir(s[3])
escribir(s[9])
escribir(s[0] == "h")
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida.trim(),
            "h
a
nulo
verdadero"
        );
    }
}

#[test]
fn builtins_de_entrada_y_de_introspeccion() {
    // `leer()` al final de la entrada devuelve nulo (y concatena como "nulo"),
    // `unir_elementos` une por el tag de cada casilla y `tipo` pregunta al tag
    // del runtime cuando el tipo estático no alcanza (`arr[i]` de un anidado).
    let fuente = r#"
variable a = [1, 2, 3]
escribir(unir_elementos(a, "-"))
escribir(tipo(a))
escribir(tipo(a[0]))
escribir(tipo("x"))
escribir(tipo(verdadero))
variable anidado = [[1, 2], [3], 5]
variable i = 0
mientras (i < longitud(anidado)) {
    escribir(tipo(anidado[i]))
    i = i + 1
}
variable linea = leer()
escribir("leido: " + linea)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida.trim(),
            "1-2-3\narreglo\nentero\ntexto\nbooleano\narreglo\narreglo\nentero\nleido: nulo"
        );
    }
}

#[test]
fn mapas_con_valores_de_texto() {
    // La casilla del mapa guarda un i64 con su tag: `escribir(m[clave])` usa el
    // tag y muestra el texto (antes convertía con `a_entero` y daba 0).
    let fuente = r#"
variable p = {"nombre": "Ana", "edad": 25}
escribir(p["nombre"])
escribir(p["edad"])
variable m = {}
m["ciudad"] = "Lima"
escribir(m["ciudad"])
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "Ana\n25\nLima");
    }
}

#[test]
fn campos_tipados_por_el_constructor() {
    // Un campo que el constructor llena desde un parámetro toma el tipo del
    // parámetro (antes quedaba `Entero` y el texto se imprimía `0`).
    let fuente = r#"
clase Nodo {
    clave
    valor
    constructor(c, v) {
        este.clave = c
        este.valor = v
    }
}
variable n = nuevo Nodo("a", "b")
escribir(n.clave + "-" + n.valor)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "a-b");
    }
}

#[test]
fn arreglo_que_arranca_vacio_y_se_llena_de_textos() {
    // El literal `[]` no dice el tipo de las casillas: lo marca el `empujar` de
    // un texto (y su `tipo` en el runtime, para imprimirlo).
    let fuente = r#"
variable nombres = []
nombres.empujar("Ana")
nombres.empujar("Luis")
escribir(nombres)
escribir(nombres[1])
escribir("primero: " + nombres[0])
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "[Ana,Luis]\nLuis\nprimero: Ana");
    }
}

#[test]
fn resultado_y_opcion_se_imprimen_como_la_vm() {
    let fuente = r#"
funcion f() {
    retornar Ok(1)
}
funcion g() {
    retornar Algo("x")
}
variable r = f()
variable o = g()
escribir(r)
escribir("r: " + r)
escribir(o)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "<Resultado>\nr: <Resultado>\n<Opcion>");
    }
}

#[test]
fn booleanos_como_la_vm() {
    // `Booleano` se imprime `verdadero`/`falso` (y al concatenar), no 1/0.
    let fuente = r#"
variable activo = verdadero
variable negativo = 1 > 2
escribir(activo)
escribir(negativo)
escribir("es: " + activo)
escribir(activo == verdadero)
funcion es_par(n) -> Booleano {
    retornar n % 2 == 0
}
escribir(es_par(4))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida.trim(),
            "verdadero\nfalso\nes: verdadero\nverdadero\nverdadero"
        );
    }
}

#[test]
fn campos_de_receptor_sin_tipo_estatico() {
    // `nodo.siguiente` sobre una variable sin anotar: el offset del campo sale
    // del tag de la clase (cada clase ubica sus campos distinto).
    let fuente = r#"
clase Nodo {
    variable nombre: Texto
    siguiente
    constructor(n: Texto, s) {
        este.nombre = n
        este.siguiente = s
    }
}

funcion largo(mut nodo) {
    variable n = 0
    mientras (nodo != nulo) {
        n = n + 1
        nodo = nodo.siguiente
    }
    retornar n
}

variable c = nuevo Nodo("c", nulo)
variable b = nuevo Nodo("b", c)
variable a = nuevo Nodo("a", b)
escribir(largo(a))
escribir(a.siguiente.nombre)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "3\nb");
    }
}

#[test]
fn escribe_en_el_indice_y_crece_al_final() {
    // `arr[i] = v` escribe en `i` (antes escribía en 0) y `arr[longitud(arr)] = v`
    // crece, como en la VM.
    let fuente = r#"
variable a = [0, 0, 0]
a[1] = 7
escribir(a)
variable b = []
b[longitud(b)] = 5
b[longitud(b)] = 6
escribir(b)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "[0,7,0]\n[5,6]");
    }
}

#[test]
fn llamada_a_metodo_como_sentencia() {
    // `p.metodo(x)` como sentencia llega del parser como la función "p.metodo".
    let fuente = r#"
clase Caja {
    variable n: Entero
    constructor() {
        este.n = 0
    }
    funcion sumar(x) {
        este.n = este.n + x
    }
}
variable c = nuevo Caja()
c.sumar(2)
c.sumar(3)
escribir(c.n)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "5");
    }
}

#[test]
fn texto_mas_arreglo_usa_la_repr() {
    // Concatenar un arreglo con Texto da `[1,2,3]`, igual que `escribir(arreglo)`
    // y que la VM (antes salía la dirección).
    let fuente = r#"
variable a = [1, 2, 3]
escribir("a = " + a)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "a = [1,2,3]");
    }
}

#[test]
fn constructos_no_soportados_fallan_con_mensaje() {
    // Lo que la ruta nativa todavía no baja devuelve error, nunca IR que miente.
    for (fuente, esperado) in [
        (
            "variable e, r = canal()
escribir(e)
",
            "no está soportado",
        ),
        (
            "tipo Color = Rojo | Verde
variable c = Rojo
escribir(c)
",
            "no está soportado",
        ),
    ] {
        let err = forja::compilar_a_llvm(fuente).expect_err("debía fallar");
        let texto = format!("{err:?}");
        assert!(
            texto.contains(esperado),
            "se esperaba `{esperado}` en: {}",
            &texto[..texto.len().min(300)]
        );
    }
}

#[test]
fn resultado_y_operador_try() {
    let fuente = r#"
funcion dividir(a: Entero, b: Entero) -> Resultado<Entero, Texto> {
    si (b == 0) { retornar Error("division por cero") }
    retornar Ok(a / b)
}
funcion calcular(x: Entero) -> Resultado<Entero, Texto> {
    variable r = dividir(100, x)?
    retornar Ok(r + 1)
}
variable bueno = calcular(4)
coincidir (bueno) {
    caso Ok(n) { escribir("ok: " + a_texto(n)) }
    caso Error(e) { escribir("error: " + e) }
}
variable malo = calcular(0)
coincidir (malo) {
    caso Ok(n) { escribir("ok: " + a_texto(n)) }
    caso Error(e) { escribir("error: " + e) }
}
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "ok: 26
error: division por cero
"
        );
    }
}

#[test]
fn enums_con_payload_de_texto() {
    let fuente = r#"
tipo Forma = Circulo(Entero) | Etiqueta(Texto)
funcion describir(f: Forma) -> Texto {
    coincidir (f) {
        caso Circulo(r) { retornar "circulo de " + a_texto(r) }
        caso Etiqueta(e) { retornar "etiqueta: " + e }
    }
}
escribir(describir(Etiqueta("hola")))
escribir(describir(Circulo(3)))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "etiqueta: hola
circulo de 3
"
        );
    }
}

#[test]
fn exacto_decimal_de_precision_arbitraria() {
    // Misma semantica que la VM: coeficiente i128 + escala, con el formato
    // `1.50` (punto decimal, sin recortar ceros).
    let fuente = r#"
variable p: Exacto = 1999e-2
escribir(p)
variable t: Exacto = p * 3
escribir(t)
variable pi: Exacto = 314159265358979323846e-20
escribir(pi)
variable r: Exacto = 1 - 0.9
escribir(r)
variable cien: Exacto = 1000
variable frac: Exacto = 456
escribir(frac / cien)
// split + índice de arreglo de textos (lo usa std/matematica)
variable partes = "123.456".dividir(".")
escribir(partes[0])
escribir(partes[1])
escribir(a_numero(partes[0]))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "19.99
59.97
3.14159265358979323846
0.1
0.45600000000000000000
123
456
123
"
        );
    }
}

#[test]
fn reservar_capacidad_de_arreglo() {
    let fuente = r#"
variable v = []
reservar(v, 100)
variable j = 0
mientras (j < 100) { empujar(v, j)
 j = j + 1 }
escribir(longitud(v))
escribir(v[99])
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "100
99
"
        );
    }
}

#[test]
fn imprimir_agregados_y_repetir_arreglo() {
    let fuente = r#"
variable a = [1, 2, 3]
escribir(a)
variable textos = ["uno", "dos"]
escribir(textos)
variable m = {}
m[7] = 70
m["k"] = 3
escribir(m)
variable r = repetir_arreglo(9, 5)
escribir(r)
escribir(longitud(r))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "[1,2,3]
[uno,dos]
{7: 70, k: 3}
[9,9,9,9,9]
5
"
        );
    }
}

#[test]
fn closures_con_captura() {
    let fuente = r#"
variable base = 100
variable sumar_base = func(n) { retornar n + base }
variable doble = func(n) { retornar n * 2 }
escribir(doble(21))
escribir(sumar_base(5))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "42
105
"
        );
    }
}

#[test]
fn mapas_con_clave_entera_y_texto() {
    let fuente = r#"
variable m = {}
m[1] = 10
m[2] = 32
m["nombre"] = 7
escribir(m[1] + m[2] + m["nombre"])
escribir(longitud(m))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "49
3
"
        );
    }
}

#[test]
fn enums_y_coincidir() {
    let fuente = r#"
tipo Forma = Circulo(Entero) | Rectangulo(Entero, Entero)
funcion area(f: Forma) -> Entero {
    coincidir (f) {
        caso Circulo(r) {
            retornar 3 * r * r
        }
        caso Rectangulo(a, b) {
            retornar a * b
        }
    }
}
escribir(area(Circulo(2)))
escribir(area(Rectangulo(3, 4)))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "12
12
"
        );
    }
}

#[test]
fn hilos_y_canales() {
    let fuente = r#"
funcion cuadrado(n: Entero) -> Entero {
    retornar n * n
}
variable emisor, receptor = canal()
variable k = 0
mientras (k < 3) {
    hilo { emisor.enviar(cuadrado(k + 10)) }
    k = k + 1
}
variable total = 0
variable i = 0
mientras (i < 3) {
    total = total + receptor.recibir()
    i = i + 1
}
escribir(total)
"#;
    // 100 + 121 + 144
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "365");
    }
}

#[test]
fn el_lowering_vive_en_el_backend_y_no_hay_legacy() {
    let raiz = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    assert!(
        raiz.join("src/backend_llvm/lowering.rs").exists(),
        "el lowering debe vivir en src/backend_llvm/lowering.rs"
    );
    assert!(
        !raiz.join("src/backend_llvm/legacy.rs").exists(),
        "el backend legacy fue eliminado: sus capacidades están migradas"
    );
    assert!(
        !raiz.join("src/compiler_llvm.rs").exists(),
        "src/compiler_llvm.rs no debe existir"
    );
}

#[test]
fn postcondiciones_abortan_con_el_mensaje_de_la_vm() {
    // `asegura` se verificaba sólo en la VM: la ruta nativa ignoraba el contrato
    // y seguía (handoff5 §2, shortcut `sentencias.rs:588`).
    let fuente = r#"
funcion doble(x: Entero) -> Entero
    asegura resultado == x * 3, "tiene que ser el triple"
{
    retornar x * 2
}
escribir("antes")
escribir(doble(5))
escribir("no se llega")
"#;
    if let Some((codigo, salida)) = ejecutar_nativo_con_codigo(fuente) {
        assert!(
            salida.contains("Error en ejecución: contrato violado: postcondición no se cumple: tiene que ser el triple"),
            "salida inesperada: {salida}"
        );
        assert!(!salida.contains("no se llega"), "siguió tras el contrato");
        assert_ne!(codigo, 0, "debería abortar");
    }
}

#[test]
fn postcondiciones_con_anterior_y_resultado() {
    // `anterior(expr)` se captura al entrar; `resultado` es el valor devuelto.
    // Los tres contratos se cumplen, así que el programa termina bien.
    let fuente = r#"
funcion duplicar(x: Entero) -> Entero
    requiere x > 0
    asegura resultado == anterior(x) * 2
{
    retornar x * 2
}
funcion raiz(n: Entero) -> Entero
    asegura resultado >= 0
    asegura resultado * resultado <= n
    asegura (resultado + 1) * (resultado + 1) > n
{
    variable i = 0
    mientras (i * i <= n) {
        i = i + 1
    }
    retornar i - 1
}
escribir(duplicar(7))
escribir(raiz(25))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim().replace("\r\n", "\n"), "14\n5");
    }
}

#[test]
fn postcondicion_de_metodo_usa_anterior_del_campo() {
    let fuente = r#"
clase Contador {
    variable
    valor: Entero

    constructor() {
        este.valor = 0
    }

    funcion incrementar()
        asegura este.valor == anterior(este.valor) + 1
    {
        este.valor = este.valor + 1
    }
}
variable c = nuevo Contador()
c.incrementar()
c.incrementar()
escribir(c.valor)
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim().replace("\r\n", "\n"), "2");
    }
}

#[test]
fn postcondicion_de_metodo_que_falla_aborta() {
    let fuente = r#"
clase Caja {
    variable
    n: Entero

    constructor(n: Entero) {
        este.n = n
    }

    funcion romper()
        asegura este.n == 99, "el campo tiene que quedar en 99"
    {
        este.n = este.n + 1
    }
}
variable c = nuevo Caja(1)
c.romper()
escribir("no se llega")
"#;
    if let Some((codigo, salida)) = ejecutar_nativo_con_codigo(fuente) {
        assert!(
            salida.contains("postcondición no se cumple: el campo tiene que quedar en 99"),
            "salida inesperada: {salida}"
        );
        assert!(!salida.contains("no se llega"), "siguió tras el contrato");
        assert_ne!(codigo, 0, "debería abortar");
    }
}

#[test]
fn closure_que_devuelve_texto_se_llama_por_su_firma() {
    // Antes la llamada asumía `i64(i64, …)`: una closure que devuelve texto
    // salía por bits y se imprimía un número (handoff5 §2, shortcut 1108).
    let fuente = r#"
funcion main() {
    variable f = func(n: Entero) { retornar "n=" + n }
    escribir(f(7))
    variable g = func(a: Entero, b: Entero) { retornar a * b }
    escribir(g(3, 4))
    variable h = func(x: Texto) { retornar x + "!" }
    escribir(h("hola"))
}
main()
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim().replace("\r\n", "\n"), "n=7\n12\nhola!");
    }
}

#[test]
fn llamada_indirecta_usa_la_firma_de_la_funcion() {
    // `variable op = restar` + `op(9, 4)`: antes se asumía i64(i64, …) y el
    // retorno se tomaba como entero aunque devolviera texto (handoff5 §2, 1134).
    let fuente = r#"
funcion restar(a: Entero, b: Entero) -> Entero {
    retornar a - b
}
funcion nombrar(x: Texto) -> Texto {
    retornar "Hola " + x
}
funcion main() {
    variable op = restar
    escribir(op(9, 4))
    variable saludo = nombrar
    escribir(saludo("Ana"))
}
main()
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim().replace("\r\n", "\n"), "5\nHola Ana");
    }
}

#[test]
fn nulo_vs_cero_en_texto_queda_documentado() {
    // handoff5 §1.4. Lo que **sí** anda: un `Texto` nulo concatena "nulo"
    // (igual que la VM). Lo que no: `variable n = nulo` sin anotación se tipa
    // `Entero`, así que la casilla es el 0 y se imprime "0" donde la VM dice
    // "nulo". Es el mismo agujero que `nulo`/`0` (handoff4 §3.1): se cierra
    // cuando haya valores con tag. Si esto empieza a fallar, es que se arregló:
    // actualizar también el `shortcut:` y este comentario.
    let fuente = r#"
funcion main() {
    variable t: Texto
    variable n = nulo
    escribir("x" + n)
    escribir(t + "y")
    escribir("a" + t)
}
main()
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim().replace("\r\n", "\n"), "x0\nnuloy\nanulo");
    }
}

#[test]
fn debug_tags_no_aborta_en_un_programa_normal() {
    // `FORJA_DEBUG_TAGS=1` (handoff5 §1.5) aborta si al registro de objetos
    // entra algo que no sea texto o arreglo. Un programa con textos, arreglos y
    // escalares mezclados tiene que correr igual: los escalares no se registran.
    let fuente = r#"
funcion main() {
    variable nombres = ["ana", "beto"]
    variable i = 0
    mientras (i < longitud(nombres)) {
        escribir(nombres[i] + " " + i)
        i = i + 1
    }
    variable frase = "hola" + 2.5
    escribir(frase)
    escribir(longitud(frase))
}
main()
"#;
    if !hay_clang() {
        return;
    }
    let directorio: PathBuf = std::env::temp_dir().join(format!(
        "forja_llvm_dbg_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&directorio).expect("dir temporal");
    let ll = directorio.join("main.ll");
    let rt = directorio.join("forja_rt.c");
    let exe = directorio.join(if cfg!(windows) { "main.exe" } else { "main" });
    std::fs::write(&ll, ir(fuente)).expect("escribir .ll");
    std::fs::write(&rt, forja::backend_llvm::RUNTIME_C).expect("escribir runtime");
    let compilado = Command::new("clang")
        .args(["-O1", "-w"])
        .arg("-o")
        .arg(&exe)
        .arg(&ll)
        .arg(&rt)
        .output()
        .expect("clang");
    assert!(
        compilado.status.success(),
        "clang falló:\n{}",
        String::from_utf8_lossy(&compilado.stderr)
    );
    let corrida = Command::new(&exe)
        .env("FORJA_DEBUG_TAGS", "1")
        .output()
        .expect("ejecutar");
    let _ = std::fs::remove_dir_all(&directorio);
    let salida = String::from_utf8_lossy(&corrida.stdout).replace("\r\n", "\n");
    assert_eq!(corrida.status.code(), Some(0), "abortó: {salida}");
    assert!(
        !salida.contains("FORJA_DEBUG_TAGS"),
        "el chequeo se disparó: {salida}"
    );
    assert!(salida.starts_with("ana 0"), "salida inesperada: {salida}");
}

#[test]
fn retorno_con_nulo_en_una_rama_no_declara_void() {
    // `retornar nulo` en una rama dejaba el `define` en `void` mientras el
    // cuerpo devolvía un valor (`i64`/`ptr`), y clang rechazaba el módulo
    // (`193_json_basico`, `165_numeros_complejos`, `166_matrices`).
    let fuente = r#"
funcion f(cond) {
    si (cond) { retornar nulo }
    retornar "hola"
}
escribir("v=" + f(falso))
"#;
    let ir = ir(fuente);
    assert!(
        !ir.contains("define void @f("),
        "f quedó `void` con ramas que devuelven valor:\n{ir}"
    );
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "v=hola");
    }
}

#[test]
fn global_que_arranca_en_nulo_se_tipa_por_la_asignacion() {
    // `variable g = nulo` seguido de `g = nuevo C()` dentro de una función: el
    // global era `i64` y el `store` de un puntero no compilaba (`171_singleton`).
    let fuente = r#"
clase C {
    x
    constructor() { este.x = 7 }
    funcion v() -> Entero { retornar este.x }
}
variable g = nulo
funcion usar() {
    si (g == nulo) { g = nuevo C() }
}
usar()
escribir(g.v())
"#;
    let ir = ir(fuente);
    assert!(
        ir.contains("@g_g = internal global ptr null"),
        "el global @g_g no se tipó como puntero:\n{ir}"
    );
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "7");
    }
}

#[test]
fn slots_locales_con_el_mismo_nombre_no_colisionan() {
    // Dos `variable _ = …` en bloques distintos compartían etiqueta y el
    // módulo definía `%_.N` dos veces (`201_login_gui`).
    let fuente = r#"
funcion dame() { retornar 1 }
funcion main() {
    si (verdadero) { variable _ = dame() }
    si (falso) { variable _ = dame() }
    escribir("fin")
}
main()
"#;
    let ir = ir(fuente);
    let mut vistos = std::collections::HashSet::new();
    for l in ir.lines() {
        let t = l.trim();
        if let Some(resto) = t.strip_prefix('%') {
            if let Some(nombre) = resto.split_whitespace().next() {
                if t.contains("= alloca") && !vistos.insert(nombre.to_string()) {
                    panic!("slot `%{nombre}` definido dos veces:\n{ir}");
                }
            }
        }
    }
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "fin");
    }
}
