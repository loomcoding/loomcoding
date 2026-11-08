//! Pruebas de la guardia de `guardia.rs` y recorrido del árbol real de `docs/user`.

use crate::guardia::*;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

/// Páginas `.md` de un idioma (ruta relativa y texto), sin los ficheros que empiezan por `_`.
pub(crate) fn paginas(dir: &Path) -> Vec<(String, String)> {
    let mut salida = Vec::new();
    recoge(dir, dir, &mut salida);
    salida.sort();
    salida
}

fn recoge(base: &Path, dir: &Path, salida: &mut Vec<(String, String)>) {
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if entrada.file_name().to_string_lossy().starts_with('_') {
            continue;
        }
        if ruta.is_dir() {
            recoge(base, &ruta, salida);
        } else if ruta.extension() == Some(OsStr::new("md")) {
            let relativa = ruta
                .strip_prefix(base)
                .unwrap_or(&ruta)
                .to_string_lossy()
                .replace('\\', "/");
            let texto = fs::read_to_string(&ruta)
                .unwrap_or_else(|e| panic!("no se puede leer {}: {e}", ruta.display()));
            salida.push((relativa, texto));
        }
    }
}

/// Raíz del repositorio, subiendo desde `crates/loomcoding-cli`.
pub(crate) fn raiz_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("la raíz del repositorio existe")
        .to_path_buf()
}

const RUTA: &str = "docs/user/en/start/index.md";

/// Página de prueba con la apertura del idioma indicado y un cuerpo añadido.
fn pagina(idioma: Idioma, cuerpo: &str) -> String {
    let [a, b, c] = cabecera(idioma);
    format!("# Título\n\n{a}\n\nUn ejemplo corto.\n\n{b}\n\nPara empezar.\n\n{c}\n\nUn comando.\n\n{cuerpo}")
}

fn linea_de(texto: &str, marcador: &str) -> usize {
    texto
        .lines()
        .position(|l| l.contains(marcador))
        .expect("el marcador existe en el texto")
        + 1
}

fn motivos(fallos: &[Fallo]) -> String {
    fallos
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn pagina_buena_no_produce_fallos() {
    for idioma in [Idioma::En, Idioma::Es] {
        let fallos = revisa(
            RUTA,
            &pagina(idioma, "## Uso\n\n- Un punto.\n- Otro punto.\n"),
            idioma,
        );
        assert!(fallos.is_empty(), "{}", motivos(&fallos));
    }
}

#[test]
fn parrafo_de_cinco_lineas_falla_con_su_linea() {
    let texto = pagina(Idioma::En, "## Uso\n\nUno\ndos\ntres\ncuatro\ncinco\n");
    let fallos = revisa(RUTA, &texto, Idioma::En);
    let esperada = linea_de(&texto, "Uno");
    assert!(
        fallos
            .iter()
            .any(|f| f.linea == esperada && f.motivo.contains("párrafo de 5 líneas")),
        "{}",
        motivos(&fallos)
    );
}

#[test]
fn bloques_de_codigo_tablas_listas_y_citas_no_son_parrafo() {
    let cuerpo = "## Uso\n\n```text\na\nb\nc\nd\ne\nf\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |\n\n- uno\n  dos\n  tres\n  cuatro\n  cinco\n\n> c1\n> c2\n> c3\n> c4\n> c5\n";
    let fallos = revisa(RUTA, &pagina(Idioma::En, cuerpo), Idioma::En);
    assert!(fallos.is_empty(), "{}", motivos(&fallos));
}

#[test]
fn dato_personal_de_windows_falla() {
    let texto = pagina(
        Idioma::En,
        "Ruta: C:\\Users\\juan\\x\n\nOtra: C:\\\\Users\\\\juan\n",
    );
    let fallos = revisa(RUTA, &texto, Idioma::En);
    let personales = fallos
        .iter()
        .filter(|f| f.motivo.starts_with("dato personal"))
        .count();
    assert_eq!(personales, 2, "{}", motivos(&fallos));
}

#[test]
fn dato_personal_de_linux_y_mac_falla_con_su_linea() {
    let texto = pagina(Idioma::En, "Ruta: /home/juan/.loom y /Users/juan/.loom\n");
    let fallos = revisa(RUTA, &texto, Idioma::En);
    let esperada = linea_de(&texto, "/home/juan");
    assert!(
        fallos
            .iter()
            .any(|f| f.linea == esperada && f.motivo.starts_with("dato personal")),
        "{}",
        motivos(&fallos)
    );
}

#[test]
fn marcadores_de_usuario_pasan() {
    let texto = pagina(
        Idioma::En,
        "Ruta: C:\\Users\\<user>\\x, /home/<usuario>/.loom y /Users/<user>/.loom\n",
    );
    let fallos = revisa(RUTA, &texto, Idioma::En);
    assert!(fallos.is_empty(), "{}", motivos(&fallos));
}

#[test]
fn correo_real_falla_y_example_invalid_pasa() {
    let malo = pagina(Idioma::En, "Escribe a juan.perez@gmail.com.\n");
    let bueno = pagina(Idioma::En, "Escribe a usuario@example.invalid.\n");
    assert!(revisa(RUTA, &malo, Idioma::En)
        .iter()
        .any(|f| f.motivo.contains("correo")));
    let fallos = revisa(RUTA, &bueno, Idioma::En);
    assert!(fallos.is_empty(), "{}", motivos(&fallos));
}

#[test]
fn apertura_incompleta_falla() {
    let texto = "# Título\n\n**What it is**\n\nUno.\n\n**How**\n\nDos.\n";
    let fallos = revisa(RUTA, texto, Idioma::En);
    assert!(
        fallos
            .iter()
            .any(|f| f.motivo.contains("**What it is for**")),
        "{}",
        motivos(&fallos)
    );
}

#[test]
fn apertura_en_otro_idioma_falla() {
    let texto = pagina(Idioma::Es, "## Uso\n\nTexto.\n");
    let fallos = revisa(RUTA, &texto, Idioma::En);
    assert!(
        fallos.iter().any(|f| f.motivo.contains("**What it is**")),
        "{}",
        motivos(&fallos)
    );
}

#[test]
fn apertura_despues_de_una_seccion_falla() {
    let texto = "# Título\n\n## Uso\n\n**What it is**\n\nUno.\n\n**What it is for**\n\nDos.\n\n**How**\n\nTres.\n";
    let fallos = revisa(RUTA, texto, Idioma::En);
    assert!(
        fallos
            .iter()
            .any(|f| f.motivo.contains("antes de la primera sección")),
        "{}",
        motivos(&fallos)
    );
}

#[test]
fn imagen_sin_texto_alternativo_falla() {
    let malo = pagina(Idioma::En, "![](img/pantalla.svg)\n");
    let bueno = pagina(Idioma::En, "![Pantalla principal](img/pantalla.svg)\n");
    assert!(revisa(RUTA, &malo, Idioma::En)
        .iter()
        .any(|f| f.motivo.contains("texto alternativo")));
    let fallos = revisa(RUTA, &bueno, Idioma::En);
    assert!(fallos.is_empty(), "{}", motivos(&fallos));
}

#[test]
fn enlace_relativo_roto_falla_y_los_validos_pasan() {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    fs::create_dir_all(raiz.join("docs/user/en/start")).unwrap();
    fs::write(raiz.join("AGENTS.md"), "x").unwrap();
    fs::write(raiz.join("docs/user/en/start/otra.md"), "x").unwrap();
    let texto = "[ok](otra.md) [raiz](../../../../AGENTS.md) [web](https://example.org/a)\n\n[ancla](#seccion) [roto](falta.md)\n\n```text\n[dentro](nope.md)\n```\n";
    let fallos = enlaces_rotos(raiz, RUTA, texto);
    assert_eq!(fallos.len(), 1, "{}", motivos(&fallos));
    assert_eq!(fallos[0].linea, linea_de(texto, "[roto]"));
    assert!(fallos[0].motivo.contains("falta.md"));
}

#[test]
fn paridad_detecta_paginas_ausentes_y_secciones_distintas() {
    let en = vec![("start/index.md".to_string(), "## A\n\n## B\n".to_string())];
    let sin_es: Vec<(String, String)> = Vec::new();
    assert!(revisa_paridad(&en, &sin_es)
        .iter()
        .any(|f| f.motivo.contains("falta su versión en es/")));

    let es_corto = vec![("start/index.md".to_string(), "## A\n".to_string())];
    assert!(revisa_paridad(&en, &es_corto)
        .iter()
        .any(|f| f.motivo.contains("secciones")));

    let es_igual = vec![("start/index.md".to_string(), "## A\n\n## B\n".to_string())];
    let fallos = revisa_paridad(&en, &es_igual);
    assert!(fallos.is_empty(), "{}", motivos(&fallos));

    let es_extra = vec![
        ("start/index.md".to_string(), "## A\n\n## B\n".to_string()),
        ("otra.md".to_string(), "## A\n".to_string()),
    ];
    assert!(revisa_paridad(&en, &es_extra)
        .iter()
        .any(|f| f.motivo.contains("no existe en en/")));
}

#[test]
fn docs_usuario_cumple_la_guardia() {
    let raiz = raiz_repo();
    let user = raiz.join("docs").join("user");
    let en = paginas(&user.join("en"));
    let es = paginas(&user.join("es"));
    assert!(
        en.iter().any(|(r, _)| r == "start/index.md")
            && es.iter().any(|(r, _)| r == "start/index.md"),
        "faltan las páginas de ejemplo docs/user/{{en,es}}/start/index.md"
    );

    let mut fallos = Vec::new();
    for (idioma, lista, carpeta) in [(Idioma::En, &en, "en"), (Idioma::Es, &es, "es")] {
        for (relativa, texto) in lista.iter() {
            let ruta = format!("docs/user/{carpeta}/{relativa}");
            fallos.extend(revisa(&ruta, texto, idioma));
            fallos.extend(enlaces_rotos(&raiz, &ruta, texto));
        }
    }
    fallos.extend(revisa_paridad(&en, &es));
    assert!(
        fallos.is_empty(),
        "la guardia de docs/user encontró {} fallo(s):\n{}",
        fallos.len(),
        motivos(&fallos)
    );
}
