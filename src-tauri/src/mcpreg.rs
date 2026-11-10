//! The official MCP registry — a search index of MCP servers, and nothing else.
//! It is not a source of quality (no ratings, no installs — the rows come back
//! in the order the registry gives them) and not an installer: the only way out
//! of a row is the link to the repository. Port of
//! `src/server/remote/mcpregistry.ts`.
//!
//! Pages are cached on disk for a day, keyed by query and cursor. A search
//! costs 9–15 s: without the cache every return to the tab pays it again. Only
//! successful answers are stored, so an outage is retried rather than remembered.

use crate::config;
use crate::http;
use crate::lang::{Lang, t};
use crate::netcache;
use crate::paths;
use serde_json::{Value, json};

fn str_of(v: &Value) -> Option<String> {
    v.as_str()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

/// `https://github.com/owner/repo(.git)?(/…)` → `owner/repo`. The TS regex is
/// `/^https?:\/\/github\.com\/([^/?#]+\/[^/?#]+)/` — exactly two path segments.
pub fn repo_slug(url: &Value) -> Option<String> {
    let s = str_of(url)?;
    let rest = s
        .strip_prefix("https://github.com/")
        .or_else(|| s.strip_prefix("http://github.com/"))?;
    let rest = rest.split(['?', '#']).next()?;
    let mut parts = rest.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    Some(format!("{}/{}", owner, repo))
}

/// `owner/repo` and nothing else. The repository comes from a foreign record
/// and travels into a URL we then fetch, so it is checked here, not trusted.
/// The TS whitelist is `/^[\w.-]+\/[\w.-]+$/` — ASCII only.
pub fn valid_repo(repo: &Value) -> Option<String> {
    let s = str_of(repo)?;
    let (owner, repo) = s.split_once('/')?;
    let part_ok = |p: &str| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    };
    if part_ok(owner) && part_ok(repo) {
        Some(s)
    } else {
        None
    }
}

/// How a person would connect: a remote's own type, or the type of a package's
/// transport. `registryType` is which package registry it lives in (npm, pypi) —
/// reading that as a transport would print "npm" where the answer is "stdio".
fn transports_of(s: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in list(&s["remotes"]) {
        if let Some(x) = str_of(&r["type"])
            && !out.contains(&x)
        {
            out.push(x);
        }
    }
    for p in list(&s["packages"]) {
        if let Some(x) = str_of(&p["transport"]["type"])
            && !out.contains(&x)
        {
            out.push(x);
        }
    }
    out
}

/// A connection needs a secret when a header or an env var is marked as one.
fn needs_secret(s: &Value) -> bool {
    let mut marks: Vec<Value> = Vec::new();
    for r in list(&s["remotes"]) {
        marks.extend(list(&r["headers"]));
    }
    for p in list(&s["packages"]) {
        marks.extend(list(&p["environmentVariables"]));
    }
    marks.iter().any(|h| h["isSecret"] == json!(true))
}

/// A record with no name, or with nothing that can be connected, is a dead row:
/// dropped, rather than shown with a dash where the transport should be.
pub fn normalize_server(raw: &Value) -> Option<Value> {
    let s = raw["server"].as_object()?;
    let s = Value::Object(s.clone());
    let name = str_of(&s["name"])?;
    let transports = transports_of(&s);
    if transports.is_empty() {
        return None;
    }

    let mut row = json!({
        "name": name,
        "title": str_of(&s["title"]).unwrap_or_else(|| name.clone()),
        "description": str_of(&s["description"]).unwrap_or_default(),
        "version": str_of(&s["version"]).unwrap_or_default(),
        "transports": transports,
    });
    if let Some(repo) = repo_slug(&s["repository"]["url"]) {
        row["repo"] = json!(repo);
    }
    if needs_secret(&s) {
        row["secret"] = json!(true);
    }
    Some(row)
}

/// Their envelope: `servers[]` + `metadata.nextCursor`.
pub fn parse_servers(payload: &Value) -> (Vec<Value>, Option<String>) {
    let entries: Vec<Value> = list(&payload["servers"])
        .iter()
        .filter_map(normalize_server)
        .collect();
    let next = str_of(&payload["metadata"]["nextCursor"]);
    (entries, next)
}

/// `version=latest` is not cosmetic: without it the registry returns every
/// published version of a server, so one server shows up three times.
///
/// A network failure becomes a note and an empty page, never an error — the
/// panel has to stay usable when the registry is down.
pub fn scan_mcp_registry(q: &str, limit: f64, cursor: Option<&str>, lang: Lang) -> Value {
    let limit = limit.clamp(1.0, 100.0);
    let mut params = vec![format!("limit={}", limit as i64), "version=latest".into()];
    if !q.is_empty() {
        params.push(format!("search={}", crate::remote::urlencode(q)));
    }
    if let Some(c) = cursor {
        params.push(format!("cursor={}", crate::remote::urlencode(c)));
    }
    let url = format!(
        "{}/servers?{}",
        config::mcp_registry_base(),
        params.join("&")
    );
    let key = netcache::cache_key(&[
        ("q", Some(q)),
        ("limit", Some(&format!("{}", limit as i64))),
        ("cursor", cursor),
    ]);

    let base = config::mcp_registry_base();
    // A throwing `load` leaves the entry unwritten — an outage is not cached.
    let loaded = netcache::memo(
        &paths::mcp_cache(),
        config::remote_ttl_ms(),
        &key,
        lang,
        || {
            http::get_json(
                &url,
                &[
                    ("accept", "application/json"),
                    ("user-agent", "claude-lineup/0.1"),
                ],
                config::timeout_mcp_ms(),
            )
        },
    );

    match loaded {
        Ok(raw) => {
            let (entries, next_cursor) = parse_servers(&raw);
            let mut out = json!({
                "entries": entries,
                "source": config::host_of(&base),
                "notes": [],
                "fetchedAt": crate::stats::iso_now(),
            });
            if let Some(c) = next_cursor {
                out["nextCursor"] = json!(c);
            }
            out
        }
        Err(reason) => json!({
            "entries": [],
            "source": config::host_of(&base),
            "notes": [t(lang, "net.mcpDown", &[("reason", &reason)])],
            "fetchedAt": crate::stats::iso_now(),
        }),
    }
}

// ─────────────────────────── README ───────────────────────────

/// Where a repository's readme usually lives. The first one that answers wins.
pub const README_PATHS: [&str; 4] = ["README.md", "readme.md", "Readme.md", "README.markdown"];

pub fn readme_urls(repo: &str) -> Vec<String> {
    README_PATHS
        .iter()
        .map(|p| format!("https://raw.githubusercontent.com/{}/HEAD/{}", repo, p))
        .collect()
}

/// A readme is a whole file and the card shows it in one pane. Its tail is
/// usually badges, licence and changelog — cut at a line, never mid-word.
const README_MAX: usize = 20_000;

pub fn readme_head(text: &str) -> String {
    if text.len() <= README_MAX {
        return text.to_string();
    }
    let cut = &text[..README_MAX];
    match cut.rfind('\n') {
        Some(i) if i > 0 => cut[..i].to_string(),
        _ => cut.to_string(),
    }
}

/// The only network read behind a card. Cached with the registry pages: a miss
/// is cached too, or a repository with no readme would cost four requests per open.
pub fn fetch_readme(repo: &Value, lang: Lang) -> Option<String> {
    let clean = valid_repo(repo)?;
    let key = format!("readme={}", clean);
    netcache::memo(
        &paths::mcp_cache(),
        config::remote_ttl_ms(),
        &key,
        lang,
        || {
            for url in readme_urls(&clean) {
                if let Ok(text) = http::get_text(&url, config::timeout_net_ms()) {
                    return Ok(json!(readme_head(&text)));
                }
            }
            // All four spellings missing: cached as absent, per the TS original.
            Ok(Value::Null)
        },
    )
    .ok()
    .and_then(|v| v.as_str().map(String::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_slug_strips_the_decorations() {
        assert_eq!(
            repo_slug(&json!("https://github.com/o/r")).as_deref(),
            Some("o/r")
        );
        assert_eq!(
            repo_slug(&json!("https://github.com/o/r.git")).as_deref(),
            Some("o/r")
        );
        assert_eq!(
            repo_slug(&json!("https://github.com/o/r/tree/x")).as_deref(),
            Some("o/r")
        );
        assert_eq!(repo_slug(&json!("https://gitlab.com/o/r")), None);
        assert_eq!(repo_slug(&json!("not a url")), None);
    }

    #[test]
    fn valid_repo_whitelists_the_shape() {
        assert_eq!(
            valid_repo(&json!("owner/repo")).as_deref(),
            Some("owner/repo")
        );
        assert_eq!(
            valid_repo(&json!("my.org_x-1/my.repo_2")).as_deref(),
            Some("my.org_x-1/my.repo_2")
        );
        assert!(valid_repo(&json!("owner")).is_none());
        assert!(valid_repo(&json!("ow ner/repo")).is_none());
        assert!(valid_repo(&json!("owner/re\"po")).is_none());
        assert!(valid_repo(&json!("")).is_none());
        // ASCII-only whitelist, exactly like the TS `[\w.-]`.
        assert!(valid_repo(&json!("bad/owner!")).is_none());
    }

    #[test]
    fn transports_and_secret_from_foreign_record() {
        let s = json!({
            "name": "io/x/y",
            "remotes": [{ "type": "streamable-http", "headers": [{ "isSecret": true }] }],
            "packages": [{ "transport": { "type": "stdio" }, "registryType": "npm" }],
        });
        let row = normalize_server(&json!({ "server": s })).unwrap();
        // registryType is NOT a transport: "npm" would be a lie where "stdio" is the answer.
        assert_eq!(row["transports"], json!(["streamable-http", "stdio"]));
        assert_eq!(row["secret"], true);
        assert_eq!(row["title"], "io/x/y", "title falls back to the name");
    }

    #[test]
    fn dead_rows_dropped() {
        assert!(normalize_server(&json!({})).is_none());
        assert!(
            normalize_server(&json!({ "server": { "name": "x" } })).is_none(),
            "no transports, no row"
        );
        assert!(
            normalize_server(&json!({ "server": { "transports": ["stdio"] } })).is_none(),
            "no name, no row"
        );
    }

    #[test]
    fn servers_envelope_and_cursor() {
        let payload = json!({
            "servers": [
                { "server": { "name": "a", "remotes": [{ "type": "sse" }] } },
                { "server": { "name": "b" } },
            ],
            "metadata": { "nextCursor": "cur1" },
        });
        let (entries, cursor) = parse_servers(&payload);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "a");
        assert_eq!(cursor.as_deref(), Some("cur1"));
        let (entries, cursor) = parse_servers(&json!({ "servers": [] }));
        assert!(entries.is_empty());
        assert!(cursor.is_none());
    }

    #[test]
    fn readme_head_cuts_at_a_line() {
        assert_eq!(readme_head("short"), "short");
        let long = "ab\n".repeat(10_000);
        let head = readme_head(&long);
        assert!(head.len() < README_MAX + 4);
        assert!(head.ends_with("ab"), "the cut lands on a line boundary");
    }

    #[test]
    fn readme_urls_cover_the_spellings() {
        let urls = readme_urls("o/r");
        assert_eq!(urls.len(), 4);
        assert!(urls[0].ends_with("/HEAD/README.md"));
    }
}
