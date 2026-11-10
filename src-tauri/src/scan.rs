//! The inventory scan — the port of `src/server/scan/*` (settings, skills,
//! hooks, memory, permissions) and the fold in `index.ts`. MCP rows and the
//! catalog live in their own modules; this file reads the settings files once
//! and hands every scanner its slice.

use crate::catalog;
use crate::cost;
use crate::creport;
use crate::jsonutil::{exists, list_dir, read_json};
use crate::lang::{Lang, t};
use crate::mcp::scan_mcp;
use crate::paths;
use crate::plugins::{merged_enabled_plugins, read_installs, scan_plugins};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct SettingsFile {
    pub scope: &'static str,
    pub file: PathBuf,
    pub data: Value,
}

/// Reads three project settings files in ascending order of priority.
/// managed/enterprise is not read: it is out of reach and we cannot override it.
pub fn read_settings_files(cwd: &Path, lang: Lang) -> (Vec<SettingsFile>, Vec<Value>) {
    let candidates: [(&'static str, PathBuf); 3] = [
        ("user", paths::user_settings()),
        ("project", paths::project_settings(cwd)),
        ("local", paths::project_local_settings(cwd)),
    ];
    let mut files = Vec::new();
    let mut unreadable = Vec::new();
    for (scope, file) in candidates {
        match read_json(&file, lang) {
            Ok(data) => files.push(SettingsFile {
                scope,
                file: file.clone(),
                data,
            }),
            Err(error) => {
                if file.exists() {
                    unreadable.push(json!({
                        "file": file.to_string_lossy(),
                        "error": error,
                    }));
                }
            }
        }
    }
    (files, unreadable)
}

const SKILL_STATES: [&str; 4] = ["on", "name-only", "user-invocable-only", "off"];

/// Whether a skill state leaves the skill in reach. Only `off` and
/// `user-invocable-only` take it out of the model's hands.
pub fn skill_state_on(state: &str) -> bool {
    state != "off" && state != "user-invocable-only"
}

/// skillOverrides from all levels, the highest priority winning per name.
/// A skill missing from it is a skill no file has an opinion about — not `on`.
pub fn merged_skill_overrides(files: &[SettingsFile]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for f in files {
        let Some(so) = f.data.get("skillOverrides").and_then(|s| s.as_object()) else {
            continue;
        };
        for (name, v) in so {
            let Some(state) = v.as_str().filter(|s| SKILL_STATES.contains(s)) else {
                continue;
            };
            out.insert(name.clone(), state.to_string());
        }
    }
    out
}

/// Server names a settings file refuses to load, each with the files that
/// named it. The key is `disabledMcpjsonServers`; `deniedMcpServers` looks
/// like the same thing and is not — it is honored from managed settings alone.
pub fn disabled_mcpjson_refs(files: &[SettingsFile]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for f in files {
        let Some(d) = f
            .data
            .get("disabledMcpjsonServers")
            .and_then(|x| x.as_array())
        else {
            continue;
        };
        for n in d {
            let Some(name) = n.as_str() else { continue };
            let arr = out.entry(name.to_string()).or_default();
            let file_str = f.file.to_string_lossy().into_owned();
            if !arr.contains(&file_str) {
                arr.push(file_str);
            }
        }
    }
    out
}

// ── hooks ──

fn parse_hooks_block(block: &Value, scope: &str, file: &str, lang: Lang, out: &mut Vec<Value>) {
    let Some(block) = block.as_object() else {
        return;
    };
    for (event, matchers) in block {
        let Some(matchers) = matchers.as_array() else {
            continue;
        };
        for m in matchers {
            for h in m
                .get("hooks")
                .and_then(|h| h.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(&[])
            {
                let Some(command) = h.get("command").and_then(|c| c.as_str()) else {
                    continue;
                };
                let is_session_start = event == "SessionStart";
                let note = if is_session_start {
                    t(lang, "scan.hookSessionStartCost", &[])
                } else {
                    t(lang, "scan.hookCost", &[])
                };
                out.push(json!({
                    "kind": "hook",
                    "event": event,
                    "matcher": m.get("matcher").and_then(|x| x.as_str()).unwrap_or("*"),
                    "command": command,
                    "scope": scope,
                    "source": { "file": file },
                    "cost": cost::unknown(&note),
                }));
            }
        }
    }
}

pub fn scan_hooks(
    files: &[SettingsFile],
    installs: &[crate::plugins::PluginInstall],
    lang: Lang,
) -> Vec<Value> {
    let mut out = Vec::new();
    for f in files {
        let file = f.file.to_string_lossy().into_owned();
        parse_hooks_block(
            &f.data.get("hooks").cloned().unwrap_or(Value::Null),
            f.scope,
            &file,
            lang,
            &mut out,
        );
    }
    for inst in installs {
        let f = inst.install_path.join("hooks").join("hooks.json");
        if let Ok(v) = read_json(&f, lang) {
            let file = f.to_string_lossy().into_owned();
            parse_hooks_block(&v, "plugin", &file, lang, &mut out);
        }
    }
    out
}

// ── permissions ──

/// The operations the Permissions section offers to decide about — one row
/// each. Pure structure: the visible title and the "why" live in messages.json
/// (`perm.<group>.<slug>.name` / `.why`), so both languages ship.
const PERMISSION_CATALOG: [(&str, &str, &str, &[&str]); 24] = [
    // files
    (
        "files",
        "code",
        "./src/**",
        &["Read(./src/**)", "Edit(./src/**)"],
    ),
    (
        "files",
        "tests",
        "./tests/**",
        &["Read(./tests/**)", "Edit(./tests/**)"],
    ),
    (
        "files",
        "depsScripts",
        "package.json",
        &["Edit(./package.json)"],
    ),
    (
        "files",
        "ci",
        ".github/workflows/**",
        &["Edit(./.github/workflows/**)"],
    ),
    (
        "files",
        "dockerImage",
        "Dockerfile",
        &["Edit(./Dockerfile)"],
    ),
    // repo
    ("repo", "status", "git status", &["Bash(git status:*)"]),
    ("repo", "diff", "git diff", &["Bash(git diff:*)"]),
    ("repo", "log", "git log", &["Bash(git log:*)"]),
    ("repo", "commit", "git commit", &["Bash(git commit:*)"]),
    ("repo", "push", "git push", &["Bash(git push:*)"]),
    ("repo", "rebase", "git rebase", &["Bash(git rebase:*)"]),
    (
        "repo",
        "resetHard",
        "git reset --hard",
        &["Bash(git reset --hard:*)"],
    ),
    ("repo", "publish", "npm publish", &["Bash(npm publish:*)"]),
    (
        "repo",
        "deploy",
        "npm run deploy",
        &["Bash(npm run deploy:*)"],
    ),
    // system
    ("system", "test", "npm test", &["Bash(npm test:*)"]),
    (
        "system",
        "build",
        "npm run build",
        &["Bash(npm run build:*)"],
    ),
    ("system", "typecheck", "npx tsc", &["Bash(npx tsc:*)"]),
    ("system", "install", "npm install", &["Bash(npm install:*)"]),
    (
        "system",
        "pipInstall",
        "pip install",
        &["Bash(pip install:*)"],
    ),
    ("system", "curl", "curl", &["Bash(curl:*)"]),
    ("system", "wget", "wget", &["Bash(wget:*)"]),
    ("system", "rm", "rm", &["Bash(rm:*)"]),
    ("system", "rmRf", "rm -rf", &["Bash(rm -rf:*)"]),
    ("system", "sudo", "sudo", &["Bash(sudo:*)"]),
];

/// The data rows — `deny` beats `ask` beats `allow`, wherever the rule sits.
const PERMISSION_CATALOG_DATA: [(&str, &str, &str, &[&str]); 4] = [
    ("data", "secrets", ".env", &["Read(./.env)", "Edit(./.env)"]),
    (
        "data",
        "envFiles",
        ".env.*",
        &["Read(./.env.*)", "Edit(./.env.*)"],
    ),
    ("data", "secretsDir", "secrets/**", &["Read(./secrets/**)"]),
    ("data", "sshKeys", "~/.ssh/**", &["Read(~/.ssh/**)"]),
];

fn rule_rank(state: &str) -> u8 {
    match state {
        "allow" => 0,
        "ask" => 1,
        _ => 2,
    }
}

/// What the three settings files already say, rule by rule — every rule, not
/// only the ones the catalog ships a row for (the server cards read this map).
/// Files arrive in ascending priority: the later file wins a tie — and `>=`
/// keeps that true while still letting a deny in a lower file beat an ask in
/// a higher one.
pub fn file_rule_states(files: &[SettingsFile]) -> HashMap<String, (String, String)> {
    let mut found: HashMap<String, (String, String)> = HashMap::new();
    for f in files {
        let Some(perms) = f.data.get("permissions").and_then(|p| p.as_object()) else {
            continue;
        };
        for state in ["allow", "ask", "deny"] {
            let Some(list) = perms.get(state).and_then(|l| l.as_array()) else {
                continue;
            };
            for rule in list {
                let Some(rule) = rule.as_str() else { continue };
                let prev = found.get(rule);
                let better = match prev {
                    None => true,
                    Some((prev_state, _)) => rule_rank(state) >= rule_rank(prev_state),
                };
                if better {
                    found.insert(rule.to_string(), (state.to_string(), f.scope.to_string()));
                }
            }
        }
    }
    found
}

/// The catalog's rows, with the files' own state where they have one. A row
/// may need two rules and the files may disagree — the stronger answer decides
/// what happens.
pub fn scan_permissions(files: &[SettingsFile], lang: Lang) -> Vec<Value> {
    let all = file_rule_states(files);
    let shipped: HashSet<&str> = PERMISSION_CATALOG
        .iter()
        .chain(PERMISSION_CATALOG_DATA.iter())
        .flat_map(|(_, _, _, rules)| rules.iter())
        .copied()
        .collect();
    let found: HashMap<&str, &(String, String)> = all
        .iter()
        .filter(|(rule, _)| shipped.contains(rule.as_str()))
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    PERMISSION_CATALOG
        .iter()
        .chain(PERMISSION_CATALOG_DATA.iter())
        .map(|(group, slug, cmd, rules)| {
            let mut hit: Option<&&(String, String)> = None;
            for r in *rules {
                if let Some(f) = found.get(r)
                    && hit.is_none_or(|h| rule_rank(&f.0) > rule_rank(&h.0))
                {
                    hit = Some(f);
                }
            }
            let state = match hit {
                Some(h) => json!(h.0),
                None => Value::Null,
            };
            let mut row = json!({
                "name": t(lang, &format!("perm.{group}.{slug}.name"), &[]),
                "cmd": cmd,
                "why": t(lang, &format!("perm.{group}.{slug}.why"), &[]),
                "rules": rules,
                "state": state,
                "group": group,
            });
            if let Some(h) = hit {
                row["scope"] = json!(h.1);
            }
            row
        })
        .collect()
}

// ── memory ──

/// CLAUDE.md files are read from the root down the tree, plus the global one.
/// Nested ones are picked up only after a file next to them is accessed — so
/// they show with a separate note.
pub fn scan_memory(cwd: &Path, report: Option<&Value>, lang: Lang) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    // Report keys already shown — what is left over is what we do not scan.
    let mut seen: HashSet<String> = HashSet::new();
    let date = report
        .and_then(|r| r.get("at"))
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .chars()
        .take(10)
        .collect::<String>();

    let cwd_norm = paths::norm(cwd);

    let mut push = |file: PathBuf, note: &str, out: &mut Vec<Value>| {
        if !exists(&file) {
            return;
        }
        let file_str = file.to_string_lossy().into_owned();
        // The session counted this file's real cost — an imported file and the
        // same file read as bytes are different numbers, and the real one wins.
        if let Some((key, tokens)) = creport::report_memory_tokens(report, &file_str) {
            seen.insert(key);
            out.push(json!({
                "kind": "memory",
                "path": file_str,
                "source": { "file": file_str },
                "cost": cost::from_session(tokens, &t(lang, "scan.memoryFromSession", &[("date", &date)])),
            }));
        } else {
            let size = cost::file_size(&file);
            out.push(json!({
                "kind": "memory",
                "path": file_str,
                "source": { "file": file_str },
                "cost": cost::estimated_chars(size, &t(lang, note, &[])),
            }));
        }
    };

    push(
        paths::claude_dir().join("CLAUDE.md"),
        "scan.memoryGlobal",
        &mut out,
    );

    let mut chain: Vec<PathBuf> = Vec::new();
    let mut dir = cwd.to_path_buf();
    loop {
        chain.insert(0, dir.clone());
        match dir.parent() {
            Some(p) if p != dir => dir = p.to_path_buf(),
            _ => break,
        }
    }
    for d in chain {
        if paths::norm(&d) == cwd_norm {
            push(d.join("CLAUDE.md"), "scan.memoryProject", &mut out);
            push(
                d.join(".claude").join("CLAUDE.md"),
                "scan.memoryProject",
                &mut out,
            );
        } else {
            push(d.join("CLAUDE.md"), "scan.memoryParent", &mut out);
        }
    }

    // Files the session loaded that are not on the CLAUDE.md path: imports
    // (`@RTK.md`) and the auto-memory index. The session is the only source
    // that knows about them.
    if let Some(mem) = report
        .and_then(|r| r.get("memory"))
        .and_then(|m| m.as_object())
    {
        for (key, tokens) in mem {
            if seen.contains(key) {
                continue;
            }
            out.push(json!({
                "kind": "memory",
                "path": key,
                "source": { "file": key },
                "cost": cost::from_session(
                    tokens.as_i64().unwrap_or(0),
                    &t(lang, "scan.memoryFromSessionExtra", &[("date", &date)]),
                ),
            }));
        }
    }
    out
}

// ── skills ──

/// Exactly the text that goes into the context: description first, otherwise
/// when_to_use.
fn skill_description(front: Option<&Map<String, Value>>) -> Option<String> {
    let get = |k: &str| {
        front
            .and_then(|f| f.get(k))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };
    let d = get("description").trim();
    let d = if d.is_empty() {
        get("when_to_use").trim()
    } else {
        d
    };
    if d.is_empty() {
        None
    } else {
        Some(d.to_string())
    }
}

fn collect(
    root: &Path,
    origin: &str,
    plugin: Option<&str>,
    overrides: &HashMap<String, String>,
    lang: Lang,
    out: &mut Vec<Value>,
) {
    for name in list_dir(root) {
        let dir = root.join(&name);
        let f = dir.join("SKILL.md");
        if !exists(&f) {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        let front = cost::parse_frontmatter(&text);
        let front_ref = front.as_ref();
        let chars = front_ref
            .and_then(|fm| fm.get("description"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .len()
            + front_ref
                .and_then(|fm| fm.get("when_to_use"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .len();
        // Without disable-model-invocation the description sits in the context
        // and costs money.
        let model_invocable = front_ref
            .and_then(|fm| fm.get("disable-model-invocation"))
            .and_then(|v| v.as_str())
            != Some("true");
        let state = overrides.get(&name);
        // A plugin skill's own state map is not read for it at all — only
        // turning the whole plugin off turns it off, so the file's word does
        // not apply.
        let off_by_file = plugin.is_none() && state.is_some_and(|s| !skill_state_on(s));
        let note = if chars == 0 {
            t(lang, "scan.skillNoDescription", &[])
        } else {
            t(lang, "scan.skillFrontmatterCost", &[])
        };
        let mut row = json!({
            "kind": "skill",
            "name": name,
            "origin": origin,
            "modelInvocable": model_invocable,
            "description": skill_description(front_ref),
            "source": { "file": f.to_string_lossy() },
            "cost": cost::estimated_chars(chars, &note),
        });
        if let Some(p) = plugin {
            row["plugin"] = json!(p);
        }
        if off_by_file {
            row["offByFile"] = json!(true);
        }
        out.push(row);
    }
}

pub fn scan_skills(
    cwd: &Path,
    installs: &[crate::plugins::PluginInstall],
    overrides: &HashMap<String, String>,
    lang: Lang,
) -> Vec<Value> {
    let mut out = Vec::new();
    collect(
        &paths::user_skills(),
        "user",
        None,
        overrides,
        lang,
        &mut out,
    );
    collect(
        &paths::project_skills(cwd),
        "project",
        None,
        overrides,
        lang,
        &mut out,
    );
    for inst in installs {
        collect(
            &inst.install_path.join("skills"),
            "plugin",
            Some(&inst.id),
            overrides,
            lang,
            &mut out,
        );
    }
    out
}

// ── the fold ──

pub fn build_inventory(cwd: &Path, lang: Lang) -> Value {
    let mut notes: Vec<String> = Vec::new();
    let (files, unreadable) = read_settings_files(cwd, lang);
    let installs = read_installs(lang);
    // The catalog is needed not only as a list of uninstalled items: plugin
    // descriptions and the "which plugin dragged in which MCP" map come from it.
    let descriptions = catalog::plugin_descriptions(lang);
    // Claude Code's own count of the last session in this project. Where it
    // has a number, that number wins over our estimate.
    let report = creport::read_context_report(&cwd.to_string_lossy());

    let plugins = scan_plugins(
        &merged_enabled_plugins(&files),
        &installs,
        &descriptions,
        lang,
    );
    let plugin_enabled: HashMap<String, bool> = plugins
        .iter()
        .filter_map(|p| {
            let id = p.get("id")?.as_str()?;
            let en = p.get("enabled")?.as_bool()?;
            Some((id.to_string(), en))
        })
        .collect();

    let mut entities: Vec<Value> = Vec::new();
    let project_settings = files
        .iter()
        .find(|f| f.scope == "project")
        .map(|f| f.data.clone())
        .unwrap_or(Value::Object(Map::new()));

    entities.extend(scan_mcp(
        cwd,
        &project_settings,
        &catalog::mcp_owners(lang),
        &descriptions,
        report.as_ref(),
        &installs,
        &plugin_enabled,
        lang,
    ));
    entities.extend(plugins);

    // The files' own switch, read for the same reason `enabledPlugins` is: the
    // rows answer "will this be in the session", and a skill a file turned off
    // stays off however the box is drawn.
    let overrides = merged_skill_overrides(&files);
    let skills = scan_skills(cwd, &installs, &overrides, lang);
    entities.extend(skills);

    let hooks = scan_hooks(&files, &installs, lang);
    entities.extend(hooks);

    entities.extend(scan_memory(cwd, report.as_ref(), lang));

    if !unreadable.is_empty() {
        notes.push(t(
            lang,
            "scan.unreadableFiles",
            &[("count", &unreadable.len().to_string())],
        ));
    }

    // One fold, read twice: the rows want the catalog's rules with their scope,
    // the card wants every rule with just its state.
    let rules = file_rule_states(&files);
    let file_rules: Map<String, Value> = rules
        .iter()
        .map(|(rule, (state, _))| (rule.clone(), json!(state)))
        .collect();

    json!({
        "entities": entities,
        "permissions": scan_permissions(&files, lang),
        "fileRules": Value::Object(file_rules),
        "unreadable": unreadable,
        "notes": notes,
        "scannedAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "report": report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sf(scope: &'static str, permissions: Value) -> SettingsFile {
        SettingsFile {
            scope,
            file: PathBuf::from(scope),
            data: serde_json::json!({ "permissions": permissions }),
        }
    }

    #[test]
    fn deny_beats_ask_beats_allow_across_files() {
        let files = vec![
            sf(
                "user",
                serde_json::json!({ "allow": ["Bash(rm:*)"], "ask": ["Bash(git push:*)"] }),
            ),
            sf("local", serde_json::json!({ "deny": ["Bash(rm:*)"] })),
        ];
        let rules = file_rule_states(&files);
        assert_eq!(rules.get("Bash(rm:*)").unwrap().0, "deny");
        assert_eq!(rules.get("Bash(git push:*)").unwrap().0, "ask");
    }

    #[test]
    fn later_file_wins_a_tie() {
        let files = vec![
            sf("user", serde_json::json!({ "deny": ["Edit(./src/**)"] })),
            sf(
                "project",
                serde_json::json!({ "allow": ["Edit(./src/**)"] }),
            ),
            sf("local", serde_json::json!({ "deny": ["Edit(./src/**)"] })),
        ];
        let rules = file_rule_states(&files);
        let (state, scope) = rules.get("Edit(./src/**)").unwrap();
        assert_eq!(state, "deny");
        assert_eq!(scope, "local");
    }

    #[test]
    fn skill_state_on_follows_the_two_off_states() {
        assert!(skill_state_on("on"));
        assert!(skill_state_on("name-only"));
        assert!(!skill_state_on("off"));
        assert!(!skill_state_on("user-invocable-only"));
    }

    /// Catalog rows: localized in both languages, the strongest file wins, a
    /// row without a rule stays undecided.
    #[test]
    fn permission_rows_localize_and_take_the_strongest_state() {
        let files = vec![
            sf("user", serde_json::json!({ "allow": ["Bash(rm:*)"] })),
            sf(
                "local",
                serde_json::json!({ "ask": ["Bash(rm:*)"], "deny": ["Bash(git push:*)"] }),
            ),
        ];
        let en = scan_permissions(&files, Lang::En);
        let rm = en.iter().find(|r| r["cmd"] == "rm").expect("rm row");
        assert_eq!(rm["name"], "Deleting files");
        assert_eq!(rm["state"], "ask", "a later ask beats an earlier allow");
        assert_eq!(rm["scope"], "local");
        let push = en.iter().find(|r| r["cmd"] == "git push").unwrap();
        assert_eq!(push["state"], "deny");
        let sudo = en.iter().find(|r| r["cmd"] == "sudo").unwrap();
        assert!(sudo["state"].is_null(), "no rule in any file — undecided");
        assert!(sudo.get("scope").is_none());

        let ru = scan_permissions(&files, Lang::Ru);
        assert_eq!(
            ru.iter().find(|r| r["cmd"] == "rm").unwrap()["name"],
            "Удаление файлов"
        );
        assert_eq!(en.len(), ru.len());
        assert_eq!(en.len(), 28, "the whole catalog ships, decided or not");
    }
}
