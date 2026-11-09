//! Trait `Anfitrion`: lo que un módulo nativo puede pedirle al runtime.
//!
//! Es el primer paso para que `native_registry` viva en `forja-native` sin
//! depender de `forja-vm`.

use crate::descriptor_clase::DescriptorClase;
use crate::sandbox::{SandboxFilesystem, SandboxProceso, SandboxRed};
use crate::shape::{IdForma, RegistroFormas};
use crate::socket::SocketState;
use crate::tipos::{CanalValor, ErrFast, ObjVal, ValorFast};
use forja_bytecode::tabla_simbolos::{IdSimbolo, TablaSimbolos};
use std::any::Any;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

pub trait Anfitrion {
    fn reservar_texto(&mut self, s: Arc<str>) -> u32;
    fn obtener_texto(&self, idx: u32) -> &Arc<str>;

    fn reservar_arr(&mut self, valores: Vec<ValorFast>) -> u32;
    fn obtener_arr(&self, idx: u32) -> &Vec<ValorFast>;
    fn obtener_arr_mut(&mut self, idx: u32) -> &mut Vec<ValorFast>;

    fn reservar_mapa(&mut self, mapa: HashMap<String, ValorFast>) -> u32;
    fn obtener_mapa(&self, idx: u32) -> &HashMap<String, ValorFast>;
    fn obtener_mapa_mut(&mut self, idx: u32) -> &mut HashMap<String, ValorFast>;

    fn reservar_objeto(&mut self, obj: ObjVal) -> u32;
    fn obtener_objeto(&self, idx: u32) -> &ObjVal;
    fn obtener_objeto_mut(&mut self, idx: u32) -> &mut ObjVal;

    fn registro_formas(&mut self) -> &mut RegistroFormas;
    fn clase_descriptors(&mut self) -> &mut HashMap<IdSimbolo, DescriptorClase>;
    fn tabla_simbolos(&mut self) -> &mut TablaSimbolos;

    fn mostrar_valor(&self, v: &ValorFast) -> String;

    fn socket_reservar(&mut self, estado: SocketState) -> u32;
    fn socket_obtener(&self, idx: u32) -> &SocketState;
    fn socket_obtener_mut(&mut self, idx: u32) -> &mut SocketState;
    fn socket_cerrar(&mut self, idx: u32);

    fn canal_try_recibir(&mut self, idx: usize) -> Option<CanalValor>;
    fn valor_de_channel(&mut self, valor: CanalValor) -> ValorFast;

    fn error_tipo(&self, mensaje: String) -> ErrFast;

    fn sandbox(&mut self) -> &mut SandboxRed;
    fn sandbox_fs(&mut self) -> &mut SandboxFilesystem;
    fn sandbox_proc(&mut self) -> &mut SandboxProceso;

    fn arreglo_monticulo(&self) -> &Vec<Vec<ValorFast>>;
    fn mapa_monticulo(&self) -> &Vec<HashMap<String, ValorFast>>;
    fn formas_objeto_mut(&mut self) -> &mut Vec<IdForma>;
    fn clases_derive_igual(&mut self) -> &mut HashSet<IdSimbolo>;
    fn clases_derive_mostrar(&mut self) -> &mut HashSet<IdSimbolo>;

    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn sym_canal_receptor(&self) -> IdSimbolo;
}
