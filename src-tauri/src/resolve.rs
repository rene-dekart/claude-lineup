//! The only resolution there is: files written into the project.
//! Port of `src/server/resolve/index.ts`.

use crate::backups::{FileChange, point_slots, save_restore_point};
use crate::jsonutil::{Obj, deep_merge, read_text, to_pretty};
use crate::lang::{Lang, t};
use crate::mcp;
use crate::patch::{disabled_servers_patch, settings_patch, split_by_origin};
use crate::paths;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize, Clone)]
pub struct Resolution {
    pub changes: Vec<FileChange>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct ApplyResult {
    pub written: Vec<String>,
    pub backups: Vec<String>,
    pub point: Option<String>,
}

fn read_json_map(file: &Path) -> Obj {
    read_text(file)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// `~/.claude.json` — the whole document, read-modify-written. Only the one
/// key of the one project entry moves. A broken `~/.claude.json` is Claude
/// Code's to fix, not ours.
fn claude_json_change(
    file: &Path,
    cwd: &str,
    in_claude_json: &[String],
    known: &[String],
) -> Option<FileChange> {
    let before = read_text(file)?;
    let mut doc: Value = serde_json::from_str(&before).ok()?;
    if !doc.get("projects").map(|p| p.is_object()).unwrap_or(false) {
        // No projects object: nothing this function could mean to change.
        if in_claude_json.is_empty() {
            return None;
        }
        doc["projects"] = json!({});
    }
    let projects = doc.get_mut("projects").unwrap().as_object_mut().unwrap();
    let found = mcp::find_project_key(projects, cwd);
    // A copy, so the entry the file already had is not half-edited on its way
    // to the equality check below.
    let mut entry: Obj = match &found {
        Some(k) => projects
            .get(k)
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default(),
        None => Obj::new(),
    };
    let next = disabled_servers_patch(
        entry.get("disabledMcpServers").unwrap_or(&Value::Null),
        in_claude_json,
        known,
    );
    if !next.is_empty() {
        entry.insert("disabledMcpServers".into(), json!(next));
    } else {
        entry.remove("disabledMcpServers");
    }
    if entry.is_empty() {
        // An entry that exists only to hold a switch goes with the switch: an
        // empty object in the user's session state is litter. Nothing to clean
        // and nothing to write — the file is not touched at all.
        let k = found?;
        projects.remove(&k);
    } else {
        let key = found.unwrap_or_else(|| mcp::project_key(cwd));
        projects.insert(key, Value::Object(entry));
    }
    // No trailing newline: the file's own shape, two-space JSON.
    let after = serde_json::to_string_pretty(&doc).ok()?;
    if after == before {
        return None;
    }
    Some(FileChange {
        path: file.to_string_lossy().into_owned(),
        before: Some(before),
        after: Some(after),
    })
}

/// `.claude/settings.local.json` — the switch for a server the project's own
/// `.mcp.json` declares.
fn project_mcp_change(cwd: &Path, names: &[String], known: &[String]) -> Option<FileChange> {
    let file = paths::project_local_settings(cwd);
    let before = read_text(&file);
    // Nothing to switch off and no file to clean up: an empty local settings
    // file would be a file we wrote for no reason.
    if before.is_none() && names.is_empty() {
        return None;
    }
    let mut current = read_json_map(&file);
    let next = disabled_servers_patch(
        current
            .get("disabledMcpjsonServers")
            .unwrap_or(&Value::Null),
        names,
        known,
    );
    if !next.is_empty() {
        current.insert("disabledMcpjsonServers".into(), json!(next));
    } else {
        current.remove("disabledMcpjsonServers");
    }
    let after = to_pretty(&Value::Object(current));
    if Some(&after) == before.as_ref() {
        return None;
    }
    Some(FileChange {
        path: file.to_string_lossy().into_owned(),
        before,
        after: Some(after),
    })
}

pub fn resolve(p: &Value, cwd: &Path, lang: Lang) -> Resolution {
    let claude_json = paths::claude_json();
    let mut warnings: Vec<String> = vec![];
    let mut changes: Vec<FileChange> = vec![];

    // MCP: a server the profile switched off is named in the one list Claude
    // Code reads for the file it is declared in. `all` is the profile saying
    // nothing about MCP, and then neither list is touched rather than
    // rewritten to match.
    let mode = p["mcp"]["mode"].as_str().unwrap_or("all");
    let known: Option<Vec<String>> = if mode == "all" {
        None
    } else {
        Some(mcp::mcp_names(cwd, &claude_json, lang))
    };
    let off: Vec<String> = match &known {
        None => vec![],
        Some(known) => {
            let servers: Vec<String> = p["mcp"]["servers"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            if mode == "disable" {
                known.clone()
            } else {
                known
                    .iter()
                    .filter(|n| !servers.contains(n))
                    .cloned()
                    .collect()
            }
        }
    };

    // The switch is a command-line flag; settings.json has no counterpart for
    // it — the honest warning instead of a pretend write.
    if p["skills"]["mode"].as_str() == Some("none") {
        warnings.push(t(lang, "sess.projectSkillsNone", &[]));
    }
    if p["memory"]["autoDiscover"].as_bool() == Some(false) {
        warnings.push(t(lang, "sess.projectMemoryNeedsBare", &[]));
    }
    if p["hooks"]["disableAll"].as_bool() != Some(true) {
        warnings.push(t(lang, "sess.projectHooksAllOrNothing", &[]));
    }

    // settings.json
    let sfile = paths::project_settings(cwd);
    let before = read_text(&sfile);
    let current: Value = match &before {
        Some(b) => serde_json::from_str(b).unwrap_or_else(|_| json!({})),
        None => json!({}),
    };
    let existing_permissions = current.get("permissions").cloned();
    let patch = settings_patch(p, existing_permissions.as_ref());
    if p["claude"].get("mcpServers").is_some() {
        warnings.push(t(lang, "sess.mcpServersDroppedFromProject", &[]));
    }
    if p["skills"]["states"]
        .as_object()
        .map(|o| !o.is_empty())
        .unwrap_or(false)
    {
        warnings.push(t(lang, "sess.skillStatesProject", &[]));
    }
    let mut merged = deep_merge(&current, &json!(patch.clone()));
    // `permissions` is the one key the per-key merge cannot handle: when the
    // profile has an opinion, the patch's object is the whole picture — and it
    // replaces, whether that means a value or no key at all.
    let rules = p["permissions"]["rules"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if !rules.is_empty() {
        match patch.get("permissions") {
            Some(perms) => {
                merged["permissions"] = perms.clone();
            }
            None => {
                if let Some(o) = merged.as_object_mut() {
                    o.remove("permissions");
                }
            }
        }
    }
    // `deniedMcpServers` is the key this app used to write, and Claude Code
    // ignores it — it goes out when the profile has an opinion about MCP.
    if known.is_some()
        && let Some(o) = merged.as_object_mut()
    {
        o.remove("deniedMcpServers");
    }
    let after = to_pretty(&merged);
    // Always listed, even when identical: the pane shows the whole file. An
    // unchanged write is skipped in apply_files instead.
    changes.push(FileChange {
        path: sfile.to_string_lossy().into_owned(),
        before,
        after: Some(after),
    });

    // The MCP switches, in the files that carry them.
    if let Some(known) = &known {
        let origins = mcp::mcp_origins(cwd, &claude_json, lang);
        let (in_claude_json, in_project_file) = split_by_origin(&off, &origins_map(&origins));
        if let Some(cj) =
            claude_json_change(&claude_json, &cwd.to_string_lossy(), &in_claude_json, known)
        {
            if crate::procs::claude_sessions_running() > 0 {
                warnings.push(t(lang, "sess.claudeJsonBusy", &[]));
            }
            changes.push(cj);
        }
        if let Some(local) = project_mcp_change(cwd, &in_project_file, known) {
            changes.push(local);
        }
    }

    Resolution { changes, warnings }
}

fn origins_map(origins: &HashMap<String, String>) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    for (k, v) in origins {
        m.insert(k.clone(), json!(v));
    }
    m
}

/// Apply: only after explicit consent. Three passes: every guard first, then
/// the restore point, then the writes.
pub fn apply_files(changes: &[FileChange], cwd: &Path, lang: Lang) -> Result<ApplyResult, String> {
    // Nothing to write: an unchanged file needs no copy and no restore point.
    let writing: Vec<&FileChange> = changes
        .iter()
        .filter(|c| c.after.is_some() && c.after != c.before)
        .collect();

    // 1. Guards, all of them, before a byte moves.
    for c in &writing {
        if read_text(Path::new(&c.path)) != c.before {
            return Err(t(lang, "sess.fileChangedSinceDiff", &[("path", &c.path)]));
        }
    }

    // 2. The point, taken from what is on disk.
    let owned: Vec<FileChange> = writing.iter().map(|c| (*c).clone()).collect();
    let point = save_restore_point(cwd, &owned, "project", lang);

    // 3. The writes.
    let mut written: Vec<String> = vec![];
    for c in &owned {
        let path = PathBuf::from(&c.path);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&path, c.after.as_deref().unwrap_or("")).map_err(|e| e.to_string())?;
        written.push(c.path.clone());
    }
    Ok(ApplyResult {
        written,
        backups: point.as_deref().map(point_slots).unwrap_or_default(),
        point,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn temp_proj(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-resolve-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn ensure_paths() {
        // Whichever test initialises first fixes the root; this module writes
        // only inside its own temp project, so a shared root is safe.
        paths::init(std::env::temp_dir().join(format!("cl-resolve-root-{}", std::process::id())));
    }

    /// mode "all" means the profile says nothing about MCP: exactly one change
    /// (settings.json), no ~/.claude.json reading, honest warnings instead of
    /// inexpressible writes.
    #[test]
    fn resolve_all_mode_yields_one_settings_change_and_warnings() {
        ensure_paths();
        let cwd = temp_proj("all");
        let p = json!({
            "claude": { "model": "opus" },
            "skills": { "mode": "none" },
            "hooks": { "disableAll": true },
            "mcp": { "mode": "all" },
        });
        let r = resolve(&p, &cwd, Lang::En);
        assert_eq!(r.changes.len(), 1, "settings.json only");
        assert!(r.changes[0].path.ends_with("settings.json"));
        assert!(r.changes[0].before.is_none(), "the file did not exist");
        let after: Value = serde_json::from_str(r.changes[0].after.as_deref().unwrap()).unwrap();
        assert_eq!(after["model"], "opus");
        assert!(
            r.warnings.iter().any(|w| w.contains("skill")),
            "the inexpressible skill mode is a warning, not a write"
        );
        assert!(
            !r.warnings.iter().any(|w| w.contains("disableAllHooks")),
            "disableAll: true is expressible — no warning"
        );
        let _ = fs::remove_dir_all(&cwd);
    }

    /// The settings.json change carries the whole merged file — the pane shows
    /// it, apply writes it.
    #[test]
    fn resolve_merges_into_the_existing_settings() {
        ensure_paths();
        let cwd = temp_proj("merge");
        let sfile = paths::project_settings(&cwd);
        fs::create_dir_all(sfile.parent().unwrap()).unwrap();
        fs::write(&sfile, "{\n  \"model\": \"sonnet\"\n}").unwrap();
        let p = json!({
            "claude": { "model": "opus", "env": { "X": "1" } },
            "mcp": { "mode": "all" },
        });
        let r = resolve(&p, &cwd, Lang::Ru);
        let change = r.changes.first().expect("settings change");
        let before = change.before.as_deref().expect("existing file");
        assert_eq!(before, "{\n  \"model\": \"sonnet\"\n}");
        let after: Value = serde_json::from_str(change.after.as_deref().unwrap()).unwrap();
        assert_eq!(after["model"], "opus", "the patch wins");
        assert_eq!(after["env"]["X"], "1");
        let _ = fs::remove_dir_all(&cwd);
    }

    #[test]
    fn apply_writes_with_a_restore_point_and_refuses_a_changed_target() {
        ensure_paths();
        let cwd = temp_proj("apply");
        let f = cwd.join(".claude").join("settings.json");
        let path = f.to_string_lossy().into_owned();
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, "{}").unwrap();

        let ok = apply_files(
            &[FileChange {
                path: path.clone(),
                before: Some("{}".into()),
                after: Some("{\"model\":\"opus\"}\n".into()),
            }],
            &cwd,
            Lang::Ru,
        )
        .expect("guards pass");
        assert_eq!(ok.written, vec![path.clone()]);
        assert!(ok.point.is_some(), "a restore point was taken");
        assert_eq!(fs::read_to_string(&f).unwrap(), "{\"model\":\"opus\"}\n");

        // The target moved since the diff: refuse, write nothing.
        fs::write(&f, "{}").unwrap();
        let err = apply_files(
            &[FileChange {
                path: path.clone(),
                before: Some("{\"model\":\"opus\"}\n".into()),
                after: Some("x".into()),
            }],
            &cwd,
            Lang::Ru,
        )
        .expect_err("the file changed since the diff");
        assert!(err.contains(&path));

        // An identical write is not a write at all.
        let same = apply_files(
            &[FileChange {
                path: path.clone(),
                before: Some("{}".into()),
                after: Some("{}".into()),
            }],
            &cwd,
            Lang::Ru,
        )
        .unwrap();
        assert!(same.written.is_empty());
        assert!(same.point.is_none());
        let _ = fs::remove_dir_all(&cwd);
    }

    /// The one case with no restore-point safety net: apply must never touch
    /// the real ~/.claude.json. Here it is only ever listed as a change —
    /// and this test never applies it.
    #[test]
    fn mcp_switches_are_changes_until_apply_is_asked() {
        ensure_paths();
        let cwd = temp_proj("dry");
        let r = resolve(
            &json!({ "mcp": { "mode": "disable" }, "hooks": { "disableAll": true } }),
            &cwd,
            Lang::Ru,
        );
        // mode "disable" reads the real machine state: whatever it holds, the
        // dry run only lists — no file below changed.
        for c in &r.changes {
            let p = Path::new(&c.path);
            let current = read_text(p);
            if let (Some(b), false) = (&c.before, p == paths::claude_json()) {
                assert_eq!(
                    current.as_deref(),
                    Some(b.as_str()),
                    "the dry run moved nothing"
                );
            }
        }
        let _ = fs::remove_dir_all(&cwd);
    }
}
