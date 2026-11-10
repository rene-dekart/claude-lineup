//! Read-only views of Claude Code's own files: the full-configuration tab
//! (`config-files`) and the readable `~/.claude.json` view. Port of
//! `configFiles()` in `src/server/index.ts` and `claudeJsonView.ts`.
//! Nothing here writes — both return text, that is the whole safety story.

use crate::jsonutil::read_json;
use crate::lang::Lang;
use crate::mcp;
use crate::paths;
use serde_json::{Value, json};
use std::path::Path;

/// Claude Code's own bookkeeping in `~/.claude.json` — startup counters, feature
/// caches, onboarding flags, tip history, machine ids. None of it changes what a
/// session loads, and it is the bulk of the file. A drop list, not a keep list:
/// a key this list has never heard of stays on screen until it is named here.
const STATE_KEYS: &[&str] = &[
    "numStartups",
    "installMethod",
    "hasSeenTasksHint",
    "tipsHistory",
    "tipsHistoryByCommand",
    "promptQueueUseCount",
    "btwUseCount",
    "cachedGrowthBookFeatures",
    "cachedExperimentFeatures",
    "cachedGrowthBookFeaturesAt",
    "unpinOpus47LaunchEffort",
    "unpinOpus48LaunchEffort",
    "unpinFable5LaunchEffort",
    "firstStartTime",
    "opusProMigrationComplete",
    "sonnet1m45MigrationComplete",
    "userID",
    "changelogLastFetched",
    "hasCompletedOnboarding",
    "lastOnboardingVersion",
    "clientDataCache",
    "lastReleaseNotesSeen",
    "officialMarketplaceAutoInstallAttempted",
    "officialMarketplaceAutoInstalled",
    "migrationVersion",
    "seenNotifications",
    "showSpinnerTree",
    "feedbackSurveyState",
    "skillUsage",
    "lastPlanModeUse",
    "hasUsedBackslashReturn",
    "hasSeenAutoModeEntryWarning",
    "tipLifetimeShownCounts",
    "pluginUsage",
    "lspRecommendationIgnoredCount",
    "closedIssuesLastChecked",
    "lastShownEmergencyTip",
    "machineID",
];

/// The TS original prints `JSON.stringify(value ?? {}, null, 2)` — no trailing
/// newline. Keep the bytes identical so the two back ends can be diffed.
fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).expect("Value is serializable")
}

fn take(files: &mut Vec<Value>, label: String, v: &Value) {
    files.push(json!({ "path": label, "content": pretty(v) }));
}

/// Every settings file that shapes one project, as it lies on disk, in the order
/// Claude Code reads them. A missing or unreadable file is skipped, not printed
/// as `{}`: a heading over an empty block says "there is a setting here" about a
/// file nobody has.
pub fn config_files_at(
    user_settings: &Path,
    claude_json: &Path,
    cwd: &Path,
    lang: Lang,
) -> Vec<Value> {
    let mut files = Vec::new();
    let cwd_str = cwd.to_string_lossy();

    if let Ok(v) = read_json(user_settings, lang) {
        let label = paths::shorten(&user_settings.to_string_lossy(), &cwd_str);
        take(&mut files, label, &v);
    }

    // `~/.claude.json` is one file for the whole machine (46 project entries on
    // this disk alone); only this project's entry is shown, under the key shape
    // the full-configuration pane labels it with.
    if let Ok(cj) = read_json(claude_json, lang) {
        let projects = cj.get("projects").and_then(|p| p.as_object());
        if let (Some(projects), Some(key)) = (
            projects,
            projects.and_then(|p| mcp::find_project_key(p, &cwd_str)),
        ) {
            let label_key = cwd_str.replace('\\', "/");
            let label_key = label_key.trim_end_matches('/');
            let label = format!(
                "{} → projects[\"{}\"]",
                paths::shorten(&claude_json.to_string_lossy(), &cwd_str),
                label_key
            );
            take(&mut files, label, &projects[&key]);
        }
    }

    for file in [
        paths::project_settings(cwd),
        paths::project_local_settings(cwd),
        paths::project_mcp_json(cwd),
    ] {
        if let Ok(v) = read_json(&file, lang) {
            let label = paths::shorten(&file.to_string_lossy(), &cwd_str);
            take(&mut files, label, &v);
        }
    }
    files
}

pub fn config_files(cwd: &Path, lang: Lang) -> Vec<Value> {
    config_files_at(&paths::user_settings(), &paths::claude_json(), cwd, lang)
}

/// `~/.claude.json` as a readable view: this project's entry plus what the file
/// says for the whole machine — the global `mcpServers` among it, which no other
/// pane shows. `None` when the file is missing or unreadable.
pub fn claude_json_view_at(claude_json: &Path, cwd: &Path, lang: Lang) -> Option<Value> {
    let cj = read_json(claude_json, lang).ok()?;
    let mut out = cj.as_object().cloned().unwrap_or_default();
    for k in STATE_KEYS {
        // shift_remove, not remove: with preserve_order on, `remove` is
        // swap_remove — it drags the last key into the freed slot and shuffles
        // the rest, while the TS `delete` keeps insertion order.
        out.shift_remove(*k);
    }

    // No key at all means the file has never seen this project; `projects` is
    // dropped rather than printed whole — the other 45 entries are not this
    // project's configuration.
    let cwd_str = cwd.to_string_lossy();
    let key = cj
        .get("projects")
        .and_then(|p| p.as_object())
        .and_then(|p| mcp::find_project_key(p, &cwd_str));
    match key {
        Some(k) => {
            let entry = cj["projects"][&k].clone();
            out.insert("projects".into(), json!({ k: entry }));
        }
        None => {
            out.remove("projects");
        }
    }

    Some(json!({
        "path": paths::shorten(&claude_json.to_string_lossy(), &cwd_str),
        "content": pretty(&Value::Object(out)),
    }))
}

pub fn claude_json_view(cwd: &Path, lang: Lang) -> Option<Value> {
    claude_json_view_at(&paths::claude_json(), cwd, lang)
}

/// The real-machine paths for a test that owns a fake home + project tree.
#[cfg(test)]
pub(crate) fn config_files_in_home(home: &Path, cwd: &Path, lang: Lang) -> Vec<Value> {
    config_files_at(
        &home.join(".claude").join("settings.json"),
        &home.join(".claude.json"),
        cwd,
        lang,
    )
}

#[cfg(test)]
pub(crate) fn claude_json_view_in_home(home: &Path, cwd: &Path, lang: Lang) -> Option<Value> {
    claude_json_view_at(&home.join(".claude.json"), cwd, lang)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-cfgfiles-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, s: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }

    #[test]
    fn files_in_read_order_missing_are_skipped() {
        let root = tmp("order");
        let cwd = root.join("proj");
        write(&root.join(".claude/settings.json"), r#"{"model":"opus"}"#);
        write(&cwd.join(".mcp.json"), r#"{"mcpServers":{}}"#);

        let files = config_files_in_home(&root, &cwd, Lang::Ru);
        let labels: Vec<&str> = files
            .iter()
            .filter_map(|f| f.get("path").and_then(|v| v.as_str()))
            .collect();
        // Only files that exist: the missing .claude.json entry and
        // settings.local.json leave no empty headings behind.
        assert_eq!(labels.len(), 2);
        assert!(labels[0].ends_with("settings.json"));
        assert!(labels[1].ends_with(".mcp.json"));
    }

    #[test]
    fn claude_json_entry_labeled_and_narrowed() {
        let root = tmp("entry");
        let cwd = root.join("proj");
        let key = mcp::project_key(&cwd.to_string_lossy());
        let mut projects = serde_json::Map::new();
        projects.insert(key.clone(), json!({ "mcpServers": { "b": {} } }));
        // another project on the same disk — must not appear
        projects.insert("D:/other".into(), json!({ "mcpServers": { "a": {} } }));
        write(
            &root.join(".claude.json"),
            &json!({
                "numStartups": 40,
                "mcpServers": { "github": {} },
                "projects": projects,
            })
            .to_string(),
        );
        write(&cwd.join(".claude/settings.json"), r#"{"permissions":{}}"#);

        let files = config_files_in_home(&root, &cwd, Lang::Ru);
        assert_eq!(files.len(), 2);
        let entry = files
            .iter()
            .find(|f| {
                f.get("path")
                    .and_then(|v| v.as_str())
                    .map(|l| l.contains("→ projects["))
                    .unwrap_or(false)
            })
            .expect("claude.json entry listed");
        let content = entry.get("content").unwrap().as_str().unwrap();
        // Only this project's entry, not the other projects.
        assert!(content.contains("\"b\""));
        assert!(!content.contains("\"a\""));
    }

    #[test]
    fn view_drops_state_keys_and_narrows_projects() {
        let root = tmp("view");
        let cwd = root.join("proj");
        let key = mcp::project_key(&cwd.to_string_lossy());
        let mut projects = serde_json::Map::new();
        projects.insert(key.clone(), json!({ "enabledMcpjsonServers": ["x"] }));
        // a second project whose name must not survive
        projects.insert("D:/z".into(), json!({ "mcpServers": { "a": {} } }));
        write(
            &root.join(".claude.json"),
            &json!({
                "numStartups": 9,
                "machineID": "m1",
                "mcpServers": { "srv": {} },
                "projects": projects,
            })
            .to_string(),
        );

        let view = claude_json_view_in_home(&root, &cwd, Lang::Ru).expect("view exists");
        let content = view.get("content").unwrap().as_str().unwrap();
        assert!(content.contains("\"mcpServers\""));
        assert!(!content.contains("numStartups"));
        assert!(!content.contains("machineID"));
        assert!(content.contains("\"enabledMcpjsonServers\""));
        assert!(!content.contains("\"other\""));
        // The path caption is the shortened home path.
        let path = view.get("path").unwrap().as_str().unwrap();
        assert!(path.ends_with(".claude.json"));
    }

    #[test]
    fn broken_json_skipped_others_still_listed() {
        let root = tmp("broken");
        let cwd = root.join("proj");
        write(&root.join(".claude/settings.json"), r#"{"ok":true}"#);
        write(&cwd.join(".claude/settings.local.json"), "{ broken");
        write(&cwd.join(".mcp.json"), r#"{"mcpServers":{}}"#);

        let files = config_files_in_home(&root, &cwd, Lang::Ru);
        let labels: Vec<&str> = files
            .iter()
            .filter_map(|f| f.get("path").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn view_is_none_when_file_missing() {
        let root = tmp("none");
        let cwd = root.join("proj");
        fs::create_dir_all(&cwd).unwrap();
        assert!(claude_json_view_in_home(&root, &cwd, Lang::Ru).is_none());
    }
}
