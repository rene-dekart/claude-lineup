//! Tolerant JSON helpers — the port of `src/server/util/json.ts`.
//! A corrupted foreign file must not crash the app: the caller decides.

use crate::lang::{Lang, t};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

pub type Obj = Map<String, Value>;

pub fn read_text(file: &Path) -> Option<String> {
    fs::read_to_string(file).ok()
}

pub fn read_json(file: &Path, lang: Lang) -> Result<Value, String> {
    let raw = fs::read_to_string(file)
        .map_err(|e| t(lang, "io.readError", &[("err", &e.to_string())]))?;
    serde_json::from_str(&raw).map_err(|e| t(lang, "io.badJson", &[("err", &e.to_string())]))
}

pub fn exists(p: &Path) -> bool {
    p.exists()
}

pub fn list_dir(p: &Path) -> Vec<String> {
    fs::read_dir(p)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Deep merge: objects merge, arrays and scalars are REPLACED.
/// (JSON never carries `undefined`, so the TS `v === undefined` skip has no port.)
pub fn deep_merge(base: &Value, patch: &Value) -> Value {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            let mut out = b.clone();
            for (k, v) in p {
                let merged = match out.get(k) {
                    Some(old) => deep_merge(old, v),
                    None => v.clone(),
                };
                out.insert(k.clone(), merged);
            }
            Value::Object(out)
        }
        _ => patch.clone(),
    }
}

pub fn write_atomic(file: &Path, content: &str) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(file, content)
}

/// Two-space JSON plus a trailing newline — the shape the web version writes.
pub fn to_pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).expect("Value is serializable");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-jsonutil-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn broken_and_missing_files_are_errors_not_panics() {
        let dir = tmp("errs");
        let broken = dir.join("broken.json");
        fs::write(&broken, "{ nope").unwrap();
        assert!(read_json(&broken, Lang::En).is_err());
        assert!(read_json(&dir.join("absent.json"), Lang::Ru).is_err());
        let ok = dir.join("ok.json");
        fs::write(&ok, r#"{"a":1}"#).unwrap();
        assert_eq!(read_json(&ok, Lang::Ru).unwrap()["a"], 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn deep_merge_objects_merge_arrays_and_scalars_are_replaced() {
        let base = json!({
            "a": 1,
            "list": [1, 2, 3],
            "nested": { "x": true, "y": "old" },
        });
        let patch = json!({
            "b": 2,
            "list": ["only"],
            "nested": { "y": "new", "z": 3 },
        });
        let out = deep_merge(&base, &patch);
        assert_eq!(out["a"], 1);
        assert_eq!(out["b"], 2);
        assert_eq!(out["list"], json!(["only"]));
        assert_eq!(out["nested"]["x"], true);
        assert_eq!(out["nested"]["y"], "new");
        assert_eq!(out["nested"]["z"], 3);
        // The base is untouched — a merge must not mutate its input.
        assert_eq!(base["list"], json!([1, 2, 3]));
    }

    #[test]
    fn write_atomic_creates_parents_and_to_pretty_ends_with_newline() {
        let dir = tmp("write");
        let file = dir.join("deep/inner/f.json");
        write_atomic(&file, "{}\n").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "{}\n");
        assert!(to_pretty(&json!({"a":1})).ends_with('\n'));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_dir_of_missing_dir_is_empty_not_a_crash() {
        assert!(list_dir(Path::new("Z:/definitely/not/here")).is_empty());
    }
}
