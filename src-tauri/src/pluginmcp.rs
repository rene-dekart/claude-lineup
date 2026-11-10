//! MCP servers that plugins bring in — the port of
//! `src/server/scan/pluginMcp.ts`. They live in no settings file: the plugin
//! declares them and Claude Code starts them with the plugin. Two declaration
//! forms exist in the wild, both handled here:
//!
//!   <plugin>/.mcp.json                 — the plugin's own file
//!   <plugin>/.claude-plugin/plugin.json — inline `mcpServers`

use crate::jsonutil::{Obj, read_json};
use crate::lang::Lang;
use crate::plugins::PluginInstall;
use serde_json::{Value, json};
use std::path::Path;

fn norm(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// `${CLAUDE_PLUGIN_ROOT}` is the only variable an install path can appear as.
fn expand(v: &str, root: &str) -> String {
    v.split("${CLAUDE_PLUGIN_ROOT}")
        .collect::<Vec<_>>()
        .join(root)
}

fn server_ok(v: &Value) -> bool {
    v.is_object() && (v.get("command").is_some() || v.get("url").is_some())
}

/// `.mcp.json` *is* the map; a plugin manifest is not — it carries its servers
/// under `mcpServers` and plenty of other keys (`author: { url: … }` looks
/// exactly like an http server). So the bare form is allowed only for `.mcp.json`.
fn servers_in(raw: &Value, bare: bool) -> Obj {
    let mut out = Obj::new();
    let Some(o) = raw.as_object() else {
        return out;
    };
    let wrapped = o.get("mcpServers").and_then(|v| v.as_object());
    let map: &Obj = match wrapped {
        Some(w) => w,
        None if bare => o,
        None => return out,
    };
    for (name, v) in map {
        if server_ok(v) {
            out.insert(name.clone(), v.clone());
        }
    }
    out
}

fn plugin_name(install_path: &Path, id: &str, lang: Lang) -> String {
    let manifest = read_json(
        &install_path.join(".claude-plugin").join("plugin.json"),
        lang,
    )
    .unwrap_or(Value::Null);
    let n = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    // Fall back to the id without its marketplace: an id is `name@marketplace`.
    let fallback = id.split('@').next().filter(|s| !s.is_empty()).unwrap_or(id);
    norm(if n.is_empty() { fallback } else { &n })
}

pub struct PluginServer {
    pub name: String,
    pub plugin_id: String,
    /// Shape parity with the TS reference; nothing in Rust reads it yet.
    #[allow(dead_code)]
    pub transport: &'static str,
    pub command: Option<String>,
    pub url: Option<String>,
    pub file: String,
    /// Untouched config — the probe needs the real argv and env.
    pub raw: Value,
}

pub fn plugin_mcp_servers(installs: &[PluginInstall], lang: Lang) -> Vec<PluginServer> {
    let mut out = Vec::new();
    for inst in installs {
        let mut declared: Vec<(String, Value, String)> = Vec::new();
        let manifest_file = inst.install_path.join(".claude-plugin").join("plugin.json");
        if let Ok(manifest) = read_json(&manifest_file, lang) {
            for (k, v) in servers_in(&manifest, false) {
                declared.push((k, v, manifest_file.to_string_lossy().into_owned()));
            }
        }
        let own_file = inst.install_path.join(".mcp.json");
        if let Ok(own) = read_json(&own_file, lang) {
            for (k, v) in servers_in(&own, true) {
                // The plugin's own file wins over the manifest's declaration.
                declared.retain(|(name, _, _)| name != &k);
                declared.push((k, v, own_file.to_string_lossy().into_owned()));
            }
        }

        let pname = plugin_name(&inst.install_path, &inst.id, lang);
        let root = inst.install_path.to_string_lossy().into_owned();
        for (server, raw, file) in declared {
            let command = raw
                .get("command")
                .and_then(|v| v.as_str())
                .map(|c| expand(c, &root));
            let url = raw
                .get("url")
                .and_then(|v| v.as_str())
                .map(|u| expand(u, &root));
            let transport = if command.is_some() && url.is_none() {
                "stdio"
            } else {
                "http"
            };
            let mut clean = serde_json::Map::new();
            if let Some(c) = &command {
                clean.insert("command".into(), json!(c));
            }
            if let Some(args) = raw.get("args").and_then(|v| v.as_array()) {
                let expanded: Vec<String> = args
                    .iter()
                    .filter_map(|a| a.as_str())
                    .map(|a| expand(a, &root))
                    .collect();
                if !expanded.is_empty() || !args.is_empty() {
                    clean.insert("args".into(), json!(expanded));
                }
            }
            if let Some(env) = raw.get("env") {
                clean.insert("env".into(), env.clone());
            }
            if let Some(u) = &url {
                clean.insert("url".into(), json!(u));
            }
            out.push(PluginServer {
                name: format!("plugin_{}_{}", pname, norm(&server)),
                plugin_id: inst.id.clone(),
                transport,
                command,
                url,
                file,
                raw: Value::Object(clean),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn manifest_mcp_servers_are_read_bare_form_is_not() {
        let manifest = json!({
            "name": "ctx",
            "author": { "name": "x", "url": "https://example.com/a" },
            "mcpServers": { "srv": { "command": "node", "args": ["m.js"] } }
        });
        let found = servers_in(&manifest, false);
        assert_eq!(found.len(), 1, "manifest: only the mcpServers key counts");
        // The bare form on a manifest would invent a server out of `author`.
        let bare_manifest = json!({ "author": { "url": "https://example.com/a" } });
        assert_eq!(servers_in(&bare_manifest, false).len(), 0);
        // `.mcp.json` IS the map.
        let own = json!({ "srv2": { "url": "https://x/y" }, "nope": { "n": 1 } });
        let found_own = servers_in(&own, true);
        assert_eq!(found_own.len(), 1);
        assert!(found_own.contains_key("srv2"));
    }

    #[test]
    fn plugin_root_expands_and_names_normalize() {
        assert_eq!(expand("a${CLAUDE_PLUGIN_ROOT}b", "R"), "aRb");
        assert_eq!(norm("my_server-1"), "my-server-1");
    }
}
