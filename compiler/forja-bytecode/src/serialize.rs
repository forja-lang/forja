//! Serialización binaria `.fbc` (magic, versión, CRC32).

use crate::metadata;
use crate::opcode::TipoIncorporada;
use crate::Opcode;
use std::sync::Arc;

/// Calcula un checksum CRC32 simple (tabla precomputada)
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Sanitiza bytecode para serialización: convierte codigos no-serializables
/// (runtime-only) a sus equivalentes genéricos.
/// Safety net para codigos que no tengan representación binaria.
pub fn sanitizar_para_serializacion(bc: &[Opcode]) -> Vec<Opcode> {
    let mut result = Vec::with_capacity(bc.len());
    for op in bc {
        match op {
            Opcode::EmpujarSumarEntero(n) => {
                result.push(Opcode::PushEntero(*n));
                result.push(Opcode::SumarEntero);
            }
            Opcode::DuplicarSumarEntero => {
                result.push(Opcode::Duplicar);
                result.push(Opcode::SumarEntero);
            }
            // Runtime-only: CallBuiltin → Call genérico
            Opcode::LlamarIncorporada(kind, nargs) => {
                let name = match kind {
                    TipoIncorporada::Escribir => "escribir",
                    TipoIncorporada::Longitud => "longitud",
                    TipoIncorporada::Len => "len",
                    TipoIncorporada::Tipo => "tipo",
                    TipoIncorporada::ATexto => "a_texto",
                    TipoIncorporada::EsNumero => "es_numero",
                    TipoIncorporada::EsTexto => "es_texto",
                    TipoIncorporada::Empujar => "empujar",
                    TipoIncorporada::Obtener => "obtener",
                    TipoIncorporada::Remover => "remover",
                    TipoIncorporada::Nuevo => "nuevo",
                };
                result.push(Opcode::Llamar(Arc::from(name), *nargs));
            }
            Opcode::LlamarDirecto(_, nargs) => {
                result.push(Opcode::Llamar(Arc::from("__direct"), *nargs));
            }
            Opcode::LlamarMetodoCacheado(_, nargs) => {
                result.push(Opcode::LlamarMetodo(Arc::from("__cached"), *nargs));
            }
            // Cualquier otro opcode se mantiene (la mayoría son serializables ahora)
            _ => result.push(op.clone()),
        }
    }
    result
}

/// Serializa bytecode a binario (formato .fbc v2 con checksum CRC32)
pub fn serializar_bytecode(codigos: &[Opcode]) -> Vec<u8> {
    let mut bytes = Vec::new();

    // Magic header "FBC\0" (v2 con checksum)
    bytes.extend_from_slice(b"FBC\0");
    // Version
    bytes.extend_from_slice(&metadata::FORMAT_VERSION.to_le_bytes());

    // Primero, recolectar todos los strings
    let mut string_pool: Vec<String> = Vec::new();
    let mut string_indices: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();

    for op in codigos {
        match op {
            Opcode::PushTexto(s)
            | Opcode::Cargar(s)
            | Opcode::Guardar(s)
            | Opcode::Declarar(s, _)
            | Opcode::Llamar(s, _)
            | Opcode::LlamadaFinal(s, _)
            | Opcode::DefinirFuncion(s, _)
            | Opcode::NuevoObjeto(s)
            | Opcode::FijarCampo(s)
            | Opcode::ObtenerCampo(s)
            | Opcode::LlamarMetodo(s, _) => {
                let s_str: &str = s.as_ref();
                if !string_indices.contains_key(s_str) {
                    let idx = string_pool.len() as u32;
                    string_indices.insert(s_str.to_string(), idx);
                    string_pool.push(s_str.to_string());
                }
            }
            _ => {}
        }
    }

    // Escribir string pool
    bytes.extend_from_slice(&(string_pool.len() as u32).to_le_bytes());
    for s in &string_pool {
        let s_bytes = s.as_bytes();
        bytes.extend_from_slice(&(s_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(s_bytes);
    }

    // Escribir codigos
    bytes.extend_from_slice(&(codigos.len() as u32).to_le_bytes());
    for op in codigos {
        bytes.push(opcode_to_byte(op));
        match op {
            Opcode::PushEntero(n) => bytes.extend_from_slice(&n.to_le_bytes()),
            Opcode::PushDecimal(d) => bytes.extend_from_slice(&d.to_le_bytes()),
            Opcode::PushTexto(s) | Opcode::Cargar(s) | Opcode::Guardar(s) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
            }
            Opcode::Declarar(s, mutable) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.push(if *mutable { 1 } else { 0 });
            }
            Opcode::CargarIndice(idx)
            | Opcode::GuardarIndice(idx)
            | Opcode::CargarIndiceGlobal(idx)
            | Opcode::GuardarIndiceGlobal(idx)
            | Opcode::CargarIndiceEntero(idx)
            | Opcode::CargarIndiceDecimal(idx)
            | Opcode::GuardarIndiceEntero(idx)
            | Opcode::GuardarIndiceDecimal(idx)
            | Opcode::SumarGuardarIndice(idx)
            | Opcode::RestarGuardarIndice(idx)
            | Opcode::MultiplicarGuardarIndice(idx) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            Opcode::DeclararIndice(idx, mutable) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.push(if *mutable { 1 } else { 0 });
            }
            Opcode::CargarSumarEntero(idx, n) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&n.to_le_bytes());
            }
            Opcode::CargarIndice2(a, b)
            | Opcode::CargarGuardarIndice(a, b)
            | Opcode::CargarSaltoSiFalso(a, b)
            | Opcode::CargarSalto(a, b) => {
                bytes.extend_from_slice(&(*a as u32).to_le_bytes());
                bytes.extend_from_slice(&(*b as u32).to_le_bytes());
            }
            Opcode::PushBooleano(b) => bytes.push(if *b { 1 } else { 0 }),
            Opcode::Salto(target) | Opcode::SaltoSiFalso(target) | Opcode::Etiqueta(target) => {
                bytes.extend_from_slice(&(*target as u32).to_le_bytes());
            }
            Opcode::DefinirFuncion(s, params) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.extend_from_slice(&(params.len() as u32).to_le_bytes());
                for p in params {
                    let p_bytes = p.as_ref().as_bytes();
                    bytes.extend_from_slice(&(p_bytes.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(p_bytes);
                }
            }
            Opcode::Llamar(s, n) | Opcode::LlamadaFinal(s, n) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.extend_from_slice(&(*n as u32).to_le_bytes());
            }
            Opcode::LlamarMetodo(s, n) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.extend_from_slice(&(*n as u32).to_le_bytes());
            }
            Opcode::NuevoObjeto(s) | Opcode::FijarCampo(s) | Opcode::ObtenerCampo(s) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
            }
            Opcode::DeclararOperacionEntero(idx, n) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&n.to_le_bytes());
            }
            Opcode::DeclararOperacionBooleano(idx, b) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.push(if *b { 1 } else { 0 });
            }
            Opcode::GuardarOperacionEntero(idx, n) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&n.to_le_bytes());
            }
            // Nuevos codigos float
            Opcode::DeclararOperacionDecimal(idx, d) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&d.to_le_bytes());
            }
            Opcode::GuardarOperacionDecimal(idx, d) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&d.to_le_bytes());
            }
            Opcode::CargarSumarDecimal(idx, d) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&d.to_le_bytes());
            }
            Opcode::InvertirSigno(idx) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            Opcode::SumarGuardarDecimal(idx)
            | Opcode::RestarGuardarDecimal(idx)
            | Opcode::MultiplicarGuardarDecimal(idx) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            // Fase A: Modulo2(src)
            Opcode::ModuloDos(src) => {
                bytes.extend_from_slice(&(*src as u32).to_le_bytes());
            }
            // Fase B: ReduceAdd(dst, src) — 2 × u32
            Opcode::ReducirSuma(dst, src) => {
                bytes.extend_from_slice(&(*dst as u32).to_le_bytes());
                bytes.extend_from_slice(&(*src as u32).to_le_bytes());
            }
            // Fase B: LoadAddPacked(dst, src1, src2) — 3 × u32
            Opcode::CargarSumarEmpaquetado(dst, src1, src2) => {
                bytes.extend_from_slice(&(*dst as u32).to_le_bytes());
                bytes.extend_from_slice(&(*src1 as u32).to_le_bytes());
                bytes.extend_from_slice(&(*src2 as u32).to_le_bytes());
            }
            // Fase 3a: Stack Bypass — 3 × u32 (dst, src1, src2)
            Opcode::DividirDecimalDirecto(dst, src1, src2)
            | Opcode::MultiplicarDecimalDirecto(dst, src1, src2)
            | Opcode::SumarDecimalDirecto(dst, src1, src2)
            | Opcode::RestarDecimalDirecto(dst, src1, src2) => {
                bytes.extend_from_slice(&(*dst as u32).to_le_bytes());
                bytes.extend_from_slice(&(*src1 as u32).to_le_bytes());
                bytes.extend_from_slice(&(*src2 as u32).to_le_bytes());
            }
            // Fase 3b: Super-fusión — 3 × u32 (dst, num_src, div_src)
            Opcode::DividirSumarFusionado(dst, num_src, div_src)
            | Opcode::DividirRestarFusionado(dst, num_src, div_src) => {
                bytes.extend_from_slice(&(*dst as u32).to_le_bytes());
                bytes.extend_from_slice(&(*num_src as u32).to_le_bytes());
                bytes.extend_from_slice(&(*div_src as u32).to_le_bytes());
            }
            // Fase 3b Const: f64 + usize + usize → 8 + 4 + 4 = 16 bytes
            Opcode::DividirSumarFusionadoConst(dst, num, div_src)
            | Opcode::DividirRestarFusionadoConst(dst, num, div_src) => {
                bytes.extend_from_slice(&(*dst as u32).to_le_bytes());
                bytes.extend_from_slice(&num.to_le_bytes());
                bytes.extend_from_slice(&(*div_src as u32).to_le_bytes());
            }
            // Opcodes para Exacto (BigDecimal)
            Opcode::PushExacto(coeff, scale) => {
                bytes.extend_from_slice(&coeff.to_le_bytes());
                bytes.extend_from_slice(&scale.to_le_bytes());
            }
            // Superinstructions Exacto
            Opcode::DeclararOperacionExacto(idx, coeff, scale) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&coeff.to_le_bytes());
                bytes.extend_from_slice(&scale.to_le_bytes());
            }
            Opcode::SumarGuardarExacto(idx) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            // Pattern matching codigos — payload: 4 bytes (usize)
            Opcode::VerificarEtiqueta(idx) | Opcode::ExtraerCampo(idx) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            Opcode::FijarCampoPorIndice(idx) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            // Design by Contract codigos — payload: 4 bytes (usize)
            Opcode::VerificarPrecondicion(idx)
            | Opcode::VerificarPostcondicion(idx)
            | Opcode::GuardarAnterior(idx)
            | Opcode::VerificarInvariante(idx) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            // Fase 2: DeclareIdxGlobal(usize, bool)
            Opcode::DeclararIndiceGlobal(idx, mutable) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.push(if *mutable { 1 } else { 0 });
            }
            // Debug: SetLine(line) — payload: 4 bytes (u32)
            Opcode::FijarLinea(line) => {
                bytes.extend_from_slice(&(*line as u32).to_le_bytes());
            }
            Opcode::EmpujarSumarEntero(n) => {
                bytes.extend_from_slice(&n.to_le_bytes());
            }
            Opcode::CrearHilo(s, nargs) | Opcode::LlamarNativa(s, nargs) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.extend_from_slice(&(*nargs as u32).to_le_bytes());
            }
            Opcode::LlamarClausura(nargs) => {
                bytes.extend_from_slice(&(*nargs as u32).to_le_bytes());
            }
            Opcode::ConsultarSocket(s) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
            }
            _ => {} // Opcodes sin payload
        }
    }

    // Agregar checksum CRC32 al final (V-07: integridad del bytecode)
    let checksum = crc32(&bytes);
    bytes.extend_from_slice(&checksum.to_le_bytes());

    bytes
}

fn opcode_to_byte(op: &Opcode) -> u8 {
    match op {
        Opcode::PushEntero(_) => 0,
        Opcode::PushDecimal(_) => 1,
        Opcode::PushTexto(_) => 2,
        Opcode::PushBooleano(_) => 3,
        Opcode::PushNulo => 4,
        Opcode::Descartar => 5,
        Opcode::Duplicar => 6,
        Opcode::Cargar(_) => 10,
        Opcode::Guardar(_) => 11,
        Opcode::Declarar(_, _) => 12,
        Opcode::CargarIndice(_) => 13,
        Opcode::GuardarIndice(_) => 14,
        Opcode::DeclararIndice(_, _) => 15,
        Opcode::DeclararIndiceGlobal(_, _) => 142,
        Opcode::CargarIndiceGlobal(_) => 143,
        Opcode::GuardarIndiceGlobal(_) => 144,
        Opcode::DeclararOperacionEntero(_, _) => 16,
        Opcode::DeclararOperacionBooleano(_, _) => 17,
        Opcode::GuardarOperacionEntero(_, _) => 18,
        Opcode::DeclararOperacionDecimal(_, _) => 19,
        Opcode::Sumar => 20,
        Opcode::Restar => 21,
        Opcode::Multiplicar => 22,
        Opcode::Dividir => 23,
        Opcode::Igual => 30,
        Opcode::Diferente => 31,
        Opcode::Menor => 32,
        Opcode::Mayor => 33,
        Opcode::MenorIgual => 34,
        Opcode::MayorIgual => 35,
        Opcode::Y => 40,
        Opcode::O => 41,
        Opcode::No => 42,
        Opcode::Salto(_) => 50,
        Opcode::SaltoSiFalso(_) => 51,
        Opcode::Etiqueta(_) => 52,
        Opcode::Detener => 53,
        Opcode::DefinirFuncion(_, _) => 55,
        Opcode::Llamar(_, _) => 60,
        Opcode::Retornar => 61,
        Opcode::NuevoObjeto(_) => 62,
        Opcode::FijarCampo(_) => 63,
        Opcode::ObtenerCampo(_) => 64,
        Opcode::LlamarMetodo(_, _) => 65,
        Opcode::FijarCampoPorIndice(_) => 66,
        Opcode::LlamarClausura(_) => 67,
        Opcode::LlamadaFinal(_, _) => 255,
        Opcode::NuevoArreglo(_) => 80,
        Opcode::ObtenerElemento => 81,
        Opcode::FijarElemento => 82,
        Opcode::LongitudArreglo => 83,
        Opcode::NuevoMapa(_) => 90,
        Opcode::ObtenerValorDeMapa => 91,
        Opcode::FijarValorDeMapa => 92,
        Opcode::GuardarOperacionDecimal(_, _) => 24,
        Opcode::CargarSumarDecimal(_, _) => 25,
        Opcode::SumarGuardarDecimal(_) => 26,
        Opcode::RestarGuardarDecimal(_) => 27,
        Opcode::MultiplicarGuardarDecimal(_) => 28,
        Opcode::InvertirSigno(_) => 29,
        Opcode::ModuloDos(_) => 93,
        Opcode::ReducirSuma(_, _) => 94,
        Opcode::CargarSumarEmpaquetado(_, _, _) => 95,
        Opcode::Imprimir => 70,
        Opcode::LeerLinea => 71,
        // Fase 3a: Stack Bypass float codigos
        Opcode::DividirDecimalDirecto(_, _, _) => 96,
        Opcode::MultiplicarDecimalDirecto(_, _, _) => 97,
        Opcode::SumarDecimalDirecto(_, _, _) => 98,
        Opcode::RestarDecimalDirecto(_, _, _) => 99,
        // Fase 3b: Super-fusión
        Opcode::DividirSumarFusionado(_, _, _) => 100,
        Opcode::DividirRestarFusionado(_, _, _) => 101,
        Opcode::DividirSumarFusionadoConst(_, _, _) => 102,
        Opcode::DividirRestarFusionadoConst(_, _, _) => 103,
        // Opcodes para Exacto (BigDecimal) — Fase Exacto
        Opcode::PushExacto(_, _) => 110,
        Opcode::SumarExacto => 111,
        Opcode::RestarExacto => 112,
        Opcode::MultiplicarExacto => 113,
        Opcode::DividirExacto => 114,
        Opcode::IgualExacto => 115,
        Opcode::MenorExacto => 116,
        Opcode::MayorExacto => 117,
        Opcode::EnteroAExacto => 118,
        Opcode::DecimalAExacto => 119,
        // Superinstructions Exacto
        Opcode::DeclararOperacionExacto(_, _, _) => 120,
        Opcode::SumarGuardarExacto(_) => 121,
        // Design by Contract (Fase 5+6)
        Opcode::VerificarPrecondicion(_) => 130,
        Opcode::VerificarPostcondicion(_) => 131,
        Opcode::GuardarAnterior(_) => 132,
        Opcode::VerificarInvariante(_) => 133,
        // Pattern matching codigos
        Opcode::VerificarEtiqueta(_) => 140,
        Opcode::ExtraerCampo(_) => 141,
        // Debug: SetLine opcode
        Opcode::FijarLinea(_) => 150,
        // Nuevos codigos de optimización (serializables)
        Opcode::CargarSumarEntero(_, _) => 160,
        Opcode::CargarIndice2(_, _) => 161,
        Opcode::CargarGuardarIndice(_, _) => 162,
        Opcode::SumarGuardarIndice(_) => 163,
        Opcode::RestarGuardarIndice(_) => 164,
        Opcode::MultiplicarGuardarIndice(_) => 165,
        Opcode::CargarSaltoSiFalso(_, _) => 166,
        Opcode::CargarSalto(_, _) => 167,
        Opcode::CargarIndiceEntero(_) => 168,
        Opcode::CargarIndiceDecimal(_) => 169,
        Opcode::GuardarIndiceEntero(_) => 170,
        Opcode::GuardarIndiceDecimal(_) => 171,
        // === Opcodes especializados por tipo (Fase 1 — type specialization) ===
        Opcode::SumarEntero => 172,
        Opcode::RestarEntero => 173,
        Opcode::MultiplicarEntero => 174,
        Opcode::DividirEntero => 175,
        Opcode::SumarDecimal => 176,
        Opcode::RestarDecimal => 177,
        Opcode::MultiplicarDecimal => 178,
        Opcode::DividirDecimal => 179,
        Opcode::IgualEntero => 180,
        Opcode::DistintoEntero => 198,
        Opcode::MenorEntero => 181,
        Opcode::MenorIgualEntero => 199,
        Opcode::MayorEntero => 182,
        Opcode::MayorIgualEntero => 200,
        Opcode::IgualDecimal => 183,
        Opcode::DistintoDecimal => 184,
        Opcode::MenorDecimal => 185,
        Opcode::MayorDecimal => 186,
        Opcode::MenorIgualDecimal => 187,
        Opcode::MayorIgualDecimal => 188,
        // === Superinstrucciones de fusión ===
        Opcode::EmpujarSumarEntero(_) => 189,
        Opcode::DuplicarSumarEntero => 190,
        // === Misc runtime codigos (serializables, generados en compile-time) ===
        Opcode::Intentar => 191,
        Opcode::ParsearEntero => 192,
        Opcode::TiempoActual => 193,
        Opcode::NuevoCanal => 194,
        Opcode::CrearHilo(_, _) => 195,
        Opcode::LlamarNativa(_, _) => 196,
        Opcode::ConsultarSocket(_) => 197,
        Opcode::NuevoCanalConCapacidad => 201,
        // Opcodes especializados (runtime-only, no serializables)
        _ => 255,
    }
}

fn byte_to_opcode(byte: u8) -> Option<Opcode> {
    // Los codigos con payload se reconstruyen en el deserializador
    match byte {
        0 => Some(Opcode::PushEntero(0)),
        1 => Some(Opcode::PushDecimal(0.0)),
        2 => Some(Opcode::PushTexto(Arc::from(""))),
        3 => Some(Opcode::PushBooleano(false)),
        4 => Some(Opcode::PushNulo),
        5 => Some(Opcode::Descartar),
        6 => Some(Opcode::Duplicar),
        10 => Some(Opcode::Cargar(Arc::from(""))),
        11 => Some(Opcode::Guardar(Arc::from(""))),
        12 => Some(Opcode::Declarar(Arc::from(""), false)),
        13 => Some(Opcode::CargarIndice(0)),
        14 => Some(Opcode::GuardarIndice(0)),
        15 => Some(Opcode::DeclararIndice(0, false)),
        16 => Some(Opcode::DeclararOperacionEntero(0, 0)),
        17 => Some(Opcode::DeclararOperacionBooleano(0, false)),
        18 => Some(Opcode::GuardarOperacionEntero(0, 0)),
        19 => Some(Opcode::DeclararOperacionDecimal(0, 0.0)),
        20 => Some(Opcode::Sumar),
        21 => Some(Opcode::Restar),
        22 => Some(Opcode::Multiplicar),
        23 => Some(Opcode::Dividir),
        24 => Some(Opcode::GuardarOperacionDecimal(0, 0.0)),
        25 => Some(Opcode::CargarSumarDecimal(0, 0.0)),
        26 => Some(Opcode::SumarGuardarDecimal(0)),
        27 => Some(Opcode::RestarGuardarDecimal(0)),
        28 => Some(Opcode::MultiplicarGuardarDecimal(0)),
        29 => Some(Opcode::InvertirSigno(0)),
        30 => Some(Opcode::Igual),
        31 => Some(Opcode::Diferente),
        32 => Some(Opcode::Menor),
        33 => Some(Opcode::Mayor),
        34 => Some(Opcode::MenorIgual),
        35 => Some(Opcode::MayorIgual),
        40 => Some(Opcode::Y),
        41 => Some(Opcode::O),
        42 => Some(Opcode::No),
        50 => Some(Opcode::Salto(0)),
        51 => Some(Opcode::SaltoSiFalso(0)),
        52 => Some(Opcode::Etiqueta(0)),
        53 => Some(Opcode::Detener),
        55 => Some(Opcode::DefinirFuncion(Arc::from(""), Vec::new())),
        60 => Some(Opcode::Llamar(Arc::from(""), 0)),
        61 => Some(Opcode::Retornar),
        62 => Some(Opcode::NuevoObjeto(Arc::from(""))),
        63 => Some(Opcode::FijarCampo(Arc::from(""))),
        64 => Some(Opcode::ObtenerCampo(Arc::from(""))),
        65 => Some(Opcode::LlamarMetodo(Arc::from(""), 0)),
        66 => Some(Opcode::FijarCampoPorIndice(0)),
        67 => Some(Opcode::LlamarClausura(0)),
        80 => Some(Opcode::NuevoArreglo(0)),
        81 => Some(Opcode::ObtenerElemento),
        82 => Some(Opcode::FijarElemento),
        83 => Some(Opcode::LongitudArreglo),
        90 => Some(Opcode::NuevoMapa(0)),
        91 => Some(Opcode::ObtenerValorDeMapa),
        92 => Some(Opcode::FijarValorDeMapa),
        93 => Some(Opcode::ModuloDos(0)),
        94 => Some(Opcode::ReducirSuma(0, 0)),
        95 => Some(Opcode::CargarSumarEmpaquetado(0, 0, 0)),
        70 => Some(Opcode::Imprimir),
        71 => Some(Opcode::LeerLinea),
        // Opcodes para Exacto (BigDecimal)
        110 => Some(Opcode::PushExacto(0, 0)),
        111 => Some(Opcode::SumarExacto),
        112 => Some(Opcode::RestarExacto),
        113 => Some(Opcode::MultiplicarExacto),
        114 => Some(Opcode::DividirExacto),
        115 => Some(Opcode::IgualExacto),
        116 => Some(Opcode::MenorExacto),
        117 => Some(Opcode::MayorExacto),
        118 => Some(Opcode::EnteroAExacto),
        119 => Some(Opcode::DecimalAExacto),
        // Superinstructions Exacto
        120 => Some(Opcode::DeclararOperacionExacto(0, 0, 0)),
        121 => Some(Opcode::SumarGuardarExacto(0)),
        // Design by Contract (Fase 5+6) — placeholder
        130 => Some(Opcode::VerificarPrecondicion(0)),
        131 => Some(Opcode::VerificarPostcondicion(0)),
        132 => Some(Opcode::GuardarAnterior(0)),
        133 => Some(Opcode::VerificarInvariante(0)),
        // Pattern matching codigos
        140 => Some(Opcode::VerificarEtiqueta(0)),
        141 => Some(Opcode::ExtraerCampo(0)),
        142 => Some(Opcode::DeclararIndiceGlobal(0, false)),
        143 => Some(Opcode::CargarIndiceGlobal(0)),
        144 => Some(Opcode::GuardarIndiceGlobal(0)),
        // Debug: SetLine opcode
        150 => Some(Opcode::FijarLinea(0)),
        // Nuevas plantillas de codigos de optimización (para deserializador)
        160 => Some(Opcode::CargarSumarEntero(0, 0)),
        161 => Some(Opcode::CargarIndice2(0, 0)),
        162 => Some(Opcode::CargarGuardarIndice(0, 0)),
        163 => Some(Opcode::SumarGuardarIndice(0)),
        164 => Some(Opcode::RestarGuardarIndice(0)),
        165 => Some(Opcode::MultiplicarGuardarIndice(0)),
        166 => Some(Opcode::CargarSaltoSiFalso(0, 0)),
        167 => Some(Opcode::CargarSalto(0, 0)),
        168 => Some(Opcode::CargarIndiceEntero(0)),
        169 => Some(Opcode::CargarIndiceDecimal(0)),
        170 => Some(Opcode::GuardarIndiceEntero(0)),
        171 => Some(Opcode::GuardarIndiceDecimal(0)),
        // Opcodes especializados por tipo
        172 => Some(Opcode::SumarEntero),
        173 => Some(Opcode::RestarEntero),
        174 => Some(Opcode::MultiplicarEntero),
        175 => Some(Opcode::DividirEntero),
        176 => Some(Opcode::SumarDecimal),
        177 => Some(Opcode::RestarDecimal),
        178 => Some(Opcode::MultiplicarDecimal),
        179 => Some(Opcode::DividirDecimal),
        180 => Some(Opcode::IgualEntero),
        181 => Some(Opcode::MenorEntero),
        182 => Some(Opcode::MayorEntero),
        183 => Some(Opcode::IgualDecimal),
        184 => Some(Opcode::DistintoDecimal),
        185 => Some(Opcode::MenorDecimal),
        186 => Some(Opcode::MayorDecimal),
        187 => Some(Opcode::MenorIgualDecimal),
        188 => Some(Opcode::MayorIgualDecimal),
        189 => Some(Opcode::EmpujarSumarEntero(0)),
        190 => Some(Opcode::DuplicarSumarEntero),
        191 => Some(Opcode::Intentar),
        192 => Some(Opcode::ParsearEntero),
        193 => Some(Opcode::TiempoActual),
        194 => Some(Opcode::NuevoCanal),
        195 => Some(Opcode::CrearHilo(Arc::from(""), 0)),
        196 => Some(Opcode::LlamarNativa(Arc::from(""), 0)),
        197 => Some(Opcode::ConsultarSocket(Arc::from(""))),
        // Opcodes de comparación Entero-Entero especializados (FASE 6A)
        198 => Some(Opcode::DistintoEntero),
        199 => Some(Opcode::MenorIgualEntero),
        200 => Some(Opcode::MayorIgualEntero),
        201 => Some(Opcode::NuevoCanalConCapacidad),
        _ => None,
    }
}

/// Límites de seguridad para deserialización de bytecode
const MAX_STRINGS: usize = 10000;
const MAX_OPCODES: usize = 100000;
const MAX_PARAMS_PER_FUNCTION: usize = 100;
const MAX_STRING_LENGTH: usize = 65536;

/// Helper seguro para obtener un string del pool por índice.
/// Retorna None si el índice está fuera de rango (seguridad contra datos corruptos).
fn string_pool_get(pool: &[String], idx: usize) -> Option<Arc<str>> {
    if idx < pool.len() {
        Some(Arc::from(pool[idx].as_str()))
    } else {
        None
    }
}

/// Deserializa bytecode desde formato binario .fbc
/// Incluye validaciones de seguridad:
/// - Límites máximos en cantidad de strings, codigos y parámetros
/// - Validación de índices del string pool antes de usarlos
/// - Verificación de tamaño de strings
pub fn deserializar_bytecode(data: &[u8]) -> Option<Vec<Opcode>> {
    if data.len() < 8 {
        return None;
    }

    let mut pos = 0;

    // Magic header
    if &data[pos..pos + 4] != b"FBC\0" {
        return None;
    }
    pos += 4;

    // Version
    if pos + 4 > data.len() {
        return None;
    }
    let version = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
    pos += 4;

    // Verificar checksum CRC32 (V-07: integridad del bytecode)
    if version >= 2 {
        if data.len() < 12 {
            return None;
        } // header(8) + checksum(4)
        let stored_checksum = u32::from_le_bytes([
            data[data.len() - 4],
            data[data.len() - 3],
            data[data.len() - 2],
            data[data.len() - 1],
        ]);
        // Calcular checksum sobre los datos sin el footer de checksum
        let data_without_checksum = &data[..data.len() - 4];
        let computed = crc32(data_without_checksum);
        if stored_checksum != computed {
            eprintln!(
                "[BC] checksum falla: stored={:08X} computed={:08X} len={}",
                stored_checksum,
                computed,
                data.len()
            );
            return None; // Datos corruptos o manipulados
        }
    }

    // String pool - con límite de seguridad
    if pos + 4 > data.len() {
        return None;
    }
    let num_strings =
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
    pos += 4;

    if num_strings > MAX_STRINGS {
        return None; // Demasiados strings, probable archivo corrupto
    }

    let mut string_pool: Vec<String> = Vec::new();
    for si in 0..num_strings {
        if pos + 4 > data.len() {
            eprintln!("[BC] string pool: pos+4 > len en string {}", si);
            return None;
        }
        let s_len =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if s_len > MAX_STRING_LENGTH {
            eprintln!("[BC] string pool: s_len {} > MAX en string {}", s_len, si);
            return None;
        }
        if pos + s_len > data.len() {
            eprintln!("[BC] string pool: pos+s_len > len en string {}", si);
            return None;
        }
        let s = match String::from_utf8(data[pos..pos + s_len].to_vec()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[BC] string pool: utf8 invalido en string {}: {}", si, e);
                return None;
            }
        };
        pos += s_len;
        string_pool.push(s);
    }
    eprintln!("[BC] string pool OK: {} strings, pos={}", num_strings, pos);

    // Opcodes - con límite de seguridad
    if pos + 4 > data.len() {
        return None;
    }
    let num_opcodes =
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
    pos += 4;

    if num_opcodes > MAX_OPCODES {
        return None; // Demasiados codigos, probable archivo corrupto
    }

    let mut codigos = Vec::with_capacity(num_opcodes.min(MAX_OPCODES));
    for oi in 0..num_opcodes {
        if pos >= data.len() {
            eprintln!(
                "[BC] codigos truncado en i={} pos={} len={}",
                oi,
                pos,
                data.len()
            );
            return None;
        }
        let byte = data[pos];
        pos += 1;
        if byte > 200 {
            eprintln!(
                "[BC] opcode byte alto: {} en i={} pos={}",
                byte,
                oi,
                pos - 1
            );
        }

        match byte {
            0 => {
                // PushEntero
                if pos + 8 > data.len() {
                    return None;
                }
                let n = i64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::PushEntero(n));
            }
            1 => {
                // PushDecimal
                if pos + 8 > data.len() {
                    return None;
                }
                let d = f64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::PushDecimal(d));
            }
            2 | 10 | 11 => {
                // PushTexto | Load | Store
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                // Validación de seguridad: el índice debe estar dentro del string_pool
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(match byte {
                    2 => Opcode::PushTexto(s),
                    10 => Opcode::Cargar(s),
                    _ => Opcode::Guardar(s),
                });
            }
            12 => {
                // Declare
                if pos + 5 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let mutable = data[pos] == 1;
                pos += 1;
                // Validación de seguridad: el índice debe estar dentro del string_pool
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(Opcode::Declarar(s, mutable));
            }
            13 | 14 | 163 | 164 | 165 | 168 | 169 | 170 | 171 => {
                // Opcodes con payload de 1x u32 (index)
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    13 => Opcode::CargarIndice(idx),
                    14 => Opcode::GuardarIndice(idx),
                    163 => Opcode::SumarGuardarIndice(idx),
                    164 => Opcode::RestarGuardarIndice(idx),
                    165 => Opcode::MultiplicarGuardarIndice(idx),
                    168 => Opcode::CargarIndiceEntero(idx),
                    169 => Opcode::CargarIndiceDecimal(idx),
                    170 => Opcode::GuardarIndiceEntero(idx),
                    _ => Opcode::GuardarIndiceDecimal(idx),
                });
            }
            15 => {
                // DeclareIdx
                if pos + 5 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let mutable = data[pos] == 1;
                pos += 1;
                codigos.push(Opcode::DeclararIndice(idx, mutable));
            }
            160 => {
                // LoadAddInt
                if pos + 12 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let n = i64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::CargarSumarEntero(idx, n));
            }
            161 | 162 | 166 | 167 => {
                // Opcodes con payload de 2x u32 (idx/dst, target/idx)
                if pos + 8 > data.len() {
                    return None;
                }
                let a = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
                pos += 4;
                let b = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
                pos += 4;
                codigos.push(match byte {
                    161 => Opcode::CargarIndice2(a, b),
                    162 => Opcode::CargarGuardarIndice(a, b),
                    166 => Opcode::CargarSaltoSiFalso(a, b),
                    _ => Opcode::CargarSalto(a, b),
                });
            }
            3 => {
                // PushBooleano
                if pos >= data.len() {
                    return None;
                }
                let b = data[pos] == 1;
                pos += 1;
                codigos.push(Opcode::PushBooleano(b));
            }
            16 => {
                // DeclareEnteroOp
                if pos + 12 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let n = i64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::DeclararOperacionEntero(idx, n));
            }
            17 => {
                // DeclareBooleanoOp
                if pos + 5 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let b = data[pos] == 1;
                pos += 1;
                codigos.push(Opcode::DeclararOperacionBooleano(idx, b));
            }
            18 => {
                // StoreEnteroOp
                if pos + 12 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let n = i64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::GuardarOperacionEntero(idx, n));
            }
            19 => {
                // DeclareFloatOp
                if pos + 12 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let d = f64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::DeclararOperacionDecimal(idx, d));
            }
            24 => {
                // StoreFloatOp
                if pos + 12 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let d = f64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::GuardarOperacionDecimal(idx, d));
            }
            25 => {
                // LoadAddFloat
                if pos + 12 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let d = f64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::CargarSumarDecimal(idx, d));
            }
            26..=28 => {
                // AddStoreFloat | SubStoreFloat | MulStoreFloat
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    26 => Opcode::SumarGuardarDecimal(idx),
                    27 => Opcode::RestarGuardarDecimal(idx),
                    _ => Opcode::MultiplicarGuardarDecimal(idx),
                });
            }
            29 => {
                // XorSign
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(Opcode::InvertirSigno(idx));
            }
            93 => {
                // Modulo2(src)
                if pos + 4 > data.len() {
                    return None;
                }
                let src =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(Opcode::ModuloDos(src));
            }
            94 => {
                // ReduceAdd(dst, src)
                if pos + 8 > data.len() {
                    return None;
                }
                let dst =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let src =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(Opcode::ReducirSuma(dst, src));
            }
            95 => {
                // LoadAddPacked(dst, src1, src2)
                if pos + 12 > data.len() {
                    return None;
                }
                let dst =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let src1 =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let src2 =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(Opcode::CargarSumarEmpaquetado(dst, src1, src2));
            }
            50..=52 => {
                // Jump | JumpSiFalso | Label
                if pos + 4 > data.len() {
                    return None;
                }
                let target =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    50 => Opcode::Salto(target),
                    51 => Opcode::SaltoSiFalso(target),
                    _ => Opcode::Etiqueta(target),
                });
            }
            55 => {
                // FunctionDef
                if pos + 8 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let nparams =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                if nparams > MAX_PARAMS_PER_FUNCTION {
                    return None;
                }
                let name = string_pool_get(&string_pool, idx)?;
                let mut params = Vec::with_capacity(nparams);
                for _ in 0..nparams {
                    if pos + 4 > data.len() {
                        return None;
                    }
                    let p_len = u32::from_le_bytes([
                        data[pos],
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                    ]) as usize;
                    pos += 4;
                    if p_len > MAX_STRING_LENGTH {
                        return None;
                    }
                    if pos + p_len > data.len() {
                        return None;
                    }
                    let p = String::from_utf8(data[pos..pos + p_len].to_vec()).ok()?;
                    pos += p_len;
                    params.push(Arc::from(p.as_str()));
                }
                codigos.push(Opcode::DefinirFuncion(name, params));
            }
            60 => {
                // Call
                if pos + 8 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let nargs =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(Opcode::Llamar(s, nargs));
            }
            65 => {
                // CallMethod
                if pos + 8 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let nargs =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(Opcode::LlamarMetodo(s, nargs));
            }
            90 => {
                // MapNew
                if pos + 4 > data.len() {
                    return None;
                }
                let n = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
                pos += 4;
                codigos.push(Opcode::NuevoMapa(n));
            }
            80 => {
                // ArrayNew
                if pos + 4 > data.len() {
                    return None;
                }
                let n = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
                pos += 4;
                codigos.push(Opcode::NuevoArreglo(n));
            }
            62..=64 => {
                // NewObject | SetField | GetField
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(match byte {
                    62 => Opcode::NuevoObjeto(s),
                    63 => Opcode::FijarCampo(s),
                    _ => Opcode::ObtenerCampo(s),
                });
            }
            // Fase 3a: Stack Bypass — 3 × u32 (dst, src1, src2)
            96..=99 => {
                if pos + 12 > data.len() {
                    return None;
                }
                let dst =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let src1 =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let src2 =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    96 => Opcode::DividirDecimalDirecto(dst, src1, src2),
                    97 => Opcode::MultiplicarDecimalDirecto(dst, src1, src2),
                    98 => Opcode::SumarDecimalDirecto(dst, src1, src2),
                    _ => Opcode::RestarDecimalDirecto(dst, src1, src2),
                });
            }
            // Fase 3b: Super-fusión FusedDivAdd/FusedDivSub — 3 × u32 (dst, num_src, div_src)
            100 | 101 => {
                if pos + 12 > data.len() {
                    return None;
                }
                let dst =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let num_src =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let div_src =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    100 => Opcode::DividirSumarFusionado(dst, num_src, div_src),
                    _ => Opcode::DividirRestarFusionado(dst, num_src, div_src),
                });
            }
            // Fase 3b Const: f64 + u32 + u32 = 16 bytes
            102 | 103 => {
                if pos + 16 > data.len() {
                    return None;
                }
                let dst =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let num = f64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                let div_src =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    102 => Opcode::DividirSumarFusionadoConst(dst, num, div_src),
                    _ => Opcode::DividirRestarFusionadoConst(dst, num, div_src),
                });
            }
            // Opcodes para Exacto: PushExacto(coeff, scale) — 16 + 4 = 20 bytes payload
            110 => {
                if pos + 20 > data.len() {
                    return None;
                }
                let coeff = i128::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                    data[pos + 8],
                    data[pos + 9],
                    data[pos + 10],
                    data[pos + 11],
                    data[pos + 12],
                    data[pos + 13],
                    data[pos + 14],
                    data[pos + 15],
                ]);
                pos += 16;
                let scale =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                pos += 4;
                codigos.push(Opcode::PushExacto(coeff, scale));
            }
            // Superinstructions Exacto: DeclareExactOp(idx, coeff, scale) — 4 + 16 + 4 = 24 bytes
            120 => {
                if pos + 24 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let coeff = i128::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                    data[pos + 8],
                    data[pos + 9],
                    data[pos + 10],
                    data[pos + 11],
                    data[pos + 12],
                    data[pos + 13],
                    data[pos + 14],
                    data[pos + 15],
                ]);
                pos += 16;
                let scale =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                pos += 4;
                codigos.push(Opcode::DeclararOperacionExacto(idx, coeff, scale));
            }
            // AddStoreExact(idx) — 4 bytes payload
            121 => {
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(Opcode::SumarGuardarExacto(idx));
            }
            // Pattern matching codigos — 4 bytes payload (usize)
            140 | 141 => {
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    140 => Opcode::VerificarEtiqueta(idx),
                    _ => Opcode::ExtraerCampo(idx),
                });
            }
            // SetFieldIdx (construcción de variantes de enum) — 4 bytes payload
            66 => {
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(Opcode::FijarCampoPorIndice(idx));
            }
            // CallClosure (nargs) — 4 bytes payload (el callee va en la pila)
            67 => {
                if pos + 4 > data.len() {
                    return None;
                }
                let nargs =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(Opcode::LlamarClausura(nargs));
            }
            // Design by Contract codigos — 4 bytes payload (usize)
            130..=133 => {
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    130 => Opcode::VerificarPrecondicion(idx),
                    131 => Opcode::VerificarPostcondicion(idx),
                    132 => Opcode::GuardarAnterior(idx),
                    _ => Opcode::VerificarInvariante(idx),
                });
            }
            // Fase 2: DeclareIdxGlobal(usize, bool) — 5 bytes payload (u32 idx + u8 mutable)
            142 => {
                if pos + 5 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let mutable = data[pos] == 1;
                pos += 1;
                codigos.push(Opcode::DeclararIndiceGlobal(idx, mutable));
            }
            // LoadIdxGlobal / StoreIdxGlobal — 4 bytes payload (u32 idx)
            143 | 144 => {
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(match byte {
                    143 => Opcode::CargarIndiceGlobal(idx),
                    _ => Opcode::GuardarIndiceGlobal(idx),
                });
            }
            // Debug: SetLine(line) — 4 bytes payload (u32 linea)
            150 => {
                if pos + 4 > data.len() {
                    return None;
                }
                let line =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                codigos.push(Opcode::FijarLinea(line));
            }
            // PushAddInt(i64) — 8 bytes payload
            189 => {
                if pos + 8 > data.len() {
                    return None;
                }
                let n = i64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]);
                pos += 8;
                codigos.push(Opcode::EmpujarSumarEntero(n));
            }
            // ThreadSpawn(Arc<str>, usize) — string_idx(4) + nargs(4)
            195 => {
                if pos + 8 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let nargs =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(Opcode::CrearHilo(s, nargs));
            }
            // CallNative(Arc<str>, usize) — string_idx(4) + nargs(4)
            196 => {
                if pos + 8 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let nargs =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(Opcode::LlamarNativa(s, nargs));
            }
            // SocketPoll(Arc<str>) — string_idx(4)
            197 => {
                if pos + 4 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(Opcode::ConsultarSocket(s));
            }
            // TailCall (255): igual formato que Call
            255 => {
                if pos + 8 > data.len() {
                    return None;
                }
                let idx =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let nargs =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                codigos.push(Opcode::LlamadaFinal(s, nargs));
            }
            _ => {
                // Opcodes sin payload
                let template = byte_to_opcode(byte)?;
                codigos.push(template);
            }
        }
    }

    Some(codigos)
}
