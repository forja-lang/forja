//! Cabecera de metadata del bytecode.
//!
//! El formato `.fbc` ya tenía magic + versión + CRC32. Esta cabecera agrega los
//! datos que faltaban para invalidar cachés/AOT cuando cambia el compilador o
//! las features: hash de plataforma y hash de contratos.

/// Magic del formato.
pub const MAGIC: [u8; 4] = *b"FBC\0";

/// Versión del formato de bytecode.
pub const FORMAT_VERSION: u32 = 2;

/// Hash estable de la plataforma/compilación. Cambia si cambia el destino o el
/// perfil; dos binarios con distinto hash no deben compartir caché.
pub fn feature_hash() -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut escribir = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    escribir(std::env::consts::ARCH.as_bytes());
    escribir(std::env::consts::OS.as_bytes());
    escribir(if cfg!(target_endian = "little") {
        b"le"
    } else {
        b"be"
    });
    escribir(if cfg!(debug_assertions) {
        b"debug"
    } else {
        b"release"
    });
    h
}

/// Cabecera independiente del string pool y de los opcodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cabecera {
    pub version: u32,
    pub feature_hash: u64,
    pub contracts_hash: u64,
}

impl Cabecera {
    pub fn nueva(contracts_hash: u64) -> Self {
        Cabecera {
            version: FORMAT_VERSION,
            feature_hash: feature_hash(),
            contracts_hash,
        }
    }

    /// Serializa `magic + versión + feature_hash + contracts_hash` (24 bytes).
    pub fn escribir(&self) -> [u8; 24] {
        let mut out = [0u8; 24];
        out[0..4].copy_from_slice(&MAGIC);
        out[4..8].copy_from_slice(&self.version.to_le_bytes());
        out[8..16].copy_from_slice(&self.feature_hash.to_le_bytes());
        out[16..24].copy_from_slice(&self.contracts_hash.to_le_bytes());
        out
    }

    pub fn leer(data: &[u8]) -> Option<Self> {
        if data.len() < 24 || data[0..4] != MAGIC {
            return None;
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let feature_hash = u64::from_le_bytes(data[8..16].try_into().ok()?);
        let contracts_hash = u64::from_le_bytes(data[16..24].try_into().ok()?);
        if version != FORMAT_VERSION {
            return None;
        }
        Some(Cabecera {
            version,
            feature_hash,
            contracts_hash,
        })
    }
}

/// Hash FNV-1a de los mensajes y tipos de contratos, para invalidar AOT/caché.
pub fn hash_contratos(contratos: &[crate::ContratoBytecode]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut escribir = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for c in contratos {
        escribir(c.mensaje.as_bytes());
        escribir(&[match c.tipo {
            crate::TipoContrato::Precondicion => 0,
            crate::TipoContrato::Postcondicion => 1,
            crate::TipoContrato::Invariante => 2,
        }]);
        escribir(&(c.condicion.len() as u64).to_le_bytes());
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabecera_ida_y_vuelta() {
        let c = Cabecera::nueva(0x1234);
        let bytes = c.escribir();
        let leida = Cabecera::leer(&bytes).unwrap();
        assert_eq!(leida, c);
        assert_eq!(leida.version, FORMAT_VERSION);
    }

    #[test]
    fn rechaza_version_desconocida() {
        let c = Cabecera::nueva(0);
        let mut bytes = c.escribir();
        bytes[4..8].copy_from_slice(&999u32.to_le_bytes());
        assert!(Cabecera::leer(&bytes).is_none());
    }

    #[test]
    fn rechaza_magic_incorrecto() {
        let mut bytes = Cabecera::nueva(0).escribir();
        bytes[0] = b'X';
        assert!(Cabecera::leer(&bytes).is_none());
    }
}

#[cfg(test)]
mod tests_formato {
    use super::*;

    #[test]
    fn serializacion_empieza_con_metadata() {
        let bytes = crate::serializar_bytecode(&[]);
        assert_eq!(&bytes[0..4], &MAGIC);
        assert_eq!(
            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            FORMAT_VERSION
        );
    }
}

#[cfg(test)]
mod tests_contenedor {
    use super::*;

    #[test]
    fn contenedor_con_metadata_ida_y_vuelta() {
        let codigos = vec![crate::Opcode::PushEntero(7), crate::Opcode::Descartar];
        let bytes = crate::serializar_con_metadata(&codigos, &[]);
        let (leidos, cab) = crate::deserializar_con_metadata(&bytes).unwrap();
        assert_eq!(leidos, codigos);
        assert_eq!(cab.version, FORMAT_VERSION);
        assert_eq!(cab.feature_hash, feature_hash());
    }

    #[test]
    fn contenedor_rechaza_cabecera_vieja() {
        let codigos = vec![crate::Opcode::PushEntero(1)];
        let mut bytes = crate::serializar_con_metadata(&codigos, &[]);
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(crate::deserializar_con_metadata(&bytes).is_none());
    }
}
