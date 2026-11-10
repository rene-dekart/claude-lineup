//! Claude Code's own `/context` report, read off the disk — the port of
//! `src/server/scan/contextReport.ts`.
//!
//! The harness counts the tools it really sent, so its numbers beat anything
//! derived from a file size. The report rides in the transcript as a service
//! message; the last one that actually parses wins.

use crate::jsonutil::list_dir;
use crate::paths;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;

/// Project transcripts live in a directory named after the cwd, non-alnum → '-'.
pub fn transcript_dir(cwd: &str) -> std::path::PathBuf {
    let name: String = cwd
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    paths::claude_dir().join("projects").join(name)
}

/// "1.9k" → 1900, "1m" → 1_000_000, "302" → 302. No number — `None`.
fn num(raw: &str) -> Option<i64> {
    let t = raw.trim();
    let (body, suffix) = match t.chars().last() {
        Some('k') | Some('K') => (&t[..t.len() - 1], 1000i64),
        Some('m') | Some('M') => (&t[..t.len() - 1], 1_000_000i64),
        _ => (t, 1i64),
    };
    let body = body.trim();
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let v: f64 = body.parse().ok()?;
    if v.is_finite() {
        Some((v * suffix as f64).round() as i64)
    } else {
        None
    }
}

fn cells(line: &str) -> Vec<&str> {
    line.split('|').map(|s| s.trim()).collect()
}

/// One side of the `X / Y` on the **Tokens:** line: leading junk skipped, then
/// digits/dots, optional spaces, optional k/m — the loose mid-line match the
/// TS regex makes.
fn side_pair(s: &str) -> Option<i64> {
    let s = s.trim();
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let rest = &s[start..];
    let digits_end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    if digits_end == 0 {
        return None;
    }
    let body = &rest[..digits_end];
    let after = rest[digits_end..].trim();
    let v: f64 = body.parse().ok()?;
    let v = match after.chars().next() {
        Some('k') | Some('K') => v * 1000.0,
        Some('m') | Some('M') => v * 1_000_000.0,
        _ => v,
    };
    if v.is_finite() {
        Some(v.round() as i64)
    } else {
        None
    }
}

fn cells_cell<'a>(c: &[&'a str], i: usize) -> &'a str {
    c.get(i).copied().unwrap_or("")
}

fn is_separator_row(name: &str) -> bool {
    name.starts_with('-') && name.chars().all(|c| c == '-')
}

/// Reads the report out of transcript message text. `None` — the human never
/// ran `/context` in that session, and inventing one would be a lie about a
/// measurement.
pub fn parse_context_report(text: &str, at: &str, file: &str) -> Option<Value> {
    if !text.contains("## Context Usage") {
        return None;
    }
    let mut categories: Map<String, Value> = Map::new();
    let mut mcp: Map<String, Value> = Map::new();
    let mut memory: Map<String, Value> = Map::new();
    let mut total: Option<i64> = None;
    let mut window: Option<i64> = None;
    let mut section = String::new();

    for line in text.split('\n') {
        if line.starts_with("###") {
            section = line.trim_start_matches('#').trim().to_string();
            continue;
        }
        if line.starts_with("**Tokens:**") {
            // The same line carries the window; a bar needs a denominator.
            let t = line.trim_start_matches("**Tokens:**").trim();
            if let Some(slash) = t.find('/')
                && let (Some(a), Some(b)) = (side_pair(&t[..slash]), side_pair(&t[slash + 1..]))
            {
                total = Some(a);
                window = Some(b);
            }
            continue;
        }
        if !line.starts_with('|') {
            continue;
        }
        let c = cells(line);
        if c.len() < 3 {
            continue;
        }
        // Column layout differs per table — cells() keeps the leading empty one,
        // so the n-th column sits at index n.
        if section == "Estimated usage by category" {
            if let Some(value) = num(cells_cell(&c, 2)) {
                let name = cells_cell(&c, 1);
                if !name.is_empty() && !is_separator_row(name) {
                    categories.insert(name.to_string(), json!(value));
                }
            }
        } else if section == "MCP Tools" {
            if let Some(value) = num(cells_cell(&c, 3)) {
                let server = cells_cell(&c, 2);
                if !server.is_empty() {
                    let prev = mcp.get(server).and_then(|v| v.as_i64()).unwrap_or(0);
                    mcp.insert(server.to_string(), json!(prev + value));
                }
            }
        } else if section == "Memory Files"
            && let Some(value) = num(cells_cell(&c, 3))
        {
            let p = cells_cell(&c, 2);
            if p.contains('\\') || p.contains('/') {
                memory.insert(p.to_string(), json!(value));
            }
        }
    }

    if categories.is_empty() {
        return None;
    }
    // The three layers the profile cannot choose. Everything else is configurable.
    let base: i64 = ["System prompt", "System tools", "Custom agents"]
        .iter()
        .map(|k| categories.get(*k).and_then(|v| v.as_i64()).unwrap_or(0))
        .sum();

    Some(json!({
        "at": at,
        "file": file,
        "total": total,
        "window": window,
        "base": base,
        "categories": Value::Object(categories),
        "mcp": Value::Object(mcp),
        "memory": Value::Object(memory),
    }))
}

/// The project's transcripts, newest first.
fn transcripts_newest_first(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out: Vec<(std::path::PathBuf, u64)> = Vec::new();
    for name in list_dir(dir) {
        if !name.ends_with(".jsonl") {
            continue;
        }
        let f = dir.join(&name);
        if let Ok(md) = fs::metadata(&f)
            && let Ok(modified) = md.modified()
            && let Ok(ms) = modified.duration_since(std::time::UNIX_EPOCH)
        {
            out.push((f, ms.as_millis() as u64));
        }
    }
    out.sort_by_key(|x| std::cmp::Reverse(x.1));
    out.into_iter().map(|(f, _)| f).collect()
}

fn message_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<&str>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Every `/context` report in one transcript; the last real one wins. Later
/// lines may merely *mention* the phrase — only a line that parses counts.
///
/// ponytail: no memo of parsed transcripts (`file|size:mtime` keys in the web
/// version) — the desktop asks for the inventory on demand; add the memo when
/// a slow open of the history is measurably annoying.
pub fn scan_file(file: &Path) -> Option<Value> {
    let raw = fs::read_to_string(file).ok()?;
    let mut best: Option<Value> = None;
    for line in raw.split('\n') {
        // Cheap filter before parsing: the report is one huge JSON line among
        // tens of thousands.
        if !line.contains("## Context Usage") {
            continue;
        }
        let Ok(o) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let empty = Value::String(String::new());
        let text = message_text(
            o.get("message")
                .unwrap_or(&empty)
                .get("content")
                .unwrap_or(&empty),
        );
        if text.is_empty() {
            continue;
        }
        let at = o
            .get("timestamp")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let file_str = file.to_string_lossy().into_owned();
        if let Some(r) = parse_context_report(&text, &at, &file_str) {
            best = Some(r);
        }
    }
    best
}

/// The last report of the project: the newest session that actually ran
/// `/context` — not simply the newest session.
pub fn read_context_report(cwd: &str) -> Option<Value> {
    for file in transcripts_newest_first(&transcript_dir(cwd)) {
        if let Some(r) = scan_file(&file) {
            return Some(r);
        }
    }
    None
}

/// Windows paths: separators and case are not part of a file's identity.
fn same_path(a: &str, b: &str) -> bool {
    a.replace('/', "\\").to_lowercase() == b.replace('/', "\\").to_lowercase()
}

/// The session's figure for a memory file, if it read the same one. Matched
/// loosely (case, separators) because the report prints paths verbatim.
pub fn report_memory_tokens(report: Option<&Value>, file: &str) -> Option<(String, i64)> {
    let mem = report?.get("memory")?.as_object()?;
    for (key, tokens) in mem {
        if same_path(key, file) {
            return Some((key.clone(), tokens.as_i64().unwrap_or(0)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_scales() {
        assert_eq!(num("1.9k"), Some(1900));
        assert_eq!(num("1m"), Some(1_000_000));
        assert_eq!(num("302"), Some(302));
        assert_eq!(num("k"), None);
        assert_eq!(num(""), None);
    }

    #[test]
    fn parses_a_report() {
        let text = "## Context Usage\n**Tokens:** `89.5k / 200k`\n\
            ### Estimated usage by category\n| |name|tokens|\n|---|---|---|\n|System prompt|1.2k|\n|System tools|10k|\n\
            ### MCP Tools\n| | |server|tokens|\n| |serv|2.5k|\n\
            ### Memory Files\n| | |path|tokens|\n| |C:\\Users\\x\\CLAUDE.md|900|\n";
        let r =
            parse_context_report(text, "2026-11-09T00:00:00Z", "f.jsonl").expect("report parses");
        assert_eq!(r.get("total"), Some(&json!(89500)));
        assert_eq!(r.get("window"), Some(&json!(200000)));
        assert_eq!(r.get("base"), Some(&json!(11200)));
        let mcp = r.get("mcp").unwrap().as_object().unwrap();
        assert_eq!(mcp.get("serv").and_then(|v| v.as_i64()), Some(2500));
        let mem = r.get("memory").unwrap().as_object().unwrap();
        assert!(mem.contains_key("C:\\Users\\x\\CLAUDE.md"));
    }

    #[test]
    fn no_report_no_invention() {
        assert!(parse_context_report("plain text", "", "f").is_none());
    }

    #[test]
    fn memory_tokens_match_loosely() {
        let report = json!({ "memory": { "C:/Users/x/CLAUDE.md": 900 } });
        let hit = report_memory_tokens(Some(&report), "c:\\Users\\X\\claude.md");
        assert!(hit.is_some());
        assert!(report_memory_tokens(Some(&report), "elsewhere").is_none());
    }
}
