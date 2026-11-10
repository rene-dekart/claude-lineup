//! Installation is performed by Claude Code ITSELF (`claude plugin install`),
//! not by our code. Reason: installed_plugins.json and settings.json belong to
//! the CLI, and a running Claude Code process overwrites settings.json itself —
//! writing there from outside is a race that guarantees losing someone's edits.
//! Port of `src/server/plugins/install.ts`.
//!
//! The -y flag is NOT passed by default: if the marketplace declared installation
//! via a command, the CLI returns it and its sha256, and we must show this to
//! the human. Silently running a foreign command is executing code without asking.

use crate::config;
use crate::lang::{Lang, t};
use serde_json::{Value, json};

const ID_OK: fn(&str) -> bool = |id| {
    let (name, mkt) = match id.split_once('@') {
        Some(x) => x,
        None => return false,
    };
    // Exactly one `@` — name and marketplace are both non-empty and whitelisted.
    !name.is_empty()
        && !mkt.is_empty()
        && !mkt.contains('@')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        && mkt
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
};

// /^[a-f0-9]{64}$/i — the `i` flag lets hex digits of either case through.
const SHA256_OK: fn(&str) -> bool = |s| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());

const SCOPES: [&str; 3] = ["user", "project", "local"];

fn scope_ok(scope: &str) -> bool {
    SCOPES.contains(&scope)
}

pub struct RunResult {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// The single entry point for launching the CLI. Both stages of the remote
/// install chain go through it. On Windows `claude` is a .cmd, it cannot be
/// launched without a shell — the whole line goes to cmd.exe, like the TS
/// original's `spawn` with `shell: true`.
fn run_claude(args: &[&str], cwd: Option<&str>, timeout_ms: u64, lang: Lang) -> RunResult {
    // A string for the shell. Quotes only what must be quoted — a `"` in argv is
    // a refusal at the whitelist level, never a mangling (see repourl/SOURCE).
    let is_win = cfg!(windows);
    let q = |s: &str| -> String {
        if is_win && s.chars().any(|c| c.is_whitespace() || c == '"') {
            format!("\"{}\"", s)
        } else {
            s.to_string()
        }
    };
    let mut parts = vec![config::claude_bin()];
    parts.extend(args.iter().map(|s| s.to_string()));
    let line = parts.iter().map(|s| q(s)).collect::<Vec<_>>().join(" ");

    let mut cmd = if is_win {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", &line]);
        c
    } else {
        let mut c = std::process::Command::new(config::claude_bin());
        c.args(args);
        c
    };
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return RunResult {
                code: None,
                stdout: String::new(),
                stderr: format!(
                    "{}\n{}",
                    t(
                        lang,
                        "net.binNotStarted",
                        &[("bin", &config::claude_bin()), ("reason", &e.to_string())]
                    ),
                    ""
                ),
            };
        }
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let (stdout, stderr) = match child.wait_with_output() {
                    Ok(o) => (
                        String::from_utf8_lossy(&o.stdout).into_owned(),
                        String::from_utf8_lossy(&o.stderr).into_owned(),
                    ),
                    Err(_) => (String::new(), String::new()),
                };
                return RunResult {
                    code: status.code(),
                    stdout,
                    stderr,
                };
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return RunResult {
                        code: None,
                        stdout: String::new(),
                        stderr: format!(
                            "\n{}",
                            t(
                                lang,
                                "net.timeout",
                                &[("seconds", &(timeout_ms / 1000).to_string())]
                            )
                        ),
                    };
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(_) => {
                return RunResult {
                    code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                };
            }
        }
    }
}

/// The marketplace source goes into the command line, and on Windows the shell
/// is enabled. Hence a character whitelist, not a blacklist: space, quotes, `;`,
/// `&`, `|`, `$`, backtick, `>`, `<`, brackets and `!` do not pass through it.
/// A leading dash is also impossible — it would turn the source into a CLI flag.
pub fn valid_marketplace_source(source: &str) -> bool {
    let mut chars = source.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() || "./\\".contains(c) => {}
        _ => return false,
    }
    source
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._/\\:@#~+=%-".contains(c))
}

/// The marketplace declared a command: it must be shown and confirmed.
fn pending_from(stdout: &str) -> Option<Value> {
    for line in stdout.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        let obj: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let cmd = obj["shownCommand"]["command"].as_str();
        let sha = obj["shownCommand"]["sha256"].as_str();
        if let (Some(c), Some(s)) = (cmd, sha) {
            return Some(json!({ "command": c, "sha256": s }));
        }
    }
    None
}

/// The first stage of the remote chain: `claude plugin marketplace add <source>`.
pub fn add_marketplace(
    source: &str,
    scope: &str,
    cwd: Option<&str>,
    lang: Lang,
) -> Result<RunResult, String> {
    if !valid_marketplace_source(source) {
        return Err(t(
            lang,
            "net.badMarketplaceSource",
            &[("source", &json!(source).to_string())],
        ));
    }
    if !scope_ok(scope) {
        return Err(t(
            lang,
            "net.badScope",
            &[("scope", scope), ("allowed", &SCOPES.join(", "))],
        ));
    }
    Ok(run_claude(
        &[
            "plugin",
            "marketplace",
            "add",
            source,
            "--scope",
            scope,
            "--json",
        ],
        cwd,
        config::timeout_install_ms(),
        lang,
    ))
}

/// Validate-and-run for one plugin id. Errors are the request's own verdict (the
/// TS original rejects the promise), so the caller turns them into `Err`.
pub fn install_plugin(
    id: &str,
    scope: &str,
    cwd: Option<&str>,
    accept: Option<&str>,
    lang: Lang,
) -> Result<Value, String> {
    if !ID_OK(id) {
        return Err(t(
            lang,
            "net.badPluginId",
            &[("id", &json!(id).to_string())],
        ));
    }
    if !scope_ok(scope) {
        return Err(t(
            lang,
            "net.badScope",
            &[("scope", scope), ("allowed", &SCOPES.join(", "))],
        ));
    }
    if let Some(a) = accept
        && !SHA256_OK(a)
    {
        return Err(t(lang, "net.badAcceptCommand", &[]));
    }

    let mut args: Vec<&str> = vec!["plugin", "install", id, "--scope", scope, "--json"];
    if let Some(a) = accept {
        args.push("--accept-command");
        args.push(a);
        args.push("--yes");
    }

    let r = run_claude(&args, cwd, config::timeout_install_ms(), lang);
    let pending = pending_from(&r.stdout);
    Ok(json!({
        "ok": r.code == Some(0) && pending.is_none(),
        "code": r.code,
        "stdout": r.stdout,
        "stderr": r.stderr,
        "pending": pending,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_whitelist() {
        assert!(ID_OK("superpowers@claude-code"));
        assert!(ID_OK("a.b-c_d@market.pl_ace-x"));
        assert!(!ID_OK("no-marketplace"));
        assert!(!ID_OK("a@b@c"));
        assert!(!ID_OK("@b"));
        assert!(!ID_OK("a@"));
        // A `"` or a space in argv is a refusal, not mangling.
        assert!(!ID_OK("a b@c"));
        assert!(!ID_OK("a\"@c"));
    }

    #[test]
    fn marketplace_source_whitelist() {
        assert!(valid_marketplace_source("owner/repo"));
        assert!(valid_marketplace_source("D:/repos/mkt"));
        assert!(valid_marketplace_source("https://github.com/o/r"));
        assert!(valid_marketplace_source("repo#ref"));
        // Shell metacharacters and a leading dash never pass.
        assert!(!valid_marketplace_source("a b"));
        assert!(!valid_marketplace_source("-flag"));
        assert!(!valid_marketplace_source("a;rm"));
        assert!(!valid_marketplace_source("a&b"));
        assert!(!valid_marketplace_source("a|b"));
        assert!(!valid_marketplace_source("a>b"));
        // `%` passes the TS whitelist too (install.ts SOURCE) — ported as is.
        assert!(valid_marketplace_source("a%1"));
    }

    #[test]
    fn pending_read_off_stdout_lines() {
        let stdout = "noise\n{\"shownCommand\":{\"command\":\"curl evil\",\"sha256\":\"a\"}}\n";
        let p = pending_from(stdout).unwrap();
        assert_eq!(p["command"], "curl evil");
        assert!(pending_from("{\"ok\":true}").is_none());
        assert!(pending_from("not json").is_none());
    }

    #[test]
    fn sha256_rule() {
        let ok = "a".repeat(64);
        assert!(SHA256_OK(&ok));
        let upper = "A".repeat(64);
        assert!(SHA256_OK(&upper));
        assert!(!SHA256_OK(&"g".repeat(64)));
        assert!(!SHA256_OK("abc"));
    }
}
