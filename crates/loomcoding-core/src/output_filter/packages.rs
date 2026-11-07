//! Regla `packages` para `npm install` y `pnpm install` (R3, R5, R6). Quita tres clases de ruido: las
//! barras de progreso de pnpm (`+++`), las líneas `Progress:` salvo la última (cada una sustituye a
//! la anterior) y los avisos de deprecación con el mismo paquete y el mismo mensaje, que se cuentan
//! en la primera aparición con «×N». Un aviso o un error con marca de señal nunca se toca.

use super::comando_simple;
use super::general::{es_senal, Compactado};
use std::collections::{HashMap, HashSet};

/// Ejecutables y subcomandos a los que se aplica la regla.
const EJECUTABLES: [&str; 2] = ["npm", "pnpm"];
const SUBCOMANDOS: [&str; 2] = ["install", "i"];

/// Prefijos que confirman el formato reconocido. Sin ninguno, la salida pasa sin tocar (R6).
const RECONOCIDAS: [&str; 7] = [
    "added ",
    "up to date",
    "npm warn deprecated ",
    "[WARN] deprecated ",
    "Progress: ",
    "Packages: +",
    "Done in ",
];

/// Prefijos de los avisos de deprecación de npm y de pnpm.
const DEPRECACION: [&str; 2] = ["npm warn deprecated ", "[WARN] deprecated "];

/// Línea de progreso de pnpm, que se reescribe con cada paso.
const PROGRESO: &str = "Progress: ";

/// Decide si el comando es `npm|pnpm install` (o `i`) como primer ejecutable, sin encadenados.
pub fn aplica(comando: &str) -> bool {
    matches!(
        comando_simple::tokens(comando).as_deref(),
        Some([ejecutable, subcomando, ..])
            if EJECUTABLES.contains(ejecutable) && SUBCOMANDOS.contains(subcomando)
    )
}

/// Compacta la salida del gestor. Devuelve `None` si el formato no se reconoce o si no hay nada que
/// omitir.
pub fn compactar(texto: &str) -> Option<Compactado> {
    if !reconocible(texto) {
        return None;
    }
    let lineas: Vec<&str> = texto.lines().collect();
    let ultimo_progreso = lineas.iter().rposition(|linea| linea.starts_with(PROGRESO));

    // Cuántas veces aparece cada aviso de deprecación, para anotar su recuento en la primera.
    let mut apariciones: HashMap<String, usize> = HashMap::new();
    for clave in lineas
        .iter()
        .filter_map(|linea| clave_de_deprecacion(linea))
    {
        *apariciones.entry(clave).or_insert(0) += 1;
    }

    let mut vistas: HashSet<String> = HashSet::new();
    let mut omitidas = 0usize;
    let mut conservadas: Vec<String> = Vec::with_capacity(lineas.len());
    for (indice, linea) in lineas.iter().enumerate() {
        // R3: una línea con marca de señal se conserva entera y nunca entra en un grupo.
        if es_senal(linea) {
            conservadas.push((*linea).to_owned());
            continue;
        }
        let progreso_superado = linea.starts_with(PROGRESO) && Some(indice) != ultimo_progreso;
        if es_barra(linea) || progreso_superado {
            omitidas += 1;
            continue;
        }
        match clave_de_deprecacion(linea) {
            Some(clave) if !vistas.insert(clave.clone()) => omitidas += 1,
            Some(clave) => {
                let veces = apariciones.get(&clave).copied().unwrap_or(1);
                if veces > 1 {
                    conservadas.push(format!("{linea} ×{veces}"));
                } else {
                    conservadas.push((*linea).to_owned());
                }
            }
            None => conservadas.push((*linea).to_owned()),
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

/// Barra de progreso de pnpm: una línea hecha solo de `+` o de `-`, sin marca de señal (R3).
fn es_barra(linea: &str) -> bool {
    let recortada = linea.trim();
    recortada.len() >= 3 && recortada.chars().all(|c| c == '+' || c == '-')
}

/// Clave de un aviso de deprecación: nombre del paquete y mensaje, sin la versión. Dos avisos con la
/// misma clave tienen la misma marca de señal, de modo que el bucle de `compactar` los trata igual.
fn clave_de_deprecacion(linea: &str) -> Option<String> {
    let resto = DEPRECACION
        .iter()
        .find_map(|prefijo| linea.strip_prefix(prefijo))?;
    let (paquete, mensaje) = resto.split_once(": ")?;
    let nombre = paquete
        .rsplit_once('@')
        .map_or(paquete, |(nombre, _version)| nombre);
    Some(format!("{nombre}: {mensaje}"))
}

/// El formato es reconocible si aparece algún prefijo conocido de npm o de pnpm (R6).
fn reconocible(texto: &str) -> bool {
    texto
        .lines()
        .any(|linea| RECONOCIDAS.iter().any(|prefijo| linea.starts_with(prefijo)))
}
