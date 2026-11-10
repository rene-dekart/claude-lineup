//! Where everything lives. The web version derived APP_ROOT from the program
//! location; a desktop app writes nothing into its install dir, so the app's
//! own state (profiles, caches, backups) goes to the per-user app data dir.
//! Claude Code's own files stay where Claude Code keeps them — read-only here.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct AppPaths {
    pub data_root: PathBuf,
}

static PATHS: OnceLock<AppPaths> = OnceLock::new();

pub fn init(data_root: PathBuf) {
    let _ = PATHS.set(AppPaths { data_root });
}

fn paths() -> &'static AppPaths {
    PATHS
        .get()
        .expect("paths initialised at app setup (or in tests)")
}

pub fn home() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub fn claude_dir() -> PathBuf {
    home().join(".claude")
}

/// Large config: user-scope MCP + project entries. Read-only.
pub fn claude_json() -> PathBuf {
    home().join(".claude.json")
}

pub fn profile_dir() -> PathBuf {
    paths().data_root.join("profiles")
}

pub fn data_dir() -> PathBuf {
    paths().data_root.join("data")
}

/// One directory per apply: the bytes the written files held before it.
pub fn backup_dir() -> PathBuf {
    data_dir().join("backups")
}

pub fn project_settings(cwd: &Path) -> PathBuf {
    cwd.join(".claude").join("settings.json")
}

pub fn project_local_settings(cwd: &Path) -> PathBuf {
    cwd.join(".claude").join("settings.local.json")
}

pub fn project_mcp_json(cwd: &Path) -> PathBuf {
    cwd.join(".mcp.json")
}

/// Case- and separator-insensitive absolute path key, for the inside() bound.
/// Pure string work on purpose: canonicalize would lie for paths that do not
/// exist yet, and path.relative (the TS original) did no I/O either.
pub fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\").to_lowercase()
}

// ── scan paths: Claude Code's own tree, read-only ──

pub fn user_settings() -> PathBuf {
    claude_dir().join("settings.json")
}

pub fn user_skills() -> PathBuf {
    claude_dir().join("skills")
}

pub fn project_skills(cwd: &Path) -> PathBuf {
    cwd.join(".claude").join("skills")
}

pub fn plugins_dir() -> PathBuf {
    claude_dir().join("plugins")
}

pub fn marketplaces_dir() -> PathBuf {
    plugins_dir().join("marketplaces")
}

pub fn plugin_cache_dir() -> PathBuf {
    plugins_dir().join("cache")
}

pub fn installed_plugins() -> PathBuf {
    plugins_dir().join("installed_plugins.json")
}

/// Anthropic catalog cache: descriptions, install counts, their tokens.
pub fn plugin_catalog_cache() -> PathBuf {
    plugins_dir().join("plugin-catalog-cache.json")
}

/// Which marketplaces are already added: name → their source.
pub fn known_marketplaces() -> PathBuf {
    plugins_dir().join("known_marketplaces.json")
}

/// Cache of live MCP probes: server name → the size of its tools/list.
pub fn mcp_probe_cache() -> PathBuf {
    data_dir().join("mcp-probe.json")
}

/// Every session transcript: one directory per project cwd, one .jsonl per session.
pub fn projects_dir() -> PathBuf {
    claude_dir().join("projects")
}

/// context-mode's per-session SQLite stores. Read-only; its schema is its own.
pub fn context_mode_dir() -> PathBuf {
    claude_dir().join("context-mode")
}

/// Per-file aggregates of the transcripts. Derived data: losing it costs one
/// slow scan, never a wrong number.
pub fn stats_cache() -> PathBuf {
    data_dir().join("stats-cache.json")
}

/// Cache of external index responses. Their data updates once a day.
pub fn remote_cache() -> PathBuf {
    data_dir().join("remote-cache.json")
}

/// Cache of MCP registry pages and readmes, keyed by query and cursor.
pub fn mcp_cache() -> PathBuf {
    data_dir().join("mcp-registry-cache.json")
}

/// Where plugin content lives on disk: installed plugins, the marketplace
/// clones and the download cache. The detail card reads files only inside
/// these, the Claude config dir and the current project — nothing else.
pub fn plugin_roots() -> Vec<PathBuf> {
    vec![plugins_dir(), marketplaces_dir(), plugin_cache_dir()]
}

/// The marketplace manifest lives in one of four places — all of them occur.
pub const MANIFEST_PATHS: [&str; 4] = [
    ".claude-plugin/marketplace.json",
    ".agents/plugins/marketplace.json",
    ".grok-plugin/marketplace.json",
    ".github/plugin/marketplace.json",
];

/// A path as a row prints it: the home directory collapses to `~`, and a file
/// inside the project reads relative to it. `C:\Users\<user>\` in a caption is
/// the user's own name, which is noise everywhere it is not the subject.
pub fn shorten(file: &str, cwd: &str) -> String {
    let home = home().to_string_lossy().into_owned();
    if !home.is_empty() && file.starts_with(&home) {
        return format!("~{}", &file[home.len()..]);
    }
    if !cwd.is_empty() && file.to_lowercase().starts_with(&cwd.to_lowercase()) {
        return file[cwd.len()..]
            .trim_start_matches(['\\', '/'])
            .to_string();
    }
    file.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_is_lowercase_with_backslashes() {
        // Same tree typed with either separator must collide into one key.
        assert_eq!(norm(Path::new("X/Y/z")), "x\\y\\z");
        assert_eq!(norm(Path::new("X\\Y\\z")), "x\\y\\z");
        assert_eq!(norm(Path::new("X\\Y/z")), "x\\y\\z");
    }

    #[test]
    fn shorten_collapses_home_to_tilde_and_project_to_relative() {
        let home_file = home().join("sub").join("f.json");
        let s = shorten(&home_file.to_string_lossy(), "");
        assert!(s.starts_with('~'), "got {s}");
        assert!(s.ends_with("f.json"));

        let cwd = "D:\\work\\proj";
        let s = shorten("D:\\work\\proj\\.claude\\settings.json", cwd);
        assert_eq!(s, ".claude\\settings.json");
        // Case-insensitive prefix: the drive letter case drifts between tools.
        let s = shorten("d:\\work\\PROJ\\.mcp.json", cwd);
        assert_eq!(s, ".mcp.json");

        assert_eq!(
            shorten("E:\\elsewhere\\f.json", cwd),
            "E:\\elsewhere\\f.json",
            "an unrelated path stays whole"
        );
    }

    #[test]
    fn project_paths_and_roots() {
        let cwd = Path::new("D:/p");
        assert_eq!(
            project_settings(cwd),
            PathBuf::from("D:/p/.claude/settings.json")
        );
        assert_eq!(
            project_local_settings(cwd),
            PathBuf::from("D:/p/.claude/settings.local.json")
        );
        assert_eq!(project_mcp_json(cwd), PathBuf::from("D:/p/.mcp.json"));
        let roots = plugin_roots();
        assert!(roots.iter().all(|r| r.starts_with(claude_dir())));
        assert_eq!(MANIFEST_PATHS.len(), 4);
    }
}
