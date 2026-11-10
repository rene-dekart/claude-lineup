//! A JSON memo on disk for network answers, one file per source. Two callers:
//! the plugin index (whose data updates daily) and the MCP registry (whose
//! search takes 9–15 s per request, so every re-open of the tab pays it again).
//! Port of `src/server/remote/cache.ts`.
//!
//! A failure is never written: `load` errs and the entry stays absent, so the
//! next ask tries the network again instead of caching an outage for a day.

use crate::jsonutil;
use crate::lang::Lang;
use crate::stats::iso_now;
use serde_json::{Map, Value, json};
use std::path::Path;

/// An entry older than the TTL, or with an unparseable date, is not fresh.
pub fn fresh(at: &str, now_ms: i64, ttl_ms: u64) -> bool {
    let t = match chrono::DateTime::parse_from_rfc3339(at) {
        Ok(d) => d.timestamp_millis(),
        Err(_) => return false,
    };
    now_ms - t >= 0 && (now_ms - t) < ttl_ms as i64
}

/// `{q: 'x', page: 2}` → `q=x&page=2`. Empty parts are dropped, not spelled as `q=`.
pub fn cache_key(parts: &[(&str, Option<&str>)]) -> String {
    parts
        .iter()
        .filter_map(|(k, v)| v.filter(|s| !s.is_empty()).map(|v| format!("{}={}", k, v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn read_cache(file: &Path, lang: Lang) -> Map<String, Value> {
    jsonutil::read_json(file, lang)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// Disk memo around one network load. Blocking on purpose — every caller here
/// is a Tauri command that can afford a thread.
pub fn memo<F>(file: &Path, ttl_ms: u64, key: &str, lang: Lang, load: F) -> Result<Value, String>
where
    F: FnOnce() -> Result<Value, String>,
{
    let now = crate::stats::now_ms();
    let cache = read_cache(file, lang);
    if let Some(hit) = cache.get(key) {
        let at = hit["at"].as_str().unwrap_or("");
        if fresh(at, now, ttl_ms) {
            return Ok(hit["value"].clone());
        }
    }
    let value = load()?;
    let mut all = read_cache(file, lang);
    all.insert(
        key.to_string(),
        json!({ "at": iso_now(), "value": value.clone() }),
    );
    // The TS original writes JSON.stringify(all, null, 2) — no trailing newline.
    if let Ok(text) = serde_json::to_string_pretty(&Value::Object(all)) {
        let _ = jsonutil::write_atomic(file, &text);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-netcache-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn freshness_window() {
        let now = crate::stats::now_ms();
        let at = chrono::DateTime::from_timestamp_millis(now - 1000)
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert!(fresh(&at, now, 60_000));
        assert!(!fresh(&at, now + 61_000, 60_000));
        assert!(!fresh("not a date", now, 60_000));
        // A clock that has not reached the entry yet: not fresh, not negative.
        assert!(!fresh(&at, now - 2000, 60_000));
    }

    #[test]
    fn key_drops_empty_parts() {
        assert_eq!(
            cache_key(&[
                ("q", Some("rust")),
                ("type", Some("")),
                ("limit", Some("30"))
            ]),
            "q=rust&limit=30"
        );
        assert_eq!(cache_key(&[("q", Some(""))]), "");
    }

    #[test]
    fn memo_stores_success_and_replays_it() {
        let dir = tmp("memo");
        let file = dir.join("cache.json");
        let v1 = memo(&file, 60_000, "q=rust", Lang::Ru, || Ok(json!({ "n": 1 }))).unwrap();
        let v2 = memo(&file, 60_000, "q=rust", Lang::Ru, || Ok(json!({ "n": 2 }))).unwrap();
        assert_eq!(v1["n"], 1);
        assert_eq!(v2["n"], 1, "the fresh entry wins over a second load");
    }

    #[test]
    fn memo_never_caches_a_failure() {
        let dir = tmp("memo-fail");
        let file = dir.join("cache.json");
        let r: Result<Value, String> = memo(&file, 60_000, "q=x", Lang::Ru, || Err("down".into()));
        assert!(r.is_err());
        assert!(!file.exists(), "an outage is not written");
    }
}
