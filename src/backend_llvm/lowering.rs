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
    Dec,
    Texto,
    Arreglo,
    Mapa,
    Enum(String),
    Clase(String),
    Funcion,
    Nulo,
}

impl T {
    fn llvm(&self) -> LlvmTipo {
        match self {
            T::Int | T::Nulo => LlvmTipo::I64,
            T::Dec => LlvmTipo::Double,
            T::Texto | T::Arreglo | T::Mapa | T::Enum(_) | T::Clase(_) | T::Funcion => {
                LlvmTipo::Ptr
            }
        }
    }

    fn de_tipo(t: &Option<Tipo>) -> Option<T> {
        t.as_ref().map(|t| match t {
            Tipo::Entero => T::Int,
            Tipo::Decimal => T::Dec,
            Tipo::Texto => T::Texto,
            Tipo::Booleano => T::Int,
            Tipo::Nulo => T::Nulo,
            Tipo::Clase(n) => T::Clase(n.clone()),
            Tipo::Arreglo(_) => T::Arreglo,
            _ => T::Int,
        })
    }

    fn nombre(&self) -> &str {
        match self {
            T::Int => "Entero",
            T::Dec => "Decimal",
            T::Texto => "Texto",
            T::Arreglo => "Arreglo",
            T::Mapa => "Mapa",
            T::Enum(n) => n,
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
    Dec(String),
    Texto(String),
    Arreglo(String),
    Mapa(String),
    Enum(String, String),
    Clase(String, String),
    Funcion(String),
    Nulo,
}

impl V {
    fn r(&self) -> &str {
        match self {
            V::Int(r)
            | V::Dec(r)
            | V::Texto(r)
            | V::Arreglo(r)
            | V::Mapa(r)
            | V::Enum(r, _)
            | V::Clase(r, _)
            | V::Funcion(r) => r,
            V::Nulo => "0",
        }
    }
    fn t(&self) -> T {
        match self {
            V::Int(_) => T::Int,
            V::Dec(_) => T::Dec,
            V::Texto(_) => T::Texto,
            V::Arreglo(_) => T::Arreglo,
            V::Mapa(_) => T::Mapa,
            V::Enum(_, c) => T::Enum(c.clone()),
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

#[derive(Clone)]
struct Clase {
    campos: Vec<(String, T)>,
    metodos: Vec<(String, Vec<T>, T)>,
}

/// Backend de lowering.
pub struct Lowering {
    header: String,
    decls: String,
    globals: String,
    cuerpos: String,
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
    variantes: HashMap<String, (String, usize, usize)>,
    consts: HashMap<String, String>,
    /// tipos inferidos por pre-escaneo (variables declaradas como `nulo`).
    tipos_pre: HashMap<String, T>,
    etiquetas: Vec<(String, String)>, // (romper, continuar)
    ret_actual: T,
    termina: bool,
    lc: u64,
    rc: u64,
    sc: u64,
    /// `FORJA_SIN_RANGOS=1`: acceso a arreglos sin chequeo de rango (UB si se
    /// pasa, como `get_unchecked`). Es lo que permite vectorizar bucles.
    sin_rangos: bool,
}

const RUNTIME: &[(&str, &str)] = &[
    ("forja_alloc", "ptr (i64)"),
    ("forja_tiempo_ms", "i64 ()"),
    ("forja_concat", "ptr (ptr, ptr)"),
    ("forja_a_texto", "ptr (i64)"),
    ("forja_decimal_a_texto", "ptr (double)"),
    ("forja_texto_nulo", "ptr ()"),
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
    ("forja_imprimir_nulo", "void ()"),
    ("forja_nueva_linea", "void ()"),
    ("forja_arreglo_nuevo", "ptr (i64)"),
    ("forja_arreglo_longitud", "i64 (ptr)"),
    ("forja_arreglo_obtener", "i64 (ptr, i64)"),
    ("forja_arreglo_fijar", "void (ptr, i64, i64)"),
    ("forja_empujar", "void (ptr, i64)"),
    ("forja_mapa_nuevo", "ptr ()"),
    ("forja_mapa_longitud", "i64 (ptr)"),
    ("forja_mapa_fijar_i64", "void (ptr, i64, i64)"),
    ("forja_mapa_obtener_i64", "i64 (ptr, i64)"),
    ("forja_mapa_fijar_texto", "void (ptr, ptr, i64)"),
    ("forja_mapa_obtener_texto", "i64 (ptr, ptr)"),
    ("exit", "void (i32)"),
];

impl Lowering {
    pub fn nuevo() -> Self {
        Lowering {
            header: String::new(),
            decls: String::new(),
            globals: String::new(),
            cuerpos: String::new(),
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
            etiquetas: Vec::new(),
            ret_actual: T::Int,
            termina: false,
            lc: 0,
            rc: 0,
            sc: 0,
            sin_rangos: std::env::var("FORJA_SIN_RANGOS")
                .map(|v| matches!(v.to_lowercase().as_str(), "1" | "si" | "sí" | "on" | "verdadero"))
                .unwrap_or(false),
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
            T::Enum(c) => V::Enum(r, c.clone()),
            T::Clase(c) => V::Clase(r, c.clone()),
            T::Funcion => V::Funcion(r),
            T::Nulo | T::Int => V::Int(r),
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
            // shortcut: arreglo/clase concatenados con Texto se imprimen como la
            // dirección; `escribir` de esos tipos ya devuelve error explícito.
            // Upgrade: cuando haya `mostrar` para arreglos, concatenar su repr.
            (
                V::Arreglo(r) | V::Mapa(r) | V::Enum(r, _) | V::Clase(r, _) | V::Funcion(r),
                T::Texto,
            ) => {
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
            | (v, T::Enum(_))
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
            V::Dec(r) => {
                let c = self.r();
                self.linea(&format!("{} = fcmp one double {}, 0.0", c, r));
                c
            }
            V::Texto(r)
            | V::Arreglo(r)
            | V::Mapa(r)
            | V::Enum(r, _)
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

        // 1. Firmas, clases y prototipos de FFI.
        for d in decls {
            match d {
                Declaracion::Funcion {
                    nombre,
                    parametros,
                    tipo_retorno,
                    externa,
                    enlace_nombre,
                    ..
                } => {
                    let params: Vec<T> = parametros
                        .iter()
                        .map(|p| self.tipo_declarado(&p.tipo).unwrap_or(T::Int))
                        .collect();
                    let ret = if *externa && tipo_retorno.is_none() {
                        T::Nulo
                    } else {
                        self.tipo_declarado(tipo_retorno)
                            .or_else(|| {
                                // sin anotación: se infiere del primer `retornar expr`
                                if let Declaracion::Funcion { cuerpo, .. } = d {
                                    primer_retorno(cuerpo)
                                } else {
                                    None
                                }
                            })
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
                    ..
                } => {
                    let campos: Vec<(String, T)> = campos
                        .iter()
                        .map(|c| {
                            (
                                c.nombre.clone(),
                                self.tipo_declarado(&c.tipo).unwrap_or(T::Int),
                            )
                        })
                        .collect();
                    let ms: Vec<(String, Vec<T>, T)> = metodos
                        .iter()
                        .map(|m| {
                            (
                                m.nombre.clone(),
                                m.parametros
                                    .iter()
                                    .map(|p| self.tipo_declarado(&p.tipo).unwrap_or(T::Int))
                                    .collect(),
                                self.tipo_declarado(&m.tipo_retorno).unwrap_or(T::Nulo),
                            )
                        })
                        .collect();
                    self.clases.insert(
                        nombre.clone(),
                        Clase {
                            campos,
                            metodos: ms,
                        },
                    );
                }
                Declaracion::Enum {
                    nombre, variantes, ..
                } => {
                    let campos: Vec<(String, usize)> = variantes
                        .iter()
                        .map(|v| (v.nombre.clone(), v.tipos.len()))
                        .collect();
                    let max_campos = campos.iter().map(|(_, n)| *n).max().unwrap_or(0);
                    for (i, (vn, aridad)) in campos.iter().enumerate() {
                        self.variantes
                            .insert(vn.clone(), (nombre.clone(), i, *aridad));
                    }
                    self.enums
                        .insert(nombre.clone(), EnumInformacion { max_campos });
                }
                Declaracion::Rasgo { .. } | Declaracion::Implementacion { .. } => {}
                _ => {}
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
                self.funcion(nombre, parametros, &params, ret.clone(), cuerpo)?;
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
        }

        // 6. Globals: se declaran acá y se inicializan en `@forja_init`.
        let mut globals_iniciales: Vec<(String, T, Option<Expresion>)> = Vec::new();
        for d in decls {
            if let Declaracion::Variable {
                nombre,
                valor,
                tipo,
                ..
            } = d
            {
                let t = self.tipo_declarado(tipo).unwrap_or_else(|| {
                    valor
                        .as_ref()
                        .map(|v| self.tipo_estatico(v))
                        .unwrap_or(T::Int)
                });
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
                globals_iniciales.push((nombre.clone(), t, valor.clone()));
            }
        }

        // Las funciones ya están emitidas en `self.cuerpos`: se aparta ese
        // buffer para que las instrucciones del init no caigan en medio.
        let cuerpo_funciones = std::mem::take(&mut self.cuerpos);
        // `self.vars` ya tiene los globals registrados: el init los necesita.
        self.prologo.clear();
        self.termina = false;
        for (nombre, t, valor) in &globals_iniciales {
            if let Some(v) = valor {
                let val = self.expr(v)?;
                let r = self.como(&val, t.clone());
                let _ = writeln!(
                    self.cuerpos,
                    "  store {} {}, ptr @g_{}",
                    t.llvm().to_llvm_ir(),
                    r,
                    nombre
                );
            }
        }
        for d in decls {
            if matches!(
                d,
                Declaracion::Funcion { .. }
                    | Declaracion::Clase { .. }
                    | Declaracion::Rasgo { .. }
                    | Declaracion::Implementacion { .. }
                    | Declaracion::Importar(_)
                    | Declaracion::ImportarExterna(_)
                    | Declaracion::Enum { .. }
            ) {
                continue;
            }
            if matches!(d, Declaracion::Variable { .. }) {
                continue; // ya inicializada arriba
            }
            self.sentencia(d)?;
        }
        let prologo_inicializar = std::mem::take(&mut self.prologo);
        let mut cuerpo_inicializar_fn = prologo_inicializar;
        cuerpo_inicializar_fn.push_str(&std::mem::take(&mut self.cuerpos));
        self.cuerpos = cuerpo_funciones;

        let tiene_principal_usuario = decls
            .iter()
            .any(|d| matches!(d, Declaracion::Funcion { nombre, .. } if nombre == "main"));

        let mut salida = String::new();
        salida.push_str(&self.header);
        // Layout del arreglo del runtime: { len, cap, datos }.
        let _ = writeln!(salida, "%ForjaArreglo = type {{ i64, i64, ptr }}");
        salida.push('\n');
        for (nombre, campos) in &clases {
            let fs: Vec<String> = campos.iter().map(|(_, t)| t.llvm().to_llvm_ir()).collect();
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
        salida.push_str(&self.cuerpos);
        let _ = writeln!(salida, "define void @forja_init() {{");
        let _ = writeln!(salida, "entry:");
        salida.push_str(&cuerpo_inicializar_fn);
        let _ = writeln!(salida, "  ret void");
        let _ = writeln!(salida, "}}");
        let _ = writeln!(salida, "define i32 @main() {{");
        let _ = writeln!(salida, "entry:");
        let _ = writeln!(salida, "  call void @forja_init()");
        if tiene_principal_usuario {
            let _ = writeln!(salida, "  call i64 @forja_main_usuario()");
        }
        let _ = writeln!(salida, "  ret i32 0");
        let _ = writeln!(salida, "}}");
        Ok(salida)
    }

    /// Tipo declarado en el fuente, resolviendo si el nombre es un `tipo`
    /// (enum) y no una clase (el parser no distingue en la anotación).
    fn tipo_declarado(&self, t: &Option<Tipo>) -> Option<T> {
        T::de_tipo(t).map(|t| match t {
            T::Clase(n) if self.enums.contains_key(&n) => T::Enum(n),
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
            Expresion::LiteralBooleano(_) => T::Int,
            Expresion::LiteralNulo => T::Nulo,
            Expresion::LiteralNumero(_) => T::Int,
            Expresion::Arreglo(_) => T::Arreglo,
            Expresion::Mapa(_) => T::Mapa,
            Expresion::LlamadaFuncion { nombre, .. } if self.variantes.contains_key(nombre) => {
                T::Enum(self.variantes[nombre].0.clone())
            }
            Expresion::Grupo(g) => self.tipo_estatico(g),
            Expresion::Instanciacion { clase, .. } => T::Clase(clase.clone()),
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
                | Operador::O => T::Int,
                _ => {
                    let a = self.tipo_estatico(izquierda);
                    let b = self.tipo_estatico(derecha);
                    if a == T::Dec || b == T::Dec {
                        T::Dec
                    } else if a == T::Texto || b == T::Texto {
                        T::Texto
                    } else {
                        T::Int
                    }
                }
            },
            Expresion::Identificador { nombre, .. } => {
                self.vars.get(nombre).map(|v| v.t.clone()).unwrap_or(T::Int)
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
        T::Texto | T::Arreglo | T::Mapa | T::Enum(_) | T::Clase(_) | T::Funcion => "null",
        _ => "0",
    }
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
        "contiene" | "empieza_con" | "termina_con" => T::Int,
        _ => return None,
    })
}

mod expresiones;
mod sentencias;

/// Tipo del primer `retornar <expr>` del cuerpo (para funciones sin anotación).
fn primer_retorno(cuerpo: &[Declaracion]) -> Option<T> {
    for d in cuerpo {
        match d {
            Declaracion::Retornar { valor: Some(e) } => {
                return Some(match e {
                    Expresion::LiteralTexto(_) => T::Texto,
                    Expresion::LiteralDecimal(_) => T::Dec,
                    Expresion::Arreglo(_) => T::Arreglo,
                    Expresion::Mapa(_) => T::Mapa,
                    Expresion::Instanciacion { clase, .. } => T::Clase(clase.clone()),
                    _ => T::Int,
                })
            }
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
                if let Some(t) = primer_retorno(bloque_verdadero) {
                    return Some(t);
                }
                if let Some(b) = bloque_falso {
                    if let Some(t) = primer_retorno(b) {
                        return Some(t);
                    }
                }
            }
            Declaracion::Mientras { bloque, .. }
            | Declaracion::Repetir { bloque, .. }
            | Declaracion::Para { bloque, .. } => {
                if let Some(t) = primer_retorno(bloque) {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    None
}
