//! The MCP probe: what a previous measurement left behind, and the live
//! measurement itself — spawn a stdio server, ask tools/list, measure the
//! answer, kill the process. Port of `src/server/scan/mcpProbe.ts`.
//!
//! The live probe runs someone else's code, hence only on explicit action.

use crate::config;
use crate::cost;
use crate::jsonutil::{read_json, write_atomic};
use crate::lang::{Lang, t};
use crate::paths;
use crate::pluginmcp;
use crate::plugins;
use crate::stats::iso_now;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::sync::mpsc;

/// `v` is the measurement's own version: 1 counted the raw tools/list JSON
/// including icons and envelope, divided by 4; 2 counts the tool as the model
/// is sent it, divided by 2. An older entry is not stale but wrong — it reads
/// as unmeasured until taken again.
const MEASURE_VERSION: f64 = 2.0;

fn load_cache() -> serde_json::Map<String, Value> {
    // Order preserved: a rewritten cache must not churn every key's position.
    read_json(&paths::mcp_probe_cache(), Lang::Ru)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// Without a live probe the size of tools/list is unknown. No figure is
/// invented — unknown until the user clicks "Measure".
pub fn estimate_tokens_for_mcp(name: &str, lang: Lang) -> Value {
    let hit = load_cache().get(name).cloned().unwrap_or(Value::Null);
    let version = hit.get("v").and_then(|v| v.as_f64());
    if version != Some(MEASURE_VERSION) {
        return cost::unknown(&t(lang, "scan.probeNotMeasured", &[]));
    }
    let chars = hit.get("chars").and_then(|c| c.as_i64()).unwrap_or(0) as usize;
    let at = hit
        .get("at")
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();
    cost::measured_tool_json(
        chars,
        &t(
            lang,
            "scan.probeMeasuredNote",
            &[("date", &at.chars().take(10).collect::<String>())],
        ),
    )
}

/// Tools from the last probe: each one's size and what it says it does. The
/// only honest description of an MCP server with no owning plugin.
pub fn probed_tools(name: &str) -> Vec<Value> {
    let hit = load_cache().get(name).cloned().unwrap_or(Value::Null);
    let version = hit.get("v").and_then(|v| v.as_f64());
    if version != Some(MEASURE_VERSION) {
        return Vec::new();
    }
    hit.get("tools")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter(|x| x.is_object())
                .cloned()
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default()
}

/// The first line of a description, capped: a tool's blurb can run for paragraphs.
fn blurb(desc: &Value) -> Option<String> {
    let s = desc.as_str()?.trim();
    let line = s.split('\n').next().unwrap_or("").trim();
    if line.is_empty() {
        return None;
    }
    if line.chars().count() > 160 {
        Some(line.chars().take(159).collect::<String>() + "…")
    } else {
        Some(line.to_string())
    }
}

/// The list a server answered with, whatever shape it arrived in.
fn tool_list(result: &Value) -> Vec<Value> {
    result["tools"].as_array().cloned().unwrap_or_default()
}

/// One tool as the model is sent it: what it is called, and how big it is.
/// Three fields and no more — `icons` alone was 57 % of what the github server
/// answered with, and counting it made the figure wrong in both directions.
/// JSON.stringify's `.length` is UTF-16 units; the count follows, not bytes.
fn counted_tool(raw: &Value) -> Option<(String, usize)> {
    let name = raw["name"].as_str().filter(|s| !s.is_empty())?;
    let shape = json!({
        "name": name,
        "description": raw["description"],
        "inputSchema": raw["inputSchema"],
    });
    let text = serde_json::to_string(&shape).ok()?;
    Some((name.to_string(), text.encode_utf16().count()))
}

/// tools/list response → one entry per tool, sized as the model receives it.
pub fn tool_infos_of(result: &Value) -> Vec<Value> {
    tool_list(result)
        .iter()
        .filter_map(|raw| {
            let (name, chars) = counted_tool(raw)?;
            let desc = blurb(&raw["description"]);
            Some(match desc {
                Some(d) => {
                    json!({ "name": name, "tokens": cost::tokens_from_tool_json(chars), "desc": d })
                }
                None => json!({ "name": name, "tokens": cost::tokens_from_tool_json(chars) }),
            })
        })
        .collect()
}

/// The per-tool sizes added up — what the whole server costs in tools.
fn tools_chars_of(result: &Value) -> usize {
    tool_list(result)
        .iter()
        .filter_map(counted_tool)
        .map(|(_, c)| c)
        .sum()
}

/// A server's declaration: `~/.claude.json` first, then the plugins. A plugin
/// server is declared inside the plugin, not in ~/.claude.json — without the
/// fallback the measure button refuses to work on it.
fn read_server_raw(name: &str, lang: Lang) -> Option<Value> {
    if let Ok(cj) = read_json(&paths::claude_json(), lang)
        && let Some(s) = cj["mcpServers"][name].as_object()
    {
        return Some(Value::Object(s.clone()));
    }
    let installs = plugins::read_installs(lang);
    pluginmcp::plugin_mcp_servers(&installs, lang)
        .into_iter()
        .find(|s| s.name == name)
        .map(|s| s.raw)
}

fn q(s: &str) -> String {
    if cfg!(windows) && s.chars().any(|c| c.is_whitespace() || c == '"') {
        format!("\"{}\"", s)
    } else {
        s.to_string()
    }
}

/// Spawns a stdio server, asks for tools/list, measures the response size,
/// kills the process. This is the ONLY way to learn the server's real cost.
/// Two things land in context, so both are measured: the tools/list response
/// and the `instructions` string a server returns from initialize.
pub fn probe_mcp(name: &str, lang: Lang) -> Value {
    let raw = read_server_raw(name, lang);
    let raw = match raw {
        Some(r) => r,
        None => return cost::unknown(&t(lang, "scan.probeServerNotFound", &[("name", name)])),
    };
    let command = raw["command"].as_str().unwrap_or("");
    if !raw["url"].is_null() || command.is_empty() {
        return cost::unknown(&t(lang, "scan.probeHttpUnsupported", &[]));
    }

    // `cmd /d /s /c` is what `shell: true` does on Windows: it makes `npx.cmd`
    // runnable, and the quoting below is what keeps a spaced path one argument.
    let is_win = cfg!(windows);
    let args: Vec<String> = raw["args"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(q)).collect())
        .unwrap_or_default();
    let mut cmd = if is_win {
        let line = std::iter::once(q(command))
            .chain(args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        let mut c = Command::new("cmd");
        c.args(["/d", "/s", "/c", &line]);
        c
    } else {
        let mut c = Command::new(command);
        c.args(&args);
        c
    };
    if let Some(env) = raw["env"].as_object() {
        for (k, v) in env {
            if let Some(val) = v.as_str() {
                cmd.env(k, val);
            }
        }
    }

    let mut child = match cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return cost::unknown(&t(lang, "scan.probeNoResponse", &[])),
    };

    let mut stdin = child.stdin.take();
    let send = |stdin: &mut Option<std::process::ChildStdin>, msg: &Value| {
        if let Some(s) = stdin {
            let _ = writeln!(s, "{}", msg);
            let _ = s.flush();
        }
    };

    // The reader runs on its own thread: a silent server must not block the
    // read forever — the deadline below kills the child instead.
    let (tx, rx) = mpsc::channel::<String>();
    let mut out = child.stdout.take();
    let reader = std::thread::spawn(move || {
        if let Some(o) = out.take() {
            for line in BufReader::new(o).lines() {
                if tx.send(line.unwrap_or_default()).is_err() {
                    break;
                }
            }
        }
    });

    send(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "claude-lineup", "version": "0.1.0" },
            },
        }),
    );

    let timeout = std::time::Duration::from_millis(config::timeout_probe_ms());
    let deadline = std::time::Instant::now() + timeout;
    let mut tools_chars: Option<usize> = None;
    let mut instr_chars = 0usize;
    let mut tool_infos: Vec<Value> = Vec::new();

    'outer: loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            break;
        }
        match rx.recv_timeout(left.min(std::time::Duration::from_millis(50))) {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                let msg: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let id = msg["id"].as_i64();
                if id == Some(1) {
                    if let Some(s) = msg["result"]["instructions"].as_str() {
                        instr_chars = s.encode_utf16().count();
                    }
                    send(
                        &mut stdin,
                        &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
                    );
                    send(
                        &mut stdin,
                        &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
                    );
                } else if id == Some(2) {
                    tools_chars = Some(tools_chars_of(&msg["result"]) + instr_chars);
                    tool_infos = tool_infos_of(&msg["result"]);
                    break 'outer;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();

    let chars = match tools_chars {
        Some(c) => c,
        None => return cost::unknown(&t(lang, "scan.probeNoResponse", &[])),
    };

    let mut cache = load_cache();
    cache.insert(
        name.to_string(),
        json!({
            "chars": chars,
            "at": iso_now(),
            "tools": tool_infos,
            "v": MEASURE_VERSION,
        }),
    );
    // The TS original writes JSON.stringify(cache, null, 2) — no trailing newline.
    if let Ok(text) = serde_json::to_string_pretty(&Value::Object(cache)) {
        let _ = write_atomic(&paths::mcp_probe_cache(), &text);
    }
    cost::measured_tool_json(chars, &t(lang, "scan.probeLiveMeasure", &[]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::now_ms;

    /// A stub MCP server script that answers initialize and tools/list — the
    /// live probe can be tested without launching anyone's real server.
    fn stub_server_js() -> String {
        r#"const replies = {
  1: { result: { instructions: "Use politely." } },
  2: { result: { tools: [
    { name: "echo", description: "Echoes.\nLong second line.", inputSchema: { type: "object" } },
    { name: "big", description: "x".repeat(300), inputSchema: { type: "object", x: 1 } },
    { noName: true },
  ] } },
};
let buf = "";
process.stdin.on("data", (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf("\n")) >= 0) {
    const line = buf.slice(0, i); buf = buf.slice(i + 1);
    let msg; try { msg = JSON.parse(line); } catch { continue; }
    const r = replies[msg.id];
    if (r) process.stdout.write(JSON.stringify({ id: msg.id, ...r }) + "\n");
  }
});
setTimeout(() => process.exit(0), 30000);"#
            .to_string()
    }

    #[test]
    fn tool_infos_count_what_the_model_receives() {
        let result = json!({ "tools": [
            { "name": "echo", "description": "Echoes.\nMore.", "inputSchema": { "type": "object" } },
            { "noName": true },
        ]});
        let infos = tool_infos_of(&result);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0]["name"], "echo");
        assert!(infos[0]["tokens"].as_i64().unwrap() > 0);
        // The blurb is the first line only.
        assert_eq!(infos[0]["desc"], "Echoes.");
        let chars = tools_chars_of(&result);
        assert!(chars > 0);
    }

    #[test]
    fn blurb_capped_at_160_chars() {
        let long = json!("x".repeat(300));
        let b = blurb(&long).unwrap();
        assert_eq!(b.chars().count(), 160);
        assert!(b.ends_with('…'));
        assert_eq!(blurb(&json!("")), None);
    }

    #[test]
    fn live_probe_against_a_stub_child() {
        let dir = std::env::temp_dir().join(format!("cl-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let js = dir.join("stub.js");
        std::fs::write(&js, stub_server_js()).unwrap();

        let (tx, rx) = mpsc::channel::<String>();
        let mut child = Command::new("node")
            .arg(&js)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take();
        let mut out = child.stdout.take();
        let _ = std::thread::spawn(move || {
            for line in BufReader::new(out.take().unwrap())
                .lines()
                .map_while(Result::ok)
            {
                let _ = tx.send(line);
            }
        });
        let mut send = |m: Value| {
            let _ = writeln!(stdin.as_mut().unwrap(), "{}", m);
        };
        send(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }));
        send(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }));
        // A generous deadline: the suite runs tests in parallel and a cold node
        // start under load must not fail the check.
        let deadline = now_ms() + 30_000;
        let mut got2: Option<Value> = None;
        while now_ms() < deadline {
            match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(line) => {
                    let v: Value = serde_json::from_str(&line).unwrap();
                    if v["id"].as_i64() == Some(2) {
                        got2 = Some(v);
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        let v = got2.expect("stub answered tools/list");
        let infos = tool_infos_of(&v["result"]);
        assert_eq!(infos.len(), 2, "the nameless tool is not counted");
        // instructions land in the total, not in the tool rows.
        let without = tools_chars_of(&v["result"]);
        assert!(without > 0);
        assert!(infos.iter().any(|i| i["desc"].as_str() == Some("Echoes.")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
