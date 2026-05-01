use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

#[test]
fn arka_mcp_server_handles_initialize_and_tools_list() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_arka-mcp-server"))
        .env("ARKA_CHAIN", "base")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn arka-mcp-server");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut lines = BufReader::new(stdout).lines();

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "test-version",
                "clientInfo": {
                    "name": "arka-test",
                    "version": "0.0.0"
                },
                "capabilities": {}
            }
        }),
    );

    let initialize = read_response(&mut lines);
    assert_eq!(initialize["id"], 1);
    assert_eq!(initialize["result"]["protocolVersion"], "test-version");
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "arka-mcp-server"
    );

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    );

    let tools = read_response(&mut lines);
    let names = tools["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect::<Vec<_>>();
    assert!(names.contains(&"balance"));
    assert!(names.contains(&"swap_quote"));
    assert!(names.contains(&"agent_account"));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "agent_account",
                "arguments": {
                    "action": "deposit",
                    "amount": "7"
                }
            }
        }),
    );

    let call = read_response(&mut lines);
    assert_eq!(call["id"], 3);
    assert_eq!(call["result"]["isError"], false);
    assert_eq!(call["result"]["structuredContent"]["balance"], "7");

    drop(stdin);
    child.kill().ok();
    child.wait().ok();
}

fn write_request(stdin: &mut std::process::ChildStdin, request: Value) {
    writeln!(stdin, "{request}").expect("write request");
    stdin.flush().expect("flush request");
}

fn read_response(lines: &mut impl Iterator<Item = std::io::Result<String>>) -> Value {
    let line = lines.next().expect("response line").expect("read response");
    serde_json::from_str(&line).expect("response json")
}
