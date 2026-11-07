//! Regla `cargo` para `cargo test`, `build`, `check` y `clippy` (R3, R6). Quita solo el ruido que se
//! conoce (compilación en curso, descargas, `test ... ok`, los `Finished` anteriores al último) y
//! conserva todo lo demás: errores, avisos, fallos, pánicos, contexto y resumen. Una línea que no
//! reconoce se queda. Si el formato no es el inglés de siempre, o el comando encadena o redirige,
//! no hay regla y la salida pasa sin tocar.

use super::comando_simple;
use super::general::{es_senal, Compactado};

/// Subcomandos de `cargo` a los que se aplica la regla.
const SUBCOMANDOS: [&str; 4] = ["test", "build", "check", "clippy"];

/// Líneas de progreso que se omiten por completo. Ninguna lleva marca de fallo (R3).
const PROGRESO: [&str; 4] = ["Compiling ", "Checking ", "Downloading ", "Downloaded "];

/// Prefijos de las líneas en inglés que confirman que el formato es el reconocido (R6).
const RECONOCIDAS: [&str; 8] = [
    "Compiling ",
    "Checking ",
    "Finished ",
    "Running ",
    "Downloading ",
    "Downloaded ",
    "Doc-tests ",
    "running ",
];

/// Decide si el comando es `cargo test|build|check|clippy` como primer ejecutable, sin tuberías, `&&`
/// ni redirecciones. La redirección `2>&1` se tolera, porque no encadena nada.
pub fn aplica(comando: &str) -> bool {
    matches!(
        comando_simple::tokens(comando).as_deref(),
        Some(["cargo", sub, ..]) if SUBCOMANDOS.contains(sub)
    )
}

/// Compacta la salida de `cargo` aplicando la lista de ruido. Devuelve `None` si el formato no se
/// reconoce o si no hay nada que omitir.
pub fn compactar(texto: &str) -> Option<Compactado> {
    if !reconocible(texto) {
        return None;
    }
    let lineas: Vec<&str> = texto.lines().collect();
    // Solo se conserva el último `Finished`: los anteriores son repeticiones de otras compilaciones.
    let ultimo_fin = lineas
        .iter()
        .rposition(|linea| linea.trim_start().starts_with("Finished "));

    let mut omitidas = 0usize;
    let mut conservadas: Vec<&str> = Vec::with_capacity(lineas.len());
    for (indice, linea) in lineas.iter().enumerate() {
        if es_ruido(linea, Some(indice) == ultimo_fin) {
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

/// Una línea es ruido si está en la lista y no lleva ninguna marca de señal. Nunca se descarta una
/// línea con error, fallo, pánico o aviso (R3, propiedad central).
fn es_ruido(linea: &str, es_ultimo_fin: bool) -> bool {
    if es_senal(linea) {
        return false;
    }
    // Cargo alinea a la derecha el verbo de estado: `   Compiling x` y `    Finished x`.
    let recortada = linea.trim_start();
    if recortada.starts_with("Finished ") {
        return !es_ultimo_fin;
    }
    PROGRESO
        .iter()
        .any(|prefijo| recortada.starts_with(prefijo))
        || es_test_ok(linea)
}

/// Resultado de un test que sale bien: `test nombre ... ok`.
fn es_test_ok(linea: &str) -> bool {
    linea.starts_with("test ") && linea.ends_with(" ... ok")
}

/// El formato es reconocible si aparece alguna línea en inglés conocida y ninguna línea JSON de
/// `--message-format json` (R6).
fn reconocible(texto: &str) -> bool {
    let mut reconoce = false;
    for linea in texto.lines() {
        if linea.trim_start().starts_with("{\"reason\"") {
            return false;
        }
        let recortada = linea.trim_start();
        let test_con_resultado = recortada.starts_with("test ") && recortada.contains(" ... ");
        if test_con_resultado
            || RECONOCIDAS
                .iter()
                .any(|prefijo| recortada.starts_with(prefijo))
        {
            reconoce = true;
        }
    }
    reconoce
}
