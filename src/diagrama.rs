/// Generador de diagrams HTML - forja diagram|grafico <archivo.fa>
use crate::ast::*;

pub struct DiagramGenerator {
    output: String,
    node_id: usize,
}

impl DiagramGenerator {
    pub fn new() -> Self { DiagramGenerator { output: String::new(), node_id: 0 } }

    fn next_id(&mut self) -> usize { let id = self.node_id; self.node_id += 1; id }

    pub fn generar(&mut self, p: &Programa) -> String {
        self.output = String::new(); self.node_id = 0;
        let n = p.declaraciones.len();

        let h = format!(r#"<!DOCTYPE html>
<html lang='es'><head><meta charset='UTF-8'><meta name='viewport' content='width=device-width,initial-scale=1.0'>
<title>diagram Forja</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:'Segoe UI',system-ui,sans-serif;background:#0d1117;color:#c9d1d9;padding:20px}}
.hd{{margin-bottom:20px;padding:12px 16px;background:#161b22;border-radius:8px;border:1px solid#30363d;display:flex;align-items:center;gap:10px;flex-wrap:wrap}}
.hd h1{{font-size:1.2em;margin-right:auto}}
button{{background:#21262d;color:#c9d1d9;border:1px solid#30363d;padding:4px 12px;border-radius:6px;cursor:pointer;font-size:.8em}}
button:hover{{background:#30363d}}
button.a{{background:#1f6feb;border-color:#1f6feb}}
.v{{background:#161b22;border-radius:8px;border:1px solid#30363d;padding:16px;overflow-x:auto}}

/* ARBOL */
.t ul{{list-style:none;padding-left:22px}}
.t li{{margin:2px 0}}
.t .n{{display:inline-flex;align-items:center;gap:5px;padding:2px 8px;border-radius:5px;cursor:pointer;font-size:.82em;font-family:'Cascadia Code',monospace;white-space:nowrap;border:1px solid transparent}}
.t .n:hover{{border-color:#30363d}}
.t .arw{{display:inline-block;width:10px;text-align:center;color:#484f58;font-size:.65em;transition:transform .15s}}
.t .cl .arw{{transform:rotate(-90deg)}}
.t .b{{font-size:.65em;padding:1px 5px;border-radius:8px}}
.t .ch{{margin-left:4px;border-left:1px solid#21262d;padding-left:8px}}
.t .hd{{display:none}}
.nv{{background:#0a2a2a}}.bv{{background:#238636;color:#fff}}
.nf{{background:#0a0a2a}}.bf{{background:#1f6feb;color:#fff}}
.nc{{background:#1a0a2a}}.bc{{background:#8957e5;color:#fff}}
.ns{{background:#2a1a0a}}.bs{{background:#d29922;color:#fff}}
.nb{{background:#0a2a1a}}.bb{{background:#2ea043;color:#fff}}
.nl{{background:#1a0a2a}}.bl{{background:#bc8cff;color:#fff}}
.nr{{background:#2a0a0a}}.br{{background:#f85149;color:#fff}}
.na{{background:#0a1a2a}}.ba{{background:#58a6ff;color:#fff}}
.ni{{background:#1a2a0a}}.bi{{background:#3fb950;color:#fff}}
.ne{{background:#0a0a2a}}.be{{background:#79c0ff;color:#fff}}

/* FLUJO - CSS puro con bordes */
.f{{display:none}}
.fw{{display:flex;flex-direction:column;align-items:center;padding:4px 0}}
.fw .bx{{padding:8px 16px;font-size:.82em;font-family:'Cascadia Code',monospace;text-align:center;min-width:100px;max-width:320px;border:2px solid;border-radius:5px;display:inline-block}}
.fw .tg{{font-size:.6em;opacity:.7;display:block;text-transform:uppercase;letter-spacing:1px}}
.fw .ar{{width:2px;height:22px;background:#484f58;position:relative}}
.fw .ar::after{{content:'';position:absolute;bottom:-5px;left:50%;transform:translateX(-50%);width:0;height:0;border-left:5px solid transparent;border-right:5px solid transparent;border-top:6px solid #484f58}}
.fw .sc{{background:#0a2a1a;border-color:#2ea043}}
.fw .ec{{background:#2a0a0a;border-color:#f85149}}
.fw .vc{{background:#0a2a2a;border-color:#238636}}
.fw .ac{{background:#0a1a2a;border-color:#58a6ff}}
.fw .fc{{background:#0a0a2a;border-color:#1f6feb}}
.fw .cc{{background:#1a0a2a;border-color:#8957e5}}
.fw .ic{{background:#2a1a0a;border-color:#d29922}}
.fw .lc{{background:#1a0a2a;border-color:#bc8cff}}
.fw .rc{{background:#2a0a0a;border-color:#f85149}}
.fw .ioc{{background:#1a2a0a;border-color:#3fb950}}
.fw .br{{display:flex;gap:30px;justify-content:center;padding:2px 0;width:100%}}
.fw .bc{{display:flex;flex-direction:column;align-items:center}}
.fw .bl{{font-size:.75em;color:#d29922;margin:3px 0;padding:2px 10px;background:#2a1a0a;border:1px solid#d29922;border-radius:4px;display:inline-block}}
.fw .jl{{width:2px;height:16px;background:#484f58}}

@media print{{body{{background:#fff;color:#000;padding:0}}.hd{{display:none}}.v{{border:none;padding:10px;background:#fff}}.t ul{{padding-left:14px}}.t .n{{border-color:#ccc!important}}.t .hd{{display:block!important}}.t .cl .arw{{transform:none}}.fw .ar,.fw .jl{{background:#999!important}}.fw .ar::after{{border-top-color:#999!important}}.fw .bx{{border-color:#333!important}}@page{{margin:1cm}}}}
</style></head><body>
<div class='hd'><h1>diagram Forja</h1><span class='stats'>{n} declaraciones</span>
<button id='ba' class='a' onclick="v('a')">Arbol</button>
<button id='bf' onclick="v('f')">Flujo</button>
<button onclick="ex()">Expandir</button><button onclick="co()">Colapsar</button><button onclick="window.print()">PDF</button></div>
<div id='va' class='v t'><ul>"#);
        self.output.push_str(&h);
        self.gd(&p.declaraciones, "");
        self.output.push_str("</ul></div>\n<div id='vf' class='v f'><div class='fw'>");
        self.bf("INICIO", "sc");
        self.gf(&p.declaraciones);
        self.bf("FIN", "ec");
        self.output.push_str("</div></div>\n<script>
function v(x){document.getElementById('va').style.display=x=='a'?'block':'none';document.getElementById('vf').style.display=x=='f'?'block':'none';document.getElementById('ba').className=x=='a'?'a':'';document.getElementById('bf').className=x=='f'?'a':''}
function t(id){var e=document.getElementById('c'+id),n=document.getElementById('n'+id);if(e.classList.contains('hd')){e.classList.remove('hd');n.classList.remove('cl')}else{e.classList.add('hd');n.classList.add('cl')}}
function ex(){document.querySelectorAll('.ch').forEach(function(e){e.classList.remove('hd')});document.querySelectorAll('.n').forEach(function(e){e.classList.remove('cl')})}
function co(){document.querySelectorAll('.ch').forEach(function(e){e.classList.add('hd')});document.querySelectorAll('.n').forEach(function(e){e.classList.add('cl')})}
</script></body></html>");
        self.output.clone()
    }

    fn bf(&mut self, t: &str, c: &str) { self.output.push_str(&format!("<div class='bx {}'>{}</div>", c, t)); }

    fn bx(&mut self, t: &str, c: &str) {
        self.output.push_str(&format!("<div class='ar'></div><div class='bx {}'>{}</div>", c, t));
    }

    fn rama(&mut self, cond: &str, si: &[Declaracion], no: &[Declaracion]) {
        self.output.push_str(&format!("<div class='ar'></div><div class='bx ic'><span class='tg'>CONDICION</span><b>si</b> ({cond})</div>"));
        // linea horizontal hacia las ramas
        self.output.push_str("<div style='display:flex;width:100%;align-items:center;height:24px;position:relative'>");
        self.output.push_str("<div style='flex:1;height:2px;background:#484f58'></div>");
        self.output.push_str("</div>");
        self.output.push_str("<div class='br'>");
        // SI
        self.output.push_str("<div class='bc'>");
        self.output.push_str("<span class='bl'>SI</span><div class='jl'></div>");
        if !si.is_empty() {
            self.output.push_str("<div class='fw' style='padding:0'>");
            for d in si { self.sd(d); }
            self.output.push_str("</div>");
        }
        self.output.push_str("</div>");
        // SINO
        if !no.is_empty() {
            self.output.push_str("<div class='bc'>");
            self.output.push_str("<span class='bl'>SINO</span><div class='jl'></div>");
            self.output.push_str("<div class='fw' style='padding:0'>");
            for d in no { self.sd(d); }
            self.output.push_str("</div></div>");
        }
        self.output.push_str("</div>");
    }

    fn sd(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { mutable, nombre, tipo, valor } => {
                let kw = if *mutable { "var" } else { "const" };
                let ts = tipo.as_ref().map(|t| format!(":{}", self.ts(t))).unwrap_or_default();
                let vs = valor.as_ref().map(|v| format!(" = {}", self.ec(v))).unwrap_or_default();
                self.bx(&format!("<span class='tg'>VARIABLE</span>{kw} {nombre}{ts}{vs}"), "vc");
            }
            Declaracion::Asignacion { nombre, valor } => self.bx(&format!("<span class='tg'>ASIGNACION</span>{nombre} = {}", self.ec(valor)), "ac"),
            Declaracion::AsignacionMiembro { objeto, miembro, valor } => self.bx(&format!("<span class='tg'>ASIGNACION</span>{}.{miembro} = {}", self.ec(objeto), self.ec(valor)), "ac"),
            Declaracion::AsignacionIndex { nombre, indice, valor } => self.bx(&format!("<span class='tg'>ASIGNACION</span>{nombre}[{}] = {}", self.ec(indice), self.ec(valor)), "ac"),
            Declaracion::Funcion { nombre, parametros, tipo_retorno, cuerpo, .. } => {
                let ps: Vec<String> = parametros.iter().map(|p| p.nombre.clone()).collect();
                let ret = tipo_retorno.as_ref().map(|t| format!("->{}", self.ts(t))).unwrap_or_default();
                self.bx(&format!("<span class='tg'>FUNCION</span>{nombre}({}){ret}", ps.join(",")), "fc");
                for d in cuerpo { self.sd(d); }
            }
            Declaracion::Clase { nombre, metodos, .. } => {
                self.bx(&format!("<span class='tg'>CLASE</span>{nombre}"), "cc");
                for m in metodos {
                    let ps: Vec<String> = m.parametros.iter().map(|p| p.nombre.clone()).collect();
                    self.bx(&format!("<span class='tg'>METODO</span>{}({ps})", m.nombre, ps=ps.join(",")), "fc");
                    for d in &m.cuerpo { self.sd(d); }
                }
            }
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let no = bloque_falso.as_deref().unwrap_or(&[]);
                self.rama(&self.ec(condicion), bloque_verdadero, no);
            }
            Declaracion::Mientras { condicion, bloque } => {
                self.bx(&format!("<span class='tg'>MIENTRAS</span>{}", self.ec(condicion)), "ic");
                for d in bloque { self.sd(d); }
                self.bx("<span class='tg'>REPETIR</span>", "vc");
            }
            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                let init = inicializacion.as_ref().map(|d| self.dc(d)).unwrap_or_default();
                let cond = condicion.as_ref().map(|e| self.ec(e)).unwrap_or_default();
                let inc = incremento.as_ref().map(|d| self.dc(d)).unwrap_or_default();
                self.bx(&format!("<span class='tg'>PARA</span>{init} ; {cond} ; {inc}"), "vc");
                for d in bloque { self.sd(d); }
            }
            Declaracion::Repetir { cantidad, bloque } => {
                self.bx(&format!("<span class='tg'>REPETIR</span>{} veces", self.ec(cantidad)), "vc");
                for d in bloque { self.sd(d); }
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos.iter().map(|a| self.ec(a)).collect();
                self.bx(&format!("<span class='tg'>LLAMADA</span>{nombre}({})", args.join(",")), "lc");
            }
            Declaracion::AccesoMiembro { objeto, miembro } => self.bx(&format!("<span class='tg'>ACCESO</span>{}.{miembro}", self.ec(objeto)), "ne"),
            Declaracion::Retornar { valor } => {
                if let Some(v) = valor { self.bx(&format!("<span class='tg'>RETORNAR</span>{}", self.ec(v)), "rc"); }
                else { self.bx("<span class='tg'>RETORNAR</span>", "rc"); }
            }
            Declaracion::Importar(r) => self.bx(&format!("<span class='tg'>IMPORTAR</span>\"{r}\""), "ioc"),
            Declaracion::Enum { nombre, variantes, .. } => {
                let vars: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                self.bx(&format!("<span class='tg'>TIPO</span>{nombre} = {}", vars.join(" | ")), "cc");
            }
            Declaracion::Trait { nombre, metodos } => {
                let mnames: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                self.bx(&format!("<span class='tg'>TRAIT</span>{nombre} ({})", mnames.join(", ")), "cc");
            }
            Declaracion::Implementacion { trait_nombre, clase_nombre, metodos } => {
                let mnames: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                self.bx(&format!("<span class='tg'>IMPLEMENTA</span>{} para {} ({})", trait_nombre, clase_nombre, mnames.join(", ")), "cc");
            }
            Declaracion::Expresion(expr) => self.bx(&format!("<span class='tg'>EXPRESION</span>{}", self.ec(expr)), "ne"),
            Declaracion::AsignacionMultiple { variables, valor, .. } => self.bx(&format!("<span class='tg'>ASIGNACION_MULTIPLE</span>{} = {}", variables.join(", "), self.ec(valor)), "ac"),
        }
    }

    // ======================= ARBOL =======================
    fn gd(&mut self, d: &[Declaracion], _i: &str) { for x in d { self.gd1(x); } }

    fn no(&mut self, t: &str, c: &str, b: &str, h: bool) -> usize {
        let id = self.next_id(); let f = if h { "▼" } else { " " }; let cl = if !h { " cl" } else { "" };
        let oc = if h { format!(" onclick='t({})'", id) } else { String::new() };
        self.output.push_str(&format!("<li><span id='n{}' class='n n{}{}'{}><span class='arw'>{}</span>{} <span class='b b{}'>{}</span></span>\n", id, c, cl, oc, f, t, b, b)); id
    }
    fn ah(&mut self, id: usize) { self.output.push_str(&format!("<ul id='c{}' class='ch'>\n", id)); }
    fn ch(&mut self) { self.output.push_str("</ul>\n"); }
    fn cli(&mut self) { self.output.push_str("</li>\n"); }

    fn gd1(&mut self, d: &Declaracion) {
        match d {
            Declaracion::Variable { mutable, nombre, tipo, valor } => {
                let kw = if *mutable { "var" } else { "const" };
                let badge = if *mutable { "variable" } else { "constante" };
                let ts = tipo.as_ref().map(|t| format!(":{}", self.ts(t))).unwrap_or_default();
                let vs = valor.as_ref().map(|v| format!("={}", self.ec(v))).unwrap_or_default();
                let _ = self.no(&format!("<b>{kw}</b> {nombre}{ts}{vs}"), "v", badge, false); self.cli();
            }
            Declaracion::Asignacion { nombre, valor } => { let _ = self.no(&format!("asignar {nombre} = {}", self.ec(valor)), "a", "asignar", false); self.cli(); }
            Declaracion::AsignacionMiembro { objeto, miembro, valor } => { self.no(&format!("asignar {}.{miembro} = {}", self.ec(objeto), self.ec(valor)), "a", "asignar", false); self.cli(); }
            Declaracion::AsignacionIndex { nombre, indice, valor } => { self.no(&format!("asignar {nombre}[{}] = {}", self.ec(indice), self.ec(valor)), "a", "asignar", false); self.cli(); }
            Declaracion::Funcion { nombre, parametros, tipo_retorno, cuerpo, .. } => {
                let ps: Vec<String> = parametros.iter().map(|p| { let mut s = p.nombre.clone(); if p.prestado { s = format!("&{s}"); } if let Some(ref t) = p.tipo { s = format!("{s}:{}", self.ts(t)); } s }).collect();
                let ret = tipo_retorno.as_ref().map(|t| format!("->{}", self.ts(t))).unwrap_or_default();
                let id = self.no(&format!("<b>funcion</b> {nombre}({}){ret}", ps.join(",")), "f", "funcion", !cuerpo.is_empty());
                if !cuerpo.is_empty() { self.ah(id); for d in cuerpo { self.gd1(d); } self.ch(); } self.cli();
            }
            Declaracion::Clase { nombre, metodos, .. } => {
                let id = self.no(&format!("<b>clase</b> {nombre}"), "c", "clase", !metodos.is_empty());
                if !metodos.is_empty() { self.ah(id); for m in metodos { let ps: Vec<String> = m.parametros.iter().map(|p| p.nombre.clone()).collect(); let mid = self.no(&format!("<b>metodo</b> {}({})", m.nombre, ps.join(",")), "f", "metodo", !m.cuerpo.is_empty()); if !m.cuerpo.is_empty() { self.ah(mid); for d in &m.cuerpo { self.gd1(d); } self.ch(); } self.cli(); } self.ch(); }
                self.cli();
            }
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let c = self.ec(condicion); let id = self.no(&format!("<b>si</b> ({c})"), "s", "condicional", true);
                self.ah(id); let vid = self.no("verdadero", "s", "then", !bloque_verdadero.is_empty());
                if !bloque_verdadero.is_empty() { self.ah(vid); for d in bloque_verdadero { self.gd1(d); } self.ch(); } self.cli();
                if let Some(bf) = bloque_falso { let fid = self.no("falso", "s", "else", !bf.is_empty()); if !bf.is_empty() { self.ah(fid); for d in bf { self.gd1(d); } self.ch(); } self.cli(); }
                self.ch(); self.cli();
            }
            Declaracion::Mientras { condicion, bloque } => {
                let c = self.ec(condicion); let id = self.no(&format!("<b>mientras</b> ({c})"), "b", "bucle", !bloque.is_empty());
                if !bloque.is_empty() { self.ah(id); for d in bloque { self.gd1(d); } self.ch(); } self.cli();
            }
            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                let init = inicializacion.as_ref().map(|d| self.dc(d)).unwrap_or_default(); let cond = condicion.as_ref().map(|e| self.ec(e)).unwrap_or_default(); let inc = incremento.as_ref().map(|d| self.dc(d)).unwrap_or_default();
                let id = self.no(&format!("<b>para</b>({init};{cond};{inc})"), "b", "bucle", !bloque.is_empty());
                if !bloque.is_empty() { self.ah(id); for d in bloque { self.gd1(d); } self.ch(); } self.cli();
            }
            Declaracion::Repetir { cantidad, bloque } => {
                let c = self.ec(cantidad); let id = self.no(&format!("<b>repetir</b>({c})"), "b", "bucle", !bloque.is_empty());
                if !bloque.is_empty() { self.ah(id); for d in bloque { self.gd1(d); } self.ch(); } self.cli();
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos.iter().map(|a| self.ec(a)).collect();
                let label = if nombre == "escribir" { "escribir" } else if nombre == "BD" { "BD" } else { nombre };
                self.no(&format!("llamada {label}({})", args.join(",")), "l", "llamada", false); self.cli();
            }
            Declaracion::AccesoMiembro { objeto, miembro } => { self.no(&format!("acceso {}.{miembro}", self.ec(objeto)), "e", "acceso", false); self.cli(); }
            Declaracion::Retornar { valor } => { if let Some(v) = valor { self.no(&format!("retornar {}", self.ec(v)), "r", "retorno", false); } else { self.no("retornar", "r", "retorno", false); } self.cli(); }
            Declaracion::Importar(r) => { self.no(&format!("importar \"{r}\""), "i", "importar", false); self.cli(); }
            Declaracion::Enum { nombre, variantes, .. } => { let vars: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect(); self.no(&format!("tipo {nombre} = {}", vars.join(" | ")), "c", "enum", false); self.cli(); }
            Declaracion::Trait { nombre, metodos } => {
                let mnames: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                self.no(&format!("<b>trait</b> {nombre} ({})", mnames.join(", ")), "c", "trait", false);
                self.cli();
            }
            Declaracion::Implementacion { trait_nombre, clase_nombre, metodos } => {
                let mnames: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                let id = self.no(&format!("<b>implementa</b> {trait_nombre} para {clase_nombre}"), "c", "implementacion", !metodos.is_empty());
                if !metodos.is_empty() {
                    self.ah(id);
                    for m in metodos {
                        let ps: Vec<String> = m.parametros.iter().map(|p| p.nombre.clone()).collect();
                        let mid = self.no(&format!("<b>metodo</b> {}({})", m.nombre, ps.join(",")), "f", "metodo", !m.cuerpo.is_empty());
                        if !m.cuerpo.is_empty() { self.ah(mid); for d in &m.cuerpo { self.gd1(d); } self.ch(); }
                        self.cli();
                    }
                    self.ch();
                }
                self.cli();
            }
            Declaracion::Expresion(expr) => { self.no(&format!("expresion {}", self.ec(expr)), "e", "expresion", false); self.cli(); }
            Declaracion::AsignacionMultiple { variables, valor, .. } => { self.no(&format!("asignacion multiple {} = {}", variables.join(", "), self.ec(valor)), "a", "asignar", false); self.cli(); }
        }
    }

    fn gf(&mut self, d: &[Declaracion]) { for decl in d { self.sd(decl); } }

    fn ec(&self, e: &Expresion) -> String {
        match e {
            Expresion::LiteralNumero(n) => n.to_string(), Expresion::LiteralDecimal(d) => d.to_string(),
            Expresion::LiteralTexto(s) => format!("\"{s}\""), Expresion::LiteralBooleano(b) => (if *b { "true" } else { "false" }).to_string(),
            Expresion::LiteralNulo => "nulo".to_string(), Expresion::Identificador(n) => n.clone(),
            Expresion::Binaria { izquierda, operador, derecha } => {
                let op = match operador {
                    Operador::Suma => "+", Operador::Resta => "-", Operador::Multiplicacion => "*",
                    Operador::Division => "/", Operador::Modulo => "%",
                    Operador::Mayor => ">", Operador::Menor => "<",
                    Operador::MayorIgual => ">=", Operador::MenorIgual => "<=",
                    Operador::IgualIgual => "==", Operador::Diferente => "!=",
                    Operador::Y => "&&", Operador::O => "||",
                }; format!("{}{op}{}", self.ec(izquierda), self.ec(derecha))
            }
            Expresion::Unaria { operador, expr: ex } => format!("{operador}{}", self.ec(ex)),
            Expresion::LlamadaFuncion { nombre, argumentos } => { let args: Vec<String> = argumentos.iter().map(|a| self.ec(a)).collect(); format!("{nombre}({})", args.join(",")) }
            Expresion::AccesoMiembro { objeto, miembro } => format!("{}.{miembro}", self.ec(objeto)),
            Expresion::Instanciacion { clase, argumentos } => { let args: Vec<String> = argumentos.iter().map(|a| self.ec(a)).collect(); format!("nuevo {clase}({})", args.join(",")) }
            Expresion::Referencia { expr: ex, mutable } => { if *mutable { format!("&mut {}", self.ec(ex)) } else { format!("&{}", self.ec(ex)) } }
            Expresion::Arreglo(elementos) => { let elems: Vec<String> = elementos.iter().map(|e| self.ec(e)).collect(); format!("[{}]", elems.join(",")) }
            Expresion::Index { objeto, indice } => format!("{}[{}]", self.ec(objeto), self.ec(indice)),
            Expresion::Mapa(pares) => { let entries: Vec<String> = pares.iter().map(|(k, v)| format!("{}:{}", self.ec(k), self.ec(v))).collect(); format!("{{{}}}", entries.join(",")) }
            Expresion::Grupo(ex) => format!("({})", self.ec(ex)),
            Expresion::Coincidir { .. } => "coincidir...".to_string(), Expresion::Closure { .. } => "closure...".to_string(),
            Expresion::Hilo { .. } => "hilo{...}".to_string(), Expresion::CanalNuevo => "canal()".to_string(),
            Expresion::Seleccionar { brazos } => {
                let mut out = String::from("seleccionar{");
                for brazo in brazos {
                    if let Some((var, canal)) = &brazo.recepcion {
                        out.push_str(&format!("caso {var}={canal}.recibir()|"));
                    } else if brazo.timeout_ms > 0 {
                        out.push_str(&format!("tiempo {}|", brazo.timeout_ms));
                    } else {
                        out.push_str("otro|");
                    }
                }
                out.push_str("}");
                out
            }
            Expresion::Try(expr) => format!("{}?", self.ec(expr)),
        }
    }
    fn dc(&self, d: &Declaracion) -> String { match d { Declaracion::Variable { nombre, valor, .. } => { if let Some(v) = valor { format!("{nombre}={}", self.ec(v)) } else { nombre.clone() } } Declaracion::Asignacion { nombre, valor } => format!("{nombre}={}", self.ec(valor)), _ => "?".to_string() } }
    fn ts(&self, t: &Tipo) -> String { match t { Tipo::Entero => "Entero".to_string(), Tipo::Decimal => "Decimal".to_string(), Tipo::Texto => "Texto".to_string(), Tipo::Booleano => "Booleano".to_string(), Tipo::Nulo => "Nulo".to_string(), Tipo::Clase(n) => n.clone(), Tipo::Arreglo(t) => format!("[{}]", self.ts(t)), Tipo::Funcion(params, ret) => { let p: Vec<String> = params.iter().map(|t| self.ts(t)).collect(); format!("({})->{}", p.join(","), self.ts(ret)) }, Tipo::Resultado(ok, err) => format!("Resultado<{},{}>", self.ts(ok), self.ts(err)), Tipo::Opcion(inner) => format!("Opcion<{}>", self.ts(inner)), Tipo::TraitObjeto(n) => format!("Trait<{}>", n), Tipo::Parametro(n) => format!("Parametro<{}>", n) } }
}
