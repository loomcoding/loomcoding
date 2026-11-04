use super::{Tool, ToolCtx, ToolOutput};
use crate::{
    error::{CoreError, Result},
    permission::{Decision, PermissionRequest},
};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    process::Command,
    time::{timeout, Duration},
};
use tokio_util::sync::CancellationToken;

pub struct LspTool;

async fn read_message<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> std::io::Result<Value> {
    let mut length = None;
    loop {
        let mut line = Vec::new();
        if reader.read_until(b'\n', &mut line).await? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "LSP server closed stdout",
            ));
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        if let Some(v) = line.strip_prefix(b"Content-Length: ") {
            length = std::str::from_utf8(v)
                .ok()
                .and_then(|s| s.trim().parse::<usize>().ok());
        }
    }
    let length = length.filter(|n| *n <= 1_000_000).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid LSP content length",
        )
    })?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await?;
    serde_json::from_slice(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

async fn send<W: AsyncWrite + Unpin>(writer: &mut W, message: Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(&message).map_err(std::io::Error::other)?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await?;
    writer.write_all(&body).await?;
    writer.flush().await
}

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "lsp"
    }
    fn description(&self) -> &str {
        "Query rust-analyzer for diagnostics, definition, references, or document symbols. Params: operation, path, line, column, query."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","properties":{"operation":{"type":"string","enum":["diagnostics","definition","references","symbols"]},"path":{"type":"string"},"line":{"type":"integer"},"column":{"type":"integer"},"query":{"type":"string"}},"required":["operation","path"]})
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let op = input["operation"].as_str().unwrap_or_default();
        if !["diagnostics", "definition", "references", "symbols"].contains(&op) {
            return Ok(ToolOutput::err("unsupported LSP operation"));
        }
        let cwd = std::fs::canonicalize(&ctx.cwd).map_err(|e| CoreError::Tool(e.to_string()))?;
        let file = std::fs::canonicalize(cwd.join(input["path"].as_str().unwrap_or_default()))
            .map_err(|e| CoreError::Tool(e.to_string()))?;
        if !file.starts_with(&cwd) || !file.is_file() || file.extension().is_none_or(|e| e != "rs")
        {
            return Ok(ToolOutput::err(
                "LSP v1 supports Rust files inside the workspace",
            ));
        }
        let req = PermissionRequest {
            tool: "lsp".into(),
            action: "start".into(),
            detail: "rust-analyzer".into(),
        };
        if tokio::select! { _ = cancel.cancelled() => return Ok(ToolOutput::err("cancelled")), d = ctx.permission.check(req) => d }
            == Decision::Deny
        {
            return Ok(ToolOutput::err("permission denied"));
        }
        let text = tokio::fs::read_to_string(&file)
            .await
            .map_err(|e| CoreError::Tool(e.to_string()))?;
        let uri = reqwest::Url::from_file_path(&file)
            .map_err(|_| CoreError::Tool("cannot encode file URI".into()))?
            .to_string();
        let mut child = match Command::new("rust-analyzer")
            .current_dir(&cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolOutput::err(format!(
                    "could not start rust-analyzer: {e}"
                )))
            }
        };
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| CoreError::Tool("LSP stdin unavailable".into()))?;
        let mut stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| CoreError::Tool("LSP stdout unavailable".into()))?,
        );
        let operation = async {
            send(&mut stdin, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":std::process::id(),"rootUri":reqwest::Url::from_directory_path(&cwd).ok().map(|u|u.to_string()),"capabilities":{}}})).await?;
            let initialized = timeout(Duration::from_secs(10), read_message(&mut stdout))
                .await
                .map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "LSP initialize timeout")
                })??;
            if initialized.get("error").is_some() {
                return Err(std::io::Error::other(initialized["error"].to_string()));
            }
            send(
                &mut stdin,
                json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            )
            .await?;
            send(&mut stdin, json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"rust","version":1,"text":text}}})).await?;
            if op == "diagnostics" {
                let mut results = Vec::new();
                while let Ok(Ok(msg)) =
                    timeout(Duration::from_secs(3), read_message(&mut stdout)).await
                {
                    if msg["method"] == "textDocument/publishDiagnostics" {
                        results = msg["params"]["diagnostics"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default();
                        break;
                    }
                }
                return Ok(json!(results));
            }
            let line = input["line"].as_u64().unwrap_or(1).saturating_sub(1);
            let character = input["column"].as_u64().unwrap_or(1).saturating_sub(1);
            let (id, method, params) = match op {
                "definition" => (
                    2,
                    "textDocument/definition",
                    json!({"textDocument":{"uri":uri},"position":{"line":line,"character":character}}),
                ),
                "references" => (
                    2,
                    "textDocument/references",
                    json!({"textDocument":{"uri":uri},"position":{"line":line,"character":character},"context":{"includeDeclaration":true}}),
                ),
                _ => (
                    2,
                    "textDocument/documentSymbol",
                    json!({"textDocument":{"uri":uri}}),
                ),
            };
            send(
                &mut stdin,
                json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
            )
            .await?;
            loop {
                let msg = timeout(Duration::from_secs(10), read_message(&mut stdout))
                    .await
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::TimedOut, "LSP request timeout")
                    })??;
                if msg["id"] == id {
                    if msg.get("error").is_some() {
                        return Err(std::io::Error::other(msg["error"].to_string()));
                    }
                    return Ok(msg["result"].clone());
                }
            }
        };
        let result = tokio::select! { _ = cancel.cancelled() => return Ok(ToolOutput::err("cancelled")), r = operation => r };
        match result {
            Ok(value) => Ok(ToolOutput::ok(format_lsp_result(op, &value))),
            Err(e) => Ok(ToolOutput::err(format!("LSP request failed: {e}"))),
        }
    }
}

/// Máximo de símbolos o referencias que se listan.
///
/// Cabe señalar que el volcado crudo del protocolo llegaba a 20 KB para un solo fichero:
/// cada símbolo trae su `range` con cuatro números, su `kind` numérico, `deprecated` y
/// `tags`, y nada de eso le dice al agente dónde está la función. Con el techo bajo y el
/// formato compacto, lo que llega es una lista de `nombre (línea)` que sí se puede leer.
const MAX_LSP_ITEMS: usize = 60;

/// Formatea el resultado de una operación LSP de forma compacta y legible.
///
/// En vez de volcar el JSON del protocolo, extrae lo que el agente necesita: el nombre y
/// la línea. Para `diagnostics`, el mensaje y la posición.
fn format_lsp_result(op: &str, value: &Value) -> String {
    let items: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        Value::Null => Vec::new(),
        otro => vec![otro],
    };
    if items.is_empty() {
        return format!("[no results for {op}]");
    }

    let total = items.len();
    let mut lineas: Vec<String> = Vec::new();
    for item in items.iter().take(MAX_LSP_ITEMS) {
        let linea = match op {
            "diagnostics" => {
                let mensaje = item["message"].as_str().unwrap_or("(sin mensaje)");
                let gravedad = match item["severity"].as_u64() {
                    Some(1) => "error",
                    Some(2) => "warning",
                    Some(3) => "info",
                    Some(4) => "hint",
                    _ => "?",
                };
                let pos = &item["range"]["start"];
                format!(
                    "{}:{}: {}: {}",
                    pos["line"].as_u64().unwrap_or(0) + 1,
                    pos["character"].as_u64().unwrap_or(0) + 1,
                    gravedad,
                    mensaje
                )
            }
            "symbols" => {
                // `documentSymbol` devuelve `name`/`kind`/`range`; `SymbolInformation`
                // devuelve `name`/`kind`/`location`. Se aceptan las dos formas.
                let nombre = item["name"].as_str().unwrap_or("(sin nombre)");
                let linea = item["range"]["start"]["line"]
                    .as_u64()
                    .or_else(|| item["location"]["range"]["start"]["line"].as_u64())
                    .map(|l| l + 1);
                match linea {
                    Some(l) => format!("{nombre} (línea {l})"),
                    None => nombre.to_string(),
                }
            }
            _ => {
                // `definition` y `references` devuelven `Location`: uri + range.
                let uri = item["uri"].as_str().unwrap_or("");
                let fichero = uri.rsplit('/').next().unwrap_or(uri);
                let linea = item["range"]["start"]["line"]
                    .as_u64()
                    .map(|l| l + 1)
                    .unwrap_or(0);
                format!("{fichero}:{linea}")
            }
        };
        lineas.push(linea);
    }

    let mut text = format!("[{total} results, {} shown]\n", lineas.len());
    text.push_str(&lineas.join("\n"));
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn reads_lsp_content_length_frames() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        let framed = format!(
            "Content-Length: {}\r\n\r\n{}",
            body.len(),
            String::from_utf8_lossy(body)
        );
        let mut reader = BufReader::new(framed.as_bytes());
        assert_eq!(read_message(&mut reader).await.unwrap()["id"], 1);
    }

    #[test]
    fn symbols_se_formatean_como_nombre_y_linea() {
        // La forma de `documentSymbol`: name + range.
        let value = json!([
            {"name":"render","kind":12,"range":{"start":{"line":41,"character":0},"end":{"line":60,"character":1}},"deprecated":false,"tags":[]},
            {"name":"draw","kind":12,"range":{"start":{"line":99,"character":0},"end":{"line":120,"character":1}},"deprecated":false,"tags":[]}
        ]);
        let texto = format_lsp_result("symbols", &value);
        assert!(texto.contains("render (línea 42)"), "{texto}");
        assert!(texto.contains("draw (línea 100)"), "{texto}");
        // No debe arrastrar el JSON del protocolo.
        assert!(!texto.contains("deprecated"), "{texto}");
        assert!(!texto.contains("\"range\""), "{texto}");
    }

    #[test]
    fn symbols_acepta_symbol_information() {
        // La otra forma: name + location.range.
        let value = json!([
            {"name":"main","kind":12,"location":{"uri":"file:///a/b.rs","range":{"start":{"line":4,"character":0},"end":{"line":9,"character":1}}}}
        ]);
        let texto = format_lsp_result("symbols", &value);
        assert!(texto.contains("main (línea 5)"), "{texto}");
    }

    #[test]
    fn diagnostics_se_formatean_con_gravedad() {
        let value = json!([
            {"message":"unused variable","severity":2,"range":{"start":{"line":9,"character":4},"end":{"line":9,"character":8}}}
        ]);
        let texto = format_lsp_result("diagnostics", &value);
        assert!(texto.contains("10:5: warning: unused variable"), "{texto}");
    }

    #[test]
    fn el_resultado_no_supera_el_techo() {
        // El fallo que ahogaba a los subagentes: 20 KB de JSON por un solo fichero.
        let mut items = Vec::new();
        for i in 0..500 {
            items.push(json!({
                "name": format!("funcion_con_nombre_largo_{i}"),
                "kind": 12,
                "range": {"start":{"line":i,"character":0},"end":{"line":i+5,"character":1}},
                "deprecated": false,
                "tags": []
            }));
        }
        let texto = format_lsp_result("symbols", &json!(items));
        assert!(
            texto.len() < 4_000,
            "el resultado debe ser compacto, y tiene {} bytes",
            texto.len()
        );
        assert!(texto.starts_with("[500 results, 60 shown]"), "{texto}");
    }

    #[test]
    fn sin_resultados_lo_dice() {
        assert_eq!(
            format_lsp_result("symbols", &json!([])),
            "[no results for symbols]"
        );
    }
}
