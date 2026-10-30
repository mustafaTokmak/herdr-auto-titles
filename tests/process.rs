use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Child, Command, Stdio},
    time::Duration,
};
const BIN: &str = env!("CARGO_BIN_EXE_herdr-auto-titles");
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn native_binary_runs_outside_checkout_without_node() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join(if cfg!(windows) {
        "titles.exe"
    } else {
        "titles"
    });
    fs::copy(BIN, &bin).unwrap();
    let output = Command::new(&bin)
        .arg("--tools")
        .current_dir(dir.path())
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["tools"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    let output = Command::new(&bin)
        .arg("--skill")
        .current_dir(dir.path())
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("herdr_auto_titles_install")
    );
}
#[test]
fn native_stdio_serves_real_mcp_messages() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(BIN)
        .env("HERDR_APP_HOME", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"initialize"})
    )
    .unwrap();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})
    )
    .unwrap();
    drop(input);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let lines: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["id"], 2);
}
#[test]
fn native_http_routes_validate_origin_host_and_body_limit() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = Server(
        Command::new(BIN)
            .args(["--http", "0"])
            .env("HERDR_APP_HOME", dir.path())
            .env("HERDR_APP_BIN", dir.path().join("missing-app"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let ready: Value = serde_json::from_str(&line).unwrap();
    let addr = ready["url"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .unwrap();
    let request = |method: &str, path: &str, host: &str, extra: &str, body: &str| {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(stream,"{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n{extra}\r\n{body}",body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    };
    let response = request("GET", "/v1/tools", addr, "", "");
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("herdr_auto_titles_install"));
    assert!(
        request(
            "POST",
            "/v1/tab-title",
            addr,
            "Origin: https://example.com\r\n",
            "{}"
        )
        .starts_with("HTTP/1.1 403")
    );
    assert!(request("GET", "/v1/titles", "evil.example", "", "").starts_with("HTTP/1.1 403"));
    assert!(
        request("POST", "/v1/tab-title", addr, "", "{\"id\":\"t1\"}")
            .contains("missing argument: name")
    );
    assert!(
        request("POST", "/v1/tab-title", addr, "", &"x".repeat(17000)).starts_with("HTTP/1.1 413")
    );
    assert!(request("GET", "/v1/status", addr, "", "").contains("\"installed\":false"));
}
