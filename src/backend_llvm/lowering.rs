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
    /// `(función, índice)` → el argumento es un arreglo de textos (literal con
    /// textos), para poder leer las casillas del parámetro como punteros.
    evidencia_texto: std::collections::HashSet<(String, usize)>,
    /// variables cuyo arreglo guarda textos (punteros) en sus casillas.
    arreglos_texto: HashMap<String, bool>,
    /// Variables/campos cuyo arreglo guarda `Decimal` (double bit-casteado en la
    /// casilla i64, como el texto guarda un puntero).
    arreglos_dec: HashMap<String, bool>,
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
    ("forja_arreglo_longitud", "i64 (ptr)"),
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
    ("forja_arreglo_fijar", "void (ptr, i64, i64)"),
    ("forja_empujar", "void (ptr, i64)"),
    ("forja_reservar", "void (ptr, i64)"),
    ("forja_mapa_nuevo", "ptr ()"),
    ("forja_mapa_longitud", "i64 (ptr)"),
    ("forja_mapa_fijar_i64", "void (ptr, i64, i64, i64)"),
    ("forja_mapa_obtener_i64", "i64 (ptr, i64)"),
    ("forja_mapa_fijar_texto", "void (ptr, ptr, i64, i64)"),
    ("forja_mapa_obtener_texto", "i64 (ptr, ptr)"),
    ("forja_mapa_valor_a_texto_i64", "ptr (ptr, i64)"),
    ("forja_mapa_valor_a_texto_texto", "ptr (ptr, ptr)"),
    ("forja_canal_nuevo", "ptr ()"),
    ("forja_canal_enviar", "void (ptr, i64)"),
    ("forja_canal_recibir", "i64 (ptr)"),
    ("forja_canal_intentar", "i64 (ptr)"),
    ("forja_esperar", "void (i64)"),
    ("forja_hilo_lanzar", "void (ptr, ptr)"),
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
            evidencia_texto: std::collections::HashSet::new(),
            arreglos_texto: HashMap::new(),
            arreglos_dec: HashMap::new(),
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
                let d = self.r();
                self.linea(&format!("{} = call ptr @forja_a_texto(i64 {})", d, r));
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
                    let params = self.tipos_params(nombre, parametros, cuerpo);
                    let ret = if *externa && tipo_retorno.is_none() {
                        T::Nulo
                    } else if matches!(tipo_retorno, Some(Tipo::Parametro(_))) {
                        T::Int
                    } else {
                        self.tipo_declarado(tipo_retorno)
                            // sin anotación: se infiere del primer `retornar expr`
                            .or_else(|| self.inferir_retorno(cuerpo, parametros, &params))
                            .unwrap_or(T::Int)
                    };
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
                            let params = self.tipos_params(&clave, &m.parametros, &m.cuerpo);
                            (m.nombre.clone(), params, self.retorno_metodo(m))
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
                            let params = self.tipos_params(&m.nombre, &m.parametros, &m.cuerpo);
                            (m.nombre.clone(), params, self.retorno_metodo(m))
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
                let t = self.tipo_declarado(tipo).unwrap_or_else(|| {
                    valor
                        .as_ref()
                        .map(|v| self.tipo_estatico(v))
                        .unwrap_or(T::Int)
                });
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
                let precondiciones = match d {
                    Declaracion::Funcion { precondiciones, .. } => precondiciones.as_slice(),
                    _ => &[],
                };
                self.funcion(
                    nombre,
                    parametros,
                    &params,
                    ret.clone(),
                    precondiciones,
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
    fn retorno_metodo(&self, m: &Metodo) -> T {
        if m.nombre == "constructor" || m.nombre == "nuevo" {
            return T::Nulo;
        }
        // `-> T` sin resolver: se queda como `Entero` (inferirlo de un
        // `retornar nulo` lo dejaba como `void` y el cuerpo devolvía un valor).
        if matches!(m.tipo_retorno, Some(Tipo::Parametro(_))) {
            return T::Int;
        }
        let params = self.tipos_params(&m.nombre, &m.parametros, &m.cuerpo);
        self.tipo_declarado(&m.tipo_retorno)
            .or_else(|| self.inferir_retorno(&m.cuerpo, &m.parametros, &params))
            .unwrap_or(T::Nulo)
    }

    /// Tipos de los parámetros de un método más los locales del cuerpo (lo que
    /// hace falta para saber de qué tipo es lo que se le asigna a un campo).
    fn pistas_de(
        &self,
        nombre: &str,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> HashMap<String, T> {
        let mut pistas: HashMap<String, T> = HashMap::new();
        self.preescanear_tipos(cuerpo, &mut pistas);
        let tipos = self.tipos_params(nombre, parametros, cuerpo);
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
                        let pistas = self.pistas_de(&m.nombre, &m.parametros, &m.cuerpo);
                        en_sentencias(&m.cuerpo, campo, &pistas, &mut tipos);
                    }
                }
                Declaracion::Implementacion {
                    clase_nombre,
                    metodos,
                    ..
                } if clase_nombre == clase => {
                    for m in metodos {
                        let pistas = self.pistas_de(&m.nombre, &m.parametros, &m.cuerpo);
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
        nombre: &str,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> Vec<T> {
        let por_llamada = self.evidencia.get(nombre).cloned().unwrap_or_default();
        parametros
            .iter()
            .enumerate()
            .map(|(i, p)| {
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
            other => self.tipo_estatico(other),
        }
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
        "dividir" => T::Arreglo,
        "recortar" => T::Texto,
        "contiene" | "empieza_con" | "termina_con" => T::Int,
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
/// por nombre de función/método (las llamadas a métodos se buscan por nombre,
/// punteadas incluidas: `x.metodo(...)`). Sólo cuenta lo que se ve por la forma
/// del argumento y exige que todas las llamadas coincidan.
///
/// shortcut: no distingue la clase del método (dos clases con el mismo nombre de
/// método comparten evidencia) ni mira argumentos que son variables.
#[allow(clippy::type_complexity)]
fn evidencia_de_llamadas(
    decls: &[Declaracion],
) -> (
    HashMap<String, Vec<Option<T>>>,
    std::collections::HashSet<(String, usize)>,
) {
    /// Tipo de un argumento: por su forma o por la variable de la que sale
    /// (`variable horas = 2.5` … `predecir(horas)`).
    fn tipo_arg(a: &Expresion, vars: &HashMap<String, T>) -> Option<T> {
        tipo_por_forma(a).or_else(|| match a {
            Expresion::Identificador { nombre, .. } => vars.get(nombre).cloned(),
            _ => None,
        })
    }
    fn anotar(
        mapa: &mut HashMap<String, Vec<Option<T>>>,
        en_conflicto: &mut std::collections::HashSet<(String, usize)>,
        vars: &HashMap<String, T>,
        args_por_llamada: &mut HashMap<String, Vec<Expresion>>,
        vars_texto: &std::collections::HashSet<String>,
        nombre: &str,
        args: &[Expresion],
    ) {
        // Las llamadas punteadas (`x.metodo`) se registran sin el receptor.
        let clave = nombre.rsplit('.').next().unwrap_or(nombre).to_string();
        args_por_llamada.insert(clave.clone(), args.to_vec());
        let _ = &vars_texto;
        let entrada = mapa.entry(clave.clone()).or_default();
        if entrada.len() < args.len() {
            entrada.resize(args.len(), None);
        }
        for (i, a) in args.iter().enumerate() {
            let Some(t) = tipo_arg(a, vars) else { continue };
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
                );
                for a in argumentos {
                    en_expr(a, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
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
                );
                en_expr(
                    objeto,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                );
                for a in argumentos {
                    en_expr(a, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
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
                );
                for a in argumentos {
                    en_expr(a, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
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
                );
            }
            Expresion::Hilo { cuerpo } => en_sentencias(
                cuerpo,
                mapa,
                en_conflicto,
                args_por_llamada,
                cadenas_texto,
                vars,
            ),
            Expresion::Coincidir { brazos, expr: o } => {
                en_expr(o, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
                for b in brazos {
                    en_sentencias(
                        &b.cuerpo,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
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
                );
                en_expr(
                    derecha,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                );
            }
            Expresion::Unaria { expr, .. } | Expresion::Grupo(expr) => en_expr(
                expr,
                mapa,
                en_conflicto,
                args_por_llamada,
                cadenas_texto,
                vars,
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
                );
                en_expr(
                    si_verdadero,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                );
                en_expr(
                    si_falso,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
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
                );
                en_expr(
                    indice,
                    mapa,
                    en_conflicto,
                    args_por_llamada,
                    cadenas_texto,
                    vars,
                );
            }
            Expresion::Arreglo(items) => {
                for i in items {
                    en_expr(i, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares {
                    en_expr(k, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
                    en_expr(v, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
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
            ),
            Expresion::AccesoMiembro { objeto, .. } => en_expr(
                objeto,
                mapa,
                en_conflicto,
                args_por_llamada,
                cadenas_texto,
                vars,
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
                    if let Some(t) = tipo_arg(v, vars) {
                        vars.insert(nombre.clone(), t);
                    }
                    en_expr(v, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
                }
                Declaracion::Variable { valor: None, .. } => {}
                Declaracion::Asignacion { nombre, valor, .. } => {
                    if let Some(t) = tipo_arg(valor, vars) {
                        vars.insert(nombre.clone(), t);
                    }
                    en_expr(
                        valor,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
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
                    );
                    en_sentencias(
                        bloque_verdadero,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
                    );
                    if let Some(b) = bloque_falso {
                        en_sentencias(b, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
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
                    );
                    en_sentencias(
                        bloque,
                        mapa,
                        en_conflicto,
                        args_por_llamada,
                        cadenas_texto,
                        vars,
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
                    )
                }
                Declaracion::Retornar { valor: Some(v) } => {
                    en_expr(v, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars)
                }
                Declaracion::Expresion(e) => {
                    en_expr(e, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars)
                }
                Declaracion::LlamadaFuncion { nombre, argumentos } => {
                    anotar(
                        mapa,
                        en_conflicto,
                        vars,
                        args_por_llamada,
                        cadenas_texto,
                        nombre,
                        argumentos,
                    );
                    for a in argumentos {
                        en_expr(a, mapa, en_conflicto, args_por_llamada, cadenas_texto, vars);
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
                    );
                }
                Declaracion::Clase { metodos, .. }
                | Declaracion::Implementacion { metodos, .. } => {
                    for m in metodos {
                        let mut propias = vars.clone();
                        for p in &m.parametros {
                            if let Some(t) = T::de_tipo(&p.tipo) {
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
    let mut por_llamada_args: HashMap<String, Vec<Expresion>> = HashMap::new();
    let mut cadenas_texto: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Dos vueltas: la segunda ya conoce los tipos de los parámetros que la
    // primera dedujo de las llamadas (`validar_usuario(email)` → `validar_email`).
    for _ in 0..2 {
        en_sentencias(
            decls,
            &mut mapa,
            &mut en_conflicto,
            &mut por_llamada_args,
            &mut cadenas_texto,
            &mut HashMap::new(),
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
                    uso.arreglo = true;
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
    } else if uso.texto && !uso.numerico && !escapa(cuerpo, nombre) {
        Some(T::Texto)
    } else {
        None
    }
}
