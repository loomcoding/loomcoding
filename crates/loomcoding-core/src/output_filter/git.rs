//! Regla `git` para `git status` en formato largo (R3, R6). Quita las pistas `(use "git ...")` que
//! el comando repite bajo cada sección y que no aportan nada al modelo; conserva todo lo demás: las
//! rutas, las secciones y el resumen final. `git log` no tiene regla propia, porque en su formato
//! largo no hay línea que R3 permita omitir. Un `git diff` es salida estructurada: solo reglas
//! generales.

use super::comando_simple;
use super::general::{es_senal, Compactado};

/// Cabeceras del formato largo de `git status`. Sin ninguna, el formato no se reconoce (R6).
const RECONOCIDAS: [&str; 2] = ["On branch ", "HEAD detached at "];

/// Inicio de las pistas que `git status` imprime bajo cada sección.
const PISTA: &str = "(use \"git ";

/// Decide si el comando es `git status` como primer ejecutable y subcomando, sin encadenados.
pub fn aplica(comando: &str) -> bool {
    matches!(
        comando_simple::tokens(comando).as_deref(),
        Some(["git", "status", ..])
    )
}

/// Quita las pistas de `git status`. Devuelve `None` si el formato no se reconoce o si no hay
/// ninguna pista que omitir.
pub fn compactar(texto: &str) -> Option<Compactado> {
    if !reconocible(texto) {
        return None;
    }
    let mut omitidas = 0usize;
    let mut conservadas: Vec<&str> = Vec::with_capacity(texto.lines().count());
    for linea in texto.lines() {
        if es_pista(linea) {
            omitidas += 1;
        } else {
            conservadas.push(linea);
        }
    }
    if omitidas == 0 {
        return None;
    }

    let mut texto_final = conservadas.join("\n");
    if texto.ends_with('\n') {
        texto_final.push('\n');
    }
    Some(Compactado {
        texto: texto_final,
        omitidas,
    })
}

/// Una pista es ruido si empieza como tal y no lleva ninguna marca de señal (R3). Una pista con
/// «failed» o «error» en su texto se conserva.
fn es_pista(linea: &str) -> bool {
    !es_senal(linea) && linea.trim_start().starts_with(PISTA)
}

/// El formato es reconocible si aparece alguna cabecera del formato largo en inglés (R6).
fn reconocible(texto: &str) -> bool {
    texto
        .lines()
        .any(|linea| RECONOCIDAS.iter().any(|prefijo| linea.starts_with(prefijo)))
}
