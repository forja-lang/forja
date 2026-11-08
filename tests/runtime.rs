//! Tests agrupados: runtime.

#[path = "runtime/async_tests.rs"]
mod async_tests;
#[path = "runtime/concurrencia_stress.rs"]
mod concurrencia_stress;
#[path = "runtime/gc_estres_indexado.rs"]
mod gc_estres_indexado;
#[path = "runtime/gc_stress.rs"]
mod gc_stress;
#[path = "runtime/runtime_bugs.rs"]
mod runtime_bugs;
#[path = "runtime/vm_comprehensive_tests.rs"]
mod vm_comprehensive_tests;
