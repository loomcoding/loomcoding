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
            Ok(value) => Ok(ToolOutput::ok(
                super::limit_output(&value.to_string()).into_owned(),
            )),
            Err(e) => Ok(ToolOutput::err(format!("LSP request failed: {e}"))),
        }
    }
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
}
