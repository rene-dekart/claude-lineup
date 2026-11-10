//! The detail card: what a skill actually tells the model, what a plugin drags
//! in, which plugin a bare MCP server came from — and, for a server that is
//! only in the registry, its readme off GitHub. Port of `src/server/detail.ts`
//! and `links.ts` (the describe/Ollama half is not ported — it was removed).
//!
//! The card is the only place that reads a file the inventory did not already
//! read, so `allowed` below is the one security-relevant line in this module:
//! only `.md` files under the config directories we ourselves scan.

use crate::catalog;
use crate::cost;
use crate::jsonutil;
use crate::lang::{Lang, t};
use crate::mcp;
use crate::mcpreg;
use crate::paths;
use crate::plugins::{self, PluginInstall};
use crate::probe;
use crate::repourl;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// Frontmatter is for the model's trigger, not for reading. The body is the skill.
pub fn strip_frontmatter(text: &str) -> String {
    if !text.starts_with("---") {
        return text.trim().to_string();
    }
    let end = match text[3..].find("\n---") {
        Some(i) => 3 + i,
        None => return text.trim().to_string(),
    };
    // The fence after `end` is "\n---"; the body starts after its own newline.
    let after = match text.get(end + 4..) {
        Some(a) => a,
        None => return text.trim().to_string(),
    };
    match after.find('\n') {
        Some(nl) => text[end + 4 + nl + 1..].trim().to_string(),
        // The TS original slices from 0 when no newline follows the fence.
        None => text.trim().to_string(),
    }
}

/// Only `.md` under the directories we ourselves scan — the path check is the
/// whole security story of the card.
fn allowed(file: &str, cwd: &str) -> bool {
    if !file.to_lowercase().ends_with(".md") {
        return false;
    }
    let full = paths::norm(Path::new(file));
    let mut roots = vec![paths::claude_dir(), PathBuf::from(cwd)];
    roots.extend(paths::plugin_roots());
    roots.iter().any(|r| {
        let r = paths::norm(r);
        !r.is_empty() && full.starts_with(&format!("{}\\", r))
    })
}

fn read_body(file: &Path) -> Option<String> {
    std::fs::read_to_string(file)
        .ok()
        .map(|s| strip_frontmatter(&s))
}

/// The plugin a file belongs to, by where it sits on disk.
pub fn owner_of_file(file: &Path, lang: Lang) -> Option<String> {
    let full = paths::norm(file);
    plugins::read_installs(lang)
        .into_iter()
        .find(|i| {
            let root = paths::norm(&i.install_path);
            !root.is_empty() && full.starts_with(&format!("{}\\", root))
        })
        .map(|i| i.id)
}

fn link_from_plugin_json(install_path: &Path, lang: Lang) -> Option<String> {
    for rel in ["plugin.json", ".claude-plugin/plugin.json"] {
        if let Ok(v) = jsonutil::read_json(&install_path.join(rel), lang) {
            let url =
                repourl::repo_url(&v["repository"]).or_else(|| repourl::repo_url(&v["homepage"]));
            if url.is_some() {
                return url;
            }
        }
    }
    None
}

/// GitHub links for what is already on the machine — nothing from the catalog
/// site. The chain per plugin, first hit wins: the installed plugin's own
/// manifest, then the marketplace entry that carries its `homepage`.
pub fn plugin_link(id: &str, lang: Lang) -> Option<String> {
    if let Some(install) = plugins::read_installs(lang)
        .into_iter()
        .find(|i| i.id == id)
        && let Some(own) = link_from_plugin_json(&install.install_path, lang)
    {
        return Some(own);
    }
    catalog::manifest_links(lang).get(id).cloned()
}

/// A bundled server inherits the plugin that ships it. A standalone one has
/// only what its own command line names — `serena` names its repo there.
/// Ownership in the catalog is claimed by bare name; only an installed plugin
/// owns anything here, or the link would point at a namesake's repo.
pub fn mcp_link(name: &str, cwd: &Path, lang: Lang) -> Option<String> {
    let installed: Vec<String> = plugins::read_installs(lang)
        .iter()
        .map(|i| i.id.clone())
        .collect();
    let owners = catalog::mcp_owners(lang)
        .get(name)
        .cloned()
        .unwrap_or_default();
    for id in owners {
        if !installed.contains(&id) {
            continue;
        }
        if let Some(url) = plugin_link(&id, lang) {
            return Some(url);
        }
    }
    let defs = mcp::mcp_defs(cwd, &paths::claude_json(), &[name.to_string()], lang);
    let def = defs.get(name)?;
    if let Some(url) = repourl::repo_url(&def["url"]) {
        return Some(url);
    }
    if let Some(args) = def["args"].as_array() {
        for a in args {
            if let Some(url) = repourl::repo_url(a) {
                return Some(url);
            }
        }
    }
    None
}

fn from_catalog(e: &Value, kind: &str) -> Value {
    let mut card = Map::new();
    card.insert("kind".into(), json!(kind));
    card.insert("title".into(), e["id"].clone());
    let desc = e["description"].as_str().unwrap_or("");
    if !desc.is_empty() {
        card.insert("description".into(), json!(desc));
    }
    card.insert("marketplace".into(), e["marketplace"].clone());
    if let Some(c) = e["category"].as_str().filter(|s| !s.is_empty()) {
        card.insert("category".into(), json!(c));
    }
    if let Some(v) = e["version"].as_str().filter(|s| !s.is_empty()) {
        card.insert("version".into(), json!(v));
    }
    if let Some(n) = e["installs"].as_i64() {
        card.insert("installs".into(), json!(n));
    }
    card.insert("bundledSkills".into(), e["bundledSkills"].clone());
    card.insert("bundledMcp".into(), e["bundledMcp"].clone());
    card.insert("owners".into(), json!([]));
    card.insert("cost".into(), e["cost"].clone());
    card.insert("notes".into(), json!([]));
    Value::Object(card)
}

fn empty_tail(kind: &str, title: &str) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("kind".into(), json!(kind));
    m.insert("title".into(), json!(title));
    m.insert("bundledSkills".into(), json!([]));
    m.insert("bundledMcp".into(), json!([]));
    m.insert("owners".into(), json!([]));
    m.insert("notes".into(), json!([]));
    m
}

/// Everything the detail card shows, in one call: kind in, card out.
// The parameter list mirrors the invoke body's fields one to one — packing
// them into a struct would just move the arity into the struct.
#[allow(clippy::too_many_arguments)]
pub fn build_card(
    kind: &str,
    name: &str,
    cwd: &str,
    file: Option<&str>,
    scope: Option<&str>,
    origin: Option<&str>,
    repo: Option<&str>,
    description: Option<&str>,
    version: Option<&str>,
    transport: Option<&str>,
    lang: Lang,
) -> Value {
    let notes: Vec<String> = Vec::new();

    // A row from a registry — either one. Nothing of it is on disk: its readme
    // is the answer to "what is this", and the slug is checked once here — it
    // becomes both the link and the fetch.
    if let Some(repo) = repo.filter(|r| !r.is_empty()) {
        let clean = mcpreg::valid_repo(&json!(repo));
        let readme = clean
            .as_ref()
            .and_then(|r| mcpreg::fetch_readme(&json!(r), lang));
        let had_clean = clean.is_some();
        let had_readme = readme.is_some();
        let mut card = Map::new();
        card.insert("kind".into(), json!(kind));
        card.insert("title".into(), json!(name));
        if let Some(d) = description.filter(|s| !s.is_empty()) {
            card.insert("description".into(), json!(d));
        }
        if let Some(b) = readme {
            card.insert("body".into(), json!(b));
        }
        if let Some(r) = clean {
            card.insert("link".into(), json!(format!("https://github.com/{}", r)));
        }
        if let Some(v) = version.filter(|s| !s.is_empty()) {
            card.insert("version".into(), json!(v));
        }
        if let Some(tr) = transport.filter(|s| !s.is_empty()) {
            card.insert("transport".into(), json!(tr));
        }
        card.insert("bundledSkills".into(), json!([]));
        card.insert("bundledMcp".into(), json!([]));
        card.insert("owners".into(), json!([]));
        let card_notes = if had_clean && !had_readme {
            json!([t(lang, "card.readmeMissing", &[])])
        } else {
            json!([])
        };
        card.insert("notes".into(), card_notes);
        return Value::Object(card);
    }

    if kind == "mcp" {
        // MCP servers carry no description of their own: the only source is a
        // plugin that bundles them, and the catalog knows which those are.
        let owners = catalog::mcp_owners(lang)
            .get(name)
            .cloned()
            .unwrap_or_default();
        let by_id = catalog::catalog_by_id(lang);
        let desc = owners.iter().find_map(|id| {
            by_id
                .get(id)
                .and_then(|e| e["description"].as_str())
                .filter(|d| !d.is_empty())
        });
        let bundled: Vec<String> = owners
            .iter()
            .filter_map(|id| by_id.get(id))
            .flat_map(|e| {
                e["bundledMcp"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
            })
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        // Keep first occurrences, like `[...new Set(bundled)]`.
        let mut bundled_dedup: Vec<String> = Vec::new();
        for b in bundled {
            if !bundled_dedup.contains(&b) {
                bundled_dedup.push(b);
            }
        }
        let link = mcp_link(name, Path::new(cwd), lang);
        // Tool names exist only after a live probe. An empty list means "not
        // measured", and the card says so rather than showing nothing.
        let tools = probe::probed_tools(name);
        let mut card = Map::new();
        card.insert("kind".into(), json!("mcp"));
        card.insert("title".into(), json!(name));
        if let Some(d) = desc {
            card.insert("description".into(), json!(d));
        }
        if let Some(s) = scope.filter(|s| !s.is_empty()) {
            card.insert("scope".into(), json!(s));
        }
        if let Some(l) = link {
            card.insert("link".into(), json!(l));
        }
        if !tools.is_empty() {
            card.insert("tools".into(), Value::Array(tools));
        }
        card.insert("bundledSkills".into(), json!([]));
        card.insert("bundledMcp".into(), json!(bundled_dedup));
        card.insert("owners".into(), json!(owners));
        card.insert("notes".into(), json!(notes));
        return Value::Object(card);
    }

    if kind == "plugin" {
        let link = plugin_link(name, lang);
        let e = catalog::catalog_by_id(lang).get(name).cloned();
        return match e {
            None => {
                // No catalog entry is normal: a plugin installed straight from
                // a marketplace has none. Its own manifest may still say what it is.
                let inst: Option<PluginInstall> = plugins::read_installs(lang)
                    .into_iter()
                    .find(|i| i.id == name);
                let desc = inst.and_then(|i| plugins::manifest_description(&i.install_path, lang));
                let mut card = Map::new();
                card.insert("kind".into(), json!("plugin"));
                card.insert("title".into(), json!(name));
                if let Some(d) = desc {
                    card.insert("description".into(), json!(d));
                }
                if let Some(l) = link {
                    card.insert("link".into(), json!(l));
                }
                card.insert("bundledSkills".into(), json!([]));
                card.insert("bundledMcp".into(), json!([]));
                card.insert("owners".into(), json!([]));
                card.insert("notes".into(), json!([t(lang, "card.noCatalogEntry", &[])]));
                Value::Object(card)
            }
            Some(e) => {
                let mut card = from_catalog(&e, "plugin");
                if let Some(l) = link {
                    card["link"] = json!(l);
                }
                if let Some(s) = scope.filter(|s| !s.is_empty()) {
                    card["scope"] = json!(s);
                }
                card
            }
        };
    }

    if kind == "skill" || kind == "memory" {
        let file = file.unwrap_or("");
        let mut card = empty_tail(kind, name);
        let mut notes: Vec<String> = Vec::new();
        if file.is_empty() || !allowed(file, cwd) {
            card["notes"] = json!([t(lang, "card.fileNotAllowed", &[])]);
            return Value::Object(card);
        }
        let text = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                let msg = t(lang, "card.fileUnreadable", &[("error", &e.to_string())]);
                notes.push(msg);
                String::new()
            }
        };
        let front = cost::parse_frontmatter(&text);
        // A plugin skill has no repo of its own: it has the repo of the plugin.
        let owner = if kind == "skill" {
            owner_of_file(Path::new(file), lang)
        } else {
            None
        };
        let link = owner.as_ref().and_then(|o| plugin_link(o, lang));
        if let Some(f) = &front {
            let desc = f.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let when = f.get("when_to_use").and_then(|v| v.as_str()).unwrap_or("");
            if !desc.is_empty() {
                card.insert("description".into(), json!(desc));
            } else if !when.is_empty() {
                card.insert("description".into(), json!(when));
            }
            card.insert(
                "modelInvocable".into(),
                json!(f.get("disable-model-invocation").and_then(|v| v.as_str()) != Some("true")),
            );
        }
        let body = read_body(Path::new(file));
        if let Some(b) = body.filter(|b| !b.is_empty()) {
            card.insert("body".into(), json!(b));
        }
        if let Some(l) = link {
            card.insert("link".into(), json!(l));
        }
        card.insert("file".into(), json!(file));
        if let Some(s) = scope.filter(|s| !s.is_empty()) {
            card.insert("scope".into(), json!(s));
        }
        if let Some(o) = origin.filter(|s| !s.is_empty()) {
            card.insert("origin".into(), json!(o));
        }
        if !notes.is_empty() {
            card["notes"] = json!(notes);
        }
        return Value::Object(card);
    }

    Value::Object(empty_tail(kind, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_is_stripped_whole_or_not_at_all() {
        assert_eq!(strip_frontmatter("plain body"), "plain body");
        assert_eq!(
            strip_frontmatter("---\nname: x\n---\n\nbody here"),
            "body here"
        );
        assert_eq!(strip_frontmatter("--- no closing"), "--- no closing");
        // A `---` inside the body is not a closing fence.
        let t = "---\na: 1\n---\n\nbody\n---\nstill body";
        assert_eq!(strip_frontmatter(t), "body\n---\nstill body");
    }

    #[test]
    fn allowed_checks_root_and_extension() {
        let root = std::env::temp_dir().join(format!("cl-card-{}", std::process::id()));
        let cwd = root.join("proj");
        std::fs::create_dir_all(cwd.join(".claude")).unwrap();
        let inside = cwd.join("SKILL.md");
        std::fs::write(&inside, "x").unwrap();
        assert!(allowed(&inside.to_string_lossy(), &cwd.to_string_lossy()));
        // Outside every root: refused even with the right extension.
        assert!(!allowed(
            &root.join("elsewhere/a.md").to_string_lossy(),
            &cwd.to_string_lossy()
        ));
        // Inside but not markdown.
        let js = cwd.join("x.js");
        std::fs::write(&js, "x").unwrap();
        assert!(!allowed(&js.to_string_lossy(), &cwd.to_string_lossy()));
    }

    fn card_cwd(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("cl-card-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The skill card: description straight from frontmatter, `when_to_use`
    /// as the fallback, and a file outside the config roots is not read.
    #[test]
    fn skill_card_reads_frontmatter_with_the_when_to_use_fallback() {
        let cwd = card_cwd("skill");
        let skill = cwd
            .join(".claude")
            .join("skills")
            .join("s1")
            .join("SKILL.md");
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        std::fs::write(
            &skill,
            "---\nwhen_to_use: use for demo\ndisable-model-invocation: \"true\"\n---\nBODY LINE",
        )
        .unwrap();
        let file = skill.to_string_lossy().into_owned();
        let cwd_s = cwd.to_string_lossy().into_owned();

        let card = build_card(
            "skill",
            "s1",
            &cwd_s,
            Some(&file),
            None,
            None,
            None,
            None,
            None,
            None,
            Lang::En,
        );
        assert_eq!(
            card["description"], "use for demo",
            "when_to_use is the fallback"
        );
        assert_eq!(card["modelInvocable"], false, "the disable flag was read");
        assert_eq!(card["body"], "BODY LINE");

        // A written description outranks when_to_use, and the skill is
        // model-invocable again.
        std::fs::write(
            &skill,
            "---\ndescription: the real one\nwhen_to_use: fallback\n---\nB",
        )
        .unwrap();
        let card = build_card(
            "skill",
            "s1",
            &cwd_s,
            Some(&file),
            None,
            None,
            None,
            None,
            None,
            None,
            Lang::En,
        );
        assert_eq!(card["description"], "the real one");
        assert_eq!(card["modelInvocable"], true);

        // Outside the roots the card refuses to read, with a note.
        let elsewhere = card_cwd("outside").join("evil.md");
        std::fs::write(&elsewhere, "---\ndescription: stolen\n---\n").unwrap();
        let card = build_card(
            "skill",
            "s1",
            &cwd_s,
            Some(&elsewhere.to_string_lossy()),
            None,
            None,
            None,
            None,
            None,
            None,
            Lang::En,
        );
        assert!(card["description"].is_null(), "nothing was read");
        assert!(
            card["notes"][0].as_str().unwrap().contains("outside"),
            "the refusal is said out loud"
        );
        let _ = std::fs::remove_dir_all(&cwd);
        let _ = std::fs::remove_dir_all(elsewhere.parent().unwrap());
    }
}
