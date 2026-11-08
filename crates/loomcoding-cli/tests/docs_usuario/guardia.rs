//! Guardia de la documentación de usuario (`docs/user/{en,es}/**/*.md`).
//!
//! `revisa` comprueba cada página sin tocar el disco: párrafos de como mucho cuatro líneas,
//! ausencia de datos personales, apertura trilingüe y texto alternativo en las imágenes.
//! `enlaces_rotos` valida los enlaces relativos contra el repositorio y `revisa_paridad`
//! exige que cada página exista en los dos idiomas. Las pruebas están en `pruebas.rs`.

use std::path::{Component, Path, PathBuf};

/// Número máximo de líneas de un párrafo de prosa.
const MAX_LINEAS_PARRAFO: usize = 4;

/// Idioma de una página; decide la apertura obligatoria.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Idioma {
    En,
    Es,
}

/// Encabezados de apertura obligatorios, en orden: *Qué es · Para qué · Cómo*.
pub(crate) fn cabecera(idioma: Idioma) -> [&'static str; 3] {
    match idioma {
        Idioma::En => ["**What it is**", "**What it is for**", "**How**"],
        Idioma::Es => ["**Qué es**", "**Para qué**", "**Cómo**"],
    }
}

/// Un incumplimiento de la guardia: fichero, línea (0 si afecta al fichero entero) y motivo.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Fallo {
    pub(crate) ruta: String,
    pub(crate) linea: usize,
    pub(crate) motivo: String,
}

impl Fallo {
    pub(crate) fn nuevo(ruta: &str, linea: usize, motivo: impl Into<String>) -> Self {
        Self {
            ruta: ruta.to_string(),
            linea,
            motivo: motivo.into(),
        }
    }
}

impl std::fmt::Display for Fallo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.linea == 0 {
            write!(f, "{}: {}", self.ruta, self.motivo)
        } else {
            write!(f, "{}:{}: {}", self.ruta, self.linea, self.motivo)
        }
    }
}

/// Comprobación pura de una página. No cubre los enlaces, que necesitan el disco (`enlaces_rotos`).
pub(crate) fn revisa(ruta: &str, texto: &str, idioma: Idioma) -> Vec<Fallo> {
    let mut fallos = Vec::new();
    for (indice, linea) in texto.lines().enumerate() {
        if let Some(motivo) = dato_personal(linea) {
            fallos.push(Fallo::nuevo(
                ruta,
                indice + 1,
                format!("dato personal: {motivo}"),
            ));
        }
    }
    for (n, linea) in lineas_prosa(texto) {
        if imagen_sin_alt(linea) {
            fallos.push(Fallo::nuevo(ruta, n, "imagen sin texto alternativo"));
        }
    }
    for (inicio, lineas) in parrafos(texto) {
        if lineas > MAX_LINEAS_PARRAFO {
            fallos.push(Fallo::nuevo(
                ruta,
                inicio,
                format!("párrafo de {lineas} líneas (máximo {MAX_LINEAS_PARRAFO}); conviértalo en lista o tabla"),
            ));
        }
    }
    fallos.extend(revisa_apertura(ruta, texto, idioma));
    fallos
}

/// Líneas de prosa con su número (desde 1), sin el contenido ni las vallas de los bloques de código.
pub(crate) fn lineas_prosa(texto: &str) -> Vec<(usize, &str)> {
    let mut en_codigo = false;
    let mut salida = Vec::new();
    for (indice, linea) in texto.lines().enumerate() {
        if linea.trim_start().starts_with("```") {
            en_codigo = !en_codigo;
        } else if !en_codigo {
            salida.push((indice + 1, linea));
        }
    }
    salida
}

/// Bloques de párrafo como (línea inicial, número de líneas). Títulos, listas (y su sangrado),
/// tablas, citas, separadores y bloques de código no son párrafo.
fn parrafos(texto: &str) -> Vec<(usize, usize)> {
    let mut bloques = Vec::new();
    let mut actual: Option<(usize, usize)> = None;
    let mut en_lista = false;
    let mut en_codigo = false;
    for (indice, linea) in texto.lines().enumerate() {
        let t = linea.trim();
        if t.starts_with("```") {
            en_codigo = !en_codigo;
            cierra(&mut actual, &mut bloques);
            continue;
        }
        if en_codigo {
            continue;
        }
        if t.is_empty() {
            en_lista = false;
            cierra(&mut actual, &mut bloques);
            continue;
        }
        if es_item_lista(t) {
            en_lista = true;
            cierra(&mut actual, &mut bloques);
            continue;
        }
        if en_lista && linea.starts_with(char::is_whitespace) {
            cierra(&mut actual, &mut bloques);
            continue;
        }
        en_lista = false;
        if es_estructural(t) {
            cierra(&mut actual, &mut bloques);
            continue;
        }
        actual = Some(match actual {
            Some((inicio, cuenta)) => (inicio, cuenta + 1),
            None => (indice + 1, 1),
        });
    }
    cierra(&mut actual, &mut bloques);
    bloques
}

fn cierra(actual: &mut Option<(usize, usize)>, bloques: &mut Vec<(usize, usize)>) {
    if let Some(bloque) = actual.take() {
        bloques.push(bloque);
    }
}

fn es_item_lista(t: &str) -> bool {
    let digitos = t.chars().take_while(|c| c.is_ascii_digit()).count();
    t.starts_with("- ")
        || t.starts_with("* ")
        || t.starts_with("+ ")
        || (digitos > 0 && t[digitos..].starts_with(". "))
}

fn es_estructural(t: &str) -> bool {
    t.starts_with('#')
        || t.starts_with('|')
        || t.starts_with('>')
        || t.starts_with('<')
        || t.starts_with("---")
}

/// Motivo si la línea contiene un dato personal: usuario de Windows, carpeta de inicio o correo real.
fn dato_personal(linea: &str) -> Option<&'static str> {
    let normal = linea.to_ascii_lowercase().replace("\\\\", "\\");
    if ["c:\\users\\", "/home/", "/users/"]
        .iter()
        .any(|patron| tiene_nombre_tras(&normal, patron))
    {
        return Some("ruta con un usuario real (use un marcador como <user>)");
    }
    if correo_real(linea) {
        return Some("correo real (use @example.invalid)");
    }
    None
}

/// Verdadero si tras alguna aparición de `patron` sigue un nombre (no un marcador `<...>`).
fn tiene_nombre_tras(texto: &str, patron: &str) -> bool {
    texto.match_indices(patron).any(|(i, _)| {
        texto[i + patron.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
    })
}

fn correo_real(linea: &str) -> bool {
    linea.match_indices('@').any(|(i, _)| {
        let antes = linea[..i].chars().next_back();
        let despues: String = linea[i + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
            .collect();
        let dominio = despues.trim_end_matches('.');
        antes.is_some_and(|c| c.is_ascii_alphanumeric())
            && dominio.contains('.')
            && dominio != "example.invalid"
    })
}

/// Verdadero si la línea tiene una imagen `![...](...)` con el texto alternativo vacío.
fn imagen_sin_alt(linea: &str) -> bool {
    let mut resto = linea;
    while let Some(i) = resto.find("![") {
        let tras = &resto[i + 2..];
        if let Some(j) = tras.find("](") {
            if tras[..j].trim().is_empty() {
                return true;
            }
        }
        resto = tras;
    }
    false
}

/// La apertura trilingüe debe estar completa, en orden y antes de la primera sección `##`.
fn revisa_apertura(ruta: &str, texto: &str, idioma: Idioma) -> Option<Fallo> {
    let encabezados = cabecera(idioma);
    let lineas: Vec<&str> = texto.lines().map(str::trim).collect();
    let primera_seccion = lineas
        .iter()
        .position(|l| l.starts_with("## "))
        .unwrap_or(lineas.len());
    let mut desde = 0;
    let mut posiciones = Vec::new();
    for encabezado in encabezados {
        match lineas[desde..].iter().position(|l| *l == encabezado) {
            Some(p) => {
                posiciones.push(desde + p);
                desde += p + 1;
            }
            None => {
                return Some(Fallo::nuevo(
                    ruta,
                    1,
                    format!("falta el encabezado de apertura `{encabezado}` (en este orden)"),
                ))
            }
        }
    }
    let ultimo = posiciones[2];
    if ultimo > primera_seccion {
        return Some(Fallo::nuevo(
            ruta,
            ultimo + 1,
            "la apertura debe ir antes de la primera sección `##`",
        ));
    }
    None
}

/// Enlaces relativos rotos de una página. `ruta` es relativa a `raiz` (la raíz del repositorio).
pub(crate) fn enlaces_rotos(raiz: &Path, ruta: &str, texto: &str) -> Vec<Fallo> {
    let carpeta = Path::new(ruta).parent().unwrap_or(Path::new(""));
    let mut fallos = Vec::new();
    for (n, linea) in lineas_prosa(texto) {
        for destino in destinos(linea) {
            if !existe_destino(raiz, carpeta, &destino) {
                fallos.push(Fallo::nuevo(
                    ruta,
                    n,
                    format!("enlace relativo roto: {destino}"),
                ));
            }
        }
    }
    fallos
}

/// Destinos relativos de los enlaces e imágenes de una línea, sin URLs, correos, anclas puras ni rutas absolutas.
pub(crate) fn destinos(linea: &str) -> Vec<String> {
    let mut salida = Vec::new();
    let mut resto = linea;
    while let Some(i) = resto.find("](") {
        let tras = &resto[i + 2..];
        let Some(fin) = tras.find(')') else {
            break;
        };
        let bruto = tras[..fin]
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');
        let destino = bruto.split_whitespace().next().unwrap_or("");
        let sin_ancla = destino.split(['#', '?']).next().unwrap_or("");
        if !sin_ancla.is_empty()
            && !sin_ancla.contains("://")
            && !sin_ancla.starts_with("mailto:")
            && !sin_ancla.starts_with('/')
        {
            salida.push(sin_ancla.to_string());
        }
        resto = &tras[fin + 1..];
    }
    salida
}

fn existe_destino(raiz: &Path, carpeta: &Path, destino: &str) -> bool {
    let Some(relativo) = normaliza(&carpeta.join(destino)) else {
        return false;
    };
    let ruta = raiz.join(relativo);
    ruta.is_file() || ruta.join("index.md").is_file()
}

/// Resuelve `.` y `..` sin tocar el disco; devuelve `None` si la ruta escapa de la raíz.
pub(crate) fn normaliza(relativa: &Path) -> Option<PathBuf> {
    let mut salida = PathBuf::new();
    for componente in relativa.components() {
        match componente {
            Component::Normal(parte) => salida.push(parte),
            Component::ParentDir => {
                if !salida.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(salida)
}

/// Paridad entre `en/` y `es/`: cada página existe en ambos y tiene el mismo número de secciones `##`.
/// Recibe pares (ruta relativa al idioma, texto).
pub(crate) fn revisa_paridad(en: &[(String, String)], es: &[(String, String)]) -> Vec<Fallo> {
    let mut fallos = Vec::new();
    for (relativa, texto) in en {
        match es.iter().find(|(r, _)| r == relativa) {
            None => fallos.push(Fallo::nuevo(
                &format!("docs/user/en/{relativa}"),
                0,
                "falta su versión en es/ (la paridad es obligatoria)",
            )),
            Some((_, otro)) => {
                let (secciones_en, secciones_es) = (secciones(texto), secciones(otro));
                if secciones_en != secciones_es {
                    fallos.push(Fallo::nuevo(
                        &format!("docs/user/es/{relativa}"),
                        0,
                        format!("{secciones_es} secciones `##` en es/ y {secciones_en} en en/"),
                    ));
                }
            }
        }
    }
    for (relativa, _) in es {
        if !en.iter().any(|(r, _)| r == relativa) {
            fallos.push(Fallo::nuevo(
                &format!("docs/user/es/{relativa}"),
                0,
                "esta página no existe en en/ (la paridad es obligatoria)",
            ));
        }
    }
    fallos
}

fn secciones(texto: &str) -> usize {
    lineas_prosa(texto)
        .iter()
        .filter(|(_, l)| l.starts_with("## "))
        .count()
}
