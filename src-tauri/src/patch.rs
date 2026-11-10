//! The settings this profile asks for — the port of `src/shared/settings-patch.ts`.
//! One builder because two consumers read it: apply, which writes the file, and
//! the pane, which previews it. Two builders would drift.

use crate::jsonutil::Obj;
use serde_json::Value;
use std::collections::HashSet;

fn obj(v: &Value) -> Obj {
    v.as_object().cloned().unwrap_or_default()
}

fn str_list(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// `enabledPlugins` is a map, and only the plugins the profile has an opinion
/// about go in.
fn enabled_plugins_patch(p: &Value) -> Obj {
    let mut out = Obj::new();
    for id in str_list(&p["plugins"]["disabled"]) {
        out.insert(id, Value::Bool(false));
    }
    for id in str_list(&p["plugins"]["enabled"]) {
        out.insert(id, Value::Bool(true));
    }
    out
}

/// `allow`, `ask` and `deny` are lists, and a merge replaces lists instead of
/// extending them. Every rule already there goes back in alongside ours, and a
/// rule being moved leaves the list it came from: deny beats ask beats allow.
/// A rule wanted in `allow` goes into no list at all.
fn with_permissions(permissions: &Value, already_there: &Value, rules: &Obj) -> Obj {
    let base = obj(permissions);
    let list = |src: &Value, key: &str| -> Vec<String> { str_list(&src[key]) };

    let mut out = base.clone();
    for state in ["allow", "ask", "deny"] {
        let mine: Vec<String> = if state == "allow" {
            vec![]
        } else {
            rules
                .iter()
                .filter(|(_, s)| s.as_str() == Some(state))
                .map(|(r, _)| r.clone())
                .collect()
        };
        let kept: Vec<String> = [list(permissions, state), list(already_there, state)]
            .concat()
            .into_iter()
            .filter(|r| !rules.contains_key(r))
            .collect();
        // Kept first, ours appended: an old rule must not jump to the top of the
        // diff because a later decision moved it.
        let mut all = kept;
        for m in mine {
            if !all.contains(&m) {
                all.push(m);
            }
        }
        if all.is_empty() {
            out.remove(state);
        } else {
            out.insert(
                state.to_string(),
                Value::Array(all.into_iter().map(Value::String).collect()),
            );
        }
    }
    out
}

/// The names already in the file come back except for the ones the profile now
/// has an opinion about; `known` is the universe the opinion is about.
pub fn disabled_servers_patch(existing: &Value, off: &[String], known: &[String]) -> Vec<String> {
    let mine: HashSet<String> = known.iter().cloned().collect();
    let kept: Vec<String> = existing
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .filter(|s| !mine.contains(*s))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let mut out = kept;
    for o in off {
        if !out.contains(o) {
            out.push(o.clone());
        }
    }
    out
}

/// Where a name is declared decides which file carries its switch.
pub fn split_by_origin(off: &[String], origins: &Obj) -> (Vec<String>, Vec<String>) {
    let mut in_claude_json = vec![];
    let mut in_project_file = vec![];
    for name in off {
        if origins.get(name).and_then(|v| v.as_str()) == Some("project") {
            in_project_file.push(name.clone());
        } else {
            in_claude_json.push(name.clone());
        }
    }
    (in_claude_json, in_project_file)
}

/// `existing_permissions` is the `permissions` object already in the target
/// file, so a rule moved out of `deny` does not survive the merge. `mcpServers`
/// is deleted rather than passed through — Claude Code ignores the key there.
pub fn settings_patch(p: &Value, existing_permissions: Option<&Value>) -> Obj {
    let mut out = obj(&p["claude"]);
    let ep = enabled_plugins_patch(p);
    if !ep.is_empty() {
        out.insert("enabledPlugins".into(), Value::Object(ep));
    }
    if p["hooks"]["disableAll"].as_bool() == Some(true) {
        out.insert("disableAllHooks".into(), Value::Bool(true));
    }
    out.remove("mcpServers");
    let states = obj(&p["skills"]["states"]);
    if !states.is_empty() {
        out.insert("skillOverrides".into(), Value::Object(states));
    }
    let rules = obj(&p["permissions"]["rules"]);
    if !rules.is_empty() {
        let current = out.get("permissions").cloned().unwrap_or(Value::Null);
        let existing = existing_permissions.cloned().unwrap_or(Value::Null);
        let perms = with_permissions(&current, &existing, &rules);
        if perms.is_empty() {
            out.remove("permissions");
        } else {
            out.insert("permissions".into(), Value::Object(perms));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_patch_passes_claude_through_and_drops_mcp_servers() {
        let p = json!({
            "claude": { "model": "opus", "mcpServers": { "x": {} } },
        });
        let out = settings_patch(&p, None);
        assert_eq!(out["model"], "opus");
        assert!(out.get("mcpServers").is_none());
    }

    #[test]
    fn enabled_plugins_map_and_enabled_wins_over_disabled() {
        let p = json!({
            "plugins": { "enabled": ["a", "b"], "disabled": ["b", "c"] },
        });
        let out = settings_patch(&p, None);
        assert_eq!(out["enabledPlugins"]["a"], true);
        assert_eq!(out["enabledPlugins"]["b"], true, "enabled wins the overlap");
        assert_eq!(out["enabledPlugins"]["c"], false);
    }

    #[test]
    fn hooks_and_skills_reach_the_file_only_when_asked() {
        let p = json!({ "hooks": { "disableAll": true }, "skills": { "states": { "s1": "off" } } });
        let out = settings_patch(&p, None);
        assert_eq!(out["disableAllHooks"], true);
        assert_eq!(out["skillOverrides"]["s1"], "off");
        let quiet = settings_patch(&json!({}), None);
        assert!(quiet.get("disableAllHooks").is_none());
        assert!(quiet.get("skillOverrides").is_none());
    }

    #[test]
    fn moved_rule_leaves_its_old_list_and_allow_writes_no_list() {
        // deny → ask: the rule leaves `deny`, lands in `ask`; the untouched
        // rule stays. A rule the profile allows disappears from every list —
        // allow is the default, writing it would be churn.
        let p = json!({
            "permissions": { "rules": { "Bash(git push:*)": "ask", "Bash(curl:*)": "allow" } },
        });
        let existing =
            json!({ "allow": ["Edit(./src/**)"], "deny": ["Bash(git push:*)", "Bash(curl:*)"] });
        let out = settings_patch(&p, Some(&existing));
        let perms = &out["permissions"];
        assert_eq!(perms["ask"], json!(["Bash(git push:*)"]));
        // Every deny had an opinion now — the whole list disappears.
        assert!(perms["deny"].is_null() || perms["deny"].as_array().unwrap().is_empty());
        assert_eq!(
            perms["allow"],
            json!(["Edit(./src/**)"]),
            "a rule with no opinion stays where it was"
        );
    }

    #[test]
    fn kept_rules_keep_their_place_in_the_list() {
        let p = json!({
            "permissions": { "rules": { "Bash(rm:*)": "deny" } },
        });
        let existing = json!({ "ask": ["Bash(a:*)", "Bash(rm:*)", "Bash(z:*)"] });
        let out = settings_patch(&p, Some(&existing));
        assert_eq!(
            out["permissions"]["ask"],
            json!(["Bash(a:*)", "Bash(z:*)"]),
            "kept keep their order, the moved rule is gone"
        );
        assert_eq!(out["permissions"]["deny"], json!(["Bash(rm:*)"]));
    }

    #[test]
    fn disabled_servers_patch_keeps_unknown_and_dedupes() {
        let existing = json!(["a", "b"]);
        let known = ["b".to_string(), "c".to_string()];
        let out = disabled_servers_patch(&existing, &["c".to_string(), "a".to_string()], &known);
        // `b` had an opinion now — dropped; `a` is not known to the profile so it
        // stays; `c` is added once.
        assert_eq!(out, vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn split_by_origin_sends_project_declarations_to_the_project_file() {
        let origins = serde_json::json!({ "a": "project", "b": "user" });
        let (cj, project) = split_by_origin(
            &["a".to_string(), "b".to_string()],
            origins.as_object().unwrap(),
        );
        assert_eq!(cj, vec!["b".to_string()]);
        assert_eq!(project, vec!["a".to_string()]);
    }
}
