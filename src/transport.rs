use crate::{Engine, VERSION, host::Host, tools};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
pub fn rpc(request: &Value, call: impl Fn(&str, &Value) -> Result<Value>) -> Option<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    if request["jsonrpc"] != "2.0" || !request["method"].is_string() {
        return Some(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"Invalid Request"}}),
        );
    }
    request.get("id")?;
    let method = request["method"].as_str().unwrap();
    let result = match method {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","serverInfo":{"name":"herdr-titles","version":VERSION},"capabilities":{"tools":{}}})
        }
        "ping" => json!({}),
        "tools/list" => {
            json!({"tools":tools().into_iter().map(|mut t|{for key in ["http","command","uiActions","cli"]{t.as_object_mut().unwrap().remove(key);}t}).collect::<Vec<_>>()})
        }
        "tools/call" => {
            let empty = json!({});
            match call(
                request["params"]["name"].as_str().unwrap_or(""),
                request["params"].get("arguments").unwrap_or(&empty),
            ) {
                Ok(v) => {
                    json!({"content":[{"type":"text","text":v.to_string()}],"structuredContent":v})
                }
                Err(e) => json!({"isError":true,"content":[{"type":"text","text":e.to_string()}]}),
            }
        }
        _ => {
            return Some(
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}}),
            );
        }
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}
pub fn stdio<H: Host>(
    engine: &Engine<H>,
    mut input: impl BufRead,
    mut output: impl Write,
) -> Result<()> {
    loop {
        let mut line = vec![];
        let n = input
            .by_ref()
            .take(1024 * 1024 + 1)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            return Ok(());
        }
        anyhow::ensure!(line.len() <= 1024 * 1024, "MCP request exceeds 1 MiB");
        let reply = match serde_json::from_slice::<Value>(&line) {
            Ok(v) => rpc(&v, |name, args| engine.call(name, args)),
            Err(_) => Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
            ),
        };
        if let Some(v) = reply {
            writeln!(output, "{v}")?;
            output.flush()?;
        }
    }
}
pub fn http<H: Host>(engine: &Engine<H>, port: u16) -> Result<()> {
    let server =
        tiny_http::Server::http(("127.0.0.1", port)).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let addr = server
        .server_addr()
        .to_ip()
        .context("Missing loopback address")?;
    println!("{}", json!({"url":format!("http://{addr}")}));
    std::io::stdout().flush()?;
    for mut request in server.incoming_requests() {
        let host = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Host"))
            .map(|h| h.value.as_str());
        let origin = request.headers().iter().any(|h| h.field.equiv("Origin"));
        let expected = addr.to_string();
        let (code, body) = if origin || host != Some(expected.as_str()) {
            (403, json!({"error":"Loopback agent clients only"}))
        } else if request.method().as_str() == "GET" && request.url() == "/v1/tools" {
            (200, json!({"tools":tools()}))
        } else if let Some(tool) = tools().into_iter().find(|t| {
            t["http"]["method"] == request.method().as_str() && t["http"]["path"] == request.url()
        }) {
            let mut bytes = vec![];
            let read = request.as_reader().take(16385).read_to_end(&mut bytes);
            if bytes.len() > 16384 {
                (413, json!({"error":"Request too large"}))
            } else {
                let result = read
                    .map_err(anyhow::Error::from)
                    .and_then(|_| {
                        if bytes.is_empty() {
                            Ok(json!({}))
                        } else {
                            Ok(serde_json::from_slice::<Value>(&bytes)?)
                        }
                    })
                    .and_then(|args| engine.call(tool["name"].as_str().unwrap(), &args));
                match result {
                    Ok(v) => (200, v),
                    Err(e) => (400, json!({"error":e.to_string()})),
                }
            }
        } else {
            (404, json!({"error":"Unknown endpoint"}))
        };
        let response = tiny_http::Response::from_string(body.to_string())
            .with_status_code(code)
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap());
        let _ = request.respond(response);
    }
    Ok(())
}
