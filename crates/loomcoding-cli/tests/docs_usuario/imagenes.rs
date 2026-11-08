//! Guardia de las imágenes de `docs/user/img/`: cada imagen figura en `ORIGEN.md` con un volcado
//! de origen que existe, y cada página enlaza solo imágenes que existen. La tabla la escribe
//! `scripts/capturas/generar.ps1`; aquí solo se comprueba.

use crate::guardia::{destinos, lineas_prosa, normaliza, Fallo};
use std::fs;
use std::path::{Path, PathBuf};

/// Carpeta de las imágenes, relativa a la raíz del repositorio.
pub(crate) const CARPETA_IMG: &str = "docs/user/img";
/// Tabla de origen de las imágenes, relativa a la raíz del repositorio.
pub(crate) const ORIGEN_MD: &str = "docs/user/img/ORIGEN.md";
/// Extensiones que cuentan como imagen.
const EXTENSIONES: [&str; 4] = ["svg", "png", "jpg", "gif"];

/// Una fila de la tabla de `ORIGEN.md`: imagen, volcado de origen y línea (desde 1).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Origen {
    pub(crate) imagen: String,
    pub(crate) volcado: String,
    pub(crate) linea: usize,
}

/// Nombres de las imágenes de primer nivel de `docs/user/img/`, ordenados.
pub(crate) fn lista_imagenes(raiz: &Path) -> Vec<String> {
    let Ok(entradas) = fs::read_dir(raiz.join(CARPETA_IMG)) else {
        return Vec::new();
    };
    let mut nombres: Vec<String> = entradas
        .flatten()
        .filter(|e| e.path().is_file() && es_imagen(&e.path()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    nombres.sort();
    nombres
}

/// Filas de `ORIGEN.md`: la primera celda (con código) es la imagen y la segunda, el volcado.
pub(crate) fn origenes(texto: &str) -> Vec<Origen> {
    let mut filas = Vec::new();
    for (indice, linea) in texto.lines().enumerate() {
        let t = linea.trim();
        if !t.starts_with('|') {
            continue;
        }
        let celdas: Vec<&str> = t.split('|').collect();
        if celdas.len() < 3 {
            continue;
        }
        let (Some(imagen), Some(volcado)) = (primer_codigo(celdas[1]), primer_codigo(celdas[2]))
        else {
            continue;
        };
        if !es_imagen_nombre(imagen) {
            continue;
        }
        filas.push(Origen {
            imagen: imagen.to_string(),
            volcado: volcado.to_string(),
            linea: indice + 1,
        });
    }
    filas
}

/// Texto entre la primera pareja de comillas invertidas de una celda.
fn primer_codigo(celda: &str) -> Option<&str> {
    celda.split('`').nth(1).map(str::trim)
}

fn es_imagen(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONES.contains(&e))
}

fn es_imagen_nombre(nombre: &str) -> bool {
    es_imagen(Path::new(nombre))
}

fn existe_imagen(raiz: &Path, carpeta: &Path, destino: &str) -> bool {
    normaliza(&carpeta.join(destino)).is_some_and(|relativa| raiz.join(relativa).is_file())
}

/// Incumplimientos de la guardia de imágenes. `imagenes` son los nombres de `docs/user/img/` y
/// `paginas` los pares (ruta desde la raíz del repositorio, texto) de `docs/user/{en,es}`.
pub(crate) fn revisa_imagenes(
    raiz: &Path,
    origen_md: &str,
    imagenes: &[String],
    paginas: &[(String, String)],
) -> Vec<Fallo> {
    let filas = origenes(origen_md);
    let mut fallos = Vec::new();
    for imagen in imagenes {
        if !filas.iter().any(|o| &o.imagen == imagen) {
            fallos.push(Fallo::nuevo(
                &format!("{CARPETA_IMG}/{imagen}"),
                0,
                "no figura en ORIGEN.md",
            ));
        }
    }
    for fila in &filas {
        if !imagenes.contains(&fila.imagen) {
            fallos.push(Fallo::nuevo(
                ORIGEN_MD,
                fila.linea,
                format!("la imagen no existe: {}", fila.imagen),
            ));
        }
        if !raiz.join(&fila.volcado).is_file() {
            fallos.push(Fallo::nuevo(
                ORIGEN_MD,
                fila.linea,
                format!("volcado de origen inexistente: {}", fila.volcado),
            ));
        }
    }
    for (ruta, texto) in paginas {
        let carpeta = Path::new(ruta).parent().unwrap_or(Path::new(""));
        for (n, linea) in lineas_prosa(texto) {
            for destino in destinos(linea) {
                if es_imagen_nombre(&destino) && !existe_imagen(raiz, carpeta, &destino) {
                    fallos.push(Fallo::nuevo(
                        ruta,
                        n,
                        format!("imagen inexistente: {destino}"),
                    ));
                }
            }
        }
    }
    fallos
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruebas::{paginas, raiz_repo};

    const FILA: &str = "| Imagen | Volcado de origen | Bloque y líneas | Comando para regenerar |\n|---|---|---|---|\n| `a.svg` | `docs/mockups/x-e2e.md` | 1 | `generar` |\n";

    /// Árbol temporal con los volcados y las imágenes indicados (rutas relativas a la raíz).
    fn raiz_temporal(volcados: &[&str], imagenes: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for volcado in volcados {
            let ruta = dir.path().join(volcado);
            fs::create_dir_all(ruta.parent().unwrap()).unwrap();
            fs::write(ruta, "volcado").unwrap();
        }
        for imagen in imagenes {
            let ruta = dir.path().join(CARPETA_IMG).join(imagen);
            fs::create_dir_all(ruta.parent().unwrap()).unwrap();
            fs::write(ruta, "<svg/>").unwrap();
        }
        dir
    }

    fn motivos(fallos: &[Fallo]) -> String {
        fallos
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn origenes_lee_solo_las_filas_con_imagen() {
        assert_eq!(
            origenes(FILA),
            vec![Origen {
                imagen: "a.svg".to_string(),
                volcado: "docs/mockups/x-e2e.md".to_string(),
                linea: 3,
            }]
        );
    }

    #[test]
    fn imagen_sin_fila_en_origen_falla() {
        let dir = raiz_temporal(&[], &["a.svg"]);
        let imagenes = lista_imagenes(dir.path());
        let fallos = revisa_imagenes(dir.path(), "", &imagenes, &[]);
        assert!(
            fallos
                .iter()
                .any(|f| f.motivo.contains("no figura en ORIGEN.md")),
            "{}",
            motivos(&fallos)
        );
    }

    #[test]
    fn volcado_de_origen_inexistente_falla_con_su_linea() {
        let dir = raiz_temporal(&[], &["a.svg"]);
        let imagenes = lista_imagenes(dir.path());
        let fallos = revisa_imagenes(dir.path(), FILA, &imagenes, &[]);
        assert!(
            fallos
                .iter()
                .any(|f| f.linea == 3 && f.motivo.contains("volcado de origen inexistente")),
            "{}",
            motivos(&fallos)
        );
    }

    #[test]
    fn fila_cuya_imagen_no_existe_falla() {
        let dir = raiz_temporal(&["docs/mockups/x-e2e.md"], &[]);
        let fallos = revisa_imagenes(dir.path(), FILA, &[], &[]);
        assert!(
            fallos
                .iter()
                .any(|f| f.linea == 3 && f.motivo.contains("la imagen no existe")),
            "{}",
            motivos(&fallos)
        );
    }

    #[test]
    fn origen_completo_no_produce_fallos() {
        let dir = raiz_temporal(&["docs/mockups/x-e2e.md"], &["a.svg"]);
        let imagenes = lista_imagenes(dir.path());
        let fallos = revisa_imagenes(dir.path(), FILA, &imagenes, &[]);
        assert!(fallos.is_empty(), "{}", motivos(&fallos));
    }

    #[test]
    fn pagina_que_enlaza_imagen_inexistente_falla_con_su_linea() {
        let dir = raiz_temporal(&["docs/mockups/x-e2e.md"], &["a.svg"]);
        let imagenes = lista_imagenes(dir.path());
        let texto = "# Título\n\n![Falta](../../img/falta.svg)\n";
        let paginas = vec![("docs/user/en/start/p.md".to_string(), texto.to_string())];
        let fallos = revisa_imagenes(dir.path(), FILA, &imagenes, &paginas);
        assert!(
            fallos
                .iter()
                .any(|f| f.linea == 3
                    && f.motivo.contains("imagen inexistente: ../../img/falta.svg")),
            "{}",
            motivos(&fallos)
        );
    }

    #[test]
    fn pagina_que_enlaza_imagen_existente_no_falla() {
        let dir = raiz_temporal(&["docs/mockups/x-e2e.md"], &["a.svg"]);
        let imagenes = lista_imagenes(dir.path());
        let texto = "# Título\n\n![Ok](../../img/a.svg)\n";
        let paginas = vec![("docs/user/en/start/p.md".to_string(), texto.to_string())];
        let fallos = revisa_imagenes(dir.path(), FILA, &imagenes, &paginas);
        assert!(fallos.is_empty(), "{}", motivos(&fallos));
    }

    #[test]
    fn imagenes_del_arbol_real_tienen_origen_y_las_paginas_las_enlazan() {
        let raiz = raiz_repo();
        let origen = fs::read_to_string(raiz.join(ORIGEN_MD))
            .unwrap_or_else(|e| panic!("falta {ORIGEN_MD}: {e}"));
        let imagenes = lista_imagenes(&raiz);
        assert!(!imagenes.is_empty(), "no hay imágenes en {CARPETA_IMG}");
        let mut todas: Vec<(String, String)> = Vec::new();
        for carpeta in ["en", "es"] {
            let dir: PathBuf = raiz.join("docs").join("user").join(carpeta);
            for (relativa, texto) in paginas(&dir) {
                todas.push((format!("docs/user/{carpeta}/{relativa}"), texto));
            }
        }
        let fallos = revisa_imagenes(&raiz, &origen, &imagenes, &todas);
        assert!(
            fallos.is_empty(),
            "la guardia de imágenes encontró {} fallo(s):\n{}",
            fallos.len(),
            motivos(&fallos)
        );
    }
}
