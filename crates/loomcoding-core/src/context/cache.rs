//! Caché del índice de símbolos, por raíz de proyecto y con huella del árbol.
//!
//! Cabe señalar que reconstruir el índice entero cuesta segundos en un repositorio grande, pero
//! mirar la fecha y el tamaño de cada fichero cuesta milisegundos: la huella decide si hace falta
//! reconstruir, de modo que una edición hecha durante el turno nunca devuelve líneas desfasadas.

use super::index::SymbolIndex;
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

/// Huella de los `.rs` de un árbol: cuántos son, la última fecha de modificación y su tamaño total.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    files: usize,
    newest: Option<SystemTime>,
    bytes: u64,
}

fn stamp(root: &Path) -> Stamp {
    let walker = WalkBuilder::new(root)
        .require_git(false)
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != "target" && n != ".git"
        })
        .build();
    let mut stamp = Stamp {
        files: 0,
        newest: None,
        bytes: 0,
    };
    for entry in walker.flatten() {
        if entry.path().extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        stamp.files += 1;
        stamp.bytes += meta.len();
        if let Ok(modified) = meta.modified() {
            stamp.newest = Some(stamp.newest.map_or(modified, |n| n.max(modified)));
        }
    }
    stamp
}

type Cache = Mutex<HashMap<PathBuf, (Stamp, Arc<SymbolIndex>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Índice vigente de `root`: el guardado si el árbol no ha cambiado, uno nuevo si sí.
pub fn index_for(root: &Path) -> Arc<SymbolIndex> {
    let now = stamp(root);
    let key = root.to_path_buf();
    if let Some((old, index)) = cache().lock().unwrap().get(&key) {
        if *old == now {
            return index.clone();
        }
    }
    // Se construye sin el cerrojo: es lo caro y no debe bloquear a otros proyectos.
    let index = Arc::new(SymbolIndex::build(root));
    cache().lock().unwrap().insert(key, (now, index.clone()));
    index
}
