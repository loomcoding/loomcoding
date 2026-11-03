use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct GrepTool;

/// Máximo de coincidencias que se devuelven al modelo; el resto se cuenta, no se muestra.
///
/// Cabe señalar que el techo es bajo a propósito. Con 200 coincidencias de 300 caracteres
/// el resultado podía llegar a 60 KB, y un patrón trivial como `1000` casa con todo el
/// repositorio: el subagente recibía decenas de miles de caracteres de código ajeno a su
/// tarea y se quedaba sin contexto antes de escribir nada.
const MAX_GREP_MATCHES: usize = 50;
/// Caracteres que se conservan de cada línea coincidente.
const MAX_GREP_LINE_CHARS: usize = 200;
/// A partir de cuántos ficheros distintos se devuelve el recuento por fichero en vez de
/// las líneas. Con muchos ficheros, lo que orienta es saber dónde hay volumen, no leer
/// cincuenta líneas sueltas de cincuenta sitios.
const MAX_FILES_WITH_LINES: usize = 8;

/// Indica si `path` casa con el filtro `glob`. Se compara con la ruta relativa a `cwd` y, si el
/// patrón no contiene `/` (`por_nombre`), también con el nombre del fichero, de modo que `*.rs`
/// case con `src/a.rs`.
fn glob_matches(
    pattern: &glob::Pattern,
    por_nombre: bool,
    cwd: &std::path::Path,
    path: &std::path::Path,
) -> bool {
    let relativa = path.strip_prefix(cwd).unwrap_or(path);
    // Con separador literal, `*` no cruza directorios (`src/*.rs` no casa con `src/hondo/a.rs`);
    // para recorrerlos está `**`.
    let opciones = glob::MatchOptions {
        require_literal_separator: true,
        ..Default::default()
    };
    if pattern.matches_path_with(relativa, opciones) {
        return true;
    }
    por_nombre
        && path
            .file_name()
            .is_some_and(|nombre| pattern.matches(&nombre.to_string_lossy()))
}

/// Recorta `line` a `max` caracteres por una frontera de carácter, marcando el corte.
fn cut_line(line: &str, max: usize) -> String {
    match line.char_indices().nth(max) {
        Some((end, _)) => format!("{} [line truncated]", &line[..end]),
        None => line.to_string(),
    }
}

/// Lo que el recorrido encuentra: las líneas que se muestran, el total de coincidencias y
/// el recuento por fichero.
struct Hallazgo {
    lineas: Vec<String>,
    total: usize,
    por_fichero: Vec<(String, usize)>,
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "Search file contents by regex. Params: pattern (string), glob (optional string). \
         Returns at most 50 matching lines, each cut to 200 characters. When the pattern \
         matches many files, returns a per-file count instead of the lines. The first line \
         of the output says how many matches there were in total."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{"pattern":{"type":"string"},"glob":{"type":"string"}},
            "required":["pattern"]
        })
    }
    fn execution_mode(&self, _i: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Parallel
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        _c: CancellationToken,
    ) -> Result<ToolOutput> {
        let pattern = input["pattern"].as_str().unwrap_or_default();
        let re = regex::Regex::new(pattern).map_err(|e| CoreError::Tool(e.to_string()))?;
        // Patrón `glob` opcional y si debe casar también con el nombre del fichero.
        let filtro = match input["glob"].as_str() {
            Some(texto) => {
                let patron = glob::Pattern::new(texto)
                    .map_err(|e| CoreError::Tool(format!("glob inválido «{texto}»: {e}")))?;
                Some((patron, !texto.contains('/')))
            }
            None => None,
        };
        let cwd = ctx.cwd.clone();
        let hallazgo = tokio::task::spawn_blocking(move || {
            let mut lineas = Vec::new();
            let mut total = 0usize;
            let mut por_fichero: Vec<(String, usize)> = Vec::new();
            for entry in ignore::WalkBuilder::new(&cwd).build().flatten() {
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                if let Some((patron, por_nombre)) = &filtro {
                    if !glob_matches(patron, *por_nombre, &cwd, entry.path()) {
                        continue;
                    }
                }
                let Ok(content) = std::fs::read_to_string(entry.path()) else {
                    continue;
                };
                let mut en_este_fichero = 0usize;
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        total += 1;
                        en_este_fichero += 1;
                        if lineas.len() < MAX_GREP_MATCHES {
                            lineas.push(format!(
                                "{}:{}: {}",
                                entry.path().display(),
                                i + 1,
                                cut_line(line, MAX_GREP_LINE_CHARS)
                            ));
                        }
                    }
                }
                if en_este_fichero > 0 {
                    por_fichero.push((entry.path().display().to_string(), en_este_fichero));
                }
            }
            Hallazgo {
                lineas,
                total,
                por_fichero,
            }
        })
        .await
        .map_err(|e| CoreError::Tool(e.to_string()))?;

        // El aviso va PRIMERO: es lo que orienta al agente sobre si el resultado está
        // completo, y al final del texto se pierde entre las líneas.
        let mut text = format!(
            "[{} of {} matches shown]",
            hallazgo.lineas.len(),
            hallazgo.total
        );
        if hallazgo.total > hallazgo.lineas.len() {
            text.push_str(" narrow the pattern or add a glob");
        }
        text.push('\n');

        // Con muchos ficheros distintos, el recuento por fichero orienta más que las líneas.
        if hallazgo.por_fichero.len() > MAX_FILES_WITH_LINES {
            text.push_str(&format!(
                "{} files matched; per-file counts:\n",
                hallazgo.por_fichero.len()
            ));
            for (fichero, cuenta) in &hallazgo.por_fichero {
                text.push_str(&format!("{fichero}: {cuenta}\n"));
            }
        } else {
            text.push_str(&hallazgo.lineas.join("\n"));
        }
        Ok(ToolOutput::ok(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::PermissionService;
    use std::sync::Arc;

    fn ctx(dir: &std::path::Path) -> ToolCtx {
        ToolCtx {
            cwd: dir.to_path_buf(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        }
    }

    #[tokio::test]
    async fn grep_finds_matching_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "foo\nbar\nfoobar\n").unwrap();
        let out = GrepTool
            .run(
                ctx(dir.path()),
                serde_json::json!({"pattern":"foo"}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(out.content.contains("a.txt:1: foo"));
        assert!(out.content.contains("a.txt:3: foobar"));
    }

    #[tokio::test]
    async fn grep_caps_matches_and_long_lines() {
        let dir = tempfile::tempdir().unwrap();
        // 250 coincidencias: la primera es una línea de 1 000 caracteres (ASCII) y el resto son cortas.
        let mut text = format!("{}hit\n", "x".repeat(1000));
        for i in 0..249 {
            text.push_str(&format!("hit {i}\n"));
        }
        std::fs::write(dir.path().join("a.txt"), text).unwrap();
        let out = GrepTool
            .run(
                ctx(dir.path()),
                serde_json::json!({"pattern":"hit"}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // El aviso va primero, luego las 50 líneas de coincidencia.
        let primera = out.content.lines().next().unwrap();
        assert!(
            primera.starts_with("[50 of 250 matches shown]"),
            "el aviso debe ir primero, y el techo es 50: {primera}"
        );
        assert_eq!(out.content.lines().count(), 51);
        let primera_linea = out.content.lines().nth(1).unwrap();
        assert!(primera_linea.contains("a.txt:1: "));
        assert!(primera_linea.contains(&"x".repeat(200)));
        assert!(!primera_linea.contains(&"x".repeat(201)));
    }

    #[tokio::test]
    async fn grep_cuts_long_lines_on_a_char_boundary() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), format!("{}\n", "ñ".repeat(400))).unwrap();
        let out = GrepTool
            .run(
                ctx(dir.path()),
                serde_json::json!({"pattern":"ñ"}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // No debe partir un carácter multibyte por la mitad.
        assert!(out.content.contains("ñ"));
    }

    #[tokio::test]
    async fn grep_devuelve_recuento_por_fichero_con_muchos_ficheros() {
        // Con más ficheros que MAX_FILES_WITH_LINES, el resultado es un recuento, no líneas.
        let dir = tempfile::tempdir().unwrap();
        for i in 0..12 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), "coincide\ncoincide\n").unwrap();
        }
        let out = GrepTool
            .run(
                ctx(dir.path()),
                serde_json::json!({"pattern":"coincide"}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(
            out.content.contains("12 files matched; per-file counts:"),
            "con muchos ficheros debe devolver recuento: {}",
            out.content
        );
        assert!(out.content.contains(": 2"));
    }

    #[tokio::test]
    async fn grep_no_supera_el_techo_de_bytes() {
        // El fallo que mataba a los subagentes: un patrón trivial devolvía decenas de KB.
        let dir = tempfile::tempdir().unwrap();
        let mut text = String::new();
        for i in 0..500 {
            text.push_str(&format!("linea con 1000 dentro {i}\n"));
        }
        std::fs::write(dir.path().join("a.txt"), text).unwrap();
        let out = GrepTool
            .run(
                ctx(dir.path()),
                serde_json::json!({"pattern":"1000"}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(
            out.content.len() < 12_000,
            "el resultado no debe pasar de ~12 KB, y tiene {} bytes",
            out.content.len()
        );
    }
}
