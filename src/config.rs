//! Reexport de la configuración movida a `forja-session`.
//!
//! Se mantiene `forja::config::*` para no romper CLI, REPL ni tests.
pub use forja_session::*;
