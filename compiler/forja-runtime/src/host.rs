//! Trait `Host`: lo que un módulo nativo puede pedirle al runtime.
//!
//! Es el primer paso para que `native_registry` viva en `forja-native` sin
//! depender de `forja-vm`.

use crate::class_descriptor::DescriptorClase;
use crate::sandbox::{SandboxFilesystem, SandboxProceso, SandboxRed};
use crate::shape::{IdForma, RegistroFormas};
use crate::socket::SocketState;
use crate::tipos::{ChannelValue, ErrFast, ObjVal, ValorFast};
use forja_bytecode::symbol_table::{IdSimbolo, TablaSimbolos};
use std::any::Any;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

pub trait Host {
    fn alloc_str(&mut self, s: Arc<str>) -> u32;
    fn get_str(&self, idx: u32) -> &Arc<str>;

    fn alloc_arr(&mut self, valores: Vec<ValorFast>) -> u32;
    fn get_arr(&self, idx: u32) -> &Vec<ValorFast>;
    fn get_arr_mut(&mut self, idx: u32) -> &mut Vec<ValorFast>;

    fn alloc_map(&mut self, mapa: HashMap<String, ValorFast>) -> u32;
    fn get_map(&self, idx: u32) -> &HashMap<String, ValorFast>;
    fn get_map_mut(&mut self, idx: u32) -> &mut HashMap<String, ValorFast>;

    fn alloc_obj(&mut self, obj: ObjVal) -> u32;
    fn get_obj(&self, idx: u32) -> &ObjVal;
    fn get_obj_mut(&mut self, idx: u32) -> &mut ObjVal;

    fn registro_formas(&mut self) -> &mut RegistroFormas;
    fn class_descriptors(&mut self) -> &mut HashMap<IdSimbolo, DescriptorClase>;
    fn tabla_simbolos(&mut self) -> &mut TablaSimbolos;

    fn mostrar_valor(&self, v: &ValorFast) -> String;

    fn socket_alloc(&mut self, estado: SocketState) -> u32;
    fn socket_get(&self, idx: u32) -> &SocketState;
    fn socket_get_mut(&mut self, idx: u32) -> &mut SocketState;
    fn socket_cerrar(&mut self, idx: u32);

    fn canal_try_recibir(&mut self, idx: usize) -> Option<ChannelValue>;
    fn valor_de_channel(&mut self, valor: ChannelValue) -> ValorFast;

    fn error_tipo(&self, mensaje: String) -> ErrFast;

    fn sandbox(&mut self) -> &mut SandboxRed;
    fn sandbox_fs(&mut self) -> &mut SandboxFilesystem;
    fn sandbox_proc(&mut self) -> &mut SandboxProceso;

    fn array_heap(&self) -> &Vec<Vec<ValorFast>>;
    fn map_heap(&self) -> &Vec<HashMap<String, ValorFast>>;
    fn formas_objeto_mut(&mut self) -> &mut Vec<IdForma>;
    fn clases_derive_igual(&mut self) -> &mut HashSet<IdSimbolo>;
    fn clases_derive_mostrar(&mut self) -> &mut HashSet<IdSimbolo>;

    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn sym_canal_receptor(&self) -> IdSimbolo;
}
