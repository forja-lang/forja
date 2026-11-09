//! Implementación de `forja_runtime::Anfitrion` para `ForjaFast`.

use crate::vm_fast::ForjaFast;
use forja_bytecode::symbol_table::{IdSimbolo, TablaSimbolos};
use forja_runtime::class_descriptor::DescriptorClase;
use forja_runtime::sandbox::{SandboxFilesystem, SandboxProceso, SandboxRed};
use forja_runtime::shape::{IdForma, RegistroFormas};
use forja_runtime::socket::SocketState;
use forja_runtime::tipos::{CanalValor, ErrFast, ObjVal, ValorFast};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

impl forja_runtime::Anfitrion for ForjaFast {
    fn reservar_texto(&mut self, s: Arc<str>) -> u32 {
        self.reservar_texto(s)
    }
    fn obtener_texto(&self, idx: u32) -> &Arc<str> {
        self.obtener_texto(idx)
    }
    fn reservar_arr(&mut self, v: Vec<ValorFast>) -> u32 {
        self.reservar_arr(v)
    }
    fn obtener_arr(&self, idx: u32) -> &Vec<ValorFast> {
        self.obtener_arr(idx)
    }
    fn obtener_arr_mut(&mut self, idx: u32) -> &mut Vec<ValorFast> {
        self.obtener_arr_mut(idx)
    }
    fn reservar_mapa(&mut self, m: HashMap<String, ValorFast>) -> u32 {
        self.reservar_mapa(m)
    }
    fn obtener_mapa(&self, idx: u32) -> &HashMap<String, ValorFast> {
        self.obtener_mapa(idx)
    }
    fn obtener_mapa_mut(&mut self, idx: u32) -> &mut HashMap<String, ValorFast> {
        self.obtener_mapa_mut(idx)
    }
    fn reservar_objeto(&mut self, o: ObjVal) -> u32 {
        self.reservar_objeto(o)
    }
    fn obtener_objeto(&self, idx: u32) -> &ObjVal {
        self.obtener_objeto(idx)
    }
    fn obtener_objeto_mut(&mut self, idx: u32) -> &mut ObjVal {
        self.obtener_objeto_mut(idx)
    }
    fn registro_formas(&mut self) -> &mut RegistroFormas {
        &mut self.runtime.registro_formas
    }
    fn clase_descriptors(&mut self) -> &mut HashMap<IdSimbolo, DescriptorClase> {
        &mut self.runtime.class_descriptors
    }
    fn tabla_simbolos(&mut self) -> &mut TablaSimbolos {
        &mut self.runtime.tabla_simbolos
    }
    fn mostrar_valor(&self, v: &ValorFast) -> String {
        self.mostrar_valor(v)
    }
    fn socket_reservar(&mut self, e: SocketState) -> u32 {
        self.socket_reservar(e)
    }
    fn socket_obtener(&self, idx: u32) -> &SocketState {
        self.socket_obtener(idx)
    }
    fn socket_obtener_mut(&mut self, idx: u32) -> &mut SocketState {
        self.socket_obtener_mut(idx)
    }
    fn socket_cerrar(&mut self, idx: u32) {
        self.socket_cerrar(idx)
    }
    fn canal_try_recibir(&mut self, idx: usize) -> Option<CanalValor> {
        self.canal_try_recibir(idx)
    }
    fn valor_de_channel(&mut self, v: CanalValor) -> ValorFast {
        self.valor_de_channel(v)
    }
    fn error_tipo(&self, mensaje: String) -> ErrFast {
        ErrFast::TipoInv(mensaje)
    }

    fn sandbox(&mut self) -> &mut SandboxRed {
        &mut self.sandbox
    }
    fn sandbox_fs(&mut self) -> &mut SandboxFilesystem {
        &mut self.sandbox_fs
    }
    fn sandbox_proc(&mut self) -> &mut SandboxProceso {
        &mut self.sandbox_proc
    }
    fn arreglo_monticulo(&self) -> &Vec<Vec<ValorFast>> {
        &self.runtime.array_heap
    }
    fn mapa_monticulo(&self) -> &Vec<HashMap<String, ValorFast>> {
        &self.runtime.map_heap
    }
    fn formas_objeto_mut(&mut self) -> &mut Vec<IdForma> {
        &mut self.runtime.formas_objeto
    }
    fn clases_derive_igual(&mut self) -> &mut HashSet<IdSimbolo> {
        &mut self.runtime.clases_derive_igual
    }
    fn clases_derive_mostrar(&mut self) -> &mut HashSet<IdSimbolo> {
        &mut self.runtime.clases_derive_mostrar
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn sym_canal_receptor(&self) -> IdSimbolo {
        self.sym_canal_receptor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exige_host<T: forja_runtime::Anfitrion>() {}

    #[test]
    fn forja_fast_implementa_host() {
        exige_host::<ForjaFast>();
    }
}
