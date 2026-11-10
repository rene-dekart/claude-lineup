//! Server-side `t`: the language comes in with every command, not ambient.
//! The web version carried it in `x-lang` + AsyncLocalStorage; here the UI puts
//! `lang` into each invoke's arguments.

use serde_json::Value;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Ru,
    En,
}

pub const DEFAULT_LANG: Lang = Lang::Ru;

static MESSAGES: OnceLock<Value> = OnceLock::new();
const MESSAGES_JSON: &str = include_str!("../messages.json");

fn messages() -> &'static Value {
    MESSAGES.get_or_init(|| {
        serde_json::from_str(MESSAGES_JSON).expect("embedded messages.json is valid")
    })
}

/// `ru`, `ru-RU`, `RU` → `ru`; anything unrecognised → the default.
pub fn parse_lang(raw: Option<&str>) -> Lang {
    match raw.map(|s| s.trim().to_lowercase()) {
        Some(s) if s.starts_with("en") => Lang::En,
        _ => DEFAULT_LANG,
    }
}

fn fill(template: &str, params: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in params {
        out = out.replace(&format!("{{{}}}", k), v);
    }
    out
}

/// Unknown key renders as the key — a visible `sess.foo` is the error message,
/// same rule as the web version.
pub fn t(lang: Lang, key: &str, params: &[(&str, &str)]) -> String {
    let dicts = messages();
    let dict = match lang {
        Lang::Ru => dicts.get("ru"),
        Lang::En => dicts.get("en"),
    };
    let template = dict
        .and_then(|d| d.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or(key);
    fill(template, params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeSet;

    #[test]
    fn lang_parsing_is_prefix_based_and_defaults_to_ru() {
        assert_eq!(parse_lang(Some("en"),), Lang::En);
        assert_eq!(parse_lang(Some("en-US")), Lang::En);
        assert_eq!(parse_lang(Some(" EN ")), Lang::En);
        assert_eq!(parse_lang(Some("ru")), Lang::Ru);
        assert_eq!(parse_lang(Some("garbage")), Lang::Ru);
        assert_eq!(parse_lang(None), Lang::Ru);
    }

    #[test]
    fn placeholders_fill_and_unknown_key_renders_as_itself() {
        let s = t(Lang::Ru, "io.readError", &[("err", "boom")]);
        assert_eq!(s, "не читается: boom");
        let s = t(Lang::En, "io.readError", &[("err", "boom")]);
        assert_eq!(s, "cannot read: boom");
        assert_eq!(t(Lang::Ru, "no.such.key", &[]), "no.such.key");
    }

    fn placeholders(template: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut rest = template;
        while let Some(open) = rest.find('{') {
            let Some(close) = rest[open..].find('}') else {
                break;
            };
            out.insert(rest[open + 1..open + close].to_string());
            rest = &rest[open + close + 1..];
        }
        out
    }

    /// The port of the TS i18n test: same keys in both languages, same
    /// placeholders per key — a `{date}` in ru without one in en is a
    /// runtime `no.such.key`-shaped hole in one locale.
    #[test]
    fn ru_and_en_dicts_carry_the_same_keys_and_placeholders() {
        let dicts = messages();
        let keys = |dict: Option<&Value>| -> BTreeSet<String> {
            dict.and_then(|d| d.as_object())
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default()
        };
        let ru = keys(dicts.get("ru"));
        let en = keys(dicts.get("en"));
        assert!(!ru.is_empty(), "the ru dict must not be empty");
        assert_eq!(ru, en, "both locales must carry the same key set");
        let ph_of = |lang: Lang, k: &str| -> BTreeSet<String> {
            placeholders(
                dicts
                    .get(match lang {
                        Lang::Ru => "ru",
                        Lang::En => "en",
                    })
                    .unwrap()
                    .get(k)
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
        };
        for k in &ru {
            assert_eq!(ph_of(Lang::Ru, k), ph_of(Lang::En, k), "key {k}");
        }
    }

    /// Every catalog row the scan builds must resolve in both languages —
    /// a missing key would render as `perm.files.code.name` in the UI.
    #[test]
    fn every_permission_catalog_key_exists_in_both_dicts() {
        for group in ["files", "repo", "system", "data"] {
            for (slug, _) in slugs_of(group) {
                for field in ["name", "why"] {
                    let key = format!("perm.{group}.{slug}.{field}");
                    for lang in [Lang::Ru, Lang::En] {
                        let s = t(lang, &key, &[]);
                        assert_ne!(s, key, "missing {key} for {lang:?}");
                    }
                }
            }
        }
    }

    /// (slug, one rule) pairs — mirrors the catalog constants in scan.rs.
    fn slugs_of(group: &str) -> Vec<(&'static str, &'static str)> {
        match group {
            "files" => vec![
                ("code", "Read(./src/**)"),
                ("tests", "Read(./tests/**)"),
                ("depsScripts", "Edit(./package.json)"),
                ("ci", "Edit(./.github/workflows/**)"),
                ("dockerImage", "Edit(./Dockerfile)"),
            ],
            "repo" => vec![
                ("status", "Bash(git status:*)"),
                ("diff", "Bash(git diff:*)"),
                ("log", "Bash(git log:*)"),
                ("commit", "Bash(git commit:*)"),
                ("push", "Bash(git push:*)"),
                ("rebase", "Bash(git rebase:*)"),
                ("resetHard", "Bash(git reset --hard:*)"),
                ("publish", "Bash(npm publish:*)"),
                ("deploy", "Bash(npm run deploy:*)"),
            ],
            "system" => vec![
                ("test", "Bash(npm test:*)"),
                ("build", "Bash(npm run build:*)"),
                ("typecheck", "Bash(npx tsc:*)"),
                ("install", "Bash(npm install:*)"),
                ("pipInstall", "Bash(pip install:*)"),
                ("curl", "Bash(curl:*)"),
                ("wget", "Bash(wget:*)"),
                ("rm", "Bash(rm:*)"),
                ("rmRf", "Bash(rm -rf:*)"),
                ("sudo", "Bash(sudo:*)"),
            ],
            _ => vec![
                ("secrets", "Read(./.env)"),
                ("envFiles", "Read(./.env.*)"),
                ("secretsDir", "Read(./secrets/**)"),
                ("sshKeys", "Read(~/.ssh/**)"),
            ],
        }
    }
}
