//! Analizador puro de la salida de `git diff --no-color -U2`.
//!
//! Es tolerante: acepta salidas truncadas a mitad de un hunk o de una cabecera
//! y devuelve lo que haya podido leer, sin entrar en pánico.

use super::{DiffLine, FileDiff, FileKind, Hunk};

/// Analiza la salida de `git diff` y devuelve un `FileDiff` por fichero.
///
/// Las líneas anteriores al primer `diff --git` (basura, texto libre) se ignoran.
pub fn parse_git_diff(text: &str) -> Vec<FileDiff> {
    let mut out = Vec::new();
    let mut actual: Option<Parcial> = None;
    for linea in text.lines() {
        if let Some(resto) = linea.strip_prefix("diff --git ") {
            if let Some(parcial) = actual.take() {
                out.push(parcial.cerrar());
            }
            actual = Some(Parcial::nuevo(resto));
        } else if let Some(parcial) = actual.as_mut() {
            parcial.procesar(linea);
        }
    }
    if let Some(parcial) = actual.take() {
        out.push(parcial.cerrar());
    }
    out
}

/// Estado de un fichero mientras se recorre su sección de la salida.
struct Parcial {
    ruta_cabecera: String,
    origen: Option<String>,
    destino: Option<String>,
    renombrado_de: Option<String>,
    renombrado_a: Option<String>,
    nuevo: bool,
    borrado: bool,
    binario: bool,
    hunks: Vec<Hunk>,
    hunk_actual: Option<Hunk>,
    added: usize,
    removed: usize,
}

impl Parcial {
    fn nuevo(resto: &str) -> Self {
        Self {
            ruta_cabecera: ruta_de_cabecera(resto),
            origen: None,
            destino: None,
            renombrado_de: None,
            renombrado_a: None,
            nuevo: false,
            borrado: false,
            binario: false,
            hunks: Vec::new(),
            hunk_actual: None,
            added: 0,
            removed: 0,
        }
    }

    fn procesar(&mut self, linea: &str) {
        // Dentro de un hunk, las líneas `---`/`+++` son contenido: solo las de
        // cabecera, anteriores al primer `@@`, son metadatos del fichero.
        if self.hunk_actual.is_some() {
            self.procesar_en_hunk(linea);
        } else {
            self.procesar_cabecera(linea);
        }
    }

    fn procesar_en_hunk(&mut self, linea: &str) {
        if linea.starts_with("@@") {
            self.cerrar_hunk();
            self.hunk_actual = Some(Hunk {
                header: linea.to_string(),
                lines: Vec::new(),
            });
            return;
        }
        let entrada = if let Some(resto) = linea.strip_prefix('+') {
            self.added += 1;
            Some(DiffLine::Added(resto.to_string()))
        } else if let Some(resto) = linea.strip_prefix('-') {
            self.removed += 1;
            Some(DiffLine::Removed(resto.to_string()))
        } else if let Some(resto) = linea.strip_prefix(' ') {
            Some(DiffLine::Context(resto.to_string()))
        } else if linea.is_empty() {
            // Contexto vacío que llega sin el espacio inicial.
            Some(DiffLine::Context(String::new()))
        } else {
            // Cabe señalar que `\ No newline at end of file` y cualquier otra
            // línea desconocida se descartan.
            None
        };
        if let (Some(entrada), Some(hunk)) = (entrada, self.hunk_actual.as_mut()) {
            hunk.lines.push(entrada);
        }
    }

    fn procesar_cabecera(&mut self, linea: &str) {
        if linea.starts_with("@@") {
            self.hunk_actual = Some(Hunk {
                header: linea.to_string(),
                lines: Vec::new(),
            });
        } else if let Some(resto) = linea.strip_prefix("--- ") {
            self.origen = ruta_de_lado(resto, "a/");
        } else if let Some(resto) = linea.strip_prefix("+++ ") {
            self.destino = ruta_de_lado(resto, "b/");
        } else if let Some(resto) = linea.strip_prefix("rename from ") {
            self.renombrado_de = Some(resto.to_string());
        } else if let Some(resto) = linea.strip_prefix("rename to ") {
            self.renombrado_a = Some(resto.to_string());
        } else if linea.starts_with("new file mode") {
            self.nuevo = true;
        } else if linea.starts_with("deleted file mode") {
            self.borrado = true;
        } else if linea.starts_with("Binary files ") || linea.starts_with("GIT binary patch") {
            self.binario = true;
        }
    }

    fn cerrar_hunk(&mut self) {
        if let Some(hunk) = self.hunk_actual.take() {
            self.hunks.push(hunk);
        }
    }

    /// Cierra el fichero. La ruta nueva prevalece; para borrados se usa la del lado `a/`.
    fn cerrar(mut self) -> FileDiff {
        self.cerrar_hunk();
        let path = self
            .renombrado_a
            .take()
            .or(self.destino.take())
            .or(self.origen.take())
            .unwrap_or_else(|| self.ruta_cabecera.clone());
        // El binario prevalece sobre el renombrado: en ese caso se pierde `from`.
        let kind = if self.binario {
            FileKind::Binary
        } else if let Some(from) = self.renombrado_de.take() {
            FileKind::Renamed { from }
        } else if self.nuevo {
            FileKind::Added
        } else if self.borrado {
            FileKind::Deleted
        } else {
            FileKind::Modified
        };
        FileDiff {
            path,
            kind,
            added: self.added,
            removed: self.removed,
            hunks: self.hunks,
        }
    }
}

/// Ruta del lado `a/` o `b/` de una cabecera `---`/`+++`; `None` para `/dev/null`.
fn ruta_de_lado(resto: &str, prefijo: &str) -> Option<String> {
    let limpia = quitar_comillas(resto);
    if limpia == "/dev/null" {
        return None;
    }
    Some(limpia.strip_prefix(prefijo).unwrap_or(limpia).to_string())
}

/// Ruta de último recurso, tomada de `diff --git a/X b/Y`.
fn ruta_de_cabecera(resto: &str) -> String {
    match resto.rfind(" b/") {
        Some(pos) => resto[pos + 3..].to_string(),
        None => resto.strip_prefix("a/").unwrap_or(resto).to_string(),
    }
}

fn quitar_comillas(texto: &str) -> &str {
    texto
        .strip_prefix('"')
        .and_then(|interior| interior.strip_suffix('"'))
        .unwrap_or(texto)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOS_FICHEROS: &str = r#"diff --git a/src/a.rs b/src/a.rs
index 1111111..2222222 100644
--- a/src/a.rs
+++ b/src/a.rs
@@ -1,4 +1,4 @@
 uno
-dos
+DOS
 tres
@@ -10,3 +10,4 @@ fn x() {
 cuatro
+cinco
 seis
diff --git a/b.txt b/b.txt
index 3333333..4444444 100644
--- a/b.txt
+++ b/b.txt
@@ -1 +1,2 @@
-old
+new
+extra
\ No newline at end of file
"#;

    #[test]
    fn dos_ficheros_con_varios_hunks_y_conteos() {
        let files = parse_git_diff(DOS_FICHEROS);
        assert_eq!(files.len(), 2);

        let a = &files[0];
        assert_eq!(a.path, "src/a.rs");
        assert_eq!(a.kind, FileKind::Modified);
        assert_eq!(a.added, 2);
        assert_eq!(a.removed, 1);
        assert_eq!(a.hunks.len(), 2);
        assert_eq!(a.hunks[0].header, "@@ -1,4 +1,4 @@");
        assert_eq!(
            a.hunks[0].lines,
            vec![
                DiffLine::Context("uno".into()),
                DiffLine::Removed("dos".into()),
                DiffLine::Added("DOS".into()),
                DiffLine::Context("tres".into()),
            ]
        );
        assert_eq!(a.hunks[1].header, "@@ -10,3 +10,4 @@ fn x() {");

        let b = &files[1];
        assert_eq!(b.path, "b.txt");
        assert_eq!(b.added, 2);
        assert_eq!(b.removed, 1);
        assert_eq!(b.hunks.len(), 1);
        assert_eq!(
            b.hunks[0].lines,
            vec![
                DiffLine::Removed("old".into()),
                DiffLine::Added("new".into()),
                DiffLine::Added("extra".into()),
            ]
        );
    }

    #[test]
    fn fichero_nuevo_es_added() {
        let texto = r#"diff --git a/nuevo.txt b/nuevo.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/nuevo.txt
@@ -0,0 +1,2 @@
+primera
+segunda
"#;
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "nuevo.txt");
        assert_eq!(files[0].kind, FileKind::Added);
        assert_eq!(files[0].added, 2);
        assert_eq!(files[0].removed, 0);
    }

    #[test]
    fn fichero_borrado_es_deleted_y_toma_la_ruta_de_a() {
        let texto = r#"diff --git a/viejo.txt b/viejo.txt
deleted file mode 100644
index e69de29..0000000
--- a/viejo.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-uno
-dos
"#;
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "viejo.txt");
        assert_eq!(files[0].kind, FileKind::Deleted);
        assert_eq!(files[0].added, 0);
        assert_eq!(files[0].removed, 2);
    }

    #[test]
    fn renombrado_toma_la_ruta_nueva_y_guarda_la_antigua() {
        let texto = r#"diff --git a/origen.rs b/destino.rs
similarity index 90%
rename from origen.rs
rename to destino.rs
index 1111111..2222222 100644
--- a/origen.rs
+++ b/destino.rs
@@ -1 +1 @@
-x
+y
"#;
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "destino.rs");
        assert_eq!(
            files[0].kind,
            FileKind::Renamed {
                from: "origen.rs".into()
            }
        );
        assert_eq!(files[0].added, 1);
        assert_eq!(files[0].removed, 1);
    }

    #[test]
    fn binario_no_tiene_hunks_ni_conteos() {
        let texto = r#"diff --git a/img.png b/img.png
index 1111111..2222222 100644
Binary files a/img.png and b/img.png differ
"#;
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "img.png");
        assert_eq!(files[0].kind, FileKind::Binary);
        assert!(files[0].hunks.is_empty());
        assert_eq!(files[0].added, 0);
        assert_eq!(files[0].removed, 0);
    }

    #[test]
    fn no_newline_at_end_se_ignora() {
        let texto = r#"diff --git a/f.txt b/f.txt
--- a/f.txt
+++ b/f.txt
@@ -1 +1 @@
-a
\ No newline at end of file
+b
\ No newline at end of file
"#;
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].hunks[0].lines,
            vec![DiffLine::Removed("a".into()), DiffLine::Added("b".into()),]
        );
        assert_eq!(files[0].added, 1);
        assert_eq!(files[0].removed, 1);
    }

    #[test]
    fn salida_cortada_a_mitad_de_hunk_no_entra_en_panico() {
        let texto = r#"diff --git a/x.rs b/x.rs
--- a/x.rs
+++ b/x.rs
@@ -1,3 +1,3 @@
 uno
-do"#;
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "x.rs");
        assert_eq!(files[0].hunks.len(), 1);
        assert_eq!(
            files[0].hunks[0].lines,
            vec![
                DiffLine::Context("uno".into()),
                DiffLine::Removed("do".into()),
            ]
        );
        assert_eq!(files[0].removed, 1);
    }

    #[test]
    fn salida_cortada_en_la_cabecera_devuelve_lo_leido() {
        let texto = "diff --git a/x.rs b/x.rs\nindex 12";
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "x.rs");
        assert_eq!(files[0].kind, FileKind::Modified);
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn texto_vacio_da_vector_vacio() {
        assert!(parse_git_diff("").is_empty());
    }

    #[test]
    fn basura_sin_diff_git_da_vector_vacio() {
        let texto = "hola\nesto no es un diff\n@@ -1 +1 @@\n+x\n";
        assert!(parse_git_diff(texto).is_empty());
    }

    #[test]
    fn contexto_vacio_sin_espacio_es_context_vacio() {
        let texto = r#"diff --git a/f b/f
--- a/f
+++ b/f
@@ -1,3 +1,3 @@

-a
+b
"#;
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].hunks[0].lines,
            vec![
                DiffLine::Context(String::new()),
                DiffLine::Removed("a".into()),
                DiffLine::Added("b".into()),
            ]
        );
    }

    #[test]
    fn rutas_con_espacios() {
        let texto = r#"diff --git a/mi carpeta/nota final.txt b/mi carpeta/nota final.txt
index 1111111..2222222 100644
--- a/mi carpeta/nota final.txt
+++ b/mi carpeta/nota final.txt
@@ -1 +1 @@
-a
+b
diff --git a/foto 1.png b/foto 1.png
Binary files a/foto 1.png and b/foto 1.png differ
"#;
        let files = parse_git_diff(texto);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "mi carpeta/nota final.txt");
        assert_eq!(files[1].path, "foto 1.png");
        assert_eq!(files[1].kind, FileKind::Binary);
    }

    #[test]
    fn guion_doble_dentro_de_hunk_cuenta_como_quitado() {
        let texto = r#"diff --git a/f b/f
--- a/f
+++ b/f
@@ -1 +1 @@
--- comentario
+x
"#;
        let files = parse_git_diff(texto);
        assert_eq!(files[0].removed, 1);
        assert_eq!(files[0].added, 1);
        assert_eq!(
            files[0].hunks[0].lines[0],
            DiffLine::Removed("-- comentario".into())
        );
    }
}
