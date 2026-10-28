use super::{Tool, ToolCtx, ToolOutput};
use crate::{
    error::{CoreError, Result},
    permission::{Decision, PermissionRequest},
};
use async_trait::async_trait;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

const MAX_WEB_BYTES: usize = 100_000;
const MAX_RESULTS: usize = 8;

pub struct WebTool;

async fn client_for(url: &reqwest::Url) -> Result<reqwest::Client> {
    let host = url
        .host_str()
        .ok_or_else(|| CoreError::Tool("URL has no host".into()))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host, port))
            .await
            .map_err(|e| CoreError::Tool(e.to_string()))?
            .collect()
    };
    if addrs.is_empty() || addrs.iter().any(|a| !public_ip(a.ip())) {
        return Err(CoreError::Tool(
            "web requests may only reach public IP addresses".into(),
        ));
    }
    reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, &addrs)
        .user_agent("CodeLoom/0.1 web tool")
        .build()
        .map_err(|e| CoreError::Tool(e.to_string()))
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 224)
        }
        IpAddr::V6(ip) => {
            (ip.octets()[0] & 0xe0) == 0x20
                && ip.to_ipv4_mapped().is_none_or(|v| public_ip(IpAddr::V4(v)))
        }
    }
}

async fn fetch_raw(url: &str) -> Result<String> {
    let url = reqwest::Url::parse(url).map_err(|e| CoreError::Tool(format!("invalid URL: {e}")))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(CoreError::Tool(
            "only credential-free HTTPS URLs are allowed".into(),
        ));
    }
    let client = client_for(&url).await?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| CoreError::Tool(e.to_string()))?;
    if response.status().is_redirection() {
        return Err(CoreError::Tool(
            "redirect refused; use the destination URL directly".into(),
        ));
    }
    if !response.status().is_success() {
        return Err(CoreError::Tool(format!("HTTP {}", response.status())));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| CoreError::Tool(e.to_string()))?
    {
        if bytes.len() + chunk.len() > MAX_WEB_BYTES {
            let remaining = MAX_WEB_BYTES.saturating_sub(bytes.len());
            bytes.extend_from_slice(&chunk[..remaining]);
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn fetch(url: &str) -> Result<String> {
    let raw = fetch_raw(url).await?;
    let text = raw.as_str();
    let text = regex::Regex::new(
        r"(?is)<script[^>]*>.*?</script>|<style[^>]*>.*?</style>|<noscript[^>]*>.*?</noscript>|<!--.*?-->|<[^>]+>",
    )
        .expect("constant regex")
        .replace_all(text, " ");
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");
    Ok(super::limit_output(&text).into_owned())
}

fn search_results(html: &str) -> String {
    let links = regex::Regex::new(
        r#"(?is)<a[^>]+class="[^"]*result__a[^"]*"[^>]+href="([^"]+)"[^>]*>(.*?)</a>"#,
    )
    .expect("constant regex");
    let tags = regex::Regex::new(r"(?is)<[^>]+>").expect("constant regex");
    let mut out = Vec::new();
    for cap in links.captures_iter(html).take(MAX_RESULTS) {
        let title = tags
            .replace_all(&cap[2], " ")
            .replace("&amp;", "&")
            .replace("&quot;", "\"");
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        let url = cap[1].replace("&amp;", "&");
        out.push(format!("{title}\n{url}"));
    }
    if out.is_empty() {
        "No search results were returned.".into()
    } else {
        out.join("\n\n")
    }
}

#[async_trait]
impl Tool for WebTool {
    fn name(&self) -> &str {
        "web"
    }
    fn description(&self) -> &str {
        "Search the public web or fetch a public HTTPS page. Params: query for search, or url for fetch."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"query":{"type":"string"},"url":{"type":"string"}},"oneOf":[{"required":["query"]},{"required":["url"]}]})
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let (action, detail) = if let Some(q) = input["query"].as_str() {
            ("search", q)
        } else if let Some(u) = input["url"].as_str() {
            ("fetch", u)
        } else {
            return Ok(ToolOutput::err("provide query or url"));
        };
        let req = PermissionRequest {
            tool: "web".into(),
            action: action.into(),
            detail: detail.chars().take(256).collect(),
        };
        if tokio::select! { _ = cancel.cancelled() => return Ok(ToolOutput::err("cancelled")), d = ctx.permission.check(req) => d }
            == Decision::Deny
        {
            return Ok(ToolOutput::err("permission denied"));
        }
        let result = if action == "fetch" {
            fetch(detail).await
        } else {
            let url = format!(
                "https://html.duckduckgo.com/html/?q={}",
                urlencoding(detail)
            );
            fetch_raw(&url).await.map(|body| search_results(&body))
        };
        result
            .map(ToolOutput::ok)
            .or_else(|e| Ok(ToolOutput::err(e.to_string())))
    }
}

fn urlencoding(input: &str) -> String {
    input
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_private_addresses_and_extracts_search_results() {
        assert!(!public_ip("127.0.0.1".parse().unwrap()));
        assert!(!public_ip("10.1.2.3".parse().unwrap()));
        assert!(public_ip("8.8.8.8".parse().unwrap()));
        let html = r#"<a class="result__a" href="https://example.com">Example <b>page</b></a>"#;
        assert!(search_results(html).contains("Example page\nhttps://example.com"));
    }
}
