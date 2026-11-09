//! Source map y spans de Forja.
//!
//! Un `Span` identifica un rango de bytes dentro de un archivo. La conversión a
//! línea/columna se hace con [`SourceMap::ubicacion`], que cuenta saltos de
//! línea una sola vez por archivo.

use std::path::{Path, PathBuf};

/// Identificador estable de un archivo fuente.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(u32);

impl FileId {
    pub fn indice(self) -> usize {
        self.0 as usize
    }
}

/// Rango de bytes `[inicio, fin)` dentro de un archivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub archivo: FileId,
    pub inicio: u32,
    pub fin: u32,
}

impl Span {
    pub fn nuevo(archivo: FileId, inicio: usize, fin: usize) -> Self {
        Span {
            archivo,
            inicio: inicio as u32,
            fin: fin.max(inicio) as u32,
        }
    }

    pub fn vacio(archivo: FileId, pos: usize) -> Self {
        Span::nuevo(archivo, pos, pos)
    }

    pub fn longitud(self) -> usize {
        (self.fin.saturating_sub(self.inicio)) as usize
    }

    pub fn contiene(self, pos: usize) -> bool {
        pos >= self.inicio as usize && pos < self.fin as usize
    }

    /// Une dos spans del mismo archivo. Si son de archivos distintos, devuelve
    /// el que empieza primero (no hay spans multiarchivo en Forja).
    pub fn hasta(self, otro: Span) -> Self {
        if self.archivo != otro.archivo {
            return self;
        }
        Span {
            archivo: self.archivo,
            inicio: self.inicio.min(otro.inicio),
            fin: self.fin.max(otro.fin),
        }
    }
}

/// Ubicación legible (1-based) de una posición.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ubicacion {
    pub linea: usize,
    pub columna: usize,
}

#[derive(Debug, Clone)]
struct Archivo {
    nombre: PathBuf,
    fuente: String,
    /// Byte de inicio de cada línea (incluye la primera en 0).
    inicios: Vec<usize>,
}

/// Registro de archivos fuente y conversión de offsets a línea/columna.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    archivos: Vec<Archivo>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn agregar(&mut self, nombre: impl Into<PathBuf>, fuente: impl Into<String>) -> FileId {
        let fuente = fuente.into();
        let mut inicios = vec![0];
        for (i, b) in fuente.bytes().enumerate() {
            if b == b'\n' {
                inicios.push(i + 1);
            }
        }
        let id = FileId(self.archivos.len() as u32);
        self.archivos.push(Archivo {
            nombre: nombre.into(),
            fuente,
            inicios,
        });
        id
    }

    pub fn archivos(&self) -> usize {
        self.archivos.len()
    }

    pub fn nombre(&self, archivo: FileId) -> Option<&Path> {
        self.archivos
            .get(archivo.indice())
            .map(|a| a.nombre.as_path())
    }

    pub fn fuente(&self, archivo: FileId) -> Option<&str> {
        self.archivos
            .get(archivo.indice())
            .map(|a| a.fuente.as_str())
    }

    pub fn ubicacion(&self, span: Span) -> Option<Ubicacion> {
        let archivo = self.archivos.get(span.archivo.indice())?;
        let pos = (span.inicio as usize).min(archivo.fuente.len());
        let linea = archivo.inicios.partition_point(|&inicio| inicio <= pos);
        let inicio_linea = archivo.inicios[linea.saturating_sub(1)];
        Some(Ubicacion {
            linea,
            columna: pos - inicio_linea + 1,
        })
    }

    pub fn texto(&self, span: Span) -> Option<&str> {
        let archivo = self.archivos.get(span.archivo.indice())?;
        archivo.fuente.get(span.inicio as usize..span.fin as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubicacion_convierte_offsets() {
        let mut mapa = SourceMap::new();
        let archivo = mapa.agregar("a.fa", "uno\ndos\n");
        assert_eq!(
            mapa.ubicacion(Span::nuevo(archivo, 0, 3)).unwrap(),
            Ubicacion {
                linea: 1,
                columna: 1
            }
        );
        assert_eq!(
            mapa.ubicacion(Span::nuevo(archivo, 4, 7)).unwrap(),
            Ubicacion {
                linea: 2,
                columna: 1
            }
        );
        assert_eq!(
            mapa.ubicacion(Span::vacio(archivo, 6)).unwrap(),
            Ubicacion {
                linea: 2,
                columna: 3
            }
        );
    }

    #[test]
    fn texto_y_longitud() {
        let mut mapa = SourceMap::new();
        let archivo = mapa.agregar("a.fa", "hola mundo");
        let span = Span::nuevo(archivo, 5, 10);
        assert_eq!(mapa.texto(span), Some("mundo"));
        assert_eq!(span.longitud(), 5);
        assert!(span.contiene(9));
        assert!(!span.contiene(10));
    }

    #[test]
    fn archivos_distintos_no_se_unen() {
        let mut mapa = SourceMap::new();
        let archivo_a = mapa.agregar("a.fa", "aaa");
        let archivo_b = mapa.agregar("b.fa", "bbb");
        let span_a = Span::nuevo(archivo_a, 0, 1);
        assert_eq!(span_a.hasta(Span::nuevo(archivo_b, 0, 2)), span_a);
    }
}
