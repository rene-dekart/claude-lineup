//! The statistics panel: the session history off the disk (`~/.claude/projects`)
//! plus what the savings tools report about themselves. Port of
//! `src/server/stats/` — index, history, transcripts, caveman, tools.
//!
//! Every assistant line of a transcript carries the API's own `usage`, so the
//! money question is answered from counts, not from an estimate. Per-file
//! aggregates are cached against the file's size and mtime: the cache is
//! derived data — losing it costs one slow scan, never a wrong number.

use crate::config;
use crate::cost;
use crate::jsonutil;
use crate::lang::{Lang, t};
use crate::paths;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

// ─────────────────────────── history.ts ───────────────────────────

/// Claude Code's default, and the number the page quotes when the setting is absent.
pub const DEFAULT_RETENTION_DAYS: f64 = 30.0;

/// A day count from an untrusted place — the settings file, or a request body.
/// Anything that is not a positive number is not a rule: a string, a zero or a
/// negative would otherwise mean "delete everything".
pub fn read_retention(value: Option<&Value>) -> f64 {
    let n = match value {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    match n {
        Some(n) if n.is_finite() && n > 0.0 => n.floor(),
        _ => DEFAULT_RETENTION_DAYS,
    }
}

pub fn retention_days_at(settings: &Path, lang: Lang) -> f64 {
    match jsonutil::read_json(settings, lang) {
        Ok(v) => read_retention(v.get("cleanupPeriodDays")),
        Err(_) => DEFAULT_RETENTION_DAYS,
    }
}

struct OnDisk {
    path: PathBuf,
    size: u64,
    mtime_ms: i64,
}

fn mtime_ms(st: &std::fs::Metadata) -> i64 {
    st.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Every .jsonl under the root, with the two numbers a retention rule needs.
fn stat_transcripts(root: &Path) -> Vec<OnDisk> {
    let mut out = Vec::new();
    walk_stat(root, &mut out);
    out
}

fn walk_stat(dir: &Path, out: &mut Vec<OnDisk>) {
    // Unreadable is not fatal: the rest of the pile is still countable.
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for e in entries.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(ft) if ft.is_dir() => walk_stat(&p, out),
            Ok(ft) if ft.is_file() && p.extension().map(|x| x == "jsonl").unwrap_or(false) => {
                if let Ok(st) = p.metadata() {
                    out.push(OnDisk {
                        path: p,
                        size: st.len(),
                        mtime_ms: mtime_ms(&st),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Retention days as JSON: 30, not 30.0 — the TS original carries a plain number.
fn days_json(days: f64) -> Value {
    if days.fract() == 0.0 {
        json!(days as i64)
    } else {
        json!(days)
    }
}

/// The pile, and the part of it past the rule. `now_ms` is a parameter so a
/// test can place itself on either side of the boundary without touching the clock.
pub fn history_size_at(root: &Path, days: f64, now_ms: i64) -> Value {
    let cutoff = now_ms - (days * 86_400_000.0) as i64;
    let files = stat_transcripts(root);
    let mut bytes = 0u64;
    let mut oldest = 0i64;
    let mut old_files = 0u32;
    let mut old_bytes = 0u64;
    for f in &files {
        bytes += f.size;
        if oldest == 0 || f.mtime_ms < oldest {
            oldest = f.mtime_ms;
        }
        // mtime, the same clock Claude Code goes by: a session resumed yesterday
        // is not old, however long ago it started.
        if f.mtime_ms < cutoff {
            old_files += 1;
            old_bytes += f.size;
        }
    }
    json!({
        "files": files.len(),
        "bytes": bytes,
        "oldest": oldest,
        "oldFiles": old_files,
        "oldBytes": old_bytes,
        "retentionDays": days_json(days),
    })
}

/// The same rule, applied now. Files only: chasing an emptied project directory
/// would mean racing a session being written into it this second.
pub fn clean_transcripts_at(root: &Path, days: f64, now_ms: i64) -> Value {
    let cutoff = now_ms - (days * 86_400_000.0) as i64;
    let mut deleted = 0u32;
    let mut bytes = 0u64;
    let mut failed = 0u32;
    for f in stat_transcripts(root) {
        if f.mtime_ms >= cutoff {
            continue;
        }
        match std::fs::remove_file(&f.path) {
            Ok(_) => {
                deleted += 1;
                bytes += f.size;
            }
            Err(_) => failed += 1,
        }
    }
    json!({ "deleted": deleted, "bytes": bytes, "failed": failed })
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `new Date().toISOString()` — milliseconds and `Z`.
pub fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

// ─────────────────────────── transcripts.ts ───────────────────────────

/// Turns farther apart than this are separate sittings, not one long one.
pub const IDLE_GAP_MS: i64 = 30 * 60_000;

const MEASURE_CACHE_VERSION: f64 = 5.0;

fn empty_tokens() -> Value {
    json!({ "input": 0, "output": 0, "cacheRead": 0, "cacheCreate": 0, "thinking": 0 })
}

fn add_tokens(into: &mut Value, from: &Value) {
    for k in ["input", "output", "cacheRead", "cacheCreate", "thinking"] {
        into[k] = json!(into[k].as_i64().unwrap_or(0) + from[k].as_i64().unwrap_or(0));
    }
}

fn line_tokens(usage: &Value) -> Value {
    let num = |v: Value| v.as_i64().unwrap_or(0);
    json!({
        "input": num(usage["input_tokens"].clone()),
        "output": num(usage["output_tokens"].clone()),
        "cacheRead": num(usage["cache_read_input_tokens"].clone()),
        "cacheCreate": num(usage["cache_creation_input_tokens"].clone()),
        "thinking": num(usage["output_tokens_details"]["thinking_tokens"].clone()),
    })
}

/// One assistant line → what it called. `mcp__<server>__<tool>` keeps the server
/// and drops the tool; a `Skill` block keeps the name it was invoked by.
fn count_usage(u: &mut Value, content: &Value) {
    let arr = match content.as_array() {
        Some(a) => a,
        None => return,
    };
    for c in arr {
        if c["type"] != "tool_use" {
            continue;
        }
        let name = match c["name"].as_str() {
            Some(n) => n,
            None => continue,
        };
        if let Some(rest) = name.strip_prefix("mcp__") {
            // Split on the LAST `__`: server names carry underscores of their own.
            let server = match rest.rfind("__") {
                Some(i) if i > 0 => &rest[..i],
                _ => rest,
            };
            let m = u["mcp"].as_object_mut().unwrap();
            bump(m, server);
        } else if name == "Skill"
            && let Some(skill) = c["input"]["skill"].as_str().filter(|s| !s.is_empty())
        {
            let m = u["skill"].as_object_mut().unwrap();
            bump(m, skill);
        }
    }
}

fn bump(m: &mut Map<String, Value>, key: &str) {
    let e = m.entry(key.to_string()).or_insert(json!(0));
    *e = json!(e.as_i64().unwrap_or(0) + 1);
}

fn new_session(id: &str, caveman: Option<&str>) -> Value {
    json!({
        "id": id,
        "cwd": "",
        "start": 0,
        "end": 0,
        "longestMs": 0,
        "messages": 0,
        "models": {},
        "tokens": empty_tokens(),
        "usage": { "mcp": {}, "skill": {} },
        "caveman": caveman,
    })
}

/// The level one transcript ran under, or None when the hook never fired in it.
/// The TS original is a regex, `/CAVEMAN MODE ACTIVE[\s\S]{0,40}?level:\s*([a-z]+)/`;
/// a hand-rolled search over the same window keeps the zero-dependency parse.
pub fn caveman_level(text: &str) -> Option<String> {
    let marker = "CAVEMAN MODE ACTIVE";
    let mut from = 0;
    while let Some(i) = text[from..].find(marker) {
        let start = from + i + marker.len();
        let window = &text[start..(start + 46).min(text.len())];
        if let Some(li) = window.find("level:") {
            let rest = window[li + "level:".len()..].trim_start();
            let level: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase())
                .collect();
            if !level.is_empty() {
                return Some(level);
            }
        }
        from = start;
    }
    None
}

/// One transcript file → its sessions. The `includes('"usage"')` prefilter is
/// what makes this affordable: the vast majority of lines never reach a parse.
/// Sidechain (subagent) turns are kept: they are really billed.
fn read_transcript(file: &Path) -> Result<(Map<String, Value>, u32), String> {
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let fallback_id = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let caveman = caveman_level(&text);
    let mut sessions: Map<String, Value> = Map::new();
    // Where the previous turn of each session left off, and how long the stretch
    // running up to it has been. Per file: a session's turns arrive in file order.
    let mut runs: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    let mut bad_lines = 0u32;

    for line in text.split('\n') {
        if !line.contains("\"usage\"") {
            continue;
        }
        let o: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                // The last line of a session being written right now is often
                // truncated. Counted, not swallowed: a partial file is a partial figure.
                bad_lines += 1;
                continue;
            }
        };
        let usage = &o["message"]["usage"];
        if usage.is_null() {
            continue;
        }

        let id = o["sessionId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(&fallback_id)
            .to_string();
        let agg = sessions
            .entry(id.clone())
            .or_insert_with(|| new_session(&id, caveman.as_deref()));
        if let Some(cwd) = o["cwd"].as_str().filter(|s| !s.is_empty())
            && agg["cwd"].as_str().unwrap_or("").is_empty()
        {
            agg["cwd"] = json!(cwd);
        }

        // Transcripts carry RFC 3339 timestamps only; anything else is no timestamp.
        let ts = o["timestamp"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.timestamp_millis());
        if let Some(ts) = ts {
            if agg["start"].as_i64().unwrap_or(0) == 0 || ts < agg["start"].as_i64().unwrap_or(0) {
                agg["start"] = json!(ts);
            }
            if ts > agg["end"].as_i64().unwrap_or(0) {
                agg["end"] = json!(ts);
            }

            let r = runs.entry(id.clone()).or_insert((0, 0));
            if r.0 != 0 {
                let d = ts - r.0;
                // A gap inside the threshold extends the stretch; anything else —
                // a long pause, or a timestamp that went backwards — starts a new one.
                r.1 = if d > 0 && d <= IDLE_GAP_MS {
                    r.1 + d
                } else {
                    0
                };
                if r.1 > agg["longestMs"].as_i64().unwrap_or(0) {
                    agg["longestMs"] = json!(r.1);
                }
            }
            r.0 = ts;
        }

        agg["messages"] = json!(agg["messages"].as_i64().unwrap_or(0) + 1);
        let billed = line_tokens(usage);
        {
            let mut t = agg["tokens"].take();
            add_tokens(&mut t, &billed);
            agg["tokens"] = t;
        }
        count_usage(&mut agg["usage"], &o["message"]["content"]);
        // Each call names the model that answered it, so the line's whole usage
        // belongs to that model.
        if let Some(model) = o["message"]["model"].as_str() {
            let models = agg["models"].as_object_mut().unwrap();
            let entry = models.entry(model.to_string()).or_insert_with(empty_tokens);
            add_tokens(entry, &billed);
        }
    }

    Ok((sessions, bad_lines))
}

/// The cache is ignored whenever it is unreadable, of the wrong version, or
/// shaped like something else — one bad entry must not cost the other three hundred.
fn load_cache(file: &Path, lang: Lang) -> Map<String, Value> {
    let mut files = Map::new();
    if let Ok(v) = jsonutil::read_json(file, lang)
        && v["version"].as_f64() == Some(MEASURE_CACHE_VERSION)
        && let Some(fs) = v["files"].as_object()
    {
        for (key, entry) in fs {
            let ok = entry["size"].is_number()
                && entry["mtimeMs"].is_number()
                && entry["sessions"].is_object();
            if ok {
                files.insert(key.clone(), entry.clone());
            }
        }
    }
    files
}

fn file_fingerprint(p: &Path) -> Option<(u64, i64)> {
    let st = p.metadata().ok()?;
    Some((st.len(), mtime_ms(&st)))
}

/// Walks the transcripts and returns their per-session aggregates, plus the
/// notes for what could not be read.
pub fn scan_transcripts_at(
    root: &Path,
    cache_file: &Path,
    lang: Lang,
) -> (Vec<Value>, Vec<String>) {
    let cache = load_cache(cache_file, lang);
    let found = transcript_files(root);
    let mut next: Map<String, Value> = Map::new();
    let mut out: Vec<Value> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut read = 0u32;

    for file in &found {
        let (size, mtime) = match file_fingerprint(file) {
            Some(x) => x,
            None => {
                notes.push(t(
                    lang,
                    "io.readError",
                    &[("err", &format!("{}: stat failed", file.display()))],
                ));
                continue;
            }
        };
        let hit = cache.get(file.to_string_lossy().as_ref());
        if let Some(hit) = hit {
            let same =
                hit["size"].as_u64() == Some(size) && hit["mtimeMs"].as_f64() == Some(mtime as f64);
            if same {
                next.insert(file.to_string_lossy().into_owned(), hit.clone());
                out.push(json!({ "path": file.to_string_lossy(), "sessions": sessions_values(&hit["sessions"]) }));
                continue;
            }
        }

        match read_transcript(file) {
            Ok((sessions, bad)) => {
                if bad > 0 {
                    notes.push(t(
                        lang,
                        "io.badJson",
                        &[("err", &format!("{} ({})", file.display(), bad))],
                    ));
                }
                read += 1;
                let mut entry = Map::new();
                entry.insert("size".into(), json!(size));
                entry.insert("mtimeMs".into(), json!(mtime));
                entry.insert("sessions".into(), Value::Object(sessions));
                let entry = Value::Object(entry);
                out.push(json!({ "path": file.to_string_lossy(), "sessions": sessions_values(&entry["sessions"]) }));
                next.insert(file.to_string_lossy().into_owned(), entry);
            }
            Err(e) => {
                // Nothing is cached for a file that could not be read: it must be
                // retried on the next scan, or a transient lock would look empty.
                notes.push(t(
                    lang,
                    "io.readError",
                    &[("err", &format!("{}: {}", file.display(), e))],
                ));
            }
        }
    }

    let dirty = read > 0 || !notes.is_empty() || cache.len() != found.len();
    if dirty {
        let all = json!({ "version": MEASURE_CACHE_VERSION, "files": Value::Object(next) });
        // The TS original writes JSON.stringify(x, null, 2) — no trailing newline.
        if let Ok(text) = serde_json::to_string_pretty(&all) {
            let _ = jsonutil::write_atomic(cache_file, &text);
        }
    }
    (out, notes)
}

fn sessions_values(sessions: &Value) -> Vec<Value> {
    sessions
        .as_object()
        .map(|m| m.values().cloned().collect())
        .unwrap_or_default()
}

/// Every .jsonl under the root, recursively, sorted — the tree nests deeper
/// than one level. Sorted as the TS original sorts the full path STRINGS:
/// a component-wise PathBuf sort would order `A\a.jsonl` against `A-b\a.jsonl`
/// differently (the separator byte beats the `-` byte only in string compare),
/// and the file order decides which keys a merged session sees first.
fn transcript_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_transcripts(root, &mut out);
    out.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    out
}

fn walk_transcripts(dir: &Path, out: &mut Vec<PathBuf>) {
    // A project directory that is gone or unreadable is not a scan failure.
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for e in entries.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(ft) if ft.is_dir() => walk_transcripts(&p, out),
            Ok(ft) if ft.is_file() && p.extension().map(|x| x == "jsonl").unwrap_or(false) => {
                out.push(p)
            }
            _ => {}
        }
    }
}

/// Folds one file's session into another's: a resumed session writes a second file.
fn merge_into(into: &mut Value, from: &Value) {
    into["messages"] =
        json!(into["messages"].as_i64().unwrap_or(0) + from["messages"].as_i64().unwrap_or(0));
    {
        let mut t = into["tokens"].take();
        add_tokens(&mut t, &from["tokens"]);
        into["tokens"] = t;
    }
    if let Some(models) = from["models"].as_object() {
        let into_models = into["models"].as_object_mut().unwrap();
        for (m, tk) in models {
            let entry = into_models.entry(m.clone()).or_insert_with(empty_tokens);
            add_tokens(entry, tk);
        }
    }
    if let Some(usage) = from["usage"].as_object() {
        let into_usage = into["usage"].as_object_mut().unwrap();
        for (kind, names) in usage {
            if let Some(names) = names.as_object() {
                let m = match into_usage.get_mut(kind).and_then(|v| v.as_object_mut()) {
                    Some(m) => m,
                    None => continue,
                };
                for (name, n) in names {
                    let e = m.entry(name.clone()).or_insert(json!(0));
                    *e = json!(e.as_i64().unwrap_or(0) + n.as_i64().unwrap_or(0));
                }
            }
        }
    }
    let from_cwd = from["cwd"].as_str().unwrap_or("");
    if !from_cwd.is_empty() && into["cwd"].as_str().unwrap_or("").is_empty() {
        into["cwd"] = json!(from_cwd);
    }
    let from_cv = from["caveman"].as_str().unwrap_or("");
    let into_cv = into["caveman"].as_str().unwrap_or("");
    if !from_cv.is_empty() && into_cv.is_empty() {
        into["caveman"] = json!(from_cv);
    }
    let fs = from["start"].as_i64().unwrap_or(0);
    if fs != 0 {
        let is = into["start"].as_i64().unwrap_or(0);
        if is == 0 || fs < is {
            into["start"] = json!(fs);
        }
    }
    let fe = from["end"].as_i64().unwrap_or(0);
    if fe > into["end"].as_i64().unwrap_or(0) {
        into["end"] = json!(fe);
    }
    into["longestMs"] =
        json!(into["longestMs"].as_i64().unwrap_or(0) + from["longestMs"].as_i64().unwrap_or(0));
}

/// caveman's saving, counted from the transcripts. The arithmetic is caveman's
/// own: it calls its output 65 % lighter than unwrapped, i.e.
/// saved = written / (1 - 0.65) - written. Stays an estimate — theirs.
pub fn caveman_saving(sessions: &[Value]) -> Value {
    let ratio_for = |level: &str| if level == "full" { Some(0.65f64) } else { None };
    let mut would_have = 0i64;
    let mut written = 0i64;
    let mut counted = 0u32;
    for s in sessions {
        let level = s["caveman"].as_str().unwrap_or("");
        let ratio = if level.is_empty() {
            None
        } else {
            ratio_for(level)
        };
        let output = s["tokens"]["output"].as_i64().unwrap_or(0);
        let (ratio, output) = match (ratio, output) {
            (Some(r), o) if o != 0 => (r, o),
            _ => continue,
        };
        would_have += ((output as f64) / (1.0 - ratio)).round() as i64;
        written += output;
        counted += 1;
    }
    // No marked session is not a saving of zero: there is nothing to answer with.
    if counted == 0 {
        return json!({ "id": "caveman" });
    }
    json!({ "id": "caveman", "saved": would_have - written, "total": would_have })
}

/// Per-session aggregates → the `Stats` the UI draws. Sessions are merged by id
/// first: a resumed session leaves turns in more than one file, and summing per
/// file would count the same session twice.
pub fn aggregate_stats(
    files: &[Value],
    notes: Vec<String>,
    savings: Vec<Value>,
    scanned_at: String,
) -> Value {
    let mut merged: Map<String, Value> = Map::new();
    for file in files {
        for s in file["sessions"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or(&[])
        {
            let id = s["id"].as_str().unwrap_or("").to_string();
            match merged.get_mut(&id) {
                // A copy, not the cache's own object: the merge below writes into it.
                None => {
                    merged.insert(id, s.clone());
                }
                Some(prev) => merge_into(prev, s),
            }
        }
    }

    let mut tokens = empty_tokens();
    let mut sessions: Vec<Value> = Vec::new();
    let mut all: Vec<Value> = Vec::new();
    for s in merged.values_mut() {
        all.push(s.clone());
        {
            let mut t = tokens.take();
            add_tokens(&mut t, &s["tokens"]);
            tokens = t;
        }
        // A model that never produced a token is not a model: Claude Code writes
        // its own `<synthetic>` marker into the same field. Deleted after the
        // merges, exactly where the TS original deletes it.
        if let Some(models) = s["models"].as_object_mut() {
            let dead: Vec<String> = models
                .iter()
                .filter(|(_, tk)| tk["output"].as_i64() == Some(0))
                .map(|(m, _)| m.clone())
                .collect();
            // shift_remove: the TS `delete` keeps insertion order.
            for d in dead {
                models.shift_remove(&d);
            }
        }
        sessions.push(json!({
            "id": s["id"],
            "project": s["cwd"],
            "start": s["start"],
            "end": s["end"],
            "messages": s["messages"],
            "models": s["models"],
            "tokens": s["tokens"],
            "usage": s["usage"],
            "longestMs": s["longestMs"],
        }));
    }
    sessions.sort_by_key(|s| s["start"].as_i64().unwrap_or(0));

    let mut savings = savings;
    savings.push(caveman_saving(&all));
    json!({
        "sessions": sessions,
        "savings": savings,
        "notes": notes,
        "scannedAt": scanned_at,
    })
}

// ─────────────────────────── tools.ts ───────────────────────────

/// `rtk gain` reads a local log — ten seconds is already far past generous.
pub const RTK_TIMEOUT_MS: u64 = 10_000;
/// The stats view asks again on every open; the log does not move that fast.
pub const RTK_TTL_MS: u64 = 60_000;

static RTK_MEMO: Mutex<Option<(Instant, Value)>> = Mutex::new(None);

struct RunResult {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// A command line with a hard timeout: spawn, wait, kill on expiry. The shape
/// of `spawn` with `shell: true` on Windows: the whole line goes to cmd.exe.
fn run_with_timeout(line: &str, timeout_ms: u64, lang: Lang) -> RunResult {
    let mut child = match std::process::Command::new("cmd")
        .args(["/C", line])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return RunResult {
                code: None,
                stdout: String::new(),
                stderr: t(
                    lang,
                    "net.binNotStarted",
                    &[("bin", line), ("reason", &e.to_string())],
                ),
            };
        }
    };
    let deadline = Instant::now() + std::time::Duration::from_millis(timeout_ms);
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
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return RunResult {
                        code: None,
                        stdout: String::new(),
                        stderr: t(
                            lang,
                            "net.timeout",
                            &[("seconds", &(timeout_ms / 1000).to_string())],
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

fn read_rtk(lang: Lang) -> Value {
    let line = format!("{} gain -f json", config::rtk_bin());
    let r = run_with_timeout(&line, RTK_TIMEOUT_MS, lang);
    if r.code != Some(0) {
        // A real non-zero exit leaves the tool's own words in stderr.
        return json!({ "id": "rtk", "note": if r.stderr.trim().is_empty() { format!("exit {}", r.code.map(|c| c.to_string()).unwrap_or_else(|| "null".into())) } else { r.stderr.trim().to_string() } });
    }
    let parsed: Result<Value, _> = serde_json::from_str(&r.stdout);
    let summary = match parsed {
        Ok(v) => v["summary"].clone(),
        Err(e) => {
            return json!({ "id": "rtk", "note": t(lang, "io.badJson", &[("err", &e.to_string())]) });
        }
    };
    let calls = summary["total_commands"].as_i64().unwrap_or(0);
    if calls == 0 {
        return json!({ "id": "rtk", "note": t(lang, "stats.note.rtkIdle", &[]) });
    }
    json!({
        "id": "rtk",
        "saved": summary["total_saved"].as_i64().unwrap_or(0),
        // What would have gone in without the proxy: the scale the saving is read against.
        "total": summary["total_input"].as_i64().unwrap_or(0),
        "calls": calls,
    })
}

fn rtk_saving(lang: Lang) -> Value {
    let mut memo = RTK_MEMO.lock().unwrap();
    if let Some((at, value)) = memo.as_ref()
        && (at.elapsed().as_millis() as u64) < RTK_TTL_MS
    {
        return value.clone();
    }
    let value = read_rtk(lang);
    *memo = Some((Instant::now(), value.clone()));
    value
}

/// context-mode's own SQLite stores. One database at a time inside a catch: a
/// store being written to right now must not take down the scan of the others.
fn context_mode_saving(lang: Lang) -> Value {
    use rusqlite::OpenFlags;
    let dir = paths::context_mode_dir().join("sessions");
    let mut dbs: Vec<PathBuf> = jsonutil::list_dir(&dir)
        .into_iter()
        .filter(|n| n.ends_with(".db"))
        .map(|n| dir.join(n))
        .collect();
    dbs.sort();

    let tilde = |p: &Path| paths::shorten(&p.to_string_lossy(), "");
    if dbs.is_empty() {
        let where_ = tilde(&dir);
        return if jsonutil::exists(&dir) {
            json!({ "id": "context-mode", "note": t(lang, "stats.note.noDbs", &[("path", &where_)]) })
        } else {
            json!({ "id": "context-mode", "note": t(lang, "stats.note.noDir", &[("path", &where_)]) })
        };
    }

    let mut avoided = 0i64;
    let mut returned = 0i64;
    let mut events = 0i64;
    for db in &dbs {
        let conn = match rusqlite::Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        {
            Ok(c) => c,
            Err(_) => continue,
        };
        let res = conn.query_row(
            "SELECT COUNT(*) AS c, COALESCE(SUM(bytes_avoided), 0) AS a, COALESCE(SUM(bytes_returned), 0) AS r FROM session_events",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
        );
        if let Ok((c, a, r)) = res {
            events += c;
            avoided += a;
            returned += r;
        }
    }

    if events == 0 {
        return json!({ "id": "context-mode", "note": t(lang, "stats.note.noEvents", &[("n", &dbs.len().to_string())]) });
    }
    json!({
        "id": "context-mode",
        "saved": cost::tokens_from_chars(avoided as usize),
        // Bytes it kept out plus bytes it let through — the total it acted on.
        "total": cost::tokens_from_chars((avoided + returned) as usize),
        "calls": events,
    })
}

/// The two that report on themselves, in the fixed order the UI reads them.
/// caveman is counted from the transcripts instead — see `caveman_saving`.
pub fn scan_savings(lang: Lang) -> Vec<Value> {
    vec![rtk_saving(lang), context_mode_saving(lang)]
}

/// The statistics panel, in one call. The two halves are independent — a missing
/// rtk binary must not cost the user their token history.
pub fn build_stats_at(root: &Path, settings: &Path, cache_file: &Path, lang: Lang) -> Value {
    let (files, notes) = scan_transcripts_at(root, cache_file, lang);
    let savings = scan_savings(lang);
    // The pile, read once more for its sizes: the session walk keeps no bytes.
    let history = history_size_at(root, retention_days_at(settings, lang), now_ms());
    let mut stats = aggregate_stats(&files, notes, savings, iso_now());
    stats["history"] = history;
    stats
}

pub fn build_stats(lang: Lang) -> Value {
    build_stats_at(
        &paths::projects_dir(),
        &paths::user_settings(),
        &paths::stats_cache(),
        lang,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-stats-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, s: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }

    fn line(ts: &str, session: &str, model: &str, out: i64, extra: &str) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","sessionId":"{session}","cwd":"D:/proj","message":{{"model":"{model}","usage":{{"input_tokens":10,"output_tokens":{out},"cache_read_input_tokens":5,"cache_creation_input_tokens":2,"output_tokens_details":{{"thinking_tokens":3}}}}{extra}}}}}"#
        )
    }

    #[test]
    fn retention_from_untrusted_places() {
        assert_eq!(read_retention(Some(&json!(7.9))), 7.0);
        assert_eq!(read_retention(Some(&json!("12"))), 12.0);
        assert_eq!(read_retention(Some(&json!(0))), DEFAULT_RETENTION_DAYS);
        assert_eq!(read_retention(Some(&json!(-3))), DEFAULT_RETENTION_DAYS);
        assert_eq!(read_retention(Some(&json!("many"))), DEFAULT_RETENTION_DAYS);
        assert_eq!(read_retention(None), DEFAULT_RETENTION_DAYS);
    }

    #[test]
    fn transcript_sessions_and_usage() {
        let root = tmp("agg");
        let f = root.join("a.jsonl");
        let mut text = String::new();
        text.push_str(&line("2026-01-01T10:00:00.000Z", "s1", "m1", 100, ""));
        text.push('\n');
        text.push_str(r#"{"type":"user","timestamp":"2026-01-01T10:00:10.000Z"}"#);
        text.push('\n');
        text.push_str(&line(
            "2026-01-01T10:00:20.000Z",
            "s1",
            "m1",
            50,
            r#","content":[{"type":"tool_use","name":"mcp__plugin_context-mode_context-mode__ctx_search","input":{}},{"type":"tool_use","name":"Skill","input":{"skill":"superpowers:brainstorming"}}]"#,
        ));
        text.push('\n');
        // A synthetic zero-output marker must vanish from the models map.
        text.push_str(&line(
            "2026-01-01T10:00:30.000Z",
            "s1",
            "<synthetic>",
            0,
            "",
        ));
        text.push('\n');
        write(&f, &text);

        let cache = root.join("cache.json");
        let (files, notes) = scan_transcripts_at(&root, &cache, Lang::Ru);
        assert!(notes.is_empty(), "notes: {notes:?}");
        let stats = aggregate_stats(
            &files,
            vec![],
            vec![],
            "2026-01-01T00:00:00.000Z".to_string(),
        );
        let sessions = stats["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s["id"], "s1");
        assert_eq!(s["messages"], 3);
        assert_eq!(s["tokens"]["output"], 150);
        assert_eq!(s["tokens"]["thinking"], 9);
        // The marker model produced nothing: not a model.
        assert!(s["models"].get("<synthetic>").is_none());
        assert_eq!(s["models"]["m1"]["output"], 150);
        // Split on the LAST `__`: the server name survives its own underscores.
        assert_eq!(s["usage"]["mcp"]["plugin_context-mode_context-mode"], 1);
        assert_eq!(s["usage"]["skill"]["superpowers:brainstorming"], 1);
        // 10:00:00 → 10:00:20 → 10:00:30: one continuous stretch of 30 s,
        // both gaps inside the idle threshold.
        assert_eq!(s["longestMs"], 30_000);

        // Second scan is a cache hit: same sessions, no dirty write cost.
        let (files2, notes2) = scan_transcripts_at(&root, &cache, Lang::Ru);
        assert!(notes2.is_empty());
        let stats2 = aggregate_stats(
            &files2,
            vec![],
            vec![],
            "2026-01-01T00:00:00.000Z".to_string(),
        );
        assert_eq!(stats2["sessions"][0]["tokens"]["output"], 150);
    }

    #[test]
    fn broken_last_line_counted_not_swallowed() {
        let root = tmp("broken");
        let mut text = line("2026-01-01T10:00:00.000Z", "s1", "m1", 10, "");
        text.push('\n');
        text.push_str("{\"type\":\"assistant\",\"message\":{\"usage\":");
        write(&root.join("b.jsonl"), &text);
        let (_, notes) = scan_transcripts_at(&root, &root.join("c.json"), Lang::Ru);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("b.jsonl (1)"));
    }

    #[test]
    fn resumed_session_merges_across_files() {
        let root = tmp("merge");
        write(
            &root.join("d.jsonl"),
            &format!("{}\n", line("2026-01-01T10:00:00.000Z", "s1", "m1", 10, "")),
        );
        write(
            &root.join("e.jsonl"),
            &format!("{}\n", line("2026-01-01T11:00:00.000Z", "s1", "m1", 20, "")),
        );
        let (files, _) = scan_transcripts_at(&root, &root.join("c.json"), Lang::Ru);
        let stats = aggregate_stats(&files, vec![], vec![], "x".to_string());
        let s = &stats["sessions"].as_array().unwrap()[0];
        assert_eq!(s["messages"], 2);
        assert_eq!(s["tokens"]["output"], 30);
        assert_eq!(
            s["start"],
            "2026-01-01T10:00:00.000Z"
                .parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap()
                .timestamp_millis()
        );
    }

    #[test]
    fn caveman_saving_uses_their_formula() {
        let sessions = vec![
            json!({ "caveman": "full", "tokens": { "output": 6500 } }),
            json!({ "caveman": null, "tokens": { "output": 100 } }),
            json!({ "caveman": "lite", "tokens": { "output": 100 } }),
        ];
        let v = caveman_saving(&sessions);
        // saved = 6500/0.35 - 6500 = 18571 - 6500
        assert_eq!(v["saved"], ((6500f64 / 0.35).round() as i64) - 6500);
        assert_eq!(v["total"], (6500f64 / 0.35).round() as i64);
        // No marked session: nothing to answer with.
        assert_eq!(
            caveman_saving(&[json!({ "caveman": null, "tokens": { "output": 5 } })])["id"],
            "caveman"
        );
    }

    #[test]
    fn caveman_level_reads_the_hook_marker() {
        let text = "<system-reminder>SessionStart:compact hook success: CAVEMAN MODE ACTIVE — level: full\n\nRespond terse.</system-reminder>";
        assert_eq!(caveman_level(text).as_deref(), Some("full"));
        assert_eq!(caveman_level("no marker here"), None);
        assert_eq!(
            caveman_level("CAVEMAN MODE ACTIVE — level: wenyan-lite").as_deref(),
            Some("wenyan")
        );
    }

    #[test]
    fn history_counts_the_pile_by_mtime() {
        let root = tmp("history");
        write(&root.join("p/a.jsonl"), "x");
        write(&root.join("p/q/b.jsonl"), "yy");
        let now = now_ms();
        let v = history_size_at(&root, 30.0, now);
        assert_eq!(v["files"], 2);
        assert_eq!(v["bytes"], 3);
        assert_eq!(v["oldFiles"], 0);
        // Everything is ancient when the clock is moved a month ahead.
        let v = history_size_at(&root, 30.0, now + 31 * 86_400_000);
        assert_eq!(v["oldFiles"], 2);
        assert_eq!(v["oldBytes"], 3);
    }

    #[test]
    fn clean_removes_only_past_the_rule() {
        let root = tmp("clean");
        write(&root.join("a.jsonl"), "old");
        write(&root.join("b.jsonl"), "new");
        let now = now_ms();
        // No mtime is in the future here; both files are recent, nothing dies.
        let v = clean_transcripts_at(&root, 30.0, now);
        assert_eq!(v["deleted"], 0);
        assert_eq!(v["failed"], 0);
        // A month later both are past the rule.
        let v = clean_transcripts_at(&root, 30.0, now + 31 * 86_400_000);
        assert_eq!(v["deleted"], 2);
        assert_eq!(v["bytes"], 6);
        assert!(!root.join("a.jsonl").exists());
    }
}
