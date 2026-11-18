//! Lowering AST → LLVM IR del backend **nuevo** (punteros opacos + valores tipados).
//!
//! Reemplaza al módulo `legacy`: emite IR válido de entrada (sin parches de texto
//! posteriores), con:
//!
//! - variables de nivel superior como `@global` reales e inicialización en
//!   `@forja_init` (el legacy las emitía a nivel de módulo, IR inválido);
//! - punteros opacos (`ptr`), no `i64*`;
//! - valores tipados: `Texto` es `ptr`, `Decimal` es `double`, así que
//!   `"x = " + n` **concatena** de verdad en vez de sumar la dirección del
//!   literal (bug §1 de `plans/BUGS-BACKEND-LLVM.md`);
//! - runtime de soporte en C (`runtime.c`) para texto, arreglos y tiempo;
//! - error explícito para los constructos que todavía no baja (mapas,
//!   enums, closures, hilos, contratos): nunca IR que miente.
//!
//! `@main` siempre es el entry point C (`i32`): llama a `@forja_init` y, si el
//! programa define `funcion main()`, a `@forja_main_usuario`.

use super::LlvmTipo;
use crate::ast::*;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Tipo de un valor de Forja en esta ruta nativa.
#[derive(Clone, Debug, PartialEq)]
enum T {
    Int,
    Bool,
    Dec,
    Texto,
    Arreglo,
    Mapa,
    Exacto,
    Enum(String),
    Canal,
    Closure,
    Clase(String),
    Funcion,
    Nulo,
}

impl T {
    fn llvm(&self) -> LlvmTipo {
        match self {
            T::Int | T::Bool | T::Nulo => LlvmTipo::I64,
            T::Dec => LlvmTipo::Double,
            T::Texto
            | T::Arreglo
            | T::Mapa
            | T::Exacto
            | T::Enum(_)
            | T::Canal
            | T::Closure
            | T::Clase(_)
            | T::Funcion => LlvmTipo::Ptr,
        }
    }

    fn de_tipo(t: &Option<Tipo>) -> Option<T> {
        let t = t.as_ref()?;
        Some(match t {
            Tipo::Entero => T::Int,
            Tipo::Decimal => T::Dec,
            Tipo::Texto => T::Texto,
            Tipo::Booleano => T::Bool,
            Tipo::Nulo => T::Nulo,
            Tipo::Clase(n) => T::Clase(n.clone()),
            Tipo::Arreglo(_) => T::Arreglo,
            Tipo::Exacto => T::Exacto,
            // `Resultado`/`Opcion` son los tipos del preludio: se representan
            // como los enums homónimos (tag + un payload i64).
            Tipo::Resultado(..) => T::Enum("Resultado".to_string()),
            Tipo::Opcion(..) => T::Enum("Opcion".to_string()),
            // Un genérico sin resolver (`funcion f(x: T)`) no dice nada: mejor
            // sin anotación que forzarlo a `Entero`, así manda la evidencia.
            Tipo::Parametro(_) => return None,
            _ => T::Int,
        })
    }

    fn nombre(&self) -> &str {
        match self {
            T::Int => "Entero",
            T::Bool => "Booleano",
            T::Dec => "Decimal",
            T::Texto => "Texto",
            T::Arreglo => "Arreglo",
            T::Mapa => "Mapa",
            T::Enum(n) => n,
            T::Canal => "Canal",
            T::Exacto => "Exacto",
            T::Closure => "closure",
            T::Clase(n) => n,
            T::Funcion => "funcion",
            T::Nulo => "Nulo",
        }
    }
}

/// Valor IR con su tipo de Forja. `r` puede ser `%v3`, un literal o un global.
#[derive(Clone, Debug)]
enum V {
    Int(String),
    Bool(String),
    Dec(String),
    Texto(String),
    Arreglo(String),
    Mapa(String),
    Exacto(String),
    Enum(String, String),
    Canal(String),
    Closure(String),
    Clase(String, String),
    Funcion(String),
    Nulo,
}

impl V {
    fn r(&self) -> &str {
        match self {
            V::Int(r)
            | V::Bool(r)
            | V::Dec(r)
            | V::Texto(r)
            | V::Arreglo(r)
            | V::Mapa(r)
            | V::Exacto(r)
            | V::Enum(r, _)
            | V::Canal(r)
            | V::Closure(r)
            | V::Clase(r, _)
            | V::Funcion(r) => r,
            V::Nulo => "0",
        }
    }
    fn t(&self) -> T {
        match self {
            V::Int(_) => T::Int,
            V::Bool(_) => T::Bool,
            V::Dec(_) => T::Dec,
            V::Texto(_) => T::Texto,
            V::Arreglo(_) => T::Arreglo,
            V::Mapa(_) => T::Mapa,
            V::Exacto(_) => T::Exacto,
            V::Enum(_, c) => T::Enum(c.clone()),
            V::Canal(_) => T::Canal,
            V::Closure(_) => T::Closure,
            V::Clase(_, c) => T::Clase(c.clone()),
            V::Funcion(_) => T::Funcion,
            V::Nulo => T::Nulo,
        }
    }
}

#[derive(Clone)]
struct Var {
    slot: String,
    t: T,
}

/// Layout de un `tipo X = A(i64) | B(Texto, i64)`.
#[derive(Clone)]
struct EnumInformacion {
    max_campos: usize,
}

#[derive(Clone, Default)]
struct Clase {
    campos: Vec<(String, T)>,
    metodos: Vec<(String, Vec<T>, T)>,
    /// Identificador de la clase en la palabra 0 del objeto (0 = nulo). Es lo
    /// que permite resolver un método cuando el receptor no tiene tipo estático.
    tag: u64,
    /// La declaración tenía parámetros de tipo (`clase Caja<T>`): es una
    /// plantilla, su layout real está en las especializaciones `Caja_Tipo`.
    generica: bool,
}

/// Backend de lowering.
pub struct Lowering {
    header: String,
    decls: String,
    globals: String,
    cuerpos: String,
    /// funciones generadas fuera de banda (hilos): van al nivel de módulo.
    extra: String,
    /// allocas de la función actual (van al inicio del bloque `entry`).
    prologo: String,
    vars: HashMap<String, Var>,
    /// firma: nombre → (params, retorno)
    firmas: HashMap<String, (Vec<T>, T)>,
    /// funciones externas (FFI): nombre → (params, retorno)
    externas: HashMap<String, (Vec<T>, T)>,
    nombres_externos: HashMap<String, String>,
    clases: HashMap<String, Clase>,
    enums: HashMap<String, EnumInformacion>,
    /// variante → (enum, índice, aridad)
    variantes: HashMap<String, (String, usize, Vec<T>)>,
    consts: HashMap<String, String>,
    /// tipos inferidos por pre-escaneo (variables declaradas como `nulo`).
    tipos_pre: HashMap<String, T>,
    /// tipos de las variables de nivel superior, visibles antes de emitirlas.
    tipos_top: HashMap<String, T>,
    /// Tipos que llegan a cada parámetro en las llamadas del programa
    /// (nombre → por posición → tipo), para los parámetros sin anotar.
    evidencia: HashMap<String, Vec<Option<T>>>,
    /// Función → tipos del payload de su `Resultado<Ok, Error>` anotado.
    resultado_fn: HashMap<String, (T, T)>,
    /// Variable → tipos del payload del `Resultado` que guarda
    /// (`variable r = f()` con `f() -> Resultado<Texto, Texto>`).
    resultado_var: HashMap<String, (T, T)>,
    /// `(función, índice)` → el argumento es un arreglo de textos (literal con
    /// textos), para poder leer las casillas del parámetro como punteros.
    evidencia_texto: std::collections::HashSet<(String, usize)>,
    /// variables cuyo arreglo guarda textos (punteros) en sus casillas.
    arreglos_texto: HashMap<String, bool>,
    /// Variables/campos cuyo arreglo guarda `Decimal` (double bit-casteado en la
    /// casilla i64, como el texto guarda un puntero).
    arreglos_dec: HashMap<String, bool>,
    /// Variables/campos cuyo **tipo es una suposición** de la inferencia (el
    /// elemento de un arreglo guardado en un campo, `este.matriz[i]`, o algo que
    /// salió de ahí). Sobre esos valores las operaciones preguntan el tipo al
    /// runtime en vez de creerle al slot, que es lo que hace la VM.
    inciertos: std::collections::HashSet<String>,
    /// Métodos que declaran `-> T` (parámetro de tipo de una clase genérica):
    /// el valor que devuelven no tiene tipo estático, así que se imprime y se
    /// concatena por su tag en runtime.
    metodos_param: std::collections::HashSet<String>,
    /// Funciones que devuelven un parámetro de tipo (`funcion f<T>(x: T) -> T`).
    funciones_param: std::collections::HashSet<String>,
    /// Firma `(parámetros, retorno)` del closure creado en cada registro
    /// (`%vN`) y de la variable que lo guarda. Sin esto la llamada asumía
    /// `i64(i64, …)` y una closure que devuelve texto daba un entero.
    firmas_closure: HashMap<String, (Vec<T>, T)>,
    /// Postcondiciones (`asegura`) del cuerpo en curso: se verifican antes de
    /// cada `retornar`, con `resultado` ya vinculado.
    postcondiciones: Vec<crate::ast::Contrato>,
    /// Snapshots de `anterior(expr)` tomados al entrar, por forma de la
    /// expresión (`{:?}` de su interior): el mismo `anterior(x)` produce la
    /// misma clave, así que repetirlo no duplica slots y el orden no importa.
    anteriores: HashMap<String, (String, T)>,
    /// Parámetros declarados con un parámetro de tipo (`x: T`): la casilla es
    /// i64 y el argumento viaja con sus bits tal cual. Clave `nombre:índice`.
    params_dinamicos: std::collections::HashSet<String>,
    /// La función/método que se está emitiendo devuelve un parámetro de tipo
    /// (`-> T`): el `retornar` pasa los bits tal cual, sin convertir (antes un
    /// texto se convertía a número y `frente()` devolvía 0).
    ret_dinamico: bool,
    /// Variables/campos cuyo arreglo guarda arreglos (las casillas son punteros
    /// a otros arreglos; se validan al leerlas, ver `forja_arreglo_validar`).
    arreglos_arreglo: HashMap<String, bool>,
    /// Clases con `@derive(Mostrar)`: `escribir(obj)` las formatea.
    derive_mostrar: std::collections::HashSet<String>,
    /// Clases con `@derive(Igual)`: `==` compara campo por campo.
    derive_igual: std::collections::HashSet<String>,
    etiquetas: Vec<(String, String)>, // (romper, continuar)
    ret_actual: T,
    termina: bool,
    lc: u64,
    rc: u64,
    sc: u64,
    /// `FORJA_SIN_RANGOS=1`: acceso a arreglos sin chequeo de rango (UB si se
    /// pasa, como `get_unchecked`). Es lo que permite vectorizar bucles.
    sin_rangos: bool,
    /// Pares (variable de bucle, arreglo) demostrados en rango por
    /// `mientras (i < longitud(arr))`: no hace falta chequear `arr[i]`.
    sin_check: Vec<(String, String)>,
}

const RUNTIME: &[(&str, &str)] = &[
    ("forja_alloc", "ptr (i64)"),
    ("forja_tiempo_ms", "i64 ()"),
    ("forja_concat", "ptr (ptr, ptr)"),
    ("forja_a_texto", "ptr (i64)"),
    ("forja_decimal_a_texto", "ptr (double)"),
    ("forja_texto_nulo", "ptr ()"),
    ("forja_texto_indice", "ptr (ptr, i64)"),
    ("forja_a_entero", "i64 (ptr)"),
    ("forja_a_decimal", "double (ptr)"),
    ("forja_longitud", "i64 (ptr)"),
    ("forja_indice_de", "i64 (ptr, ptr)"),
    ("forja_subcadena", "ptr (ptr, i64, i64)"),
    ("forja_texto_igual", "i64 (ptr, ptr)"),
    ("forja_texto_orden", "i64 (ptr, ptr)"),
    ("forja_contiene", "i64 (ptr, ptr)"),
    ("forja_empieza_con", "i64 (ptr, ptr)"),
    ("forja_termina_con", "i64 (ptr, ptr)"),
    ("forja_a_mayusculas", "ptr (ptr)"),
    ("forja_a_minusculas", "ptr (ptr)"),
    ("forja_imprimir_texto", "void (ptr)"),
    ("forja_imprimir_entero", "void (i64)"),
    ("forja_imprimir_decimal", "void (double)"),
    ("forja_imprimir_booleano", "void (i64)"),
    ("forja_booleano_a_texto", "ptr (i64)"),
    ("forja_imprimir_nulo", "void ()"),
    ("forja_nueva_linea", "void ()"),
    ("forja_arreglo_nuevo", "ptr (i64, i64)"),
    ("forja_arreglo_concat", "ptr (ptr, ptr)"),
    ("forja_leer_linea", "ptr ()"),
    ("forja_escribir_linea", "void (ptr)"),
    ("forja_unir_elementos", "ptr (ptr, ptr)"),
    ("forja_tipo_de", "ptr (ptr)"),
    ("forja_mapa_claves", "ptr (ptr)"),
    ("forja_mapa_valores", "ptr (ptr)"),
    ("forja_mapa_pares", "ptr (ptr)"),
    ("forja_registrar_texto", "void (ptr)"),
    ("forja_imprimir_dinamico", "void (ptr)"),
    ("forja_a_texto_dinamico", "ptr (ptr)"),
    ("forja_arreglo_longitud", "i64 (ptr)"),
    ("forja_indice_dinamico", "i64 (ptr, i64)"),
    ("forja_longitud_dinamica", "i64 (ptr)"),
    ("forja_arreglo_rellenar", "ptr (i64, i64, i64)"),
    ("forja_arreglo_a_texto", "ptr (ptr)"),
    ("forja_imprimir_arreglo", "void (ptr)"),
    ("forja_imprimir_mapa", "void (ptr)"),
    ("forja_exacto_nuevo", "ptr (i64, i64, i64)"),
    ("forja_exacto_desde_entero", "ptr (i64)"),
    ("forja_exacto_sumar", "ptr (ptr, ptr)"),
    ("forja_exacto_restar", "ptr (ptr, ptr)"),
    ("forja_exacto_multiplicar", "ptr (ptr, ptr)"),
    ("forja_exacto_dividir", "ptr (ptr, ptr)"),
    ("forja_exacto_comparar", "i64 (ptr, ptr)"),
    ("forja_exacto_a_entero", "i64 (ptr)"),
    ("forja_exacto_a_decimal", "double (ptr)"),
    ("forja_exacto_a_texto", "ptr (ptr)"),
    ("forja_dividir", "ptr (ptr, ptr)"),
    ("forja_recortar", "ptr (ptr)"),
    ("forja_arreglo_obtener", "i64 (ptr, i64)"),
    ("forja_arreglo_validar", "ptr (i64)"),
    ("forja_arreglo_fijar", "void (ptr, i64, i64)"),
    ("forja_empujar", "void (ptr, i64)"),
    ("forja_reservar", "void (ptr, i64)"),
    ("forja_mapa_nuevo", "ptr ()"),
    ("forja_mapa_longitud", "i64 (ptr)"),
    ("forja_mapa_fijar_i64", "void (ptr, i64, i64, i64)"),
    ("forja_mapa_obtener_i64", "i64 (ptr, i64)"),
    ("forja_mapa_fijar_texto", "void (ptr, ptr, i64, i64)"),
    ("forja_mapa_obtener_texto", "i64 (ptr, ptr)"),
    ("forja_mapa_fijar_dinamico", "void (ptr, i64, i64, i64)"),
    ("forja_mapa_obtener_dinamico", "i64 (ptr, i64)"),
    ("forja_mapa_valor_a_texto_i64", "ptr (ptr, i64)"),
    ("forja_mapa_valor_a_texto_texto", "ptr (ptr, ptr)"),
    ("forja_reemplazar", "ptr (ptr, ptr, ptr)"),
    ("forja_a_codigo", "i64 (ptr)"),
    ("forja_a_caracter", "ptr (i64)"),
    ("forja_canal_nuevo", "ptr ()"),
    ("forja_canal_enviar", "void (ptr, i64)"),
    ("forja_canal_recibir", "i64 (ptr)"),
    ("forja_canal_intentar", "i64 (ptr)"),
    ("forja_esperar", "void (i64)"),
    // Primitivas de la stdlib (`_archivo_*`, `_env_*`, `_sistema_*`): mismo
    // contrato que `forja-native` (centinela -1 / `nulo` en vez de abortar).
    ("forja_archivo_leer", "i64 (ptr)"),
    ("forja_archivo_escribir", "i64 (ptr, ptr)"),
    ("forja_archivo_agregar", "i64 (ptr, ptr)"),
    ("forja_archivo_existe", "i64 (ptr)"),
    ("forja_archivo_eliminar", "i64 (ptr)"),
    ("forja_archivo_tamano", "i64 (ptr)"),
    ("forja_env_obtener", "ptr (ptr)"),
    ("forja_env_existe", "i64 (ptr)"),
    ("forja_env_establecer", "void (ptr, ptr)"),
    ("forja_env_eliminar", "void (ptr)"),
    ("forja_sistema_so", "ptr ()"),
    ("forja_sistema_directorio_actual", "ptr ()"),
    ("forja_sistema_directorio_temporal", "ptr ()"),
    ("forja_sistema_hostname", "ptr ()"),
    ("forja_sistema_usuario", "ptr ()"),
    ("forja_sistema_cpus", "i64 ()"),
    ("forja_sistema_salir", "void (i64)"),
    ("forja_hilo_lanzar", "ptr (ptr, ptr)"),
    ("forja_hilo_unir", "i64 (ptr)"),
    ("exit", "void (i32)"),
];

impl Lowering {
    pub fn nuevo() -> Self {
        Lowering {
            header: String::new(),
            decls: String::new(),
            globals: String::new(),
            cuerpos: String::new(),
            extra: String::new(),
            prologo: String::new(),
            vars: HashMap::new(),
            firmas: HashMap::new(),
            externas: HashMap::new(),
            nombres_externos: HashMap::new(),
            clases: HashMap::new(),
            enums: HashMap::new(),
            variantes: HashMap::new(),
            consts: HashMap::new(),
            tipos_pre: HashMap::new(),
            tipos_top: HashMap::new(),
            evidencia: HashMap::new(),
            resultado_fn: HashMap::new(),
            resultado_var: HashMap::new(),
            evidencia_texto: std::collections::HashSet::new(),
            arreglos_texto: HashMap::new(),
            arreglos_dec: HashMap::new(),
            inciertos: std::collections::HashSet::new(),
            arreglos_arreglo: HashMap::new(),
            metodos_param: std::collections::HashSet::new(),
            funciones_param: std::collections::HashSet::new(),
            firmas_closure: HashMap::new(),
            postcondiciones: Vec::new(),
            anteriores: HashMap::new(),
            params_dinamicos: std::collections::HashSet::new(),
            ret_dinamico: false,
            derive_mostrar: std::collections::HashSet::new(),
            derive_igual: std::collections::HashSet::new(),
            etiquetas: Vec::new(),
            ret_actual: T::Int,
            termina: false,
            lc: 0,
            rc: 0,
            sc: 0,
            sin_rangos: std::env::var("FORJA_SIN_RANGOS")
                .map(|v| {
                    matches!(
                        v.to_lowercase().as_str(),
                        "1" | "si" | "sí" | "on" | "verdadero"
                    )
                })
                .unwrap_or_else(|_| !crate::config::actual().verificar_rangos),
            sin_check: Vec::new(),
        }
    }

    // ── utilidades de emisión ────────────────────────────────────────────

    fn r(&mut self) -> String {
        self.rc += 1;
        format!("%v{}", self.rc)
    }

    fn bloque(&mut self, prefijo: &str) -> String {
        self.lc += 1;
        format!("{}.{}", prefijo, self.lc)
    }

    fn linea(&mut self, texto: &str) {
        let _ = writeln!(self.cuerpos, "  {}", texto);
    }

    fn etiqueta(&mut self, nombre: &str) {
        let _ = writeln!(self.cuerpos, "{}:", nombre);
        self.termina = false;
    }

    fn texto_constante(&mut self, s: &str) -> String {
        // Los literales se deduplican: misma cadena → mismo global.
        if let Some(g) = self.consts.get(s) {
            return g.clone();
        }
        let id = self.sc;
        self.sc += 1;
        let nombre = format!("@.str{}", id);
        let mut esc = String::new();
        for b in s.as_bytes() {
            match b {
                b'\\' => esc.push_str("\\5C"),
                b'"' => esc.push_str("\\22"),
                b'\n' => esc.push_str("\\0A"),
                b'\r' => esc.push_str("\\0D"),
                b'\t' => esc.push_str("\\09"),
                0x20..=0x7E => esc.push(*b as char),
                _ => {
                    let _ = write!(esc, "\\{:02X}", b);
                }
            }
        }
        let _ = writeln!(
            self.globals,
            "{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1",
            nombre,
            s.len() + 1,
            esc
        );
        self.consts.insert(s.to_string(), nombre.clone());
        nombre
    }

    fn alloca(&mut self, t: &T, nombre: &str) -> String {
        // El contador se incrementa acá: dos slots del mismo nombre (`_` de un
        // descarte) en la misma etiqueta colisionaban (`201_login_gui`: `%_.98`
        // definido dos veces).
        self.lc += 1;
        let slot = format!("%{}.{}", nombre, self.lc);
        let _ = writeln!(
            self.prologo,
            "  {} = alloca {}",
            slot,
            t.llvm().to_llvm_ir()
        );
        slot
    }

    fn cargar(&mut self, slot: &str, t: &T) -> V {
        let r = self.r();
        self.linea(&format!(
            "{} = load {}, ptr {}",
            r,
            t.llvm().to_llvm_ir(),
            slot
        ));
        match t {
            T::Dec => V::Dec(r),
            T::Texto => V::Texto(r),
            T::Arreglo => V::Arreglo(r),
            T::Mapa => V::Mapa(r),
            T::Exacto => V::Exacto(r),
            T::Enum(c) => V::Enum(r, c.clone()),
            T::Canal => V::Canal(r),
            T::Closure => V::Closure(r),
            T::Clase(c) => V::Clase(r, c.clone()),
            T::Funcion => V::Funcion(r),
            T::Bool => V::Bool(r),
            // Un slot declarado `nulo` sigue siendo nulo (la VM imprime "nulo").
            T::Nulo => V::Nulo,
            T::Int => V::Int(r),
        }
    }

    fn guardar(&mut self, slot: &str, v: V, t: &T) {
        let r = self.como(&v, t.clone());
        self.linea(&format!(
            "store {} {}, ptr {}",
            t.llvm().to_llvm_ir(),
            r,
            slot
        ));
    }

    /// Convierte un valor al tipo pedido (conversiones que LLVM no hace solo).
    fn como(&mut self, v: &V, destino: T) -> String {
        if v.t() == destino {
            return v.r().to_string();
        }
        match (v, &destino) {
            (V::Nulo, T::Int) => "0".into(),
            (V::Nulo, T::Bool) => "0".into(),
            // `Booleano` y `Entero` son el mismo `i64`.
            (V::Bool(r), T::Int) => r.clone(),
            (V::Int(r), T::Bool) => r.clone(),
            (V::Bool(r), T::Texto) => {
                let d = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_booleano_a_texto(i64 {})",
                    d, r
                ));
                d
            }
            (V::Nulo, T::Dec) => "0.0".into(),
            (V::Nulo, T::Texto) => {
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_texto_nulo()", r));
                r
            }
            (V::Nulo, T::Arreglo) | (V::Nulo, T::Clase(_)) | (V::Nulo, T::Funcion) => "null".into(),
            (V::Int(r), T::Dec) => {
                let d = self.r();
                self.linea(&format!("{} = sitofp i64 {} to double", d, r));
                d
            }
            (V::Dec(r), T::Int) => {
                let d = self.r();
                self.linea(&format!("{} = fptosi double {} to i64", d, r));
                d
            }
            (V::Texto(r), T::Int) => {
                let d = self.r();
                self.linea(&format!("{} = call i64 @forja_a_entero(ptr {})", d, r));
                d
            }
            (V::Texto(r), T::Dec) => {
                let d = self.r();
                self.linea(&format!("{} = call double @forja_a_decimal(ptr {})", d, r));
                d
            }
            // Entero/Decimal → Exacto (coerción por tipo declarado).
            (V::Int(r), T::Exacto) => {
                let e = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_exacto_desde_entero(i64 {})",
                    e, r
                ));
                e
            }
            (V::Dec(r), T::Exacto) => {
                let d = self.r();
                self.linea(&format!("{} = fmul double {}, 10000000000.0", d, r));
                let ancho = self.r();
                self.linea(&format!("{} = fptosi double {} to i128", ancho, d));
                let alto = self.r();
                self.linea(&format!("{} = lshr i128 {}, 64", alto, ancho));
                let alto64 = self.r();
                self.linea(&format!("{} = trunc i128 {} to i64", alto64, alto));
                let bajo = self.r();
                self.linea(&format!("{} = trunc i128 {} to i64", bajo, ancho));
                let e = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_exacto_nuevo(i64 {}, i64 {}, i64 10)",
                    e, alto64, bajo
                ));
                e
            }
            (V::Int(r), T::Texto) => {
                // Un `Entero` puede ser en realidad un texto/arreglo que llegó
                // por un slot sin tipo (`T::Int` es el comodín): el runtime
                // decide por su tag, como la VM.
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, r));
                let d = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_a_texto_dinamico(ptr {})",
                    d, p
                ));
                d
            }
            (V::Dec(r), T::Texto) => {
                let d = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_decimal_a_texto(double {})",
                    d, r
                ));
                d
            }
            // Un arreglo concatenado con Texto va a su repr (`[1,2,3]`), como
            // `imprimir_arreglo` y como la VM.
            (V::Arreglo(r), T::Texto) => {
                let d = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_arreglo_a_texto(ptr {})",
                    d, r
                ));
                d
            }
            // Un valor de `tipo` (enum) concatenado va como `<Nombre>`: la VM no
            // expone la variante ni el payload.
            (V::Enum(_, c), T::Texto) => {
                let texto = format!("<{c}>");
                self.texto_constante(&texto)
            }
            // shortcut: mapa/enum/clase/función concatenados con Texto se
            // imprimen como la dirección; `escribir` de esos tipos ya devuelve
            // error explícito. Upgrade: `mostrar` por tipo.
            (V::Clase(r, c), T::Texto) if self.derive_mostrar.contains(c) => {
                self.clase_a_texto(r, c, 0)
            }
            (V::Mapa(r) | V::Clase(r, _) | V::Funcion(r), T::Texto) => {
                let i = self.r();
                self.linea(&format!("{} = ptrtoint ptr {} to i64", i, r));
                let d = self.r();
                self.linea(&format!("{} = call ptr @forja_a_texto(i64 {})", d, i));
                d
            }
            (v, T::Int) => {
                // arreglo/clase/funcion como entero: valor de la dirección.
                let i = self.r();
                self.linea(&format!("{} = ptrtoint ptr {} to i64", i, v.r()));
                i
            }
            (v, T::Arreglo)
            | (v, T::Mapa)
            | (v, T::Exacto)
            | (v, T::Enum(_))
            | (v, T::Canal)
            | (v, T::Closure)
            | (v, T::Clase(_))
            | (v, T::Funcion) => {
                // Si ya es un puntero (arreglo→mapa, clase→clase) no hay nada que
                // convertir; inttoptr sólo aplica desde un entero.
                match v.t() {
                    T::Int | T::Nulo => {
                        let p = self.r();
                        self.linea(&format!("{} = inttoptr i64 {} to ptr", p, v.r()));
                        p
                    }
                    _ => v.r().to_string(),
                }
            }
            _ => v.r().to_string(),
        }
    }

    fn cond(&mut self, v: &V) -> String {
        match v {
            V::Int(r) => {
                let c = self.r();
                self.linea(&format!("{} = icmp ne i64 {}, 0", c, r));
                c
            }
            V::Bool(r) => {
                let c = self.r();
                self.linea(&format!("{} = icmp ne i64 {}, 0", c, r));
                c
            }
            V::Dec(r) => {
                let c = self.r();
                self.linea(&format!("{} = fcmp one double {}, 0.0", c, r));
                c
            }
            V::Exacto(r) => {
                let cero = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_exacto_desde_entero(i64 0)",
                    cero
                ));
                let cmp = self.r();
                self.linea(&format!(
                    "{} = call i64 @forja_exacto_comparar(ptr {}, ptr {})",
                    cmp, r, cero
                ));
                let c = self.r();
                self.linea(&format!("{} = icmp ne i64 {}, 0", c, cmp));
                c
            }
            V::Texto(r)
            | V::Arreglo(r)
            | V::Mapa(r)
            | V::Enum(r, _)
            | V::Canal(r)
            | V::Closure(r)
            | V::Clase(r, _)
            | V::Funcion(r) => {
                let c = self.r();
                self.linea(&format!("{} = icmp ne ptr {}, null", c, r));
                c
            }
            V::Nulo => "false".into(),
        }
    }

    // ── programa ─────────────────────────────────────────────────────────

    pub fn compilar(&mut self, decls: &[Declaracion]) -> Result<String, String> {
        let _ = writeln!(self.header, "; forja → LLVM IR (backend nuevo, ptr opacos)");

        // 0. Tipos que llegan por argumento en cada llamada (los parámetros sin
        // anotar se tipan con esto: `nuevo Nodo("x")` → ese parámetro es Texto).
        let (evidencia, evidencia_texto) = evidencia_de_llamadas(decls);
        self.evidencia = evidencia;
        self.evidencia_texto = evidencia_texto;

        // 1. Firmas, clases y prototipos de FFI.
        for d in decls {
            match d {
                Declaracion::Funcion {
                    nombre,
                    parametros,
                    tipo_retorno,
                    cuerpo,
                    externa,
                    enlace_nombre,
                    ..
                } => {
                    let params = self.tipos_params(None, nombre, parametros, cuerpo);
                    for (i, p) in parametros.iter().enumerate() {
                        if matches!(p.tipo, Some(Tipo::Parametro(_))) {
                            self.params_dinamicos.insert(format!("{nombre}:{i}"));
                        }
                    }
                    if matches!(tipo_retorno, Some(Tipo::Parametro(_))) {
                        self.funciones_param.insert(nombre.clone());
                    }
                    let ret = if matches!(tipo_retorno, Some(Tipo::Parametro(_))) {
                        T::Int
                    } else if *externa && tipo_retorno.is_none() {
                        T::Nulo
                    } else {
                        self.tipo_declarado(tipo_retorno)
                            // sin anotación: se infiere del primer `retornar expr`
                            .or_else(|| self.inferir_retorno(cuerpo, parametros, &params))
                            .unwrap_or(T::Int)
                    };
                    if let Some(Tipo::Resultado(ok, err)) = tipo_retorno {
                        let t_ok = self.tipo_declarado(&Some(ok.as_ref().clone()));
                        let t_err = self.tipo_declarado(&Some(err.as_ref().clone()));
                        if let (Some(a), Some(b)) = (t_ok, t_err) {
                            self.resultado_fn.insert(nombre.clone(), (a, b));
                        }
                    }
                    if *externa {
                        let real = enlace_nombre.clone().unwrap_or_else(|| nombre.clone());
                        self.nombres_externos.insert(nombre.clone(), real.clone());
                        let ps: Vec<String> =
                            params.iter().map(|t| t.llvm().to_llvm_ir()).collect();
                        let _ = writeln!(
                            self.decls,
                            "declare {} @{}({})",
                            ret.llvm().to_llvm_ir(),
                            real,
                            ps.join(", ")
                        );
                        self.externas.insert(nombre.clone(), (params, ret));
                    } else {
                        self.firmas.insert(nombre.clone(), (params, ret));
                    }
                }
                Declaracion::Clase {
                    nombre,
                    campos,
                    metodos,
                    atributos,
                    parametros_tipo,
                    ..
                } => {
                    // `@derive(Mostrar)` → `escribir(obj)` da `Clase { campo: valor }`.
                    if atributos.iter().any(|a| {
                        a.nombre == "derive" && a.argumentos.iter().any(|x| x == "Mostrar")
                    }) {
                        self.derive_mostrar.insert(nombre.clone());
                    }
                    if atributos.iter().any(|a| {
                        a.nombre == "derive"
                            && a.argumentos
                                .iter()
                                .any(|x| x == "Igual" || x == "Igualdad" || x == "Eq")
                    }) {
                        self.derive_igual.insert(nombre.clone());
                    }
                    let campos: Vec<(String, T)> = campos
                        .iter()
                        .map(|c| {
                            (
                                c.nombre.clone(),
                                self.tipo_declarado(&c.tipo)
                                    .or_else(|| self.inferir_campo(nombre, &c.nombre, decls))
                                    .unwrap_or(T::Int),
                            )
                        })
                        .collect();
                    let ms: Vec<(String, Vec<T>, T)> = metodos
                        .iter()
                        .map(|m| {
                            let clave = if m.nombre == "constructor" || m.nombre == "nuevo" {
                                format!("constructor@{nombre}")
                            } else {
                                m.nombre.clone()
                            };
                            let params =
                                self.tipos_params(Some(nombre), &clave, &m.parametros, &m.cuerpo);
                            if matches!(m.tipo_retorno, Some(Tipo::Parametro(_))) {
                                // Con la clase en la clave: el `-> T` de `Caja<T>`
                                // no vuelve «incierto» el `-> Booleano` de
                                // `Caja_Booleano`.
                                self.metodos_param.insert(format!("{nombre}::{}", m.nombre));
                            }
                            // La clave lleva la clase: `nuevo:0` de `Caja<T>`
                            // no debe marcar como dinámico el de `Caja_Texto`
                            // (que sí tiene tipo concreto).
                            for (i, p) in m.parametros.iter().enumerate() {
                                if matches!(p.tipo, Some(Tipo::Parametro(_))) {
                                    self.params_dinamicos
                                        .insert(format!("{nombre}::{}:{i}", m.nombre));
                                }
                            }
                            (m.nombre.clone(), params, self.retorno_metodo(nombre, m))
                        })
                        .collect();
                    let c = self.clases.entry(nombre.clone()).or_default();
                    c.campos = campos;
                    c.metodos.extend(ms);
                    c.generica = !parametros_tipo.is_empty();
                }
                Declaracion::Implementacion {
                    clase_nombre,
                    metodos,
                    ..
                } => {
                    // Los métodos de `implementa Rasgo para Clase` son métodos
                    // de la clase: misma tabla y misma emisión que los suyos.
                    let ms: Vec<(String, Vec<T>, T)> = metodos
                        .iter()
                        .map(|m| {
                            let params = self.tipos_params(
                                Some(clase_nombre),
                                &m.nombre,
                                &m.parametros,
                                &m.cuerpo,
                            );
                            (
                                m.nombre.clone(),
                                params,
                                self.retorno_metodo(clase_nombre, m),
                            )
                        })
                        .collect();
                    self.clases
                        .entry(clase_nombre.clone())
                        .or_default()
                        .metodos
                        .extend(ms);
                }
                Declaracion::Enum {
                    nombre, variantes, ..
                } => {
                    let campos: Vec<(String, usize)> = variantes
                        .iter()
                        .map(|v| (v.nombre.clone(), v.tipos.len()))
                        .collect();
                    let max_campos = campos.iter().map(|(_, n)| *n).max().unwrap_or(0);
                    for (i, v) in variantes.iter().enumerate() {
                        // se guardan los tipos de los campos: al hacer `coincidir`
                        // el payload se enlaza con su tipo real (texto vs entero).
                        let tipos: Vec<T> = v
                            .tipos
                            .iter()
                            .map(|t| self.tipo_declarado(&Some(t.clone())).unwrap_or(T::Int))
                            .collect();
                        self.variantes
                            .insert(v.nombre.clone(), (nombre.clone(), i, tipos));
                    }
                    self.enums
                        .insert(nombre.clone(), EnumInformacion { max_campos });
                }
                Declaracion::Rasgo { .. } => {}
                _ => {}
            }
        }

        // 1.5. Tag de clase (1..N, en orden alfabético para que no dependa del
        // orden del HashMap): se guarda en la palabra 0 de cada objeto.
        let mut nombres_clase: Vec<String> = self.clases.keys().cloned().collect();
        nombres_clase.sort();
        for (i, nombre) in nombres_clase.iter().enumerate() {
            if let Some(c) = self.clases.get_mut(nombre) {
                c.tag = i as u64 + 1;
            }
        }

        // 2. Tipos de clase (se emiten en el ensamblado final, antes de los globals).
        let clases: Vec<(String, Vec<(String, T)>)> = self
            .clases
            .iter()
            .map(|(n, c)| (n.clone(), c.campos.clone()))
            .collect();

        // 3. Runtime de soporte.
        // `noalias` en el retorno de forja_alloc: el allocator nunca devuelve
        // memoria que alias otra viva, lo que habilita LICM/vectorización.
        let _ = writeln!(self.decls, "declare noalias ptr @forja_alloc(i64)");
        for (nombre, firma) in RUNTIME.iter().filter(|(n, _)| *n != "forja_alloc") {
            let (ret, params) = firma.split_once(' ').unwrap_or(("i64", "()"));
            let _ = writeln!(self.decls, "declare {} @{}{}", ret, nombre, params);
        }

        // 3.4 Preludio: `Resultado` y `Opcion` son tipos del lenguaje (los inyecta
        // el pipeline como `enum`), pero el backend los registra siempre para que
        // `Ok`/`Error`/`Algo`/`Ninguno` y `?` funcionen sin declararlos.
        // shortcut: sin genéricos en la representación, se asume el uso
        // convencional (`Ok`/`Algo` con Entero, `Error` con Texto). Upgrade:
        // propagar `Resultado<T, E>` desde la firma de la función.
        for (nombre_enum, variantes) in [
            (
                "Resultado",
                vec![("Ok", vec![T::Int]), ("Error", vec![T::Texto])],
            ),
            ("Opcion", vec![("Algo", vec![T::Int]), ("Ninguno", vec![])]),
        ] {
            if self.enums.contains_key(nombre_enum) {
                continue;
            }
            let max_campos = variantes
                .iter()
                .map(|(_, tipos)| tipos.len())
                .max()
                .unwrap_or(0);
            for (i, (v, tipos)) in variantes.iter().enumerate() {
                self.variantes
                    .insert(v.to_string(), (nombre_enum.to_string(), i, tipos.clone()));
            }
            // `Alguno(T)` es el nombre del preludio (`Opcion<T> = Alguno(T) |
            // Ninguno`) y `Algo` el atajo: los dos apuntan a la misma variante
            // (`183_opcion_propagacion`).
            if let Some(v) = self.variantes.get("Algo").cloned() {
                self.variantes.entry("Alguno".to_string()).or_insert(v);
            }
            self.enums
                .insert(nombre_enum.to_string(), EnumInformacion { max_campos });
        }

        // 3.5 Variables de nivel superior: las que usan funciones/closures/hilos
        // se declaran como globals (las necesita la función); el resto vivirá en
        // un `alloca` de `@forja_init` y LLVM lo promoverá a registro.
        let usados_en_funciones = nombres_usados_en_funciones(decls);
        // Si la stdlib inlineada y el programa declaran el mismo nombre, gana la
        // Misma variable declarada por un módulo inlineado y por el programa:
        // gana la ÚLTIMA (el programa va después del `importar`), que es la del
        // usuario. Si no, el tipo y el slot salen del módulo equivocado
        // (`variable x: Exacto` terminaba siendo un entero de la stdlib).
        let mut ultima_declaracion: HashMap<&str, usize> = HashMap::new();
        for (i, d) in decls.iter().enumerate() {
            if let Declaracion::Variable { nombre, .. } = d {
                ultima_declaracion.insert(nombre.as_str(), i);
            }
        }
        // Tipos que se le asignan a cada nombre en todo el programa (dentro y
        // fuera de funciones): un global que arranca en `nulo` se tipa con el
        // primer valor concreto que recibe.
        let mut pistas_top: HashMap<String, T> = HashMap::new();
        self.preescanear_tipos(decls, &mut pistas_top);
        // Se guardan en orden de fuente: un `variable a = 1` seguido de
        // `variable b = a + 1` necesita inicializarse en ese orden, sea global o
        // local (`@forja_init` los emite así).
        let mut iniciales: Vec<(String, T, Option<Expresion>, bool)> = Vec::new();
        for (idx, d) in decls.iter().enumerate() {
            if let Declaracion::Variable {
                nombre,
                valor,
                tipo,
                ..
            } = d
            {
                if ultima_declaracion.get(nombre.as_str()).copied() != Some(idx) {
                    continue; // redeclarado más adelante (programa del usuario)
                }
                let mut t = self.tipo_declarado(tipo).unwrap_or_else(|| {
                    valor
                        .as_ref()
                        .map(|v| self.tipo_estatico(v))
                        .unwrap_or(T::Int)
                });
                // `variable x = nulo` y después `x = nuevo C()`: el global se
                // tipa con el primer valor concreto que se le asigna (la misma
                // regla que los locales). Sin esto el global era `i64` y se
                // guardaba un puntero en él (`171`).
                if t == T::Nulo {
                    if let Some(concreto) = pistas_top
                        .get(nombre)
                        .cloned()
                        .or_else(|| self.tipo_asignado_en_funciones(decls, nombre))
                    {
                        t = concreto;
                    }
                }
                self.tipos_top.insert(nombre.clone(), t.clone());
                if usados_en_funciones.contains(nombre) {
                    let _ = writeln!(
                        self.globals,
                        "@g_{} = internal global {} {}, align 8",
                        nombre,
                        t.llvm().to_llvm_ir(),
                        cero(&t)
                    );
                    self.vars.insert(
                        nombre.clone(),
                        Var {
                            slot: format!("@g_{}", nombre),
                            t: t.clone(),
                        },
                    );
                    iniciales.push((nombre.clone(), t, valor.clone(), true));
                } else {
                    iniciales.push((nombre.clone(), t, valor.clone(), false));
                }
            }
        }

        // 4. Funciones del usuario (primero las que no son main).
        for d in decls {
            if let Declaracion::Funcion {
                nombre,
                parametros,
                tipo_retorno,
                cuerpo,
                externa,
                ..
            } = d
            {
                if *externa {
                    continue;
                }
                let (params, ret) = self
                    .firmas
                    .get(nombre)
                    .cloned()
                    .unwrap_or((Vec::new(), T::Int));
                let _ = tipo_retorno;
                let (precondiciones, postcondiciones) = match d {
                    Declaracion::Funcion {
                        precondiciones,
                        postcondiciones,
                        ..
                    } => (precondiciones.as_slice(), postcondiciones.as_slice()),
                    _ => (&[][..], &[][..]),
                };
                self.funcion(
                    nombre,
                    parametros,
                    &params,
                    ret.clone(),
                    precondiciones,
                    postcondiciones,
                    cuerpo,
                )?;
            }
        }

        // 5. Métodos de clase.
        for d in decls {
            if let Declaracion::Clase {
                nombre,
                metodos,
                campos,
                ..
            } = d
            {
                let campos_t: Vec<(String, T)> = campos
                    .iter()
                    .map(|c| {
                        (
                            c.nombre.clone(),
                            self.tipo_declarado(&c.tipo).unwrap_or(T::Int),
                        )
                    })
                    .collect();
                for m in metodos {
                    self.metodo(nombre, &campos_t, m)?;
                }
            }
            if let Declaracion::Implementacion {
                clase_nombre,
                metodos,
                ..
            } = d
            {
                for m in metodos {
                    self.metodo(clase_nombre, &[], m)?;
                }
            }
        }

        // Las funciones ya están emitidas en `self.cuerpos`: se aparta ese
        // buffer para que las instrucciones del init no caigan en medio.
        let cuerpo_funciones = std::mem::take(&mut self.cuerpos);
        // `self.vars` ya tiene los globals registrados: el init los necesita.
        self.prologo.clear();
        self.termina = false;
        // Una sola pasada en orden de fuente: inicializaciones de variables y
        // sentencias sueltas intercaladas como en el programa.
        for (idx, d) in decls.iter().enumerate() {
            match d {
                Declaracion::Funcion { .. }
                | Declaracion::Clase { .. }
                | Declaracion::Rasgo { .. }
                | Declaracion::Implementacion { .. }
                | Declaracion::Importar(_)
                | Declaracion::ImportarExterna(_)
                | Declaracion::Enum { .. } => {}
                Declaracion::Variable { nombre, .. } => {
                    if ultima_declaracion.get(nombre.as_str()).copied() != Some(idx) {
                        continue; // redeclarada más adelante
                    }
                    if let Some((_, t, valor, es_global)) =
                        iniciales.iter().find(|(n, _, _, _)| n == nombre)
                    {
                        let slot = if *es_global {
                            format!("@g_{}", nombre)
                        } else {
                            let slot = self.alloca(t, nombre);
                            self.vars.insert(
                                nombre.clone(),
                                Var {
                                    slot: slot.clone(),
                                    t: t.clone(),
                                },
                            );
                            slot
                        };
                        if let Some(v) = valor {
                            // `variable r = f()` con `f() -> Resultado<Texto, …>`:
                            // se recuerdan los tipos del payload para el `coincidir`.
                            if let Expresion::LlamadaFuncion { nombre: f, .. } = v {
                                if let Some(tipos) = self.resultado_fn.get(f).cloned() {
                                    self.resultado_var.insert(nombre.clone(), tipos);
                                }
                            }
                            let val = self.expr(v)?;
                            self.guardar(&slot, val, t);
                            // recordar arreglos de textos también a nivel superior
                            if self.arreglo_de_texto(v) {
                                self.arreglos_texto.insert(nombre.clone(), true);
                            }
                            if self.arreglo_de_dec(v) {
                                self.arreglos_dec.insert(nombre.clone(), true);
                            }
                        }
                    }
                }
                _ => self.sentencia(d)?,
            }
        }
        let prologo_inicializar = std::mem::take(&mut self.prologo);
        let mut cuerpo_inicializar_fn = prologo_inicializar;
        cuerpo_inicializar_fn.push_str(&std::mem::take(&mut self.cuerpos));
        self.cuerpos = cuerpo_funciones;

        let mut salida = String::new();
        salida.push_str(&self.header);
        // Layout del arreglo del runtime: { len, cap, datos }.
        let _ = writeln!(salida, "%ForjaArreglo = type {{ i64, i64, i64, ptr }}");
        salida.push('\n');
        for (nombre, campos) in &clases {
            // Palabra 0: tag de la clase (despacho dinámico). Campos desde 1.
            let mut fs = vec!["i64".to_string()];
            fs.extend(campos.iter().map(|(_, t)| t.llvm().to_llvm_ir()));
            let _ = writeln!(salida, "%{} = type {{ {} }}", nombre, fs.join(", "));
        }
        let enums: Vec<(String, usize)> = self
            .enums
            .iter()
            .map(|(n, e)| (n.clone(), e.max_campos))
            .collect();
        for (nombre, max) in &enums {
            let mut fs = vec!["i64".to_string()];
            for _ in 0..*max {
                fs.push("i64".to_string());
            }
            let _ = writeln!(salida, "%Enum.{} = type {{ {} }}", nombre, fs.join(", "));
        }
        salida.push_str(&self.globals);
        salida.push_str(&self.decls);
        salida.push_str(&self.extra);
        salida.push_str(&self.cuerpos);
        let _ = writeln!(salida, "define void @forja_init() {{");
        let _ = writeln!(salida, "entry:");
        // Los literales de texto se registran como textos: así `longitud`/`[i]`
        // saben qué son aunque lleguen por un slot tipado de otra cosa (la VM
        // decide por el valor, no por el slot).
        for nombre in self.consts.values() {
            let _ = writeln!(salida, "  call void @forja_registrar_texto(ptr {})", nombre);
        }
        salida.push_str(&cuerpo_inicializar_fn);
        let _ = writeln!(salida, "  ret void");
        let _ = writeln!(salida, "}}");
        let _ = writeln!(salida, "define i32 @main() {{");
        let _ = writeln!(salida, "entry:");
        let _ = writeln!(salida, "  call void @forja_init()");
        // `main()` sólo corre si el programa lo llama: así se comporta la VM
        // (antes el nativo lo invocaba solo y un ejemplo imprimía de más).
        let _ = writeln!(salida, "  ret i32 0");
        let _ = writeln!(salida, "}}");
        Ok(salida)
    }

    /// Retorno de un método: anotado o, si no, el del primer `retornar` (los
    /// métodos de `implementa` suelen devolver el propio arreglo sin anotar).
    fn retorno_metodo(&self, clase: &str, m: &Metodo) -> T {
        if m.nombre == "constructor" || m.nombre == "nuevo" {
            return T::Nulo;
        }
        // `-> T` sin resolver: se queda como `Entero` (inferirlo de un
        // `retornar nulo` lo dejaba como `void` y el cuerpo devolvía un valor).
        if matches!(m.tipo_retorno, Some(Tipo::Parametro(_))) {
            return T::Int;
        }
        let params = self.tipos_params(Some(clase), &m.nombre, &m.parametros, &m.cuerpo);
        self.tipo_declarado(&m.tipo_retorno)
            .or_else(|| self.inferir_retorno(&m.cuerpo, &m.parametros, &params))
            .unwrap_or(T::Nulo)
    }

    /// Tipos de los parámetros de un método más los locales del cuerpo (lo que
    /// hace falta para saber de qué tipo es lo que se le asigna a un campo).
    fn pistas_de(
        &self,
        clase: &str,
        nombre: &str,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> HashMap<String, T> {
        let mut pistas: HashMap<String, T> = HashMap::new();
        self.preescanear_tipos(cuerpo, &mut pistas);
        let tipos = self.tipos_params(Some(clase), nombre, parametros, cuerpo);
        for (i, p) in parametros.iter().enumerate() {
            if let Some(t) = tipos.get(i) {
                pistas.insert(p.nombre.clone(), t.clone());
            }
        }
        pistas
    }

    fn inferir_campo(&self, clase: &str, campo: &str, decls: &[Declaracion]) -> Option<T> {
        fn tipo_valor(e: &Expresion, pistas: &HashMap<String, T>) -> Option<T> {
            Some(match e {
                Expresion::LiteralTexto(_) => T::Texto,
                Expresion::LiteralDecimal(_) => T::Dec,
                Expresion::LiteralExacto(..) => T::Exacto,
                Expresion::Arreglo(_) => T::Arreglo,
                Expresion::Mapa(_) => T::Mapa,
                Expresion::Instanciacion { clase, .. } => T::Clase(clase.clone()),
                Expresion::LiteralBooleano(_) => T::Bool,
                // `este.campo = valor` con `valor` un parámetro o local con tipo.
                Expresion::Identificador { nombre, .. } => pistas.get(nombre).cloned()?,
                _ => return None,
            })
        }
        fn en_expr(e: &Expresion, campo: &str, pistas: &HashMap<String, T>, out: &mut Vec<T>) {
            match e {
                Expresion::AsignacionCampo {
                    campo: c, valor, ..
                } => {
                    if c == campo {
                        if let Some(t) = tipo_valor(valor, pistas) {
                            out.push(t);
                        }
                    }
                    en_expr(valor, campo, pistas, out);
                }
                Expresion::Binaria {
                    izquierda, derecha, ..
                } => {
                    en_expr(izquierda, campo, pistas, out);
                    en_expr(derecha, campo, pistas, out);
                }
                Expresion::Unaria { expr, .. } | Expresion::Grupo(expr) => {
                    en_expr(expr, campo, pistas, out)
                }
                Expresion::Ternario {
                    condicion,
                    si_verdadero,
                    si_falso,
                } => {
                    en_expr(condicion, campo, pistas, out);
                    en_expr(si_verdadero, campo, pistas, out);
                    en_expr(si_falso, campo, pistas, out);
                }
                Expresion::Index { objeto, indice } => {
                    en_expr(objeto, campo, pistas, out);
                    en_expr(indice, campo, pistas, out);
                }
                Expresion::LlamadaFuncion { argumentos, .. } => {
                    for a in argumentos {
                        en_expr(a, campo, pistas, out);
                    }
                }
                Expresion::LlamadaMetodo {
                    objeto, argumentos, ..
                } => {
                    en_expr(objeto, campo, pistas, out);
                    for a in argumentos {
                        en_expr(a, campo, pistas, out);
                    }
                }
                Expresion::AccesoMiembro { objeto, .. } => en_expr(objeto, campo, pistas, out),
                _ => {}
            }
        }
        fn en_sentencias(
            cuerpo: &[Declaracion],
            campo: &str,
            pistas: &HashMap<String, T>,
            out: &mut Vec<T>,
        ) {
            for d in cuerpo {
                match d {
                    Declaracion::AsignacionMiembro { miembro, valor, .. } => {
                        if miembro == campo {
                            if let Some(t) = tipo_valor(valor, pistas) {
                                out.push(t);
                            }
                        }
                        en_expr(valor, campo, pistas, out);
                    }
                    Declaracion::Variable { valor: Some(v), .. } => en_expr(v, campo, pistas, out),
                    Declaracion::Asignacion { valor, .. }
                    | Declaracion::AsignacionIndex { valor, .. } => {
                        en_expr(valor, campo, pistas, out)
                    }
                    Declaracion::Si {
                        condicion,
                        bloque_verdadero,
                        bloque_falso,
                    } => {
                        en_expr(condicion, campo, pistas, out);
                        en_sentencias(bloque_verdadero, campo, pistas, out);
                        if let Some(b) = bloque_falso {
                            en_sentencias(b, campo, pistas, out);
                        }
                    }
                    Declaracion::Mientras { condicion, bloque } => {
                        en_expr(condicion, campo, pistas, out);
                        en_sentencias(bloque, campo, pistas, out);
                    }
                    Declaracion::Para { bloque, .. } | Declaracion::Repetir { bloque, .. } => {
                        en_sentencias(bloque, campo, pistas, out)
                    }
                    Declaracion::Retornar { valor: Some(v) } => en_expr(v, campo, pistas, out),
                    Declaracion::Expresion(e) => en_expr(e, campo, pistas, out),
                    Declaracion::LlamadaFuncion { argumentos, .. } => {
                        for a in argumentos {
                            en_expr(a, campo, pistas, out)
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut tipos: Vec<T> = Vec::new();
        for d in decls {
            match d {
                Declaracion::Clase {
                    nombre, metodos, ..
                } if nombre == clase => {
                    for m in metodos {
                        // Los constructores se registran como `constructor@Clase`:
                        // sin eso sus pistas no se encontraban y los campos que el
                        // constructor llena desde un parámetro quedaban `Entero`
                        // (un texto guardado ahí se imprimía `0`).
                        let clave = if m.nombre == "constructor" || m.nombre == "nuevo" {
                            format!("constructor@{nombre}")
                        } else {
                            m.nombre.clone()
                        };
                        let pistas = self.pistas_de(nombre, &clave, &m.parametros, &m.cuerpo);
                        en_sentencias(&m.cuerpo, campo, &pistas, &mut tipos);
                    }
                }
                Declaracion::Implementacion {
                    clase_nombre,
                    metodos,
                    ..
                } if clase_nombre == clase => {
                    for m in metodos {
                        let clave = if m.nombre == "constructor" || m.nombre == "nuevo" {
                            format!("constructor@{clase_nombre}")
                        } else {
                            m.nombre.clone()
                        };
                        let pistas = self.pistas_de(clase_nombre, &clave, &m.parametros, &m.cuerpo);
                        en_sentencias(&m.cuerpo, campo, &pistas, &mut tipos);
                    }
                }
                // El campo también se llena desde fuera (`p.nombre = "Rex"`).
                Declaracion::Funcion { cuerpo, .. } => {
                    en_sentencias(cuerpo, campo, &HashMap::new(), &mut tipos)
                }
                _ => {}
            }
        }
        // Sólo interesa un tipo distinto de Entero, y que todas las asignaciones
        // coincidan: con dos candidatos distintos no hay nada seguro que elegir.
        let mut distintos: Vec<T> = Vec::new();
        for t in tipos {
            if t != T::Int && !distintos.contains(&t) {
                distintos.push(t);
            }
        }
        if distintos.len() == 1 {
            distintos.pop()
        } else {
            None
        }
    }

    /// Tipos de los parámetros: el anotado o, si no tiene, el que se deduce del
    /// uso en el cuerpo (`p[i]`, `p[i] = v`, `p.empujar(x)` → Arreglo). Sin esto
    /// `funcion ordenar(mut arr)` sin anotar devolvía un entero que en realidad
    /// era la dirección del arreglo.
    fn tipos_params(
        &self,
        clase: Option<&str>,
        nombre: &str,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> Vec<T> {
        let por_llamada = self
            .evidencia
            .get(&clave_de_evidencia(clase, nombre))
            .or_else(|| self.evidencia.get(nombre))
            .cloned()
            .unwrap_or_default();
        parametros
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // `x: T` (parámetro de tipo sin resolver): la casilla es i64 y el
                // argumento viaja con sus bits tal cual (un texto es un puntero).
                if matches!(p.tipo, Some(Tipo::Parametro(_))) {
                    return T::Int;
                }
                let evidencia = por_llamada.get(i).cloned().flatten();
                let declarado = self.tipo_declarado(&p.tipo);
                // `p: Par` con `Par<A, B>` (plantilla) y la evidencia diciendo
                // `Par_Texto_Decimal`: gana la especialización concreta.
                let declarado = match (&declarado, &evidencia) {
                    (Some(T::Clase(n)), Some(T::Clase(m)))
                        if n != m && self.clases.get(n).map(|c| c.generica).unwrap_or(false) =>
                    {
                        evidencia.clone()
                    }
                    _ => declarado,
                };
                // La evidencia (qué llegó en las llamadas) es más fiable que la
                // forma en que el cuerpo usa el parámetro: `validar_email(email)`
                // se llama con un `Texto` aunque el cuerpo lo indexe.
                declarado
                    .or(evidencia)
                    .or_else(|| inferir_parametro(&p.nombre, cuerpo))
                    .unwrap_or(T::Int)
            })
            .collect()
    }

    /// Tipo de retorno de `obj.metodo()`: directo si el receptor tiene tipo
    /// estático, o unificando las clases que declaran el método (el caso del
    /// despacho dinámico).
    fn retorno_metodo_de(&self, objeto: &Expresion, metodo: &str) -> Option<T> {
        match self.tipo_estatico(objeto) {
            T::Clase(c) => self
                .clases
                .get(&c)
                .and_then(|cl| cl.metodos.iter().find(|(m, _, _)| m == metodo))
                .map(|(_, _, r)| r.clone()),
            _ => {
                let retornos: Vec<T> = self
                    .clases
                    .values()
                    .filter_map(|cl| cl.metodos.iter().find(|(m, _, _)| m == metodo))
                    .map(|(_, _, r)| r.clone())
                    .collect();
                unificar_retorno(&retornos)
            }
        }
    }

    /// Tipo del primer `retornar <expr>` del cuerpo (para funciones sin
    /// anotación). Resuelve también los llamados: `retornar f(x)` usa el tipo de
    /// retorno ya registrado de `f`, y `retornar este.x.m()` unifica las
    /// implementaciones de `m` cuando el receptor no tiene tipo estático.
    fn inferir_retorno(
        &self,
        cuerpo: &[Declaracion],
        params: &[Parametro],
        tipos: &[T],
    ) -> Option<T> {
        fn tipo_valor(
            self_: &Lowering,
            e: &Expresion,
            params: &[Parametro],
            tipos: &[T],
            locales: &HashMap<String, T>,
        ) -> Option<T> {
            Some(match e {
                Expresion::LiteralTexto(_) => T::Texto,
                Expresion::LiteralDecimal(_) => T::Dec,
                Expresion::LiteralExacto(..) => T::Exacto,
                Expresion::Arreglo(_) => T::Arreglo,
                Expresion::Mapa(_) => T::Mapa,
                Expresion::Instanciacion { clase, .. } => T::Clase(clase.clone()),
                // `Algo`/`Ninguno` y `Ok`/`Error` son los tipos del preludio:
                // sin esto una función de la stdlib sin anotación se tomaba como
                // Entero y el `coincidir` de quien la llama fallaba.
                Expresion::Algo(_) | Expresion::Nada | Expresion::Ninguno => {
                    T::Enum("Opcion".to_string())
                }
                Expresion::Ok(_) | Expresion::Error(_) => T::Enum("Resultado".to_string()),
                // `resultado` es la palabra reservada de las postcondiciones,
                // pero también un nombre de variable normal (`variable resultado
                // = []` … `retornar resultado`).
                Expresion::Resultado => locales.get("resultado").cloned().unwrap_or(T::Int),
                // `retornar arr` devuelve el tipo del parámetro, no un entero.
                Expresion::Identificador { nombre, .. } => params
                    .iter()
                    .position(|p| &p.nombre == nombre)
                    .and_then(|i| tipos.get(i).cloned())
                    .or_else(|| locales.get(nombre).cloned())
                    .unwrap_or(T::Int),
                Expresion::LlamadaFuncion { nombre, .. } => self_
                    .firmas
                    .get(nombre)
                    .map(|(_, r)| r.clone())
                    // Los builtins también tienen tipo de retorno conocido.
                    .or_else(|| tipo_builtin(nombre))
                    .unwrap_or(T::Int),
                Expresion::LlamadaMetodo { objeto, metodo, .. } => {
                    self_.retorno_metodo_de(objeto, metodo).unwrap_or(T::Int)
                }
                // Concat/aritmética, literales, campos: lo que sabe `tipo_estatico`.
                _ => self_.tipo_estatico(e),
            })
        }
        fn recoger(
            self_: &Lowering,
            cuerpo: &[Declaracion],
            params: &[Parametro],
            tipos: &[T],
            locales: &HashMap<String, T>,
            out: &mut Vec<T>,
        ) {
            for d in cuerpo {
                match d {
                    Declaracion::Retornar { valor: Some(e) } => {
                        if let Some(t) = tipo_valor(self_, e, params, tipos, locales) {
                            out.push(t);
                        }
                    }
                    Declaracion::Si {
                        bloque_verdadero,
                        bloque_falso,
                        ..
                    } => {
                        recoger(self_, bloque_verdadero, params, tipos, locales, out);
                        if let Some(b) = bloque_falso {
                            recoger(self_, b, params, tipos, locales, out);
                        }
                    }
                    Declaracion::Mientras { bloque, .. }
                    | Declaracion::Repetir { bloque, .. }
                    | Declaracion::Para { bloque, .. } => {
                        recoger(self_, bloque, params, tipos, locales, out)
                    }
                    _ => {}
                }
            }
        }
        // Los locales del cuerpo también cuentan (`retornar moda` con `moda` un
        // arreglo local).
        let mut locales: HashMap<String, T> = HashMap::new();
        self.preescanear_tipos(cuerpo, &mut locales);
        let mut retornos = Vec::new();
        recoger(self, cuerpo, params, tipos, &locales, &mut retornos);
        // `retornar nulo` en una rama no vuelve `void` a la función: el nulo es
        // el 0 del slot y las demás ramas ya tienen su tipo (`193`/`parsear`
        // declaraba `void` y devolvía `i64`).
        let con_valor: Vec<T> = retornos
            .iter()
            .filter(|t| **t != T::Nulo)
            .cloned()
            .collect();
        if !con_valor.is_empty() {
            if let Some(t) = unificar_retorno(&con_valor) {
                return Some(t);
            }
            // Ramas que devuelven cosas distintas pero todas punteros (`Texto`
            // en una, `Arreglo` en otra): el nativo pasa el puntero tal cual y
            // el runtime decide por el tag. Antes ganaba el primer `retornar
            // nulo` y la función quedaba `void` devolviendo `i64` (`193`).
            if con_valor.iter().all(|t| t.llvm().to_llvm_ir() == "ptr") {
                return con_valor.first().cloned();
            }
        }
        // Todas las ramas tienen que caber en el tipo: una que devuelve el
        // arreglo manda sobre otra que devuelve un entero.
        unificar_retorno(&retornos).or_else(|| retornos.first().cloned())
    }

    /// Tipo declarado en el fuente, resolviendo si el nombre es un `tipo`
    /// (enum) y no una clase (el parser no distingue en la anotación).
    fn tipo_declarado(&self, t: &Option<Tipo>) -> Option<T> {
        T::de_tipo(t).map(|t| match t {
            T::Clase(n) if self.enums.contains_key(&n) => T::Enum(n),
            // `variable p: Par` con `Par<A, B>` monomorfizada a
            // `Par_Texto_Entero`: si hay una sola especialización, es esa.
            T::Clase(n) if self.clases.get(&n).map(|c| c.generica).unwrap_or(false) => {
                let prefijo = format!("{n}_");
                let mut candidatas = self.clases.keys().filter(|c| c.starts_with(&prefijo));
                match (candidatas.next(), candidatas.next()) {
                    (Some(una), None) => T::Clase(una.clone()),
                    _ => T::Clase(n),
                }
            }
            otro => otro,
        })
    }

    /// Los programas declaran `variable x = nulo` y después la asignan a una
    /// clase/arreglo/texto. El slot se tipa con el primer tipo concreto que
    /// aparezca en el cuerpo (incluidos bloques anidados).
    /// Tipo de una expresión usando las pistas del pre-escaneo (las variables
    /// de la función todavía no están en `self.vars` cuando se escanea).
    fn tipo_con_pistas(&self, e: &Expresion, pistas: &HashMap<String, T>) -> T {
        match e {
            Expresion::Identificador { nombre, .. } => pistas
                .get(nombre)
                .cloned()
                .unwrap_or_else(|| self.tipo_estatico(e)),
            Expresion::Grupo(g) => self.tipo_con_pistas(g, pistas),
            Expresion::Unaria { expr, .. } => self.tipo_con_pistas(expr, pistas),
            // `r = r * base` con `base: Decimal`: `r` es Decimal aunque arranque
            // en `variable r = 1` (la VM ensancha la casilla).
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                use crate::ast::Operador;
                let a = self.tipo_con_pistas(izquierda, pistas);
                let b = self.tipo_con_pistas(derecha, pistas);
                if a == T::Dec || b == T::Dec {
                    T::Dec
                } else if matches!(operador, Operador::Suma) && (a == T::Texto || b == T::Texto) {
                    T::Texto
                } else {
                    self.tipo_estatico(e)
                }
            }
            other => self.tipo_estatico(other),
        }
    }

    /// Primer tipo concreto que se le asigna a un global desde dentro de una
    /// función o método (`171`: `variable g = nulo` … `g = nuevo C()`).
    fn tipo_asignado_en_funciones(&self, decls: &[Declaracion], nombre: &str) -> Option<T> {
        fn en_cuerpo(self_: &Lowering, cuerpo: &[Declaracion], nombre: &str, out: &mut Option<T>) {
            for d in cuerpo {
                if out.is_some() {
                    return;
                }
                match d {
                    Declaracion::Asignacion {
                        nombre: n, valor, ..
                    } if n == nombre => {
                        let t = self_.tipo_estatico(valor);
                        if t != T::Nulo && t != T::Int {
                            *out = Some(t);
                        }
                    }
                    Declaracion::Si {
                        bloque_verdadero,
                        bloque_falso,
                        ..
                    } => {
                        en_cuerpo(self_, bloque_verdadero, nombre, out);
                        if let Some(b) = bloque_falso {
                            en_cuerpo(self_, b, nombre, out);
                        }
                    }
                    Declaracion::Mientras { bloque, .. }
                    | Declaracion::Repetir { bloque, .. }
                    | Declaracion::Para { bloque, .. } => en_cuerpo(self_, bloque, nombre, out),
                    _ => {}
                }
            }
        }
        let mut out = None;
        for d in decls {
            match d {
                Declaracion::Funcion { cuerpo, .. } => en_cuerpo(self, cuerpo, nombre, &mut out),
                Declaracion::Clase { metodos, .. }
                | Declaracion::Implementacion { metodos, .. } => {
                    for m in metodos {
                        en_cuerpo(self, &m.cuerpo, nombre, &mut out);
                    }
                }
                _ => {}
            }
            if out.is_some() {
                break;
            }
        }
        out
    }

    fn preescanear_tipos(&self, cuerpo: &[Declaracion], out: &mut HashMap<String, T>) {
        fn anotar(out: &mut HashMap<String, T>, nombre: &str, t: T) {
            if t != T::Nulo && t != T::Int {
                out.entry(nombre.to_string()).or_insert(t);
            }
        }
        for d in cuerpo {
            match d {
                Declaracion::Variable {
                    nombre,
                    valor,
                    tipo,
                    ..
                } => {
                    if let Some(t) = self.tipo_declarado(tipo) {
                        anotar(out, nombre, t);
                    } else if let Some(v) = valor {
                        let t = self.tipo_con_pistas(v, out);
                        anotar(out, nombre, t);
                    }
                }
                Declaracion::Asignacion { nombre, valor, .. } => {
                    let t = self.tipo_con_pistas(valor, out);
                    anotar(out, nombre, t);
                }
                Declaracion::Si {
                    bloque_verdadero,
                    bloque_falso,
                    ..
                } => {
                    self.preescanear_tipos(bloque_verdadero, out);
                    if let Some(b) = bloque_falso {
                        self.preescanear_tipos(b, out);
                    }
                }
                Declaracion::Mientras { bloque, .. }
                | Declaracion::Repetir { bloque, .. }
                | Declaracion::Para { bloque, .. } => self.preescanear_tipos(bloque, out),
                _ => {}
            }
        }
    }

    fn tipo_estatico(&self, e: &Expresion) -> T {
        match e {
            Expresion::LiteralTexto(_) => T::Texto,
            Expresion::LiteralDecimal(_) => T::Dec,
            Expresion::LiteralBooleano(_) => T::Bool,
            Expresion::LiteralNulo => T::Nulo,
            Expresion::LiteralNumero(_) => T::Int,
            Expresion::Arreglo(_) => T::Arreglo,
            Expresion::Mapa(_) => T::Mapa,
            // `resultado` como variable normal (fuera de una postcondición).
            Expresion::Resultado => self
                .vars
                .get("resultado")
                .map(|v| v.t.clone())
                .unwrap_or(T::Int),
            // `partes[0]` de un arreglo de textos es Texto (puntero), no Entero.
            Expresion::Index { objeto, .. } if self.arreglo_de_texto(objeto) => T::Texto,
            Expresion::Index { objeto, .. } if self.tipo_estatico(objeto) == T::Texto => T::Texto,
            Expresion::LiteralExacto(..) => T::Exacto,
            Expresion::CanalNuevo(_) => T::Canal,
            Expresion::Closure { .. } => T::Closure,
            Expresion::LlamadaFuncion { nombre, .. } if self.variantes.contains_key(nombre) => {
                T::Enum(self.variantes[nombre].0.clone())
            }
            Expresion::Grupo(g) => self.tipo_estatico(g),
            // El tipo del campo: `este.texto` es Texto si así se declaró/infirió.
            Expresion::AccesoMiembro { objeto, miembro } => match self.tipo_estatico(objeto) {
                T::Clase(c) => self
                    .clases
                    .get(&c)
                    .and_then(|cl| cl.campos.iter().find(|(n, _)| n == miembro))
                    .map(|(_, t)| t.clone())
                    .unwrap_or(T::Int),
                _ => T::Int,
            },
            // `-x` conserva el tipo del operando (p. ej. un Exacto negativo).
            Expresion::Unaria { expr, .. } => self.tipo_estatico(expr),
            Expresion::Instanciacion { clase, .. } => T::Clase(clase.clone()),
            // `obj.metodo()`: la firma registrada del método (o la unificación de
            // las implementaciones si el receptor no tiene tipo estático).
            Expresion::LlamadaMetodo { objeto, metodo, .. } => {
                self.retorno_metodo_de(objeto, metodo).unwrap_or(T::Int)
            }
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => match operador {
                Operador::Suma
                    if self.tipo_estatico(izquierda) == T::Texto
                        || self.tipo_estatico(derecha) == T::Texto =>
                {
                    T::Texto
                }
                Operador::IgualIgual
                | Operador::Diferente
                | Operador::Mayor
                | Operador::Menor
                | Operador::MayorIgual
                | Operador::MenorIgual
                | Operador::Y
                | Operador::O => T::Bool,
                _ => {
                    let a = self.tipo_estatico(izquierda);
                    let b = self.tipo_estatico(derecha);
                    if a == T::Exacto || b == T::Exacto {
                        T::Exacto
                    } else if a == T::Dec || b == T::Dec {
                        T::Dec
                    } else if a == T::Texto || b == T::Texto {
                        T::Texto
                    } else {
                        T::Int
                    }
                }
            },
            Expresion::Identificador { nombre, .. } => self
                .vars
                .get(nombre)
                .map(|v| v.t.clone())
                .or_else(|| self.tipos_top.get(nombre).cloned())
                .or_else(|| {
                    // variante sin payload (`Rojo`): el tipo es el del enum
                    self.variantes
                        .get(nombre)
                        .map(|(en, _, _)| T::Enum(en.clone()))
                })
                .unwrap_or(T::Int),
            // `absoluto(exacto)` devuelve Exacto (el builtin), aunque la
            // stdlib declare `absoluto` sin tipos.
            Expresion::LlamadaFuncion {
                nombre, argumentos, ..
            } if nombre == "absoluto"
                && argumentos
                    .first()
                    .map(|a| self.tipo_estatico(a) == T::Exacto)
                    .unwrap_or(false) =>
            {
                T::Exacto
            }
            Expresion::LlamadaFuncion { nombre, .. } => self
                .firmas
                .get(nombre)
                .map(|(_, r)| r.clone())
                .or_else(|| self.externas.get(nombre).map(|(_, r)| r.clone()))
                .unwrap_or_else(|| tipo_builtin(nombre).unwrap_or(T::Int)),
            _ => T::Int,
        }
    }
}

/// `V` a partir de un registro de IR y su tipo (el inverso de `valor_param`).
fn valor_de(t: &T, r: String) -> V {
    match t {
        T::Bool => V::Bool(r),
        T::Dec => V::Dec(r),
        T::Texto => V::Texto(r),
        T::Arreglo => V::Arreglo(r),
        T::Mapa => V::Mapa(r),
        T::Exacto => V::Exacto(r),
        T::Enum(c) => V::Enum(r, c.clone()),
        T::Canal => V::Canal(r),
        T::Closure => V::Closure(r),
        T::Clase(c) => V::Clase(r, c.clone()),
        T::Funcion => V::Funcion(r),
        T::Nulo => V::Nulo,
        T::Int => V::Int(r),
    }
}

fn cero(t: &T) -> &'static str {
    match t {
        T::Dec => "0.000000e+00",
        T::Texto
        | T::Arreglo
        | T::Mapa
        | T::Exacto
        | T::Enum(_)
        | T::Canal
        | T::Closure
        | T::Clase(_)
        | T::Funcion => "null",
        T::Bool | T::Int | T::Nulo => "0",
    }
}

/// Nombres de variables que se usan dentro de cuerpos de funciones, métodos,
/// closures o hilos. Las de nivel superior que no aparezcan acá pueden vivir en
/// un `alloca` de `@forja_init` en vez de un global (mucho más rápido: LLVM las
/// promueve a registros).
fn nombres_usados_en_funciones(decls: &[Declaracion]) -> std::collections::HashSet<String> {
    fn expr(e: &Expresion, out: &mut std::collections::HashSet<String>) {
        match e {
            Expresion::Identificador { nombre, .. } => {
                out.insert(nombre.clone());
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                expr(izquierda, out);
                expr(derecha, out);
            }
            Expresion::Unaria { expr: e, .. } | Expresion::Grupo(e) => expr(e, out),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                expr(condicion, out);
                expr(si_verdadero, out);
                expr(si_falso, out);
            }
            Expresion::LlamadaFuncion { argumentos, .. } => {
                for a in argumentos {
                    expr(a, out);
                }
            }
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                expr(objeto, out);
                for a in argumentos {
                    expr(a, out);
                }
            }
            Expresion::AccesoMiembro { objeto, .. } => expr(objeto, out),
            Expresion::Instanciacion { argumentos, .. } => {
                for a in argumentos {
                    expr(a, out);
                }
            }
            Expresion::Arreglo(items) => {
                for i in items {
                    expr(i, out);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares {
                    expr(k, out);
                    expr(v, out);
                }
            }
            Expresion::Index { objeto, indice } => {
                expr(objeto, out);
                expr(indice, out);
            }
            Expresion::Asignacion { variable, valor } => {
                out.insert(variable.clone());
                expr(valor, out);
            }
            Expresion::AsignacionCampo { objeto, valor, .. } => {
                expr(objeto, out);
                expr(valor, out);
            }
            Expresion::AsignacionCompuesta {
                objetivo, valor, ..
            } => {
                expr(objetivo, out);
                expr(valor, out);
            }
            Expresion::Closure { cuerpo, .. } | Expresion::Hilo { cuerpo } => {
                sentencias(cuerpo, out);
            }
            _ => {}
        }
    }
    fn sentencias(cuerpo: &[Declaracion], out: &mut std::collections::HashSet<String>) {
        for d in cuerpo {
            match d {
                Declaracion::Variable { nombre, valor, .. } => {
                    out.insert(nombre.clone());
                    if let Some(v) = valor {
                        expr(v, out);
                    }
                }
                Declaracion::Asignacion { nombre, valor, .. } => {
                    out.insert(nombre.clone());
                    expr(valor, out);
                }
                Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                    expr(objeto, out);
                    expr(valor, out);
                }
                Declaracion::AsignacionIndex {
                    nombre,
                    indice,
                    valor,
                    ..
                } => {
                    out.insert(nombre.clone());
                    expr(indice, out);
                    expr(valor, out);
                }
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
                    expr(condicion, out);
                    sentencias(bloque_verdadero, out);
                    if let Some(b) = bloque_falso {
                        sentencias(b, out);
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    expr(condicion, out);
                    sentencias(bloque, out);
                }
                Declaracion::Para {
                    inicializacion,
                    condicion,
                    incremento,
                    bloque,
                } => {
                    if let Some(i) = inicializacion {
                        sentencias(std::slice::from_ref(i.as_ref()), out);
                    }
                    if let Some(c) = condicion {
                        expr(c, out);
                    }
                    if let Some(i) = incremento {
                        sentencias(std::slice::from_ref(i.as_ref()), out);
                    }
                    sentencias(bloque, out);
                }
                Declaracion::Repetir { cantidad, bloque } => {
                    expr(cantidad, out);
                    sentencias(bloque, out);
                }
                Declaracion::Retornar { valor: Some(v) } => expr(v, out),
                Declaracion::Retornar { valor: None } => {}
                Declaracion::LlamadaFuncion { argumentos, .. } => {
                    for a in argumentos {
                        expr(a, out);
                    }
                }
                Declaracion::Expresion(e) => expr(e, out),
                Declaracion::AccesoMiembro { objeto, .. } => expr(objeto, out),
                Declaracion::Cuando {
                    condicion, cuerpo, ..
                } => {
                    expr(condicion, out);
                    sentencias(cuerpo, out);
                }
                Declaracion::AsignacionMultiple { valor, .. } => expr(valor, out),
                _ => {}
            }
        }
    }
    let mut out = std::collections::HashSet::new();
    for d in decls {
        match d {
            Declaracion::Funcion {
                cuerpo,
                externa: false,
                ..
            } => sentencias(cuerpo, &mut out),
            Declaracion::Clase { metodos, .. } => {
                for m in metodos {
                    sentencias(&m.cuerpo, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

fn tipo_builtin(nombre: &str) -> Option<T> {
    Some(match nombre {
        "longitud" => T::Int,
        "a_texto" => T::Texto,
        "a_entero" => T::Int,
        "a_decimal" | "a_flotante" => T::Dec,
        "indice_de" => T::Int,
        "subcadena" => T::Texto,
        "a_mayusculas" | "a_minusculas" => T::Texto,
        "tiempo_ms" => T::Int,
        "repetir_arreglo" | "arreglo_relleno" => T::Arreglo,
        // `pares(m)`/`claves(m)`/`valores(m)` arman un arreglo: sin esto el
        // retorno se infería Entero y la casilla se leía como `%ForjaArreglo`
        // por `inttoptr` (segfault en `fusionar`, ejemplo `732`).
        "_mapa_pares" | "_mapa_claves" | "_mapa_valores" => T::Arreglo,
        "dividir" => T::Arreglo,
        "recortar" => T::Texto,
        // `unir_elementos(arr, sep)` arma un Texto: sin esto el retorno de
        // `retornar unir_elementos(...)` se infería Entero y se imprimía `0`
        // (`a_titulo` de std/texto, ejemplo `709`).
        "unir_elementos" | "reemplazar" | "invertir" => T::Texto,
        "contiene" | "empieza_con" | "termina_con" => T::Int,
        // `leer()` devuelve Texto (o el nulo en el slot de texto al final de la
        // entrada); si el slot fuera `Entero` el nulo se perdía como 0.
        "leer" => T::Texto,
        _ => return None,
    })
}

mod expresiones;
mod sentencias;

/// Tipo de una expresión por su forma (sin tabla de variables): literales,
/// arreglos y objetos. `None` cuando no hay nada seguro que decir.
fn tipo_por_forma(e: &Expresion) -> Option<T> {
    Some(match e {
        Expresion::LiteralTexto(_) => T::Texto,
        Expresion::LiteralDecimal(_) => T::Dec,
        Expresion::LiteralExacto(..) => T::Exacto,
        Expresion::Arreglo(_) => T::Arreglo,
        Expresion::Mapa(_) => T::Mapa,
        Expresion::Instanciacion { clase, .. } => T::Clase(clase.clone()),
        Expresion::LiteralBooleano(_) => T::Bool,
        // `variable f = func(x) { … }`: una closure es un valor con su propio
        // tipo. Sin esto, pasarla a un parámetro sin anotar (`mapa(arr, f)`)
        // dejaba el parámetro como `Entero` y la llamada saltaba al objeto de la
        // closure como si fuera código (segfault).
        Expresion::Closure { .. } => T::Closure,
        _ => return None,
    })
}

/// ¿La expresión es un arreglo (literal) cuyos elementos son textos?
fn es_arreglo_de_textos(e: &Expresion, vars_texto: &std::collections::HashSet<String>) -> bool {
    match e {
        Expresion::Arreglo(items) => items
            .first()
            .map(|i| matches!(i, Expresion::LiteralTexto(_)))
            .unwrap_or(false),
        Expresion::Identificador { nombre, .. } => vars_texto.contains(nombre),
        _ => false,
    }
}

/// Tipos **no-Entero** que llegan a cada parámetro en las llamadas del programa,
/// por nombre de función/método. Sólo cuenta lo que se ve por la forma del
/// argumento (o por la variable de la que sale) y exige que todas las llamadas
/// coincidan.
///
/// Las llamadas a métodos se anotan dos veces: por el nombre pelado (`insertar`)
/// y, si el receptor tiene tipo estático, por `Clase::metodo` (`Cola::insertar`).
/// El segundo es el que manda: dos clases con el mismo nombre de método no se
/// contaminan entre sí (antes sí, y el método quedaba sin evidencia).
///
/// shortcut: sólo se mira la forma del argumento, no el tipo del receptor que
/// llega a un parámetro libre. Upgrade: junto con los valores con tag.
#[allow(clippy::type_complexity)]
fn evidencia_de_llamadas(
    decls: &[Declaracion],
) -> (
    HashMap<String, Vec<Option<T>>>,
    std::collections::HashSet<(String, usize)>,
) {
    /// Tipo de un argumento: por su forma o por la variable de la que sale
    /// (`variable horas = 2.5` … `predecir(horas)`).
    fn tipo_arg(
        a: &Expresion,
        vars: &HashMap<String, T>,
        retornos: &HashMap<String, T>,
    ) -> Option<T> {
        tipo_por_forma(a).or_else(|| match a {
            Expresion::Identificador { nombre, .. } => vars.get(nombre).cloned(),
            Expresion::Grupo(inner) => tipo_arg(inner, vars, retornos),
            Expresion::Unaria { expr, .. } => tipo_arg(expr, vars, retornos),
            // `variable x = f()` con `f` de retorno anotado: el tipo de `x` es
            // el de `f` (`aprox = aproximar_e(n)` es `Decimal`).
            Expresion::LlamadaFuncion { nombre, .. } => retornos.get(nombre).cloned(),
            // Una cuenta también es evidencia: `potencia((x - 1) / (x + 1), …)`
            // es `Decimal` porque `x` lo es. Sin esto el parámetro sin anotar
            // quedaba `Entero` y truncaba a 0.
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                use crate::ast::Operador;
                let ta = tipo_arg(izquierda, vars, retornos);
                let tb = tipo_arg(derecha, vars, retornos);
                if ta == Some(T::Dec) || tb == Some(T::Dec) {
                    Some(T::Dec)
                } else if matches!(operador, Operador::Suma)
                    && (ta == Some(T::Texto) || tb == Some(T::Texto))
                {
                    Some(T::Texto)
                } else if ta == Some(T::Int) && tb == Some(T::Int) {
                    Some(T::Int)
                } else {
                    None
                }
            }
            // `si cond { a } sino { b }`: vale el tipo si ambos lados coinciden.
            Expresion::Ternario {
                si_verdadero,
                si_falso,
                ..
            } => {
                let a = tipo_arg(si_verdadero, vars, retornos);
                let b = tipo_arg(si_falso, vars, retornos);
                if a == b {
                    a
                } else {
                    None
                }
            }
            _ => None,
        })
    }
    #[allow(clippy::too_many_arguments)]
    fn anotar(
        mapa: &mut HashMap<String, Vec<Option<T>>>,
        en_conflicto: &mut std::collections::HashSet<(String, usize)>,
        vars: &HashMap<String, T>,
        args_por_llamada: &mut HashMap<String, Vec<Expresion>>,
        vars_texto: &std::collections::HashSet<String>,
        nombre: &str,
        args: &[Expresion],
        retornos: &HashMap<String, T>,
    ) {
        // Las llamadas punteadas (`x.metodo`) se registran sin el receptor.
        anotar_clave(
            mapa,
            en_conflicto,
            vars,
            args_por_llamada,
            vars_texto,
            nombre.rsplit('.').next().unwrap_or(nombre),
            args,
            retornos,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn anotar_clave(
        mapa: &mut HashMap<String, Vec<Option<T>>>,
        en_conflicto: &mut std::collections::HashSet<(String, usize)>,
        vars: &HashMap<String, T>,
        args_por_llamada: &mut HashMap<String, Vec<Expresion>>,
        vars_texto: &std::collections::HashSet<String>,
        clave: &str,
        args: &[Expresion],
        retornos: &HashMap<String, T>,
    ) {
        let clave = clave.to_string();
        args_por_llamada.insert(clave.clone(), args.to_vec());
        let _ = &vars_texto;
        let entrada = mapa.entry(clave.clone()).or_default();
        if entrada.len() < args.len() {
            entrada.resize(args.len(), None);
        }
        for (i, a) in args.iter().enumerate() {
            let Some(t) = tipo_arg(a, vars, retornos) else {
                continue;
            };
            if t == T::Int {
                continue;
            }
            // Dos tipos distintos en el mismo argumento: no hay nada seguro que
            // concluir (y no se puede volver a llenar con un tercero).
            if en_conflicto.contains(&(clave.clone(), i)) {
                continue;
            }
            match &entrada[i] {
                None => entrada[i] = Some(t),
                Some(previo) if *previo == t => {}
                Some(_) => {
                    entrada[i] = None;
                    en_conflicto.insert((clave.clone(), i));
                }
            }
        }
    }
    fn en_expr(
        e: &Expresion,
        mapa: &mut HashMap<String, Vec<Option<T>>>,
        en_conflicto: &mut std::collections::HashSet<(String, usize)>,
        args_por_llamada: &mut HashMap<String, Vec<Expresion>>,
        cadenas_texto: &mut std::collections::HashSet<String>,
        vars: &mut HashMap<String, T>,
        retornos: &HashMap<String, T>,
    ) {
        match e {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                anotar(
                    mapa,
                    en_conflicto,
                    vars,
                    args_por_llamada,
                    cadenas_texto,
                    nombre,
                    argumentos,
                    retornos,
                );
                for a in argumentos {
                    en_expr(
                        a,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
            }
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                anotar(
                    mapa,
                    en_conflicto,
                    vars,
                    args_por_llamada,
                    cadenas_texto,
                    metodo,
                    argumentos,
                    retornos,
                );
                // Y con la clase del receptor, que es la clave que se consulta
                // al compilar el método (`Cola::insertar`). Sin tipo estático
                // del receptor queda sólo la clave pelada.
                if let Some(T::Clase(c)) = tipo_arg(objeto, vars, retornos) {
                    anotar_clave(
                        mapa,
                        en_conflicto,
                        vars,
                        args_por_llamada,
                        cadenas_texto,
                        &format!("{c}::{metodo}"),
                        argumentos,
                        retornos,
                    );
                }
                en_expr(
                    objeto,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
                for a in argumentos {
                    en_expr(
                        a,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
            }
            // `nuevo Clase(args)`: la evidencia de un constructor es por clase
            // (todos los constructores se llaman "constructor").
            Expresion::Instanciacion { clase, argumentos } => {
                anotar(
                    mapa,
                    en_conflicto,
                    vars,
                    args_por_llamada,
                    cadenas_texto,
                    &format!("constructor@{clase}"),
                    argumentos,
                    retornos,
                );
                for a in argumentos {
                    en_expr(
                        a,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
            }
            Expresion::Closure { parametros, cuerpo } => {
                let mut propias = vars.clone();
                for p in parametros {
                    if let Some(t) = T::de_tipo(&p.tipo) {
                        propias.insert(p.nombre.clone(), t);
                    }
                }
                en_sentencias(
                    cuerpo,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    &mut propias,
                    retornos,
                );
            }
            Expresion::Hilo { cuerpo } => en_sentencias(
                cuerpo,
                mapa,
                en_conflicto,
                args_por_llamada,
                cadenas_texto,
                vars,
                retornos,
            ),
            Expresion::Coincidir { brazos, expr: o } => {
                en_expr(
                    o,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
                for b in brazos {
                    en_sentencias(
                        &b.cuerpo,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                en_expr(
                    izquierda,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
                en_expr(
                    derecha,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
            }
            Expresion::Unaria { expr, .. } | Expresion::Grupo(expr) => en_expr(
                expr,
                mapa,
                en_conflicto,
                args_por_llamada,
                cadenas_texto,
                vars,
                retornos,
            ),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                en_expr(
                    condicion,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
                en_expr(
                    si_verdadero,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
                en_expr(
                    si_falso,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
            }
            Expresion::Index { objeto, indice } => {
                en_expr(
                    objeto,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
                en_expr(
                    indice,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                );
            }
            Expresion::Arreglo(items) => {
                for i in items {
                    en_expr(
                        i,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares {
                    en_expr(
                        k,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                    en_expr(
                        v,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
            }
            Expresion::Asignacion { valor, .. }
            | Expresion::AsignacionCampo { valor, .. }
            | Expresion::AsignacionCompuesta { valor, .. }
            | Expresion::ArregloFijar { valor, .. } => en_expr(
                valor,
                mapa,
                en_conflicto,
                args_por_llamada,
                cadenas_texto,
                vars,
                retornos,
            ),
            Expresion::AccesoMiembro { objeto, .. } => en_expr(
                objeto,
                mapa,
                en_conflicto,
                args_por_llamada,
                cadenas_texto,
                vars,
                retornos,
            ),
            Expresion::Try(inner)
            | Expresion::Ok(inner)
            | Expresion::Error(inner)
            | Expresion::Algo(inner) => en_expr(
                inner,
                mapa,
                en_conflicto,
                args_por_llamada,
                cadenas_texto,
                vars,
                retornos,
            ),
            _ => {}
        }
    }
    fn en_sentencias(
        cuerpo: &[Declaracion],
        mapa: &mut HashMap<String, Vec<Option<T>>>,
        en_conflicto: &mut std::collections::HashSet<(String, usize)>,
        args_por_llamada: &mut HashMap<String, Vec<Expresion>>,
        cadenas_texto: &mut std::collections::HashSet<String>,
        vars: &mut HashMap<String, T>,
        retornos: &HashMap<String, T>,
    ) {
        for d in cuerpo {
            match d {
                Declaracion::Variable {
                    nombre,
                    valor: Some(v),
                    ..
                } => {
                    if es_arreglo_de_textos(v, cadenas_texto) {
                        cadenas_texto.insert(nombre.clone());
                    }
                    if let Some(t) = tipo_arg(v, vars, retornos) {
                        vars.insert(nombre.clone(), t);
                    }
                    en_expr(
                        v,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
                Declaracion::Variable { valor: None, .. } => {}
                Declaracion::Asignacion { nombre, valor, .. } => {
                    if let Some(t) = tipo_arg(valor, vars, retornos) {
                        vars.insert(nombre.clone(), t);
                    }
                    en_expr(
                        valor,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
                Declaracion::AsignacionMiembro { valor, .. }
                | Declaracion::AsignacionIndex { valor, .. } => en_expr(
                    valor,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                ),
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
                    en_expr(
                        condicion,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                    en_sentencias(
                        bloque_verdadero,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                    if let Some(b) = bloque_falso {
                        en_sentencias(
                            b,
                            mapa,
                            en_conflicto,
                            args_por_llamada,
                            cadenas_texto,
                            vars,
                            retornos,
                        );
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    en_expr(
                        condicion,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                    en_sentencias(
                        bloque,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    );
                }
                Declaracion::Para { bloque, .. } | Declaracion::Repetir { bloque, .. } => {
                    en_sentencias(
                        bloque,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                        retornos,
                    )
                }
                Declaracion::Retornar { valor: Some(v) } => en_expr(
                    v,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                ),
                Declaracion::Expresion(e) => en_expr(
                    e,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                    retornos,
                ),
                Declaracion::LlamadaFuncion { nombre, argumentos } => {
                    anotar(
                        mapa,
                        en_conflicto,
                        vars,
                        args_por_llamada,
                        cadenas_texto,
                        nombre,
                        argumentos,
                        retornos,
                    );
                    for a in argumentos {
                        en_expr(
                            a,
                            mapa,
                            en_conflicto,
                            args_por_llamada,
                            cadenas_texto,
                            vars,
                            retornos,
                        );
                    }
                }
                Declaracion::Funcion {
                    nombre,
                    parametros,
                    cuerpo,
                    ..
                } => {
                    // Los parámetros anotados del propio cuerpo también son pistas
                    // (y, en la segunda vuelta, lo que ya se sabe por las llamadas).
                    let mut propias = vars.clone();
                    let conocidos = mapa.get(nombre).cloned().unwrap_or_default();
                    for (i, p) in parametros.iter().enumerate() {
                        let t = T::de_tipo(&p.tipo).or_else(|| conocidos.get(i).cloned().flatten());
                        if let Some(t) = t {
                            propias.insert(p.nombre.clone(), t);
                        }
                    }
                    en_sentencias(
                        cuerpo,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        &mut propias,
                        retornos,
                    );
                }
                Declaracion::Clase { metodos, .. }
                | Declaracion::Implementacion { metodos, .. } => {
                    for m in metodos {
                        // Igual que en `Funcion`: en la segunda vuelta el tipo del
                        // parámetro también puede venir de las llamadas
                        // (`t.insertar("x")` → `insertar(clave)` es Texto).
                        let mut propias = vars.clone();
                        let conocidos = mapa.get(&m.nombre).cloned().unwrap_or_default();
                        for (i, p) in m.parametros.iter().enumerate() {
                            let t =
                                T::de_tipo(&p.tipo).or_else(|| conocidos.get(i).cloned().flatten());
                            if let Some(t) = t {
                                propias.insert(p.nombre.clone(), t);
                            }
                        }
                        en_sentencias(
                            &m.cuerpo,
                            mapa,
                            en_conflicto,
                            args_por_llamada,
                            cadenas_texto,
                            &mut propias,
                            retornos,
                        );
                    }
                }
                _ => {}
            }
        }
    }
    let mut mapa: HashMap<String, Vec<Option<T>>> = HashMap::new();
    let mut en_conflicto: std::collections::HashSet<(String, usize)> =
        std::collections::HashSet::new();
    // Además de los tipos, se guardan los argumentos de cada llamada: sirven
    // para saber qué parámetros reciben un arreglo de textos.
    // Tipos de retorno anotados: permiten inferir `variable x = f()`.
    let mut retornos: HashMap<String, T> = HashMap::new();
    fn recoger_retornos(decls: &[Declaracion], retornos: &mut HashMap<String, T>) {
        for d in decls {
            match d {
                Declaracion::Funcion {
                    nombre,
                    tipo_retorno,
                    ..
                } => {
                    if let Some(t) = T::de_tipo(tipo_retorno) {
                        retornos.insert(nombre.clone(), t);
                    }
                }
                Declaracion::Clase { metodos, .. }
                | Declaracion::Implementacion { metodos, .. } => {
                    for m in metodos {
                        if let Some(t) = T::de_tipo(&m.tipo_retorno) {
                            retornos.insert(m.nombre.clone(), t);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    recoger_retornos(decls, &mut retornos);

    let mut por_llamada_args: HashMap<String, Vec<Expresion>> = HashMap::new();
    let mut cadenas_texto: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Tres vueltas: cada una conoce los tipos de parámetros que la anterior
    // dedujo de las llamadas (`validar_usuario(email)` → `validar_email`). La
    // última alcanza las cadenas largas: `ht.insertar("x")` → `insertar(clave)`
    // → `este.hash(clave)`.
    for _ in 0..3 {
        en_sentencias(
            decls,
            &mut mapa,
            &mut en_conflicto,
            &mut por_llamada_args,
            &mut cadenas_texto,
            &mut HashMap::new(),
            &retornos,
        );
    }
    // Argumentos que son un arreglo de textos: en el callee, las casillas de ese
    // parámetro se leen como punteros (`campos[i]` de un `["nombre", …]`).
    let mut texto = std::collections::HashSet::new();
    for (clave, args) in &por_llamada_args {
        for (i, a) in args.iter().enumerate() {
            if es_arreglo_de_textos(a, &cadenas_texto) {
                texto.insert((clave.clone(), i));
            }
        }
    }
    (mapa, texto)
}

/// Clave con la que se busca la evidencia de un parámetro: `Clase::metodo`
/// cuando se conoce la clase (`tipos_params` prueba primero ésa y después la
/// pelada), o el nombre tal cual si no.
fn clave_de_evidencia(clase: Option<&str>, nombre: &str) -> String {
    match clase {
        // Los constructores ya llevan la clase (`constructor@Clase`).
        Some(c) if !nombre.contains('@') => format!("{c}::{nombre}"),
        _ => nombre.to_string(),
    }
}

/// Unifica los tipos de retorno de varias implementaciones de un mismo método.
///
/// shortcut: `Entero` y los tipos que se representan como puntero comparten
/// representación (`i64`), así que se unifican al tipo puntero si todas las
/// implementaciones no-`Entero` coinciden. Upgrade: firmas de rasgo con tipos.
fn unificar_retorno(tipos: &[T]) -> Option<T> {
    let mut puntero: Option<T> = None;
    let mut hay_bool = false;
    for t in tipos {
        match t {
            T::Int => {}
            // `Booleano` gana a `Entero`: si alguna implementación devuelve un
            // booleano, el resultado se imprime como tal (`110`).
            T::Bool => hay_bool = true,
            // Necesitan anotación: `double`, `i128` y "sin valor" no son un slot.
            T::Dec | T::Exacto | T::Nulo => return None,
            otro => match &puntero {
                None => puntero = Some(otro.clone()),
                Some(p) if p == otro => {}
                Some(_) => return None,
            },
        }
    }
    Some(puntero.unwrap_or(if hay_bool { T::Bool } else { T::Int }))
}

/// Tipo de un campo sin anotar, deducido de lo que se le asigna dentro de la
/// clase (`este.campo = "texto"` → Texto). Sin esto los campos de texto se
/// guardaban como enteros y `"" + este.nombre` imprimía `0`.
///
/// shortcut: mira todas las asignaciones al campo en el programa (dentro y
/// fuera de la clase); si dos clases homónimas lo llenan con tipos distintos,
/// queda como Entero. Upgrade: inferencia por receptor.
/// Tipo de un parámetro sin anotar, deducido de cómo lo usa el cuerpo: si lo
/// indexa (`p[i]`, `p[i] = v`) o le empuja elementos, es un arreglo., deducido de cómo lo usa el cuerpo: si lo
/// indexa (`p[i]`, `p[i] = v`) o le empuja elementos, es un arreglo.
fn inferir_parametro(nombre: &str, cuerpo: &[Declaracion]) -> Option<T> {
    #[derive(Default)]
    struct Uso {
        arreglo: bool,
        /// `p["clave"]`: indexar con un texto es un mapa (los arreglos se
        /// indexan con enteros). Sin esto `p` quedaba como arreglo y la casilla
        /// del mapa se leía como `%ForjaArreglo` (segfault).
        mapa: bool,
        texto: bool,
        /// El parámetro se usa en aritmética: no puede ser `Texto` aunque además
        /// se concatene (`escribir("x" + n)` y `n * 2` en la misma función).
        numerico: bool,
    }
    fn es(e: &Expresion, nombre: &str) -> bool {
        matches!(e, Expresion::Identificador { nombre: n, .. } if n == nombre)
    }
    fn escapa(cuerpo: &[Declaracion], nombre: &str) -> bool {
        cuerpo.iter().any(|d| match d {
            Declaracion::Retornar { valor: Some(v) } => es(v, nombre),
            Declaracion::Variable { valor: Some(v), .. } => es(v, nombre),
            Declaracion::Asignacion { valor: v, .. }
            | Declaracion::AsignacionMiembro { valor: v, .. }
            | Declaracion::AsignacionIndex { valor: v, .. } => es(v, nombre),
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
                escapa(bloque_verdadero, nombre)
                    || bloque_falso
                        .as_ref()
                        .map(|b| escapa(b, nombre))
                        .unwrap_or(false)
            }
            Declaracion::Mientras { bloque, .. }
            | Declaracion::Para { bloque, .. }
            | Declaracion::Repetir { bloque, .. } => escapa(bloque, nombre),
            _ => false,
        })
    }
    fn en_expr(e: &Expresion, nombre: &str, uso: &mut Uso) {
        match e {
            Expresion::Index { objeto, indice } => {
                if es(objeto, nombre) {
                    if matches!(indice.as_ref(), Expresion::LiteralTexto(_)) {
                        uso.mapa = true;
                    } else {
                        uso.arreglo = true;
                    }
                }
                en_expr(objeto, nombre, uso);
                en_expr(indice, nombre, uso);
            }
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                if (metodo == "empujar" || metodo == "reservar") && es(objeto, nombre) {
                    uso.arreglo = true;
                }
                en_expr(objeto, nombre, uso);
                for a in argumentos {
                    en_expr(a, nombre, uso);
                }
            }
            Expresion::LlamadaFuncion {
                nombre: f,
                argumentos,
            } => {
                if (f == "empujar" || f == "reservar")
                    && argumentos.first().map(|a| es(a, nombre)).unwrap_or(false)
                {
                    uso.arreglo = true;
                }
                // `contiene("0123456789", ch)`: la aguja es Texto. Sin esto un
                // parámetro que sólo se usa así quedaba `Entero` y el texto
                // llegaba como número (`_es_digito` → segfault, `709`).
                if f == "contiene" && argumentos.get(1).map(|a| es(a, nombre)).unwrap_or(false) {
                    uso.texto = true;
                }
                for a in argumentos {
                    en_expr(a, nombre, uso);
                }
            }
            // `"" + parametro` / `parametro + "..."`: el parámetro es Texto; pero
            // si además se usa en aritmética es numérico (`2 * a` con `a`
            // concatenado en un `escribir`).
            Expresion::Binaria {
                izquierda,
                derecha,
                operador,
            } => {
                use forja_syntax::ast::Operador;
                let es_texto = |e: &Expresion| matches!(e, Expresion::LiteralTexto(_));
                if (es(izquierda, nombre) && es_texto(derecha))
                    || (es(derecha, nombre) && es_texto(izquierda))
                {
                    uso.texto = true;
                }
                match operador {
                    Operador::Resta
                    | Operador::Multiplicacion
                    | Operador::Division
                    | Operador::Modulo => {
                        if es(izquierda, nombre) || es(derecha, nombre) {
                            uso.numerico = true;
                        }
                    }
                    Operador::Suma
                        if (es(izquierda, nombre) && !es_texto(derecha))
                            || (es(derecha, nombre) && !es_texto(izquierda)) =>
                    {
                        uso.numerico = true;
                    }
                    _ => {}
                }
                en_expr(izquierda, nombre, uso);
                en_expr(derecha, nombre, uso);
            }
            Expresion::Unaria { expr, .. } | Expresion::Grupo(expr) => en_expr(expr, nombre, uso),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                en_expr(condicion, nombre, uso);
                en_expr(si_verdadero, nombre, uso);
                en_expr(si_falso, nombre, uso);
            }
            Expresion::Asignacion { valor, .. }
            | Expresion::AsignacionCampo { valor, .. }
            | Expresion::AsignacionCompuesta { valor, .. } => en_expr(valor, nombre, uso),
            Expresion::AccesoMiembro { objeto, .. } => en_expr(objeto, nombre, uso),
            _ => {}
        }
    }
    fn en_sentencias(cuerpo: &[Declaracion], nombre: &str, uso: &mut Uso) {
        for d in cuerpo {
            match d {
                Declaracion::Variable { valor: Some(v), .. } => en_expr(v, nombre, uso),
                Declaracion::Asignacion { valor, .. }
                | Declaracion::AsignacionMiembro { valor, .. } => en_expr(valor, nombre, uso),
                Declaracion::AsignacionIndex {
                    nombre: n,
                    indice,
                    valor,
                    ..
                } => {
                    if n == nombre {
                        uso.arreglo = true;
                    }
                    en_expr(indice, nombre, uso);
                    en_expr(valor, nombre, uso);
                }
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
                    en_expr(condicion, nombre, uso);
                    en_sentencias(bloque_verdadero, nombre, uso);
                    if let Some(b) = bloque_falso {
                        en_sentencias(b, nombre, uso);
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    en_expr(condicion, nombre, uso);
                    en_sentencias(bloque, nombre, uso);
                }
                Declaracion::Para {
                    condicion, bloque, ..
                } => {
                    if let Some(c) = condicion {
                        en_expr(c, nombre, uso);
                    }
                    en_sentencias(bloque, nombre, uso);
                }
                Declaracion::Repetir { bloque, .. } => en_sentencias(bloque, nombre, uso),
                Declaracion::Retornar { valor: Some(v) } => en_expr(v, nombre, uso),
                Declaracion::Expresion(e) => en_expr(e, nombre, uso),
                Declaracion::LlamadaFuncion { argumentos, .. } => {
                    for a in argumentos {
                        en_expr(a, nombre, uso)
                    }
                }
                _ => {}
            }
        }
    }
    let mut uso = Uso::default();
    en_sentencias(cuerpo, nombre, &mut uso);
    if uso.arreglo {
        Some(T::Arreglo)
    } else if uso.mapa && !uso.texto && !uso.numerico {
        Some(T::Mapa)
    } else if uso.texto && !uso.numerico && !escapa(cuerpo, nombre) {
        Some(T::Texto)
    } else {
        None
    }
}
