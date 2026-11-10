//! External skillselion index. This is an INDEX, not a marketplace: it knows
//! about the repository, but you cannot install from it. Installation still
//! goes through `claude plugin marketplace add <repo>` — so from the network we
//! take only "what exists and where it lives", and the CLI builds the action.
//! Port of `src/server/remote/skillselion.ts`.
//!
//! The network is optional here: an unavailable index does not break the
//! on-disk catalog, it turns into a note. Responses are cached for a day.

use crate::config;
use crate::http;
use crate::install;
use crate::jsonutil;
use crate::lang::{Lang, t};
use crate::netcache;
use crate::paths;
use serde_json::{Map, Value, json};

// ─────────────────────────── parsing a foreign response ───────────────────────────

fn str_of(v: &Value) -> Option<String> {
    v.as_str()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn num_of(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}

/// A record without `id`, `name`, or `repo` is useless: it can be displayed,
/// but nothing can be done with it. Dropped, not filled with dashes.
pub fn normalize_listing(raw: &Value) -> Option<Value> {
    let id = str_of(&raw["id"])?;
    let name = str_of(&raw["name"]).unwrap_or_else(|| id.clone());
    let repo = str_of(&raw["repo"])?;
    if name.is_empty() {
        return None;
    }

    let mut out = Map::new();
    out.insert("id".into(), json!(id));
    out.insert("name".into(), json!(name));
    out.insert(
        "type".into(),
        json!(str_of(&raw["type"]).unwrap_or_else(|| "unknown".into())),
    );
    out.insert(
        "author".into(),
        json!(str_of(&raw["author"]).unwrap_or_default()),
    );
    out.insert("repo".into(), json!(repo));
    out.insert(
        "description".into(),
        json!(str_of(&raw["description"]).unwrap_or_default()),
    );
    out.insert(
        "verifiedSafe".into(),
        json!(raw["verifiedSafe"] == json!(true)),
    );

    let opt: Vec<(&str, Option<Value>)> = vec![
        ("summary", str_of(&raw["summary"]).map(|s| json!(s))),
        ("category", str_of(&raw["category"]).map(|s| json!(s))),
        ("stars", num_of(&raw["stars"]).map(num_json)),
        ("installs", num_of(&raw["installs"]).map(num_json)),
        ("votes", num_of(&raw["votes"]).map(num_json)),
        ("trustScore", num_of(&raw["trustScore"]).map(num_json)),
        ("riskLevel", str_of(&raw["riskLevel"]).map(|s| json!(s))),
        ("qualityTier", str_of(&raw["qualityTier"]).map(|s| json!(s))),
        (
            "requiresApiKeys",
            raw["requiresApiKeys"].as_bool().map(|b| json!(b)),
        ),
        ("costToRun", str_of(&raw["costToRun"]).map(|s| json!(s))),
        (
            "skillCommitSha",
            str_of(&raw["skillCommitSha"]).map(|s| json!(s)),
        ),
        ("bestFor", str_of(&raw["bestFor"]).map(|s| json!(s))),
    ];
    for (k, v) in opt.into_iter().filter_map(|(k, v)| v.map(|val| (k, val))) {
        out.insert(k.into(), v);
    }
    Some(Value::Object(out))
}

/// Their API envelope: `items` + `hasMore`. We also accept an array — the shape is foreign.
pub fn parse_listings(payload: &Value) -> (Vec<Value>, bool) {
    let items = match payload.as_array() {
        Some(a) => a.clone(),
        None => payload["items"].as_array().cloned().unwrap_or_default(),
    };
    let entries: Vec<Value> = items.iter().filter_map(normalize_listing).collect();
    let has_more = payload.as_object().is_some() && payload["hasMore"] == json!(true);
    (entries, has_more)
}

/// Sort descending by the chosen metric, tie-break by name. A missing metric is
/// 0, not "to the end": most index records have no ratings at all.
/// // ponytail: byte compare instead of localeCompare — names here are ASCII;
/// swap in a locale crate if a localized index ever ships one that is not.
pub fn sort_listings(items: &mut [Value], sort: &str) {
    let key = |e: &Value| -> f64 {
        match sort {
            "stars" => e["stars"].as_f64().unwrap_or(0.0),
            "installs" => e["installs"].as_f64().unwrap_or(0.0),
            "trust" => e["trustScore"].as_f64().unwrap_or(0.0),
            _ => 0.0,
        }
    };
    let name = |e: &Value| e["name"].as_str().unwrap_or("").to_string();
    if sort == "name" {
        items.sort_by_key(|a| name(a));
    } else {
        items.sort_by(|a, b| {
            key(b)
                .partial_cmp(&key(a))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| name(a).cmp(&name(b)))
        });
    }
}

// ─────────────────────────── manifest and version pin ───────────────────────────

/// `HEAD` instead of a branch name: the repositories in the index are foreign,
/// and their default branch can be named anything. raw.githubusercontent
/// understands HEAD.
pub fn manifest_urls(repo: &str) -> Vec<String> {
    paths::MANIFEST_PATHS
        .iter()
        .map(|p| format!("https://raw.githubusercontent.com/{}/HEAD/{}", repo, p))
        .collect()
}

/// From the manifest we only need the marketplace name and plugin names.
pub fn parse_manifest(value: &Value) -> Option<Value> {
    let name = str_of(&value["name"])?;
    let plugins: Vec<String> = value["plugins"]
        .as_array()
        .map(|a| a.iter().filter_map(|p| str_of(&p["name"])).collect())
        .unwrap_or_default();
    Some(json!({ "name": name, "plugins": plugins }))
}

/// A plugin name inside the marketplace need not match the index record name.
/// Exact match — take it; a single plugin — take it; otherwise we do not guess.
pub fn pick_plugin(plugins: &[String], name: &str, lang: Lang) -> Value {
    if let Some(exact) = plugins.iter().find(|p| p.eq_ignore_ascii_case(name)) {
        return json!({ "plugin": exact });
    }
    if plugins.len() == 1 {
        return json!({ "plugin": plugins[0] });
    }
    if plugins.is_empty() {
        return json!({ "note": t(lang, "net.pickNoPlugins", &[]) });
    }
    json!({
        "note": t(
            lang,
            "net.pickManyPlugins",
            &[
                ("count", &plugins.len().to_string()),
                ("name", name),
                ("list", &plugins.iter().take(8).cloned().collect::<Vec<_>>().join(", ")),
            ],
        )
    })
}

/// `known_marketplaces.json`: name → source. We need a reverse lookup by repo.
pub fn known_marketplace_for(known: &Value, repo: &str) -> Option<String> {
    let obj = known.as_object()?;
    for (name, v) in obj {
        let src = &v["source"];
        let repo_ref = str_of(&src["repo"]).or_else(|| str_of(&src["url"]));
        if let Some(r) = repo_ref
            && r.to_lowercase().ends_with(&repo.to_lowercase())
        {
            return Some(name.clone());
        }
    }
    None
}

// ─────────────────────────── commands ───────────────────────────

pub fn add_argv(source: &str, scope: &str) -> Vec<String> {
    vec![
        "plugin".into(),
        "marketplace".into(),
        "add".into(),
        source.into(),
        "--scope".into(),
        scope.into(),
        "--json".into(),
    ]
}

pub fn install_argv(id: &str, scope: &str) -> Vec<String> {
    vec![
        "plugin".into(),
        "install".into(),
        id.into(),
        "--scope".into(),
        scope.into(),
        "--json".into(),
    ]
}

/// A string for showing to a human. Quotes only if it contains a space.
pub fn display(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.chars().any(|c| c.is_whitespace()) {
                json!(a).to_string()
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ─────────────────────────── network ───────────────────────────

fn get_json(url: &str, timeout_ms: u64) -> Result<Value, String> {
    http::get_json(
        url,
        &[
            ("api-version", "1"),
            ("accept", "application/json"),
            ("user-agent", "claude-lineup/0.1"),
        ],
        timeout_ms,
    )
}

const TYPE_TOTAL: [(&str, &str); 4] = [
    ("skill", "skills"),
    ("plugin", "plugins"),
    ("marketplace", "marketplaces"),
    ("mcp", "mcp"),
];

fn fetch_stats(lang: Lang) -> Result<Value, String> {
    let url = format!("{}/api/upstream/catalog/stats", config::remote_base());
    netcache::memo(
        &paths::remote_cache(),
        config::remote_ttl_ms(),
        "stats",
        lang,
        || {
            let raw = get_json(&url, config::timeout_net_ms())?;
            let mut out = Map::new();
            for k in ["skills", "plugins", "marketplaces", "mcp", "listings"] {
                if let Some(n) = num_of(&raw[k]) {
                    out.insert(k.into(), json!(n));
                }
            }
            Ok(Value::Object(out))
        },
    )
}

/// No network failure turns into an exception escaping outward: the panel must
/// show the on-disk catalog and say "index unavailable", not crash.
/// Whole-number floats are integers in JSON: 0, not 0.0 — the TS number.
fn num_json(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9e15 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

pub fn scan_remote(q: &str, type_: &str, sort: &str, limit: f64, lang: Lang) -> Value {
    let limit = limit.clamp(1.0, 100.0);
    let key = netcache::cache_key(&[
        ("q", Some(q)),
        ("type", Some(type_)),
        ("limit", Some(&format!("{}", limit as i64))),
    ]);

    let url_params = {
        let mut p = vec![format!("limit={}", limit as i64)];
        if !q.is_empty() {
            p.push(format!("q={}", urlencode(q)));
        }
        if !type_.is_empty() {
            p.push(format!("type={}", urlencode(type_)));
        }
        p.join("&")
    };

    let listings = {
        let url = format!("{}/api/v1/listings?{}", config::remote_base(), url_params);
        netcache::memo(
            &paths::remote_cache(),
            config::remote_ttl_ms(),
            &key,
            lang,
            || get_json(&url, config::timeout_net_ms()),
        )
    };

    let (mut entries, has_more, total, mut notes) = match listings {
        Ok(raw) => {
            let (entries, has_more) = parse_listings(&raw);
            let (total, more_notes) = match fetch_stats(lang) {
                Ok(stats) => {
                    // Their API does not return total for a query: take the total
                    // by type from stats, otherwise for the whole index.
                    let by_type = TYPE_TOTAL
                        .iter()
                        .find(|(k, _)| *k == type_)
                        .and_then(|(_, tk)| stats[*tk].as_f64());
                    (
                        by_type
                            .or_else(|| stats["listings"].as_f64())
                            .unwrap_or(0.0),
                        Vec::new(),
                    )
                }
                Err(_) => (0.0, vec![t(lang, "net.statsUnknown", &[])]),
            };
            (entries, has_more, num_json(total), more_notes)
        }
        Err(reason) => {
            // An outage is an empty answer with a note, never an error.
            return json!({
                "entries": [],
                "total": 0,
                "hasMore": false,
                "source": config::host_of(&config::remote_base()),
                "notes": [t(lang, "net.indexDown", &[("reason", &reason)])],
                "fetchedAt": crate::stats::iso_now(),
            });
        }
    };

    if !entries.is_empty()
        && entries.iter().all(|e| {
            e["stars"].as_f64().unwrap_or(0.0) == 0.0
                && e["installs"].as_f64().unwrap_or(0.0) == 0.0
                && e["trustScore"].as_f64().unwrap_or(0.0) == 0.0
        })
    {
        notes.push(t(lang, "net.noRatings", &[]));
    }

    sort_listings(&mut entries, sort);
    json!({
        "entries": entries,
        "total": total,
        "hasMore": has_more,
        "source": config::host_of(&config::remote_base()),
        "notes": notes,
        "fetchedAt": crate::stats::iso_now(),
    })
}

/// The TS original builds a URLSearchParams; percent-encoding is the same job.
pub fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ─────────────────────────── install plan ───────────────────────────

fn fetch_manifest(repo: &str, lang: Lang) -> Option<Value> {
    for url in manifest_urls(repo) {
        if let Ok(text) = http::get_text(&url, config::timeout_net_ms())
            && let Ok(v) = serde_json::from_str::<Value>(&text)
            && let Some(parsed) = parse_manifest(&v)
        {
            return Some(parsed);
        }
        let _ = lang;
    }
    None
}

/// A two-stage plan: add → install. The plan runs nothing, it only shows the
/// exact commands — execution is a separate request.
pub fn plan_remote(
    repo: &str,
    name: &str,
    scope: Option<&str>,
    ref_: Option<&str>,
    skill_commit_sha: Option<&str>,
    lang: Lang,
) -> Value {
    let scope = scope.unwrap_or("user");
    let mut notes: Vec<String> = Vec::new();
    let mut steps: Vec<Value> = Vec::new();

    let known =
        jsonutil::read_json(&paths::known_marketplaces(), lang).unwrap_or_else(|_| json!({}));
    let already = known_marketplace_for(&known, repo);

    let manifest = if already.is_some() {
        None
    } else {
        fetch_manifest(repo, lang)
    };
    let marketplace_name = already.clone().or_else(|| {
        manifest
            .as_ref()
            .and_then(|m| m["name"].as_str().map(String::from))
    });

    let source = match ref_ {
        Some(r) if !r.is_empty() => format!("{}#{}", repo, r),
        _ => repo.to_string(),
    };
    let unpinned = ref_.map(|r| r.is_empty()).unwrap_or(true);
    if unpinned {
        notes.push(match skill_commit_sha {
            Some(sha) => t(
                lang,
                "net.unpinnedSha",
                &[("sha", &sha.chars().take(10).collect::<String>())],
            ),
            None => t(lang, "net.unpinned", &[]),
        });
    }

    if already.is_none() {
        let argv = add_argv(&source, scope);
        steps.push(json!({ "display": display(&argv), "argv": argv }));
    } else {
        notes.push(t(
            lang,
            "net.marketplaceAlreadyAdded",
            &[("name", already.as_deref().unwrap())],
        ));
    }

    let marketplace_name = match marketplace_name {
        Some(m) => m,
        None => {
            notes.push(t(
                lang,
                "net.manifestNotFound",
                &[("repo", repo), ("paths", &paths::MANIFEST_PATHS.join(", "))],
            ));
            return json!({
                "ok": false,
                "repo": repo,
                "steps": steps,
                "alreadyAdded": already.is_some(),
                "unpinned": unpinned,
                "notes": notes,
                "error": t(lang, "net.noMarketplaceName", &[]),
            });
        }
    };

    let plugins: Vec<String> = manifest
        .as_ref()
        .and_then(|m| m["plugins"].as_array())
        .map(|a| {
            a.iter()
                .filter_map(|p| p.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let picked = pick_plugin(&plugins, name, lang);
    let plugin = picked["plugin"].as_str().map(String::from);
    let plugin = match plugin {
        Some(p) => p,
        None => {
            if let Some(note) = picked["note"].as_str() {
                notes.push(note.to_string());
            }
            return json!({
                "ok": false,
                "repo": repo,
                "steps": steps,
                "marketplaceName": marketplace_name,
                "alreadyAdded": already.is_some(),
                "unpinned": unpinned,
                "notes": notes,
                "error": t(lang, "net.cannotPickPlugin", &[]),
            });
        }
    };

    let plugin_id = format!("{}@{}", plugin, marketplace_name);
    let argv = install_argv(&plugin_id, scope);
    steps.push(json!({ "display": display(&argv), "argv": argv }));

    json!({
        "ok": true,
        "repo": repo,
        "steps": steps,
        "marketplaceName": marketplace_name,
        "pluginId": plugin_id,
        "alreadyAdded": already.is_some(),
        "unpinned": unpinned,
        "notes": notes,
    })
}

/// Runs exactly the stages that were shown in the plan. The first stage goes
/// through the CLI (`marketplace add`), the second — the same path as the
/// on-disk catalog, including the rule: code 0 with an unconfirmed marketplace
/// command is not success.
// The parameter list mirrors the invoke body's fields one to one.
#[allow(clippy::too_many_arguments)]
pub fn run_remote_install(
    repo: &str,
    name: &str,
    scope: Option<&str>,
    ref_: Option<&str>,
    skill_commit_sha: Option<&str>,
    cwd: Option<&str>,
    accept_command: Option<&str>,
    lang: Lang,
) -> Value {
    let scope = scope.unwrap_or("user");
    let plan = plan_remote(repo, name, Some(scope), ref_, skill_commit_sha, lang);
    let mut steps: Vec<Value> = plan["steps"].as_array().cloned().unwrap_or_default();
    if plan["ok"] != json!(true) {
        return json!({
            "ok": false,
            "steps": steps,
            "error": plan["error"],
        });
    }

    if plan["alreadyAdded"] != json!(true) {
        let first = &mut steps[0];
        let argv = first["argv"].as_array().unwrap();
        let source = argv[3].as_str().unwrap_or_default().to_string();
        match install::add_marketplace(&source, scope, cwd, lang) {
            Ok(r) => {
                first["ok"] = json!(r.code == Some(0));
                first["code"] = json!(r.code);
                first["stdout"] = json!(r.stdout);
                first["stderr"] = json!(r.stderr);
                if r.code != Some(0) {
                    return json!({
                        "ok": false,
                        "steps": steps,
                        "error": t(lang, "net.marketplaceAddFailed", &[("code", &r.code.map(|c| c.to_string()).unwrap_or_else(|| "null".into()))]),
                    });
                }
            }
            Err(e) => {
                return json!({ "ok": false, "steps": steps, "error": e });
            }
        }
    }

    let last = steps.len() - 1;
    let plugin_id = plan["pluginId"].as_str().unwrap_or_default().to_string();
    let res = install::install_plugin(&plugin_id, scope, cwd, accept_command, lang);
    let install_step = &mut steps[last];
    match res {
        Ok(res) => {
            install_step["ok"] = json!(res["ok"]);
            install_step["code"] = json!(res["code"]);
            install_step["stdout"] = json!(res["stdout"]);
            install_step["stderr"] = json!(res["stderr"]);
            let pending = res["pending"].clone();
            if !pending.is_null() {
                let mut p = pending;
                p["step"] = json!(last);
                return json!({
                    "ok": false,
                    "steps": steps,
                    "pending": p,
                    "error": t(lang, "net.marketplaceCommandAnnounced", &[]),
                });
            }
            let ok = res["ok"] == json!(true);
            let code = res["code"].to_string();
            json!({
                "ok": ok,
                "steps": steps,
                "error": if ok { Value::Null } else { json!(t(lang, "net.installFailed", &[("code", &code)])) },
            })
        }
        Err(e) => json!({ "ok": false, "steps": steps, "error": e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-remote-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn listing(id: &str, stars: f64, installs: f64, trust: f64) -> Value {
        json!({ "id": id, "name": id, "repo": format!("o/{}", id), "stars": stars, "installs": installs, "trustScore": trust })
    }

    #[test]
    fn records_without_repo_dropped() {
        let (entries, has_more) = parse_listings(&json!({
            "items": [
                { "id": "a", "name": "a", "repo": "o/a", "stars": 5 },
                { "id": "b", "name": "b" },
                { "name": "no id", "repo": "o/c" },
                "junk",
                { "id": "d", "name": "  ", "repo": "o/d" },
            ],
            "hasMore": true,
        }));
        // `b` has no repo, the third has no id, "junk" is not an object. The
        // blank name falls back to the id — the TS `str(o['name']) ?? id` keeps it.
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["id"], "a");
        assert_eq!(entries[0]["stars"], 5);
        assert_eq!(entries[0]["type"], "unknown");
        assert_eq!(entries[1]["name"], "d");
        assert!(has_more);
    }

    #[test]
    fn array_payload_accepted() {
        let (entries, has_more) = parse_listings(&json!([
            { "id": "x", "name": "x", "repo": "o/x" },
        ]));
        assert_eq!(entries.len(), 1);
        assert!(!has_more);
    }

    #[test]
    fn sort_desc_by_metric_then_name() {
        let mut items = vec![
            listing("b", 10.0, 0.0, 0.0),
            listing("a", 10.0, 0.0, 0.0),
            listing("c", 50.0, 0.0, 0.0),
            // A missing metric is 0, not "to the end by name".
            listing("d", 0.0, 0.0, 0.0),
        ];
        sort_listings(&mut items, "stars");
        let ids: Vec<&str> = items.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["c", "a", "b", "d"]);
        sort_listings(&mut items, "name");
        let ids: Vec<&str> = items.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["a", "b", "c", "d"]);
    }

    #[test]
    fn manifest_parse_only_what_is_needed() {
        let m = parse_manifest(&json!({
            "name": "mkt",
            "plugins": [{ "name": "p1" }, { "name": "" }, { "x": 1 }, "junk"],
            "extra": "ignored",
        }))
        .unwrap();
        assert_eq!(m["name"], "mkt");
        assert_eq!(m["plugins"], json!(["p1"]));
        assert!(parse_manifest(&json!({ "plugins": [] })).is_none());
    }

    #[test]
    fn pick_plugin_exact_single_or_refusal() {
        let lang = Lang::Ru;
        let p = pick_plugin(&["Alpha".into(), "beta".into()], "alpha", lang);
        assert_eq!(p["plugin"], "Alpha", "exact match wins over case");
        assert_eq!(
            pick_plugin(&["only".into()], "other", lang)["plugin"],
            "only"
        );
        assert!(pick_plugin(&[], "x", lang)["note"].is_string());
        let many = pick_plugin(&["a".into(), "b".into(), "c".into()], "x", lang);
        assert!(many["note"].as_str().unwrap().contains("3"));
        assert!(many["plugin"].is_null());
    }

    #[test]
    fn known_marketplace_reverse_lookup() {
        let known = json!({
            "claude-code": { "source": { "repo": "anthropics/claude-code" } },
            "git": { "source": { "url": "https://github.com/o/r" } },
        });
        assert_eq!(
            known_marketplace_for(&known, "anthropics/claude-code").as_deref(),
            Some("claude-code")
        );
        assert_eq!(known_marketplace_for(&known, "o/r").as_deref(), Some("git"));
        // `…r.git`.endsWith("o/r") is false — the .git suffix hides it, in TS too.
        let with_git = json!({ "git": { "source": { "url": "https://github.com/o/r.git" } } });
        assert_eq!(known_marketplace_for(&with_git, "o/r"), None);
        assert_eq!(known_marketplace_for(&known, "other/repo"), None);
        assert_eq!(known_marketplace_for(&json!(null), "o/r"), None);
    }

    #[test]
    fn manifest_urls_cover_all_four_paths() {
        let urls = manifest_urls("o/r");
        assert_eq!(urls.len(), 4);
        assert!(urls[0].ends_with("/HEAD/.claude-plugin/marketplace.json"));
        assert!(urls[0].starts_with("https://raw.githubusercontent.com/o/r/"));
    }

    #[test]
    fn display_quotes_only_spaces() {
        let d = display(&[
            "claude".into(),
            "plugin".into(),
            "install".into(),
            "a b@c".into(),
        ]);
        assert!(d.contains("\"a b@c\""));
        assert!(d.contains("claude plugin install"));
    }

    #[test]
    fn urlencode_matches_search_params() {
        assert_eq!(urlencode("a b&c"), "a%20b%26c");
        assert_eq!(urlencode("ascii"), "ascii");
    }

    #[test]
    fn plan_without_network_says_no_marketplace() {
        // A repo whose manifest paths 404 (offline test env): the plan is honest
        // about the missing name instead of inventing a second stage.
        let v = plan_remote(
            "o/definitely-missing",
            "x",
            Some("user"),
            None,
            None,
            Lang::Ru,
        );
        assert_eq!(v["ok"], false);
        assert!(v["error"].is_string());
        assert_eq!(
            v["steps"].as_array().unwrap().len(),
            1,
            "the add step is still shown"
        );
        assert_eq!(v["unpinned"], true);
    }

    #[test]
    fn argv_builders_match_the_ts_shape() {
        assert_eq!(
            add_argv("o/r", "user"),
            vec![
                "plugin",
                "marketplace",
                "add",
                "o/r",
                "--scope",
                "user",
                "--json"
            ]
        );
        assert_eq!(
            install_argv("p@m", "project"),
            vec!["plugin", "install", "p@m", "--scope", "project", "--json"]
        );
    }

    #[test]
    fn tmp_works() {
        let _ = tmp("touch");
    }
}
