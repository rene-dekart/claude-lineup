//! The port of `src/server/cost/estimate.ts`: honesty levels and the SKILL.md
//! frontmatter parser. A fake digit is worse than a dash, so every cost leaves
//! this module carrying its level.

use crate::jsonutil::Obj;
use crate::lang::{Lang, t};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub fn tokens_from_chars(chars: usize) -> i64 {
    (chars as f64 / 4.0).round() as i64
}

/// A tool definition costs about 2 chars per token, not 4 — measured against
/// Claude Code's own /context report; the old /4 counted icons and envelope.
pub fn tokens_from_tool_json(chars: usize) -> i64 {
    (chars as f64 / 2.0).round() as i64
}

pub fn measured_tool_json(chars: usize, note: &str) -> Value {
    json!({ "level": "measured", "tokens": tokens_from_tool_json(chars), "note": note, "source": "probe" })
}

pub fn estimated_chars(chars: usize, note: &str) -> Value {
    json!({ "level": "estimated", "tokens": tokens_from_chars(chars), "note": note, "source": "size" })
}

pub fn unknown(note: &str) -> Value {
    json!({ "level": "unknown", "note": note })
}

/// The session's own figure — Claude Code counted it while running.
pub fn from_session(tokens: i64, note: &str) -> Value {
    json!({ "level": "measured", "tokens": tokens, "note": note, "source": "session" })
}

pub fn file_size(file: &Path) -> usize {
    fs::metadata(file).map(|m| m.len() as usize).unwrap_or(0)
}

/// Sum of a list: unknown when nothing is known, measured when any part is.
pub fn sum_costs(costs: &[Value], lang: Lang) -> Value {
    let known: Vec<&Value> = costs.iter().filter(|c| c.get("tokens").is_some()).collect();
    if known.is_empty() {
        return unknown(&t(
            lang,
            "cost.unknownCount",
            &[("n", &costs.len().to_string())],
        ));
    }
    let tokens: i64 = known
        .iter()
        .filter_map(|c| c.get("tokens").and_then(|v| v.as_i64()))
        .sum();
    let level = if known
        .iter()
        .any(|c| c.get("level").and_then(|v| v.as_str()) == Some("measured"))
    {
        "measured"
    } else {
        "estimated"
    };
    json!({ "level": level, "tokens": tokens, "note": t(lang, "cost.sum", &[("n", &known.len().to_string())]) })
}

/// `description: >` and `description: |` fold a block body; without unfolding,
/// the context would get the marker character instead of the text.
fn fold_block(lines: &[&str], start: usize, marker: &str) -> (String, usize) {
    let mut body: Vec<&str> = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let line = lines[i].trim_end_matches('\r');
        if line.trim().is_empty() {
            body.push("");
            i += 1;
            continue;
        }
        // Continuation is indentation only. A line without indentation starts a new key.
        if !line.starts_with([' ', '\t']) {
            break;
        }
        body.push(line);
        i += 1;
    }
    let indented: Vec<&str> = body
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .collect();
    let indent = indented
        .iter()
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut dedented: Vec<String> = body
        .iter()
        .map(|l| l.chars().skip(indent).collect())
        .collect();
    while dedented.last().is_some_and(|l| l.is_empty()) {
        dedented.pop();
    }
    let sep = if marker.starts_with('>') { " " } else { "\n" };
    (dedented.join(sep), i)
}

/// `^[|>][-+]?$` — block style, no regex crate for one test.
fn is_block_marker(raw: &str) -> bool {
    let mut cs = raw.chars();
    match cs.next() {
        Some('|') | Some('>') => match cs.next() {
            None => true,
            Some('-') | Some('+') => cs.next().is_none(),
            _ => false,
        },
        _ => false,
    }
}

fn strip_quotes(raw: &str) -> String {
    let b = raw.trim_start_matches(['"', '\'']);
    b.trim_end_matches(['"', '\'']).to_string()
}

fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// SKILL.md frontmatter. `None` when there is no block — then the description
/// never reaches context and there is nothing to count.
pub fn parse_frontmatter(text: &str) -> Option<Obj> {
    if !text.starts_with("---") {
        return None;
    }
    let end = text.find("\n---")?;
    let lines: Vec<&str> = text[3..end].split('\n').collect();
    let mut out = Obj::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end_matches('\r');
        let colon = line.find(':');
        let (key, raw) = match colon {
            Some(c) if line[..c].chars().all(is_key_char) && !line[..c].is_empty() => {
                let k = line[..c].to_string();
                let v = line[c + 1..].trim().to_string();
                (k, v)
            }
            _ => {
                i += 1;
                continue;
            }
        };
        if is_block_marker(&raw) {
            let (folded, next) = fold_block(&lines, i + 1, &raw);
            out.insert(key, Value::String(folded));
            i = next;
        } else {
            out.insert(key, Value::String(strip_quotes(&raw)));
            i += 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_block_style_folds() {
        let text = "---\nname: x\ndescription: >\n  first line\n  second line\n---\nbody";
        let fm = parse_frontmatter(text).expect("frontmatter parses");
        assert_eq!(
            fm.get("description").unwrap().as_str(),
            Some("first line second line")
        );
        assert_eq!(fm.get("name").unwrap().as_str(), Some("x"));
    }

    #[test]
    fn frontmatter_preserve_style_keeps_breaks() {
        let text = "---\ndescription: |\n  one\n  two\n---";
        let fm = parse_frontmatter(text).unwrap();
        assert_eq!(fm.get("description").unwrap().as_str(), Some("one\ntwo"));
    }

    #[test]
    fn frontmatter_strips_quotes_and_absent_is_none() {
        let fm = parse_frontmatter("---\nd: \"quoted\"\n---");
        assert_eq!(fm.unwrap().get("d").unwrap().as_str(), Some("quoted"));
        assert!(parse_frontmatter("no frontmatter here").is_none());
    }
}
