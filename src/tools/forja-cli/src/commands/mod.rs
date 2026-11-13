//! Comandos de la CLI. Se irán separando por dominio.

use super::*;

mod aot;
pub(crate) use aot::*;
mod ayuda;
pub(crate) use ayuda::*;
mod highlight;
pub(crate) use highlight::*;
mod learn;
pub(crate) use learn::*;
mod paquetes;
mod web;
pub(crate) use paquetes::*;
pub(crate) use web::*;
mod asm;
pub(crate) use asm::*;
mod bench;
pub(crate) use bench::*;
mod pruebas;
pub(crate) use pruebas::*;
mod ejecucion;
pub(crate) use ejecucion::*;
mod formato;
pub(crate) use formato::*;
mod consola;
pub(crate) use consola::*;
