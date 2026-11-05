//! Ordenación de símbolos por importancia (PageRank personalizado).
//!
//! `rank` devuelve todos los nombres con definición, del más importante al
//! menos, con su puntuación. El grafo se construye con las llamadas reales:
//! para cada `callee` con definición, cada referencia es una llamada hecha
//! desde un símbolo "llamador" (el definido en el mismo fichero cuya línea de
//! definición sea la mayor que no pase de la línea de la referencia).

use std::collections::HashMap;
use std::path::Path;

use crate::context::SymbolIndex;

/// Amortiguación estándar de PageRank.
const DAMPING: f64 = 0.85;
/// Número de vueltas de la iteración.
const ITERATIONS: usize = 30;

/// Ordena los nombres con definición por importancia.
///
/// Devuelve todos los nombres con definición, del más importante al menos,
/// con su puntuación.
pub fn rank(index: &SymbolIndex, seeds: &[String]) -> Vec<(String, f64)> {
    // Nodos: nombres con definición, ordenados para un resultado estable.
    let mut nodes: Vec<String> = index.names().map(String::from).collect();
    nodes.sort();
    nodes.dedup();
    let n = nodes.len();
    if n == 0 {
        return Vec::new();
    }

    // Posición de cada nombre entre los nodos.
    let node_index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, s): (usize, &String)| (s.as_str(), i))
        .collect();

    // Precomputamos, por fichero, las líneas de definición ordenadas junto al
    // índice de nodo. Permite localizar el llamador de una referencia con una
    // búsqueda binaria en vez de recorrer todos los nombres.
    let mut defs_by_path: HashMap<&Path, Vec<(u32, usize)>> = HashMap::new();
    for name in index.names() {
        if let Some(&i) = node_index.get(name) {
            for sym in index.definitions(name) {
                defs_by_path
                    .entry(sym.path.as_ref())
                    .or_default()
                    .push((sym.line, i));
            }
        }
    }
    for v in defs_by_path.values_mut() {
        v.sort_by_key(|&(line, _)| line);
    }

    // Aristas: llamador -> callee, con peso = cuenta de llamadas iguales.
    let mut out_edges: Vec<Vec<(usize, u64)>> = vec![Vec::new(); n];
    let mut out_weight: Vec<f64> = vec![0.0; n];

    for callee in index.names() {
        let callee_idx = match node_index.get(callee) {
            Some(i) => *i,
            // Sin definición no es nodo; no aporta aristas salientes.
            None => continue,
        };
        for r in index.references(callee) {
            if let Some(callers_idx) = caller_of(&defs_by_path, r.path.as_ref(), r.line) {
                // Ignorar las auto-llamadas.
                if callers_idx == callee_idx {
                    continue;
                }
                out_edges[callers_idx].push((callee_idx, 1));
            }
        }
    }
    for u in 0..n {
        out_weight[u] = out_edges[u].iter().map(|(_, w)| *w as f64).sum();
    }

    let custom = customization(seeds, &nodes);

    // Iteración de PageRank con nodos colgantes.
    let mut score: Vec<f64> = vec![1.0 / n as f64; n];
    for _ in 0..ITERATIONS {
        let dangling: f64 = (0..n)
            .filter(|i| out_weight[*i] == 0.0)
            .map(|i| score[i])
            .sum();
        let mut contributions = vec![0.0; n];
        for u in 0..n {
            if out_weight[u] == 0.0 {
                continue;
            }
            for &(v, w) in &out_edges[u] {
                contributions[v] += score[u] * (w as f64) / out_weight[u];
            }
        }
        for v in 0..n {
            score[v] =
                (1.0 - DAMPING) / n as f64 + DAMPING * (contributions[v] + dangling * custom[v]);
        }
    }

    // Orden: puntuación descendente; empates por nombre ascendente (estable).
    let mut result: Vec<(String, f64)> = nodes
        .iter()
        .enumerate()
        .map(|(i, name): (usize, &String)| (name.clone(), score[i]))
        .collect();
    result.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    result
}

/// Busca el símbolo "llamador" de una referencia: el definido en el mismo
/// fichero (`path`) cuya línea de definición sea la mayor que no pase de
/// `line`. Devuelve su índice de nodo, o `None` si no hay ninguno.
///
/// `defs_by_path` ya viene ordenado por línea, así que la búsqueda se hace con
/// una búsqueda binaria en vez de recorrer todos los nombres.
fn caller_of(
    defs_by_path: &HashMap<&Path, Vec<(u32, usize)>>,
    path: &Path,
    line: u32,
) -> Option<usize> {
    let defs = defs_by_path.get(path)?;
    // Definiciones con línea <= `line`, ordenadas de menor a mayor.
    let idx = defs.partition_point(|&(l, _)| l <= line);
    if idx == 0 {
        return None;
    }
    Some(defs[idx - 1].1)
}

/// Vector de personalización: uniforme sobre las semillas que existen como
/// nombre con definición; si ninguna existe o `seeds` está vacío, uniforme
/// sobre todos los nodos.
fn customization(seeds: &[String], nodes: &[String]) -> Vec<f64> {
    let custom: Vec<String> = seeds
        .iter()
        .filter(|s| nodes.contains(s))
        .cloned()
        .collect();
    if custom.is_empty() {
        return vec![1.0 / nodes.len() as f64; nodes.len()];
    }
    let share = 1.0 / custom.len() as f64;
    nodes
        .iter()
        .map(|n| if custom.contains(n) { share } else { 0.0 })
        .collect()
}
