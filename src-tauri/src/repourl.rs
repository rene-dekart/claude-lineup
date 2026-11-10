//! The port of `src/server/repo-url.ts`: one GitHub link out of whatever form
//! the source wrote it in. Deliberately narrow — only github.com and
//! raw.githubusercontent.com count. A link is never guessed from a package
//! name; a wrong link is worse than none.

/// `owner` must not contain these — the TS character class `[^/\s#?]`.
fn segment(s: &str) -> Option<&str> {
    let end = s
        .find(['/', '?', '#', ' ', '\t', '\n', '\r'])
        .unwrap_or(s.len());
    let seg = &s[..end];
    if seg.is_empty() { None } else { Some(seg) }
}

/// The three TS patterns, done by hand: no regex crate for three matches over
/// bounded character sets. `raw` pins a trailing `/` after the repo, and its
/// owner must follow a literal `/`; plain github.com accepts `:` too.
fn owner_repo_at(s: &str, host: &str, raw: bool) -> Option<(String, String)> {
    let mut from = 0usize;
    while let Some(pos) = s[from..].find(host) {
        let at = from + pos;
        let boundary = at == 0 || matches!(s[..at].chars().next_back(), Some('/') | Some('@'));
        let after = &s[at + host.len()..];
        let ok = if raw {
            after.starts_with('/')
        } else {
            after.starts_with(['/', ':'])
        };
        if boundary && ok {
            let rest = &after[1..];
            let owner = segment(rest)?;
            let after_owner = &rest[owner.len()..];
            if let Some(rest2) = after_owner.strip_prefix('/') {
                let repo = segment(rest2)?;
                let repo = repo.strip_suffix(".git").unwrap_or(repo);
                let tail_ok = !raw || rest2[repo.len()..].starts_with('/');
                if tail_ok {
                    return Some((owner.to_string(), repo.to_string()));
                }
            }
        }
        from = at + host.len();
    }
    None
}

pub fn repo_url(v: &serde_json::Value) -> Option<String> {
    let raw = match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(o) => match o.get("repo") {
            Some(serde_json::Value::String(repo)) => Some(format!("github:{repo}")),
            _ => o.get("url").and_then(|u| u.as_str()).map(String::from),
        },
        _ => None,
    }?;
    let s = raw.trim();
    let s = s.strip_prefix("git+").unwrap_or(s);

    let pair = owner_repo_at(s, "github.com", false)
        .or_else(|| owner_repo_at(s, "raw.githubusercontent.com", true))
        .or_else(|| {
            // `github:owner/name`
            s.strip_prefix("github:").and_then(|rest| {
                let owner = segment(rest)?;
                let rest2 = rest[owner.len()..].strip_prefix('/')?;
                let repo = segment(rest2)?;
                let repo = repo.strip_suffix(".git").unwrap_or(repo);
                Some((owner.to_string(), repo.to_string()))
            })
        })?;
    Some(format!("https://github.com/{}/{}", pair.0, pair.1))
}

#[cfg(test)]
mod tests {
    use super::repo_url;
    use serde_json::{Value, json};

    fn u(v: Value) -> Option<String> {
        repo_url(&v)
    }

    #[test]
    fn forms_the_original_handles() {
        assert_eq!(
            u(json!("https://github.com/o/p")),
            Some("https://github.com/o/p".into())
        );
        assert_eq!(
            u(json!("git+https://github.com/o/p.git")),
            Some("https://github.com/o/p".into())
        );
        assert_eq!(
            u(json!("github:o/p")),
            Some("https://github.com/o/p".into())
        );
        assert_eq!(
            u(json!({"repo": "o/p"})),
            Some("https://github.com/o/p".into())
        );
        assert_eq!(
            u(json!("https://raw.githubusercontent.com/o/p/main/x")),
            Some("https://github.com/o/p".into())
        );
        assert_eq!(
            u(json!("https://github.com/o/p/tree/main#readme")),
            Some("https://github.com/o/p".into())
        );
    }

    #[test]
    fn no_guessing() {
        assert_eq!(u(json!("npm:lodash")), None);
        assert_eq!(u(json!("just-a-package-name")), None);
        assert_eq!(u(json!("https://gitlab.com/o/p")), None);
    }
}
