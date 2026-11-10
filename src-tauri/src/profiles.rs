//! Profile storage — the port of `src/server/profiles/index.ts`.
//! One profile = one set of decisions: no layers, no "not set".

use crate::backups::save_restore_point;
use crate::jsonutil::{deep_merge, list_dir, read_json, to_pretty, write_atomic};
use crate::lang::Lang;
use crate::paths;
use serde_json::{Value, json};
use std::fs;

pub fn empty_profile(id: &str) -> Value {
    json!({
        "id": id,
        "title": id,
        "permissions": { "rules": {} },
        "mcp": { "mode": "all", "servers": [] },
        "plugins": { "enabled": [], "disabled": [] },
        "skills": { "mode": "all", "states": {} },
        "hooks": { "disableAll": false },
        "memory": { "autoDiscover": true },
        "flags": [],
        "claude": {}
    })
}

fn normalize(raw: &Value, fallback_id: &str) -> Value {
    let base = empty_profile(fallback_id);
    let mut merged = deep_merge(&base, raw);

    // Profiles written when the app could also build a one-run launcher carry
    // two keys that decide nothing now: `mode` and the `settingSources` it
    // narrowed. Left in, they would be saved back and read as settings.
    if let Some(o) = merged.as_object_mut() {
        o.remove("mode");
        o.remove("settingSources");
    }

    // Files written before per-skill states carry {mode:"only", names:[…]}.
    // There is no such mode — drop `names`, and drop the mode back to "all"
    // rather than guess that the listed names were the wanted ones.
    if let Some(skills) = merged.get_mut("skills").and_then(|v| v.as_object_mut()) {
        let mode_ok = matches!(
            skills.get("mode").and_then(|v| v.as_str()),
            Some("all") | Some("none")
        );
        if !mode_ok {
            skills.insert("mode".into(), json!("all"));
        }
        skills.remove("names");
    }

    // Denies used to live in `mcp.deniedTools` as a bare list. They are
    // permission rules like any other, so they move into the one place that
    // holds them — as `deny`, which is what a ticked-off tool meant.
    let old_denied: Vec<String> = raw
        .get("mcp")
        .and_then(|m| m.get("deniedTools"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    if !old_denied.is_empty()
        && let Some(rules) = merged
            .get_mut("permissions")
            .and_then(|p| p.get_mut("rules"))
            .and_then(|r| r.as_object_mut())
    {
        for r in old_denied {
            if !rules.contains_key(&r) {
                rules.insert(r, json!("deny"));
            }
        }
    }
    if let Some(mcp) = merged.get_mut("mcp").and_then(|v| v.as_object_mut()) {
        mcp.remove("deniedTools");
    }

    if let Some(o) = merged.as_object_mut() {
        let id_empty = o
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.is_empty())
            .unwrap_or(true);
        if id_empty {
            o.insert("id".into(), json!(fallback_id));
        }
        let title_empty = o
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.is_empty())
            .unwrap_or(true);
        if title_empty {
            let id = o
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            o.insert("title".into(), json!(id));
        }
    }
    merged
}

pub fn list_profiles() -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for f in list_dir(&paths::profile_dir()) {
        if !f.ends_with(".json") {
            continue;
        }
        let r = read_json(&paths::profile_dir().join(&f), crate::lang::DEFAULT_LANG);
        if let Ok(v) = r {
            let fallback = f.trim_end_matches(".json").to_string();
            out.push(normalize(&v, &fallback));
        }
    }
    out.sort_by(|a, b| {
        let ai = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let bi = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
        ai.cmp(bi)
    });
    out
}

pub fn get_profile(id: &str) -> Option<Value> {
    list_profiles()
        .into_iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(id))
}

/// The safe id: `[^a-zA-Z0-9._-]` → `_` — a profile file name is a file name.
fn safe_id(raw: &Value) -> String {
    let s = match raw {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn save_profile(p: &Value, lang: Lang) -> Result<Value, String> {
    let safe = safe_id(&p["id"]);
    let mut patch = p.clone();
    if let Some(o) = patch.as_object_mut() {
        o.insert("id".into(), json!(safe));
    }
    let normalized = normalize(&patch, &safe);
    let file = paths::profile_dir().join(format!("{}.json", safe));
    let after = to_pretty(&normalized);
    // The version being replaced is kept before it is replaced. A profile is
    // the user's work, and saving over it is the one write with nothing behind
    // it. `save_restore_point` skips identical bytes, so re-saving an untouched
    // profile does not fill the history with rows that do nothing.
    let before = std::fs::read_to_string(&file).ok();
    let change = crate::backups::FileChange {
        path: file.to_string_lossy().into_owned(),
        before,
        after: Some(after.clone()),
    };
    let _ = save_restore_point(&paths::profile_dir(), &[change], "profile", lang);
    write_atomic(&file, &after).map_err(|e| e.to_string())?;
    Ok(normalized)
}

pub fn delete_profile(id: &str) -> bool {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file = paths::profile_dir().join(format!("{}.json", safe));
    if !file.exists() {
        return false;
    }
    fs::remove_file(file).is_ok()
}

// Profile layers are deliberately absent. Profile is a complete record: it has
// no "not set", so any layer would always override the previous one entirely.
// See docs/superpowers/specs/2026-10-05-*.md

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn temp_root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-profiles-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn ensure_paths(root: PathBuf) {
        // OnceLock: whichever test initialises first sets the root for the
        // process. Every test works with unique file names, so a shared root
        // is safe; if it is already set, that root is used as is.
        paths::init(root);
    }

    #[test]
    fn normalize_migrates_legacy_shapes() {
        // The launcher-era keys decide nothing and must not survive.
        // Skills' {mode:"only", names:[…]} has no mode — falls back to "all".
        // mcp.deniedTools moves into permissions.rules as deny.
        let raw = json!({
            "id": "old",
            "title": "",
            "mode": "session",
            "settingSources": ["user"],
            "skills": { "mode": "only", "names": ["a", "b"] },
            "mcp": { "mode": "named", "servers": ["s"], "deniedTools": ["t1", "t2"] },
            "permissions": { "rules": { "t2": "allow" } },
        });
        let out = normalize(&raw, "fallback");
        assert!(out.get("mode").is_none());
        assert!(out.get("settingSources").is_none());
        assert_eq!(out["skills"]["mode"], "all");
        assert!(out["skills"].get("names").is_none());
        assert!(out["mcp"].get("deniedTools").is_none());
        assert_eq!(
            out["permissions"]["rules"]["t1"], "deny",
            "ticked-off tool → deny"
        );
        assert_eq!(
            out["permissions"]["rules"]["t2"], "allow",
            "an existing opinion is not overwritten by the migration"
        );
        assert_eq!(out["title"], "old", "empty title falls back to the id");
    }

    #[test]
    fn save_list_get_delete_roundtrip_with_safe_ids() {
        ensure_paths(temp_root("roundtrip"));
        let mut p = empty_profile("my/Profile:1");
        p["title"] = json!("First");
        let saved = save_profile(&p, Lang::Ru).expect("save");
        assert_eq!(saved["id"], "my_Profile_1", "the id became a file name");

        let list = list_profiles();
        assert!(
            list.iter()
                .any(|x| x["id"] == "my_Profile_1" && x["title"] == "First")
        );

        assert_eq!(
            get_profile("my_Profile_1").expect("found")["title"],
            "First"
        );
        assert!(get_profile("absent").is_none());

        assert!(delete_profile("my_Profile_1"), "delete an existing profile");
        assert!(!delete_profile("my_Profile_1"), "delete twice is false");
        assert!(get_profile("my_Profile_1").is_none());
    }

    #[test]
    fn empty_profile_carries_the_neutral_decisions() {
        let p = empty_profile("x");
        assert_eq!(p["mcp"]["mode"], "all");
        assert_eq!(p["skills"]["mode"], "all");
        assert_eq!(p["hooks"]["disableAll"], false);
        assert_eq!(p["memory"]["autoDiscover"], true);
        assert_eq!(p["permissions"]["rules"], json!({}));
        assert_eq!(p["id"], "x");
        assert_eq!(p["title"], "x");
    }
}
