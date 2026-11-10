//! The MCP pool of a project — the slice of `src/server/scan/mcp.ts` that
//! resolve needs: which servers a profile's switch can name, and where each is
//! declared. Plus the inventory rows themselves (`scan_mcp`).

use crate::cost;
use crate::jsonutil::{Obj, exists, read_json};
use crate::lang::{Lang, t};
use crate::paths;
use crate::pluginmcp::plugin_mcp_servers;
use crate::plugins::PluginInstall;
use crate::probe;
use crate::scan::disabled_mcpjson_refs;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The project key in `~/.claude.json` is the cwd exactly as the session saw
/// it: forward slashes, no trailing one.
pub fn project_key(cwd: &str) -> String {
    cwd.replace('\\', "/").trim_end_matches('/').to_string()
}

/// Windows paths differ in case and in which slash was typed, so an existing
/// entry is looked up that way before a new one is created.
pub fn find_project_key(projects: &Obj, cwd: &str) -> Option<String> {
    let want = project_key(cwd);
    if projects.contains_key(&want) {
        return Some(want);
    }
    let lower = want.to_lowercase();
    projects
        .keys()
        .find(|k| k.replace('\\', "/").to_lowercase() == lower)
        .cloned()
}

fn project_entry<'a>(claude_json: &'a Value, cwd: &str) -> Option<&'a Obj> {
    let key = find_project_key(claude_json.get("projects")?.as_object()?, cwd)?;
    claude_json
        .get("projects")?
        .get(&key)
        .and_then(|v| v.as_object())
}

/// Every server a settings file defines for this project: user → local →
/// project, the last one wins.
fn mcp_pool(cwd: &Path, claude_json_path: &Path, lang: Lang) -> Obj {
    let mut pool = Obj::new();
    let cj = read_json(claude_json_path, lang).unwrap_or(Value::Null);

    if let Some(servers) = cj.get("mcpServers").and_then(|v| v.as_object()) {
        pool.extend(servers.clone());
    }
    if let Some(entry) = project_entry(&cj, &cwd.to_string_lossy())
        && let Some(servers) = entry.get("mcpServers").and_then(|v| v.as_object())
    {
        pool.extend(servers.clone());
    }
    let mcp_file = paths::project_mcp_json(cwd);
    if exists(&mcp_file) {
        let r = read_json(&mcp_file, lang).unwrap_or(Value::Null);
        if let Some(servers) = r.get("mcpServers").and_then(|v| v.as_object()) {
            pool.extend(servers.clone());
        }
    }
    pool
}

/// The names in that pool — what "all" covers when the profile is read back.
pub fn mcp_names(cwd: &Path, claude_json_path: &Path, lang: Lang) -> Vec<String> {
    mcp_pool(cwd, claude_json_path, lang)
        .keys()
        .cloned()
        .collect()
}

/// The raw definitions for named servers — what the detail card reads a
/// repository out of when no installed plugin owns the name.
pub fn mcp_defs(
    cwd: &Path,
    claude_json_path: &Path,
    names: &[String],
    lang: Lang,
) -> HashMap<String, Value> {
    let pool = mcp_pool(cwd, claude_json_path, lang);
    names
        .iter()
        .filter_map(|n| pool.get(n).map(|d| (n.clone(), d.clone())))
        .collect()
}

/// Where each name is declared, which decides which file has to carry its
/// switch. Claude Code's project entry outranks the user's file for a name in
/// both, so that is the answer given.
pub fn mcp_origins(cwd: &Path, claude_json_path: &Path, lang: Lang) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let cj = read_json(claude_json_path, lang).unwrap_or(Value::Null);
    if let Some(servers) = cj.get("mcpServers").and_then(|v| v.as_object()) {
        for n in servers.keys() {
            out.insert(n.clone(), "user".to_string());
        }
    }
    if let Some(entry) = project_entry(&cj, &cwd.to_string_lossy())
        && let Some(servers) = entry.get("mcpServers").and_then(|v| v.as_object())
    {
        for n in servers.keys() {
            out.insert(n.clone(), "local".to_string());
        }
    }
    let mcp_file = paths::project_mcp_json(cwd);
    if exists(&mcp_file) {
        let r = read_json(&mcp_file, lang).unwrap_or(Value::Null);
        if let Some(servers) = r.get("mcpServers").and_then(|v| v.as_object()) {
            for n in servers.keys() {
                out.entry(n.clone())
                    .or_insert_with(|| "project".to_string());
            }
        }
    }
    out
}

// ── the inventory rows ──

fn transport_of(s: &Value) -> &'static str {
    let ty = s.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if ty == "http" || ty == "sse" || s.get("url").is_some() {
        "http"
    } else {
        "stdio"
    }
}

/// What the server does. MCP has no static description, so exactly two
/// sources: the owning plugin or tool names from a live probe. Neither
/// present — no row, rather than "MCP server".
fn describe(
    name: &str,
    owners: &HashMap<String, Vec<String>>,
    desc: &HashMap<String, String>,
    lang: Lang,
) -> Option<String> {
    for id in owners.get(name).map(|v| v.as_slice()).unwrap_or(&[]) {
        if let Some(d) = desc.get(id) {
            return Some(t(lang, "scan.mcpFromPlugin", &[("id", id), ("desc", d)]));
        }
    }
    let tools = probe::probed_tools(name);
    if !tools.is_empty() {
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|x| x.get("name").and_then(|n| n.as_str()))
            .collect();
        let head: String = names
            .iter()
            .take(6)
            .copied()
            .collect::<Vec<&str>>()
            .join(", ")
            + if names.len() > 6 { "…" } else { "" };
        return Some(t(
            lang,
            "scan.mcpTools",
            &[("count", &names.len().to_string()), ("head", &head)],
        ));
    }
    None
}

/// Cost of one server, in order of trust: the session's own count (it really
/// sent those tools), then our live probe, then nothing. Never a guess dressed
/// as a fact.
fn cost_of(name: &str, report: Option<&Value>, lang: Lang) -> Value {
    let real = report
        .and_then(|r| r.get("mcp"))
        .and_then(|m| m.get(name))
        .and_then(|v| v.as_i64());
    if let Some(tokens) = real {
        let date = report
            .and_then(|r| r.get("at"))
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .chars()
            .take(10)
            .collect::<String>();
        return cost::from_session(tokens, &t(lang, "scan.mcpFromSession", &[("date", &date)]));
    }
    probe::estimate_tokens_for_mcp(name, lang)
}

/// The inventory rows for MCP. Does NOT read settings.json — the key there is
/// silently ignored by Claude Code. The real sources: ~/.claude.json (user +
/// local scope) and <cwd>/.mcp.json (project), plus what plugins declare.
#[allow(clippy::too_many_arguments)]
pub fn scan_mcp(
    cwd: &Path,
    project_settings: &Value,
    owners: &HashMap<String, Vec<String>>,
    plugin_desc: &HashMap<String, String>,
    report: Option<&Value>,
    installs: &[PluginInstall],
    plugin_enabled: &HashMap<String, bool>,
    lang: Lang,
) -> Vec<Value> {
    let cwd_str = cwd.to_string_lossy().into_owned();
    let mut out: Vec<Value> = Vec::new();
    let cj = read_json(&paths::claude_json(), lang).unwrap_or(Value::Null);

    let proj_entry = project_entry(&cj, &cwd_str).cloned().unwrap_or_default();
    let locally_disabled: HashSet<&str> = proj_entry
        .get("disabledMcpServers")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    let from_files = disabled_mcpjson_refs(&read_settings_files_for_mcp(cwd, lang));
    // The two lists that really switch a server off, read together because
    // they are one switch to the row. `deniedMcpServers` is NOT read here: it
    // is a managed (enterprise) key, and a settings file that carries it
    // changes nothing.
    let mut locally_rejected: HashSet<String> = proj_entry
        .get("disabledMcpjsonServers")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    for name in from_files.keys() {
        locally_rejected.insert(name.clone());
    }
    let off_by_file =
        |name: &str| locally_disabled.contains(name) || locally_rejected.contains(name);

    // The same switch with names on it, for the row to print: which file, and
    // which key in that file, switches this server off for this project.
    let mut off_by: HashMap<String, Vec<Value>> = HashMap::new();
    let mut note = |name: &str, file: &str, key: &str| {
        let arr = off_by.entry(name.to_string()).or_default();
        if !arr.iter().any(|r| {
            r.get("file").and_then(|f| f.as_str()) == Some(file)
                && r.get("key").and_then(|k| k.as_str()) == Some(key)
        }) {
            arr.push(json!({ "file": paths::shorten(file, &cwd_str), "key": key }));
        }
    };
    if let Some(list) = proj_entry
        .get("disabledMcpServers")
        .and_then(|v| v.as_array())
    {
        for n in list.iter().filter_map(|x| x.as_str()) {
            note(
                n,
                &paths::claude_json().to_string_lossy(),
                "disabledMcpServers",
            );
        }
    }
    if let Some(list) = proj_entry
        .get("disabledMcpjsonServers")
        .and_then(|v| v.as_array())
    {
        for n in list.iter().filter_map(|x| x.as_str()) {
            note(
                n,
                &paths::claude_json().to_string_lossy(),
                "disabledMcpjsonServers",
            );
        }
    }
    for (n, files) in &from_files {
        for f in files {
            note(n, f, "disabledMcpjsonServers");
        }
    }
    let off_of = |name: &str| -> Value {
        match off_by.get(name) {
            Some(arr) if !arr.is_empty() => json!({ "offBy": arr }),
            _ => json!({}),
        }
    };

    let claude_json_path = paths::claude_json().to_string_lossy().into_owned();
    let describe_for = |name: &str, owners: &HashMap<String, Vec<String>>| {
        describe(name, owners, plugin_desc, lang)
    };

    let mut push_row = |name: &str,
                        raw: &Value,
                        scope: &str,
                        source_file: &str,
                        enabled: bool,
                        owners: &HashMap<String, Vec<String>>| {
        let mut row = json!({
            "kind": "mcp",
            "name": name,
            "scope": scope,
            "transport": transport_of(raw),
            "command": raw.get("command").cloned().unwrap_or(Value::Null),
            "url": raw.get("url").cloned().unwrap_or(Value::Null),
            "source": { "file": source_file },
            "enabled": enabled,
            "description": describe_for(name, owners),
            "cost": cost_of(name, report, lang),
        });
        if raw.get("command").and_then(|c| c.as_str()).is_none() {
            row.as_object_mut().unwrap().remove("command");
        }
        if raw.get("url").and_then(|u| u.as_str()).is_none() {
            row.as_object_mut().unwrap().remove("url");
        }
        if off_by_file(name) {
            row["offByFile"] = json!(true);
        }
        let extra = off_of(name);
        if let Some(obj) = extra.as_object() {
            for (k, v) in obj {
                row[k] = v.clone();
            }
        }
        out.push(row);
    };

    // user scope
    if let Some(servers) = cj.get("mcpServers").and_then(|v| v.as_object()) {
        for (name, raw) in servers {
            push_row(
                name,
                raw,
                "user",
                &claude_json_path,
                !locally_disabled.contains(name.as_str()),
                owners,
            );
        }
    }

    // local scope — MCP bound to a specific directory
    if let Some(servers) = proj_entry.get("mcpServers").and_then(|v| v.as_object()) {
        for (name, raw) in servers {
            push_row(
                name,
                raw,
                "local",
                &claude_json_path,
                !locally_disabled.contains(name.as_str()),
                owners,
            );
        }
    }

    // project scope — .mcp.json
    let mcp_file = paths::project_mcp_json(cwd);
    if exists(&mcp_file)
        && let Ok(r) = read_json(&mcp_file, lang)
    {
        let approved: HashSet<&str> = project_settings
            .get("enabledMcpjsonServers")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_default();
        let mut rejected: HashSet<&str> = project_settings
            .get("disabledMcpjsonServers")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_default();
        if let Some(list) = proj_entry
            .get("disabledMcpjsonServers")
            .and_then(|v| v.as_array())
        {
            for n in list.iter().filter_map(|x| x.as_str()) {
                rejected.insert(n);
            }
        }
        if let Some(servers) = r.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, raw) in servers {
                let mcp_file_str = mcp_file.to_string_lossy().into_owned();
                push_row(
                    name,
                    raw,
                    "project",
                    &mcp_file_str,
                    !rejected.contains(name.as_str()) && approved.contains(name.as_str()),
                    owners,
                );
            }
        }
    }

    // plugin scope — the server exists only as long as its plugin is on. The
    // declaring plugin is known here, so the description comes from it
    // directly rather than from the catalog's reverse map.
    let plugin_servers = plugin_mcp_servers(installs, lang);
    let mut with_owners = owners.clone();
    for s in &plugin_servers {
        with_owners
            .entry(s.name.clone())
            .or_default()
            .insert(0, s.plugin_id.clone());
    }
    for s in &plugin_servers {
        let mut raw = json!({});
        if let Some(c) = &s.command {
            raw["command"] = json!(c);
        }
        if let Some(u) = &s.url {
            raw["url"] = json!(u);
        }
        push_row(
            &s.name,
            &raw,
            "plugin",
            &s.file,
            plugin_enabled.get(&s.plugin_id).copied().unwrap_or(true),
            &with_owners,
        );
    }

    out
}

/// The settings files, read again here: the MCP rows need them, and scan.rs
/// hands nothing over — one extra small read beats widening the signature.
fn read_settings_files_for_mcp(cwd: &Path, lang: Lang) -> Vec<crate::scan::SettingsFile> {
    crate::scan::read_settings_files(cwd, lang).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-mcp-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, v: Value) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, v.to_string()).unwrap();
    }

    #[test]
    fn project_key_normalizes_separators_and_trailing_slash() {
        assert_eq!(project_key("D:\\a\\b\\"), "D:/a/b");
        assert_eq!(project_key("D:/a/b"), "D:/a/b");
    }

    #[test]
    fn existing_project_entries_are_found_despite_case_and_slashes() {
        let mut projects = Obj::new();
        projects.insert("D:/Work/Proj".into(), json!({}));
        let found = find_project_key(&projects, "d:\\work\\proj\\");
        assert_eq!(
            found.as_deref(),
            Some("D:/Work/Proj"),
            "the stored key is kept"
        );
        assert_eq!(find_project_key(&projects, "E:/nope"), None);
    }

    /// The pool layers user → local → project; a name declared twice keeps its
    /// first position but takes the last definition.
    #[test]
    fn pool_layers_and_the_last_declaration_wins() {
        let root = tmp("pool");
        let cj = root.join(".claude.json");
        let cwd = root.join("proj");
        let key = project_key(&cwd.to_string_lossy());
        write(
            &cj,
            json!({
                "mcpServers": { "u1": { "command": "a" }, "shared": { "command": "user" } },
                "projects": {
                    key: { "mcpServers": { "l1": { "command": "b" }, "shared": { "command": "local" } } }
                },
            }),
        );
        write(
            &paths::project_mcp_json(&cwd),
            json!({
                "mcpServers": { "p1": { "command": "c" }, "shared": { "command": "project" } },
            }),
        );

        let names = mcp_names(&cwd, &cj, Lang::Ru);
        assert_eq!(
            names,
            vec![
                "u1".to_string(),
                "shared".to_string(),
                "l1".to_string(),
                "p1".to_string()
            ],
            "existing keys keep their place, new ones append — the TS spread semantics"
        );
        let defs = mcp_defs(
            &cwd,
            &cj,
            &["shared".to_string(), "p1".to_string()],
            Lang::Ru,
        );
        assert_eq!(defs["shared"]["command"], "project");
        assert_eq!(defs["p1"]["command"], "c");
    }

    #[test]
    fn origins_follow_declaration_ownership_and_the_project_entry_outranks_user() {
        let root = tmp("origins");
        let cj = root.join(".claude.json");
        let cwd = root.join("proj");
        let key = project_key(&cwd.to_string_lossy());
        write(
            &cj,
            json!({
                "mcpServers": { "u1": {}, "shared": {} },
                "projects": {
                    key: { "mcpServers": { "shared": {}, "l1": {} } }
                },
            }),
        );
        write(
            &paths::project_mcp_json(&cwd),
            json!({
                "mcpServers": { "shared": {}, "p1": {} },
            }),
        );
        let o = mcp_origins(&cwd, &cj, Lang::Ru);
        assert_eq!(o["u1"], "user");
        assert_eq!(
            o["shared"], "local",
            "Claude Code's project entry outranks the user file"
        );
        assert_eq!(o["l1"], "local");
        assert_eq!(o["p1"], "project");
    }
}
