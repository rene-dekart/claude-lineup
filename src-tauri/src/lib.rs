//! claude-lineup desktop. Commands mirror the web version's routes one to one:
//! the UI keeps its shapes, only the transport changed (fetch → invoke).

mod backups;
mod card;
mod catalog;
mod config;
mod configfiles;
mod cost;
mod creport;
mod http;
mod install;
mod jsonutil;
mod lang;
mod mcp;
mod mcpreg;
mod netcache;
mod patch;
mod paths;
mod pluginmcp;
mod plugins;
mod probe;
mod procs;
mod profiles;
mod remote;
mod repourl;
mod resolve;
mod scan;
mod stats;

use lang::{parse_lang, t};
use serde_json::{Value, json};
use std::path::PathBuf;
use tauri::Manager;

fn lang_of(raw: &str) -> lang::Lang {
    parse_lang(Some(raw))
}

// ── profiles ──

#[tauri::command]
fn get_profiles(lang: String) -> Result<Vec<Value>, String> {
    let _ = lang_of(&lang);
    Ok(profiles::list_profiles())
}

#[tauri::command]
fn profile_get(lang: String, id: String) -> Result<Option<Value>, String> {
    let _ = lang_of(&lang);
    Ok(profiles::get_profile(&id))
}

#[tauri::command]
fn profile_save(lang: String, profile: Value) -> Result<Value, String> {
    profiles::save_profile(&profile, lang_of(&lang))
}

#[tauri::command]
fn profile_delete(lang: String, id: String) -> Result<Value, String> {
    let _ = lang_of(&lang);
    Ok(json!({ "deleted": profiles::delete_profile(&id) }))
}

// ── resolve / apply ──

#[tauri::command]
fn resolve_profile(lang: String, profile: Value, cwd: String) -> Result<Value, String> {
    let l = lang_of(&lang);
    let r = resolve::resolve(&profile, std::path::Path::new(&cwd), l);
    Ok(json!({ "resolution": r }))
}

#[tauri::command]
fn apply_profile(lang: String, profile: Value, cwd: String) -> Result<Value, String> {
    let l = lang_of(&lang);
    let r = resolve::resolve(&profile, std::path::Path::new(&cwd), l);
    let result = resolve::apply_files(&r.changes, std::path::Path::new(&cwd), l)?;
    Ok(json!({
        "written": result.written,
        "backups": result.backups,
        "point": result.point,
        "warnings": r.warnings,
    }))
}

// ── restore points ──

#[tauri::command]
fn backups_list(lang: String) -> Result<Value, String> {
    let _ = lang_of(&lang);
    Ok(json!({ "points": backups::list_restore_points() }))
}

#[tauri::command]
fn backup_restore(lang: String, id: String) -> Result<Value, String> {
    let l = lang_of(&lang);
    let r = backups::restore_point(&id, l)?;
    Ok(json!({ "restored": r.restored, "deleted": r.deleted, "point": r.point }))
}

#[tauri::command]
fn backup_delete(lang: String, id: String) -> Result<Value, String> {
    let _ = lang_of(&lang);
    Ok(json!({ "deleted": backups::delete_restore_point(&id) }))
}

#[tauri::command]
fn backups_clear(lang: String) -> Result<Value, String> {
    let _ = lang_of(&lang);
    Ok(json!({ "cleared": backups::clear_restore_points() }))
}

// ── the OS folder picker ──

#[tauri::command]
async fn pick_dir(app: tauri::AppHandle, initial: Option<String>) -> Result<Value, String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};
    let mut dialog = app.dialog().file();
    if let Some(i) = initial
        && !i.trim().is_empty()
    {
        dialog = dialog.set_directory(std::path::Path::new(&i));
    }
    // `path: null` means the user closed it — not an error, same as the web version.
    let picked = dialog.blocking_pick_folder();
    Ok(json!({ "path": picked.map(|f| match f {
        FilePath::Path(p) => p.to_string_lossy().into_owned(),
        FilePath::Url(u) => u.to_string(),
    }) }))
}

// ── read-only views of the machine ──

#[tauri::command]
fn get_inventory(lang: String, cwd: String) -> Result<Value, String> {
    Ok(scan::build_inventory(
        std::path::Path::new(&cwd),
        lang_of(&lang),
    ))
}

#[tauri::command]
fn get_config_files(lang: String, cwd: String) -> Result<Value, String> {
    Ok(json!({
        "files": configfiles::config_files(std::path::Path::new(&cwd), lang_of(&lang))
    }))
}

#[tauri::command]
fn get_claude_json(lang: String, cwd: String) -> Result<Value, String> {
    Ok(json!({
        "file": configfiles::claude_json_view(std::path::Path::new(&cwd), lang_of(&lang))
    }))
}

// ── statistics: session history + what the savings tools report ──

#[tauri::command]
fn get_stats(lang: String) -> Result<Value, String> {
    Ok(stats::build_stats(lang_of(&lang)))
}

/// The retention number comes from the page that showed it — the button cannot
/// delete a wider span than the line above it promised.
#[tauri::command]
fn clean_history(lang: String, days: Option<f64>) -> Result<Value, String> {
    let l = lang_of(&lang);
    let days = match days {
        Some(d) => d,
        None => return Err(t(l, "stats.cleanupNoDays", &[])),
    };
    Ok(stats::clean_transcripts_at(
        &paths::projects_dir(),
        stats::read_retention(Some(&json!(days))),
        stats::now_ms(),
    ))
}

// ── the detail card ──

// The arguments mirror the invoke body's fields one to one.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn get_card(
    lang: String,
    kind: String,
    name: String,
    cwd: Option<String>,
    file: Option<String>,
    scope: Option<String>,
    origin: Option<String>,
    repo: Option<String>,
    description: Option<String>,
    version: Option<String>,
    transport: Option<String>,
) -> Result<Value, String> {
    let cwd = cwd.filter(|s| !s.is_empty()).unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    Ok(card::build_card(
        &kind,
        &name,
        &cwd,
        file.as_deref(),
        scope.as_deref(),
        origin.as_deref(),
        repo.as_deref(),
        description.as_deref(),
        version.as_deref(),
        transport.as_deref(),
        lang_of(&lang),
    ))
}

/// Spawns the server and asks it for tools/list — runs someone else's code,
/// hence only on the Measure button's explicit click.
#[tauri::command]
fn probe_mcp(lang: String, name: String) -> Result<Value, String> {
    Ok(json!({ "cost": probe::probe_mcp(&name, lang_of(&lang)) }))
}

// ── install: always through the Claude Code CLI ──

#[tauri::command]
fn install_plugin(
    lang: String,
    id: String,
    scope: Option<String>,
    cwd: Option<String>,
    accept_command: Option<String>,
) -> Result<Value, String> {
    install::install_plugin(
        &id,
        scope.as_deref().unwrap_or("user"),
        cwd.as_deref(),
        accept_command.as_deref(),
        lang_of(&lang),
    )
}

// ── the external index and the official MCP registry ──

#[tauri::command]
fn get_remote(
    lang: String,
    q: Option<String>,
    sort: Option<String>,
    r#type: Option<String>,
    limit: Option<f64>,
) -> Result<Value, String> {
    Ok(remote::scan_remote(
        q.as_deref().unwrap_or(""),
        r#type.as_deref().unwrap_or(""),
        sort.as_deref().unwrap_or("stars"),
        limit.unwrap_or(30.0),
        lang_of(&lang),
    ))
}

#[tauri::command]
fn get_mcp_registry(
    lang: String,
    q: Option<String>,
    limit: Option<f64>,
    cursor: Option<String>,
) -> Result<Value, String> {
    Ok(mcpreg::scan_mcp_registry(
        q.as_deref().unwrap_or(""),
        limit.unwrap_or(30.0),
        cursor.as_deref().filter(|s| !s.is_empty()),
        lang_of(&lang),
    ))
}

/// Dry-run: shows the exact commands and runs NOTHING.
#[tauri::command]
fn plan_remote_install(
    lang: String,
    repo: String,
    name: String,
    scope: Option<String>,
    cwd: Option<String>,
    skill_commit_sha: Option<String>,
) -> Result<Value, String> {
    let _ = cwd;
    Ok(remote::plan_remote(
        &repo,
        &name,
        scope.as_deref(),
        None,
        skill_commit_sha.as_deref(),
        lang_of(&lang),
    ))
}

#[tauri::command]
fn run_remote_install(
    lang: String,
    repo: String,
    name: String,
    scope: Option<String>,
    cwd: Option<String>,
    skill_commit_sha: Option<String>,
    accept_command: Option<String>,
) -> Result<Value, String> {
    Ok(remote::run_remote_install(
        &repo,
        &name,
        scope.as_deref(),
        None,
        skill_commit_sha.as_deref(),
        cwd.as_deref(),
        accept_command.as_deref(),
        lang_of(&lang),
    ))
}

/// The optional modules went away with openkb: an empty list, not an error.
#[tauri::command]
fn get_modules(lang: String, probe: Option<bool>) -> Result<Value, String> {
    let _ = (lang_of(&lang), probe);
    Ok(json!({ "modules": [] }))
}

#[tauri::command]
fn set_module_enabled(lang: String, id: String, enabled: bool) -> Result<Value, String> {
    let _ = (lang_of(&lang), id, enabled);
    Ok(json!({ "modules": [] }))
}

fn not_ported(lang: &str) -> String {
    t(lang_of(lang), "app.notPorted", &[])
}

#[tauri::command]
fn describe_card(lang: String) -> Result<Value, String> {
    Err(t(lang_of(&lang), "app.describeRemoved", &[]))
}

#[tauri::command]
fn mod_get(lang: String, id: String, path: String) -> Result<Value, String> {
    let _ = (lang_of(&lang), id, path);
    Err(not_ported(&lang))
}

#[tauri::command]
fn mod_post(lang: String, id: String, path: String) -> Result<Value, String> {
    let _ = (lang_of(&lang), id, path);
    Err(not_ported(&lang))
}

/// The app entry point. Lives in the library crate, the shape the Tauri
/// template uses — mobile builds link this library through platform
/// frameworks instead of running the binary (`main.rs` stays untouched).
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // The app's own state lives in the per-user app data dir; the
            // env override keeps the old layout and the test roots working.
            let default_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("claude-lineup"));
            let root = std::env::var("CC_DATA_ROOT")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or(default_dir);
            paths::init(root);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_profiles,
            profile_get,
            profile_save,
            profile_delete,
            resolve_profile,
            apply_profile,
            backups_list,
            backup_restore,
            backup_delete,
            backups_clear,
            get_inventory,
            get_config_files,
            get_claude_json,
            probe_mcp,
            get_card,
            describe_card,
            install_plugin,
            get_remote,
            get_mcp_registry,
            plan_remote_install,
            run_remote_install,
            get_stats,
            clean_history,
            get_modules,
            set_module_enabled,
            mod_get,
            mod_post,
            pick_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
