//! Installed plugins — the port of `src/server/scan/plugins.ts`. Plugins
//! matter because their skills are NOT disabled via skillOverrides — only by
//! turning the whole plugin off.

use crate::cost;
use crate::jsonutil::{exists, list_dir, read_json};
use crate::lang::Lang;
use crate::paths;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct PluginInstall {
    pub id: String,
    /// Shape parity with the TS `PluginInstall`; nothing in Rust reads these yet.
    #[allow(dead_code)]
    pub scope: String,
    pub install_path: PathBuf,
    #[allow(dead_code)]
    pub version: String,
}

pub fn read_installs(lang: Lang) -> Vec<PluginInstall> {
    let file = paths::installed_plugins();
    let Ok(v) = read_json(&file, lang) else {
        return Vec::new();
    };
    let Some(plugins) = v.get("plugins").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (id, arr) in plugins {
        let Some(first) = arr.as_array().and_then(|a| a.first()) else {
            continue;
        };
        let Some(install_path) = first.get("installPath").and_then(|v| v.as_str()) else {
            continue;
        };
        out.push(PluginInstall {
            id: id.clone(),
            scope: first
                .get("scope")
                .and_then(|s| s.as_str())
                .unwrap_or("user")
                .to_string(),
            install_path: PathBuf::from(install_path),
            version: first
                .get("version")
                .and_then(|s| s.as_str())
                .unwrap_or("?")
                .to_string(),
        });
    }
    out
}

fn plugin_skills_cost(install_path: &Path, lang: Lang) -> (Value, usize) {
    let dir = install_path.join("skills");
    let mut costs: Vec<Value> = Vec::new();
    let mut count = 0usize;
    for name in list_dir(&dir) {
        let f = dir.join(&name).join("SKILL.md");
        if !exists(&f) {
            continue;
        }
        count += 1;
        // An unreadable skill should not bring down the plugin.
        if let Ok(text) = std::fs::read_to_string(&f) {
            let front = cost::parse_frontmatter(&text);
            let chars = front
                .as_ref()
                .and_then(|fm| fm.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .len()
                + front
                    .as_ref()
                    .and_then(|fm| fm.get("when_to_use"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .len();
            costs.push(cost::estimated_chars(chars, &name));
        }
    }
    (cost::sum_costs(&costs, lang), count)
}

/// The plugin's own manifest description — no catalog, no guessing.
pub fn manifest_description(install_path: &Path, lang: Lang) -> Option<String> {
    for rel in ["plugin.json", ".claude-plugin/plugin.json"] {
        let f = install_path.join(rel);
        if let Ok(v) = read_json(&f, lang) {
            let d = v
                .get("description")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if !d.is_empty() {
                return Some(d);
            }
        }
    }
    None
}

/// Plugin description — first from the catalog (the fullest), otherwise from
/// its own plugin.json. Neither there nor there — no line at all.
fn plugin_description(
    id: &str,
    install_path: &Path,
    from_catalog: &HashMap<String, String>,
    lang: Lang,
) -> Option<String> {
    from_catalog
        .get(id)
        .cloned()
        .or_else(|| manifest_description(install_path, lang))
}

pub fn scan_plugins(
    from_settings: &HashMap<String, bool>,
    installs: &[PluginInstall],
    descriptions: &HashMap<String, String>,
    lang: Lang,
) -> Vec<Value> {
    let installed_file = read_json(&paths::installed_plugins(), lang).unwrap_or(Value::Null);
    let from_installed: HashMap<String, bool> = installed_file
        .get("enabledPlugins")
        .and_then(|e| e.as_object())
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
                .collect()
        })
        .unwrap_or_default();

    installs
        .iter()
        .map(|inst| {
            let (cost, count) = plugin_skills_cost(&inst.install_path, lang);
            let enabled = from_settings
                .get(&inst.id)
                .copied()
                .or_else(|| from_installed.get(&inst.id).copied())
                .unwrap_or(true);
            json!({
                "kind": "plugin",
                "id": inst.id,
                "scope": "user",
                "source": { "file": paths::installed_plugins().to_string_lossy() },
                "enabled": enabled,
                "skillCount": count,
                "description": plugin_description(&inst.id, &inst.install_path, descriptions, lang),
                "cost": cost,
            })
        })
        .collect()
}

/// enabledPlugins from all settings levels; the highest priority wins per key.
pub fn merged_enabled_plugins(files: &[crate::scan::SettingsFile]) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    for f in files {
        let Some(ep) = f.data.get("enabledPlugins").and_then(|e| e.as_object()) else {
            continue;
        };
        for (id, v) in ep {
            if let Some(b) = v.as_bool() {
                out.insert(id.clone(), b);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::SettingsFile;
    use serde_json::json;
    use std::path::PathBuf;

    fn settings_file(data: Value) -> SettingsFile {
        SettingsFile {
            scope: "user",
            file: PathBuf::from("test"),
            data,
        }
    }

    #[test]
    fn merged_enabled_plugins_later_file_wins() {
        let files = vec![
            settings_file(json!({ "enabledPlugins": { "a": true, "b": false } })),
            settings_file(json!({ "enabledPlugins": { "b": true } })),
            settings_file(json!({ "model": "opus" })),
        ];
        let m = merged_enabled_plugins(&files);
        assert_eq!(m["a"], json!(true));
        assert_eq!(m["b"], json!(true));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn manifest_description_reads_both_manifest_spots() {
        let dir = std::env::temp_dir().join(format!("cl-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
        std::fs::write(
            dir.join(".claude-plugin/plugin.json"),
            r#"{"description": "  from the nested manifest  "}"#,
        )
        .unwrap();
        assert_eq!(
            manifest_description(&dir, Lang::Ru).as_deref(),
            Some("from the nested manifest"),
            "the description is trimmed"
        );

        // A top-level plugin.json wins by being read first.
        std::fs::write(dir.join("plugin.json"), r#"{"description": "top"}"#).unwrap();
        assert_eq!(manifest_description(&dir, Lang::Ru).as_deref(), Some("top"));

        // A blank top-level description falls through to the nested manifest.
        std::fs::write(dir.join("plugin.json"), r#"{"description": "   "}"#).unwrap();
        assert_eq!(
            manifest_description(&dir, Lang::Ru).as_deref(),
            Some("from the nested manifest")
        );

        // Neither manifest, neither spot — no line at all.
        std::fs::remove_file(dir.join(".claude-plugin/plugin.json")).unwrap();
        assert_eq!(manifest_description(&dir, Lang::Ru), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_plugins_reads_the_switch_from_settings_then_installs() {
        let dir = std::env::temp_dir().join(format!("cl-plugins-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let inst = dir.join("my-plugin");
        let skill = inst.join("skills").join("demo").join("SKILL.md");
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        std::fs::write(&skill, "---\ndescription: demo skill\n---\nbody").unwrap();
        let installs = vec![PluginInstall {
            id: "my-plugin".into(),
            scope: "user".into(),
            install_path: inst,
            version: "1.0".into(),
        }];

        // Settings outrank installed_plugins.json.
        let mut from_settings = HashMap::new();
        from_settings.insert("my-plugin".to_string(), false);
        let rows = scan_plugins(&from_settings, &installs, &HashMap::new(), Lang::Ru);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["kind"], "plugin");
        assert_eq!(rows[0]["enabled"], false);
        assert_eq!(rows[0]["skillCount"], 1);
        assert!(
            rows[0]["description"].is_null(),
            "no catalog, no manifest — no line"
        );

        // Without a settings opinion the installed file's switch applies, and
        // its absence means enabled.
        let empty: HashMap<String, bool> = HashMap::new();
        let rows = scan_plugins(&empty, &installs, &HashMap::new(), Lang::Ru);
        assert_eq!(rows[0]["enabled"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
