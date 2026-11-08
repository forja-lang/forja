//! Implementación de `forja_runtime::Host` para `ForjaFast`.

use crate::vm_fast::ForjaFast;
use forja_bytecode::symbol_table::{IdSimbolo, TablaSimbolos};
use forja_runtime::class_descriptor::DescriptorClase;
use forja_runtime::sandbox::{SandboxFilesystem, SandboxProceso, SandboxRed};
use forja_runtime::shape::{IdForma, RegistroFormas};
use forja_runtime::socket::SocketState;
use forja_runtime::tipos::{ChannelValue, ErrFast, ObjVal, ValorFast};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

impl forja_runtime::Host for ForjaFast {
    fn alloc_str(&mut self, s: Arc<str>) -> u32 {
        self.alloc_str(s)
    }
    fn get_str(&self, idx: u32) -> &Arc<str> {
        self.get_str(idx)
    }
    fn alloc_arr(&mut self, v: Vec<ValorFast>) -> u32 {
        self.alloc_arr(v)
    }
    fn get_arr(&self, idx: u32) -> &Vec<ValorFast> {
        self.get_arr(idx)
    }
    fn get_arr_mut(&mut self, idx: u32) -> &mut Vec<ValorFast> {
        self.get_arr_mut(idx)
    }
    fn alloc_map(&mut self, m: HashMap<String, ValorFast>) -> u32 {
        self.alloc_map(m)
    }
    fn get_map(&self, idx: u32) -> &HashMap<String, ValorFast> {
        self.get_map(idx)
    }
    fn get_map_mut(&mut self, idx: u32) -> &mut HashMap<String, ValorFast> {
        self.get_map_mut(idx)
    }
    fn alloc_obj(&mut self, o: ObjVal) -> u32 {
        self.alloc_obj(o)
    }
    fn get_obj(&self, idx: u32) -> &ObjVal {
        self.get_obj(idx)
    }
    fn get_obj_mut(&mut self, idx: u32) -> &mut ObjVal {
        self.get_obj_mut(idx)
    }
    fn registro_formas(&mut self) -> &mut RegistroFormas {
        &mut self.runtime.registro_formas
    }
    fn class_descriptors(&mut self) -> &mut HashMap<IdSimbolo, DescriptorClase> {
        &mut self.runtime.class_descriptors
    }
    fn tabla_simbolos(&mut self) -> &mut TablaSimbolos {
        &mut self.runtime.tabla_simbolos
    }
    fn mostrar_valor(&self, v: &ValorFast) -> String {
        self.mostrar_valor(v)
    }
    fn socket_alloc(&mut self, e: SocketState) -> u32 {
        self.socket_alloc(e)
    }
    fn socket_get(&self, idx: u32) -> &SocketState {
        self.socket_get(idx)
    }
    fn socket_get_mut(&mut self, idx: u32) -> &mut SocketState {
        self.socket_get_mut(idx)
    }
    fn socket_cerrar(&mut self, idx: u32) {
        self.socket_cerrar(idx)
    }
    fn canal_try_recibir(&mut self, idx: usize) -> Option<ChannelValue> {
        self.canal_try_recibir(idx)
    }
    fn valor_de_channel(&mut self, v: ChannelValue) -> ValorFast {
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
    fn array_heap(&self) -> &Vec<Vec<ValorFast>> {
        &self.runtime.array_heap
    }
    fn map_heap(&self) -> &Vec<HashMap<String, ValorFast>> {
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

    fn exige_host<T: forja_runtime::Host>() {}

    #[test]
    fn forja_fast_implementa_host() {
        exige_host::<ForjaFast>();
    }
}
