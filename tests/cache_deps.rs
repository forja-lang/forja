//! El grafo de imports de la caché invalida dependientes transitivos.

use forja_driver::IncrementalCache;
use std::path::PathBuf;

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("forja_cache_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn invalida_dependientes_transitivos() {
    let d = dir();
    let mut c = IncrementalCache::new(&d);
    c.put("a.fa", 1, &[], &[]);
    c.put("b.fa", 1, &[], &["a.fa".into()]);
    c.put("c.fa", 1, &[], &["b.fa".into()]);

    assert_eq!(c.dependientes_de("a.fa"), vec!["b.fa", "c.fa"]);
    c.invalidate("a.fa");
    assert_eq!(c.stats().total_entries, 0);
    let _ = std::fs::remove_dir_all(d);
}
