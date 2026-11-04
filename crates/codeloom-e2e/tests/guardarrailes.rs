//! Guardarraíles automáticos del repositorio (Fase 1 del plan de organización).
//!
//! Cabe señalar que estos tests no ejercitan el producto: vigilan la forma del código y de la
//! documentación para que no vuelva a degradarse. Solo usan `std`; el raíz del repo se calcula
//! desde `CARGO_MANIFEST_DIR`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Techo duro de líneas por archivo fuente (decisión cerrada en el spec de organización).
const TECHO: usize = 1000;

/// Fecha de corte del check de cobertura del índice: los documentos de primer nivel con fecha
/// posterior a esta deben figurar en `docs/INDEX.md`.
const CORTE_INDICE: &str = "2026-11-05";

/// Lista blanca de archivos que superan el techo: ruta → (tope actual, justificación).
///
/// Reglas: **solo puede decrecer**. Un archivo que se parte o baja del techo se retira de aquí
/// (el test falla si queda una entrada obsoleta); un archivo no listado que supere el techo
/// falla; y uno listado no puede pasar de su tope (redondeado a la centena superior, que da
/// margen para tocar el archivo sin convertir cada commit en un fallo).
const LISTA_BLANCA: &[(&str, usize, &str)] = &[
    (
        "crates/batten/src/widgets/composer.rs",
        1700,
        "pendiente de evaluar en una oleada posterior",
    ),
    (
        "crates/batten/src/widgets/transcript.rs",
        1400,
        "pendiente de evaluar en una oleada posterior",
    ),
    (
        "crates/batten/src/widgets/theme.rs",
        1100,
        "excepción: tabla de datos del tema, cohesiva",
    ),
];

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("raíz del repositorio")
}

/// Todos los `.rs` bajo `dir`, recursivamente.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Directorios de compilación (`target`, `.verify-target`…): no son código fuente.
            let nombre = entry.file_name().to_string_lossy().to_string();
            if nombre == "target" || nombre.starts_with('.') {
                continue;
            }
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Líneas de cada `crates/*/src/**/*.rs`, con ruta relativa al raíz y barras `/`.
fn lineas_por_archivo() -> BTreeMap<String, usize> {
    let raiz = raiz();
    let mut mapa = BTreeMap::new();
    for crate_dir in fs::read_dir(raiz.join("crates")).unwrap().flatten() {
        let mut files = Vec::new();
        rust_files(&crate_dir.path().join("src"), &mut files);
        for file in files {
            let rel = file
                .strip_prefix(&raiz)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let lineas = fs::read_to_string(&file).unwrap().lines().count();
            mapa.insert(rel, lineas);
        }
    }
    mapa
}

#[test]
fn ningun_archivo_nuevo_supera_el_techo() {
    let lineas = lineas_por_archivo();
    let mut fallos = Vec::new();
    for (ruta, n) in &lineas {
        if *n <= TECHO {
            continue;
        }
        match LISTA_BLANCA.iter().find(|(r, _, _)| r == ruta) {
            None => fallos.push(format!(
                "{ruta}: {n} líneas, supera el techo de {TECHO} y no está en la lista blanca"
            )),
            Some((_, tope, _)) if n > tope => fallos.push(format!(
                "{ruta}: {n} líneas, supera su tope de {tope}; la lista blanca solo puede decrecer"
            )),
            Some(_) => {}
        }
    }
    assert!(fallos.is_empty(), "\n{}", fallos.join("\n"));
}

#[test]
fn la_lista_blanca_no_tiene_entradas_obsoletas() {
    let lineas = lineas_por_archivo();
    let mut fallos = Vec::new();
    for (ruta, _, _) in LISTA_BLANCA {
        match lineas.get(*ruta) {
            None => fallos.push(format!("{ruta}: ya no existe; retírala de la lista blanca")),
            Some(n) if *n <= TECHO => fallos.push(format!(
                "{ruta}: ya solo tiene {n} líneas; retírala de la lista blanca"
            )),
            Some(_) => {}
        }
    }
    assert!(fallos.is_empty(), "\n{}", fallos.join("\n"));
}

#[test]
fn core_no_depende_de_la_interfaz() {
    let raiz = raiz();
    let cargo = fs::read_to_string(raiz.join("crates/codeloom-core/Cargo.toml")).unwrap();
    for prohibido in ["ratatui", "crossterm"] {
        assert!(
            !cargo.contains(prohibido),
            "codeloom-core/Cargo.toml declara `{prohibido}`, prohibido por la regla de dependencias"
        );
    }
    let mut files = Vec::new();
    rust_files(&raiz.join("crates/codeloom-core/src"), &mut files);
    for file in files {
        let texto = fs::read_to_string(&file).unwrap();
        for prohibido in ["use ratatui", "use crossterm", "ratatui::", "crossterm::"] {
            assert!(
                !texto.contains(prohibido),
                "{} usa `{prohibido}`: codeloom-core no puede tocar la interfaz",
                file.display()
            );
        }
    }
}

/// Nombre corto de un documento: sin fecha ni prefijo `codeloom-`/`loom-`, sin `.md`.
fn nombre_corto(fichero: &str) -> String {
    let sin_fecha = fichero.get(11..).unwrap_or(fichero);
    let sin_ext = sin_fecha.trim_end_matches(".md");
    sin_ext
        .strip_prefix("codeloom-")
        .or_else(|| sin_ext.strip_prefix("loom-"))
        .unwrap_or(sin_ext)
        .to_string()
}

#[test]
fn los_documentos_nuevos_estan_en_el_indice() {
    let raiz = raiz();
    let indice = fs::read_to_string(raiz.join("docs/INDEX.md")).expect("docs/INDEX.md");
    let mut faltan = Vec::new();
    for carpeta in ["docs/specs", "docs/plans"] {
        for entry in fs::read_dir(raiz.join(carpeta)).unwrap().flatten() {
            let nombre = entry.file_name().to_string_lossy().to_string();
            // Solo ficheros `AAAA-MM-DD-*.md` posteriores al corte (comparación de texto ISO).
            let fecha = nombre.get(..10).unwrap_or("");
            if !nombre.ends_with(".md") || fecha <= CORTE_INDICE || fecha.len() < 10 {
                continue;
            }
            if !indice.contains(&nombre_corto(&nombre)) {
                faltan.push(format!("{carpeta}/{nombre}"));
            }
        }
    }
    assert!(
        faltan.is_empty(),
        "documentos posteriores a {CORTE_INDICE} sin entrada en docs/INDEX.md:\n{}",
        faltan.join("\n")
    );
}
