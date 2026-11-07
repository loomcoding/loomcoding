//! Reglas generales, válidas para cualquier salida de texto: secuencias ANSI, retornos de carro de
//! barras de progreso, líneas idénticas consecutivas y líneas en blanco consecutivas.

use regex::Regex;
use std::borrow::Cow;
use std::sync::OnceLock;

/// Texto ya compactado y número de líneas (o fragmentos sobrescritos) que se han dejado fuera.
pub struct Compactado {
    pub texto: String,
    pub omitidas: usize,
}

/// Marcas de fallo y de aviso que nunca se descartan (R3, propiedad central). En minúsculas.
const MARCAS: [&str; 4] = ["error", "failed", "panic", "warning:"];

/// Cabe señalar que basta una marca en cualquier parte de la línea para considerarla señal.
pub fn es_senal(linea: &str) -> bool {
    let minusculas = linea.to_ascii_lowercase();
    MARCAS.iter().any(|marca| minusculas.contains(marca))
}

/// Secuencias de control ANSI: CSI, OSC y escapes de dos caracteres.
fn ansi() -> &'static Regex {
    static PATRON: OnceLock<Regex> = OnceLock::new();
    PATRON.get_or_init(|| {
        Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-Z\\-_]")
            .expect("patrón ANSI válido")
    })
}

/// Aplica las reglas generales. Devuelve `None` si no hay nada que compactar, de modo que la salida
/// original pase sin tocar (R6). Las líneas con señal conservan su texto (R3).
pub fn compactar(texto: &str) -> Option<Compactado> {
    let sin_ansi = ansi().replace_all(texto, "");
    let ansi_quitado = matches!(sin_ansi, Cow::Owned(_));

    let mut omitidas = 0usize;
    // Cada grupo es una línea distinta seguida de cuántas veces aparece de forma consecutiva.
    let mut grupos: Vec<(String, usize)> = Vec::new();
    for linea in sin_ansi.lines() {
        let linea = progreso(linea, &mut omitidas);
        let linea = if linea.trim().is_empty() {
            String::new()
        } else {
            linea
        };
        match grupos.last_mut() {
            Some((previa, veces)) if *previa == linea => {
                *veces += 1;
                omitidas += 1;
            }
            _ => grupos.push((linea, 1)),
        }
    }
    if omitidas == 0 && !ansi_quitado {
        return None;
    }

    // R5: cada colapso lleva su recuento. Un bloque de líneas en blanco se reduce a una sola, sin marca.
    let lineas: Vec<String> = grupos
        .into_iter()
        .map(|(linea, veces)| {
            if veces > 1 && !linea.is_empty() {
                format!("{linea} ×{veces}")
            } else {
                linea
            }
        })
        .collect();
    let mut texto_final = lineas.join("\n");
    if texto.ends_with('\n') {
        texto_final.push('\n');
    }
    Some(Compactado {
        texto: texto_final,
        omitidas,
    })
}

/// Una barra de progreso reescribe la línea con `\r`: se conserva el último estado no vacío. Una línea
/// con señal se devuelve entera, porque un fallo no puede perderse por sobrescribirse.
fn progreso(linea: &str, omitidas: &mut usize) -> String {
    if !linea.contains('\r') || es_senal(linea) {
        return linea.to_owned();
    }
    let segmentos: Vec<&str> = linea.split('\r').collect();
    *omitidas += segmentos.len() - 1;
    segmentos
        .iter()
        .rev()
        .find(|segmento| !segmento.is_empty())
        .map_or_else(String::new, |segmento| (*segmento).to_owned())
}
