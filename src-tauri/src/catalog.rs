//! The catalog — what the machine knows about, whether or not it is actually
//! installed. Port of `src/server/scan/catalog.ts` (the parts the inventory
//! needs; rank/page come with the remote-catalog command).
//!
//! Two sources: the Anthropic catalog cache (descriptions, install counts,
//! tokens counted by THEM) and marketplace manifests on disk. Nothing is
//! invented: no description in the source means no line, and token figures are
//! marked as someone else's.

use crate::cost;
use crate::jsonutil::{exists, list_dir, read_json};
use crate::lang::{Lang, t};
use crate::paths;
use crate::repourl::repo_url;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

pub struct CatalogSources {
    pub entries: Vec<Value>,
    /// id → entry; the cache entry wins over a manifest's.
    pub by_id: HashMap<String, Value>,
    /// Shape parity with the TS reference (`catalog/sources.ts`): the fields
    /// exist there and every consumer may reach for them. Unused in Rust so far.
    #[allow(dead_code)]
    pub marketplaces: Vec<Value>,
    #[allow(dead_code)]
    pub notes: Vec<String>,
    /// Links the manifests carry, kept apart from `by_id` on purpose: the cache
    /// entry wins there and has no link, so a link read off the winning entry
    /// would be missing exactly where the manifest does have one.
    pub manifest_links: HashMap<String, String>,
}

pub fn marketplace_manifest_file(dir: &Path) -> Option<std::path::PathBuf> {
    for rel in paths::MANIFEST_PATHS {
        let f = dir.join(rel);
        if exists(&f) {
            return Some(f);
        }
    }
    None
}

fn split_id(id: &str) -> (&str, &str) {
    match id.rfind('@') {
        Some(at) => (&id[..at], &id[at + 1..]),
        None => (id, ""),
    }
}

/// Tokens from the catalog cache were counted by Anthropic, not by us — hence
/// `estimated`, and the note says outright whose figures these are.
fn cache_cost(p: &Value, lang: Lang) -> Value {
    let tokens = p.get("tokens").and_then(|v| v.as_object());
    let Some(tokens) = tokens else {
        return cost::unknown(&t(lang, "scan.catalogNoTokenFigures", &[]));
    };
    let models: Vec<&String> = tokens.keys().collect();
    let model = models
        .iter()
        .find(|m| m.contains("opus"))
        .or_else(|| models.first())
        .map(|m| m.as_str());
    let Some(model) = model else {
        return cost::unknown(&t(lang, "scan.catalogNoTokenFigures", &[]));
    };
    let entry = tokens.get(model);
    let always = entry
        .and_then(|e| e.get("always_on"))
        .and_then(|v| v.as_f64());
    let Some(always) = always else {
        return cost::unknown(&t(lang, "scan.catalogNoTokenFigures", &[]));
    };
    let invoke = entry
        .and_then(|e| e.get("on_invoke"))
        .and_then(|v| v.as_f64())
        .map(|n| {
            t(
                lang,
                "scan.catalogCostOnInvoke",
                &[("tokens", &format_tokens(n))],
            )
        })
        .unwrap_or_default();
    json!({
        "level": "estimated",
        "tokens": always as i64,
        "note": t(
            lang,
            "scan.catalogCost",
            &[
                ("model", model),
                ("always", &format_tokens(always)),
                ("invoke", &invoke),
            ],
        ),
    })
}

fn format_tokens(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn cache_entry(id: &str, p: &Value, lang: Lang) -> Value {
    let (name, marketplace) = split_id(id);
    let entry = p.get("marketplace_entry").cloned().unwrap_or(Value::Null);
    let display = entry
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| p.get("plugin").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_else(|| name.to_string());
    let bundled_skills: Vec<Value> =
        entry_skip(entry.get("components").and_then(|c| c.get("skills")))
            .into_iter()
            .map(Value::String)
            .collect();
    json!({
        "id": id,
        "name": display,
        "marketplace": marketplace,
        "description": entry.get("description").and_then(|v| v.as_str()).unwrap_or("").trim(),
        "category": entry.get("category").cloned().unwrap_or(Value::Null),
        "version": p.get("version").cloned().unwrap_or(Value::Null),
        "installs": p.get("unique_installs").and_then(|v| v.as_i64()),
        "bundledSkills": bundled_skills,
        "bundledMcp": p.get("components").and_then(|c| c.get("mcpServers")).and_then(|v| v.as_array()).cloned().unwrap_or_default(),
        "cost": cache_cost(p, lang),
        "installed": false,
    })
}

fn entry_skip(v: Option<&Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
                .filter(|n| !n.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Catalog cache: `{ catalog: { plugins: { "name@marketplace": {...} } } }`.
fn parse_catalog_cache(value: &Value, lang: Lang) -> Vec<Value> {
    let plugins = value
        .get("catalog")
        .and_then(|c| c.get("plugins"))
        .and_then(|p| p.as_object());
    let Some(plugins) = plugins else {
        return Vec::new();
    };
    plugins
        .iter()
        .map(|(id, p)| cache_entry(id, p, lang))
        .collect()
}

/// Marketplace manifest. Everyone has descriptions, nobody has tokens — so the
/// cost is honestly unknown, not "zero".
fn parse_marketplace_manifest(value: &Value, marketplace: &str, lang: Lang) -> Vec<Value> {
    let plugins = value.get("plugins").and_then(|p| p.as_array());
    let Some(plugins) = plugins else {
        return Vec::new();
    };
    plugins
        .iter()
        .filter_map(|p| {
            let name = p.get("name").and_then(|n| n.as_str())?;
            Some(json!({
                "id": format!("{}@{}", name, marketplace),
                "name": name,
                "marketplace": marketplace,
                "description": p.get("description").and_then(|v| v.as_str()).unwrap_or("").trim(),
                "category": p.get("category").cloned().unwrap_or(Value::Null),
                "version": p.get("version").cloned().unwrap_or(Value::Null),
                "installs": Value::Null,
                "bundledSkills": [],
                "bundledMcp": [],
                "cost": cost::unknown(&t(lang, "scan.manifestNoCost", &[])),
                "installed": false,
            }))
        })
        .collect()
}

fn catalog_sources(lang: Lang) -> &'static CatalogSources {
    // Memoised per language: the notes and cost captions are localised, and
    // the read costs one 337 KB parse plus 2.5k manifests.
    static RU: OnceLock<CatalogSources> = OnceLock::new();
    static EN: OnceLock<CatalogSources> = OnceLock::new();
    let store = match lang {
        Lang::Ru => &RU,
        Lang::En => &EN,
    };
    store.get_or_init(|| build_catalog_sources(lang))
}

/// The catalog's id → entry map, for the detail card (kind in, card out).
pub fn catalog_by_id(lang: Lang) -> &'static HashMap<String, Value> {
    &catalog_sources(lang).by_id
}

fn build_catalog_sources(lang: Lang) -> CatalogSources {
    let mut notes: Vec<String> = Vec::new();
    let mut by_id: HashMap<String, Value> = HashMap::new();

    let cache_file = paths::plugin_catalog_cache();
    match read_json(&cache_file, lang) {
        Ok(cache) => {
            for e in parse_catalog_cache(&cache, lang) {
                let id = e
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                by_id.insert(id, e);
            }
        }
        Err(error) => {
            if exists(&cache_file) {
                notes.push(t(
                    lang,
                    "scan.catalogCacheUnread",
                    &[("file", &cache_file.to_string_lossy()), ("error", &error)],
                ));
            }
        }
    }

    let known = read_json(&paths::known_marketplaces(), lang).unwrap_or(Value::Null);
    let known_map = known.as_object().cloned().unwrap_or_default();

    let mut marketplaces: Vec<Value> = Vec::new();
    let mut manifest_links: HashMap<String, String> = HashMap::new();
    let mkt_dir = paths::marketplaces_dir();
    for dir in list_dir(&mkt_dir) {
        let root = mkt_dir.join(&dir);
        if !root.is_dir() {
            continue;
        }
        let Some(file) = marketplace_manifest_file(&root) else {
            notes.push(t(lang, "scan.marketplaceNoManifest", &[("name", &dir)]));
            continue;
        };
        let Ok(value) = read_json(&file, lang) else {
            notes.push(t(
                lang,
                "scan.marketplaceReadError",
                &[
                    ("name", &dir),
                    ("error", &t(lang, "io.badJson", &[("err", "-")])),
                ],
            ));
            continue;
        };
        let entries = parse_marketplace_manifest(&value, &dir, lang);
        // The cache is richer (descriptions + tokens + composition), so the
        // manifest only adds what the cache lacks.
        for e in &entries {
            let id = e
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            by_id.entry(id).or_insert_with(|| e.clone());
        }

        // A plugin's own link, else the repo the marketplace itself lives in —
        // for the majority of entries whose `source` is a relative path inside it.
        let market_repo = known_map
            .get(&dir)
            .and_then(|k| k.get("source"))
            .and_then(repo_url);
        if let Some(plugins) = value.get("plugins").and_then(|p| p.as_array()) {
            for p in plugins {
                let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.is_empty() || manifest_links.contains_key(&format!("{}@{}", name, dir)) {
                    continue;
                }
                let link = p
                    .get("homepage")
                    .and_then(repo_url)
                    .or_else(|| p.get("source").and_then(repo_url))
                    .or_else(|| market_repo.clone());
                if let Some(link) = link {
                    manifest_links.insert(format!("{}@{}", name, dir), link);
                }
            }
        }

        let src = known_map.get(&dir).and_then(|k| k.get("source"));
        let source_str = match src {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => t(lang, "scan.onDisk", &[]),
        };
        marketplaces.push(json!({
            "name": dir,
            "plugins": entries.len(),
            "source": source_str,
        }));
    }
    marketplaces.sort_by(|a, b| {
        let pa = a.get("plugins").and_then(|v| v.as_i64()).unwrap_or(0);
        let pb = b.get("plugins").and_then(|v| v.as_i64()).unwrap_or(0);
        pb.cmp(&pa)
    });

    CatalogSources {
        entries: by_id.values().cloned().collect(),
        by_id,
        marketplaces,
        notes,
        manifest_links,
    }
}

/// Plugin descriptions by id — for the inventory rows of installed plugins.
pub fn plugin_descriptions(lang: Lang) -> HashMap<String, String> {
    catalog_sources(lang)
        .by_id
        .iter()
        .filter_map(|(id, e)| {
            e.get("description")
                .and_then(|v| v.as_str())
                .filter(|d| !d.is_empty())
                .map(|d| (id.clone(), d.to_string()))
        })
        .collect()
}

/// Reverse map: MCP server → plugins that dragged it in. The only source of a
/// description for MCP: the servers themselves never have descriptions.
pub fn mcp_owners(lang: Lang) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for e in &catalog_sources(lang).entries {
        let id = e
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(servers) = e.get("bundledMcp").and_then(|v| v.as_array()) {
            for s in servers {
                if let Some(name) = s.as_str() {
                    out.entry(name.to_string()).or_default().push(id.clone());
                }
            }
        }
    }
    out
}

pub fn manifest_links(lang: Lang) -> HashMap<String, String> {
    catalog_sources(lang).manifest_links.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-catalog-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn manifest_spots_are_checked_in_order_and_none_is_none() {
        let dir = tmp("spots");
        assert!(marketplace_manifest_file(&dir).is_none());
        fs::create_dir_all(dir.join(".github/plugin")).unwrap();
        fs::write(dir.join(".github/plugin/marketplace.json"), "{}").unwrap();
        assert_eq!(
            marketplace_manifest_file(&dir),
            Some(dir.join(".github/plugin/marketplace.json")),
            "the only existing spot wins"
        );
        fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
        fs::write(dir.join(".claude-plugin/marketplace.json"), "{}").unwrap();
        assert_eq!(
            marketplace_manifest_file(&dir),
            Some(dir.join(".claude-plugin/marketplace.json")),
            "the canonical spot is checked first"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_entries_prefer_the_richer_shape_and_opus_tokens() {
        let cache = json!({
            "catalog": {
                "plugins": {
                    "tools@example": {
                        "version": "2.0",
                        "unique_installs": 1200,
                        "components": { "mcpServers": [ "github" ] },
                        "tokens": {
                            "claude-sonnet": { "always_on": 100, "on_invoke": 50.5 },
                            "claude-opus": { "always_on": 300, "on_invoke": 25 },
                        },
                        "marketplace_entry": {
                            "name": "Tools",
                            "description": "  does things  ",
                            "category": "dev",
                            "components": {
                                "skills": [ { "name": "s1" }, { "name": "" }, { "noName": true } ],
                            },
                        },
                    },
                    "bare@other": { "plugin": "BareName" },
                },
            },
        });
        let entries = parse_catalog_cache(&cache, Lang::En);
        assert_eq!(entries.len(), 2);
        let tools = entries.iter().find(|e| e["id"] == "tools@example").unwrap();
        assert_eq!(tools["name"], "Tools", "marketplace_entry.name first");
        assert_eq!(tools["description"], "does things", "trimmed");
        assert_eq!(tools["marketplace"], "example");
        assert_eq!(tools["installs"], 1200);
        assert_eq!(
            tools["bundledSkills"],
            json!(["s1"]),
            "empty names are skipped"
        );
        assert_eq!(tools["bundledMcp"], json!(["github"]));
        assert_eq!(tools["cost"]["level"], "estimated");
        assert_eq!(
            tools["cost"]["tokens"], 300,
            "opus is preferred over the first model"
        );
        assert!(
            tools["cost"]["note"].as_str().unwrap().contains("25"),
            "on_invoke rides in the note"
        );

        let bare = entries.iter().find(|e| e["id"] == "bare@other").unwrap();
        assert_eq!(
            bare["name"], "BareName",
            "plugin field is the second source"
        );
        assert_eq!(
            bare["cost"]["level"], "unknown",
            "no tokens — honestly unknown"
        );
    }

    /// TS parity: a record with no name at all is dropped; a blank-but-present
    /// name survives (JS truthiness), and every record's cost stays honestly
    /// unknown.
    #[test]
    fn manifest_records_without_a_name_are_dropped() {
        let manifest = json!({
            "plugins": [
                { "name": "good", "description": "desc" },
                { "noName": true },
                { "name": "  " },
            ],
        });
        let entries = parse_marketplace_manifest(&manifest, "mkt", Lang::En);
        assert_eq!(
            entries.len(),
            2,
            "the missing name is dropped, the blank one stays"
        );
        assert!(entries.iter().any(|e| e["id"] == "good@mkt"));
        assert_eq!(entries[0]["cost"]["level"], "unknown");
    }
}
