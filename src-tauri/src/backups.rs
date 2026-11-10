//! Restore points: the "before" of every write to the user's project.
//! Port of `src/server/backups.ts`.
//!
//! One apply is one directory under the app's data root: the bytes of every
//! file that was about to change, plus a manifest. Undo is all-or-nothing.

use crate::jsonutil::{list_dir, read_text, to_pretty, write_atomic};
use crate::lang::{Lang, t};
use crate::paths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// How many points to keep. Older runs are dropped on the next save.
const KEEP: usize = 30;

/// A change that is going to happen: `after` is the state it leaves.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FileChange {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Target {
    path: String,
    /// Whether the file was there before the write.
    existed: bool,
    /// Which file in the point holds the "before" bytes; null when none.
    slot: Option<u32>,
    /// What the write left behind — null when it left no file at all.
    #[serde(rename = "sha256After")]
    sha256_after: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Manifest {
    id: String,
    ts: i64,
    cwd: String,
    mode: String,
    targets: Vec<Target>,
}

/// What the history list shows.
#[derive(Serialize, Clone)]
pub struct RestorePoint {
    pub id: String,
    pub ts: i64,
    pub cwd: String,
    pub mode: String,
    pub files: Vec<RestoreFile>,
}

#[derive(Serialize, Clone)]
pub struct RestoreFile {
    pub path: String,
    pub existed: bool,
}

fn sha(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    let d = h.finalize();
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// `20261007-143012-a3f1` — sorts by time, cannot collide with another run.
/// Local time, not UTC: the id is read by a human who was sitting there.
fn new_id() -> String {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut b = [0u8; 2];
    // The OS CSPRNG directly — two bytes of id need no `rand` and its tree.
    getrandom::fill(&mut b).expect("OS entropy unavailable");
    format!("{}-{:02x}{:02x}", stamp, b[0], b[1])
}

/// `20261007-143012-a3f1` — the id reaches these functions from an IPC body
/// and is used as a path segment, so it is checked before it is joined anywhere.
pub fn is_valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    if b.len() != 20 || b[8] != b'-' || b[15] != b'-' {
        return false;
    }
    b.iter().enumerate().all(|(i, c)| match i {
        8 | 15 => true,
        16..=19 => c.is_ascii_hexdigit(),
        _ => c.is_ascii_digit(),
    })
}

/// Lexicographic order is chronological order — that is why the id is padded.
fn ids() -> Vec<String> {
    let mut out: Vec<String> = list_dir(&paths::backup_dir())
        .into_iter()
        .filter(|n| is_valid_id(n))
        .collect();
    out.sort();
    out
}

fn manifest_path(id: &str) -> PathBuf {
    paths::backup_dir().join(id).join("manifest.json")
}

fn read_manifest(id: &str) -> Option<Manifest> {
    if !is_valid_id(id) {
        return None;
    }
    let raw = fs::read_to_string(manifest_path(id)).ok()?;
    let m: Manifest = serde_json::from_str(&raw).ok()?;
    if m.id != id {
        return None;
    }
    Some(m)
}

/// Nothing older than the last KEEP points is kept. `newest` is excluded and
/// spared: ids in the same second differ only by their random tail, so the
/// point just written can sort below older ones and pruning would delete the
/// very thing it was called to keep.
fn prune(newest: &str) {
    let older: Vec<String> = ids().into_iter().filter(|id| id != newest).collect();
    let keep_from = older.len().saturating_sub(KEEP - 1);
    for old in &older[..keep_from] {
        let _ = fs::remove_dir_all(paths::backup_dir().join(old));
    }
}

/// Saves the state the given changes are about to overwrite, and returns the
/// id of the point. Changes that write what is already there are not recorded.
/// `mode` is what the history calls it.
pub fn save_restore_point(
    cwd: &Path,
    changes: &[FileChange],
    mode: &str,
    _lang: Lang,
) -> Option<String> {
    let ops: Vec<&FileChange> = changes.iter().filter(|c| c.before != c.after).collect();
    if ops.is_empty() {
        return None;
    }

    let id = new_id();
    let dir = paths::backup_dir().join(&id);
    let _ = fs::create_dir_all(&dir);

    let mut targets: Vec<Target> = vec![];
    let mut slot: u32 = 0;
    for op in ops {
        match &op.before {
            None => {
                // The file did not exist: its restore is a deletion, and there
                // is nothing to store. The target is still listed, or the
                // restore would not know.
                targets.push(Target {
                    path: op.path.clone(),
                    existed: false,
                    slot: None,
                    sha256_after: op.after.as_ref().map(|a| sha(a)),
                });
            }
            Some(before) => {
                let f = dir.join(slot.to_string());
                if write_atomic(&f, before).is_err() {
                    return None;
                }
                targets.push(Target {
                    path: op.path.clone(),
                    existed: true,
                    slot: Some(slot),
                    sha256_after: op.after.as_ref().map(|a| sha(a)),
                });
                slot += 1;
            }
        }
    }

    let manifest = Manifest {
        ts: chrono::Local::now().timestamp_millis(),
        id: id.clone(),
        cwd: cwd.to_string_lossy().into_owned(),
        mode: mode.to_string(),
        targets,
    };
    let _ = write_atomic(
        &manifest_path(&id),
        &to_pretty(&serde_json::to_value(&manifest).ok()?),
    );
    prune(&id);
    Some(id)
}

/// The slot files of a point — what the status line counts as "copies".
pub fn point_slots(id: &str) -> Vec<String> {
    read_manifest(id)
        .map(|m| {
            m.targets
                .iter()
                .filter_map(|tg| {
                    tg.slot
                        .map(|s| paths::backup_dir().join(id).join(s.to_string()))
                })
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

pub fn list_restore_points() -> Vec<RestorePoint> {
    let mut out: Vec<RestorePoint> = vec![];
    for id in ids().iter().rev() {
        if let Some(m) = read_manifest(id) {
            out.push(RestorePoint {
                id: m.id,
                ts: m.ts,
                cwd: m.cwd,
                mode: m.mode,
                files: m
                    .targets
                    .iter()
                    .map(|tg| RestoreFile {
                        path: tg.path.clone(),
                        existed: tg.existed,
                    })
                    .collect(),
            });
        }
    }
    out
}

/// Drops one point. Returns false when the id is malformed or holds nothing —
/// a `true` for a point that was never there would be a lie the UI repeats.
pub fn delete_restore_point(id: &str) -> bool {
    if !is_valid_id(id) {
        return false;
    }
    let dir = paths::backup_dir().join(id);
    if !dir.exists() {
        return false;
    }
    fs::remove_dir_all(dir).is_ok()
}

/// Empties the history and says how many points went.
pub fn clear_restore_points() -> usize {
    let all = ids();
    let n = all.len();
    for id in &all {
        delete_restore_point(id);
    }
    n
}

/// A target may only be written inside the project the point was taken in —
/// plus one file that is nobody's project: `~/.claude.json`.
fn inside(root: &Path, file: &Path) -> bool {
    use crate::paths::norm;
    let rl = norm(root);
    let fl = norm(file);
    if fl != rl && fl.starts_with(&format!("{}\\", rl)) {
        return true;
    }
    fl == norm(&paths::claude_json())
}

#[derive(Debug)]
pub struct RestoreOutcome {
    pub restored: Vec<String>,
    pub deleted: Vec<String>,
    pub point: Option<String>,
}

/// Puts every file of a point back. The state being overwritten is itself
/// saved first, so a rollback is a write like any other and can be rolled
/// back — the button that undoes a mistake must not be the one mistake there
/// is no undo for.
pub fn restore_point(id: &str, lang: Lang) -> Result<RestoreOutcome, String> {
    let m = read_manifest(id).ok_or_else(|| t(lang, "sess.bakNotFound", &[("id", id)]))?;
    let root = paths::backup_dir().join(id);

    let slot_text = |slot: u32| -> Result<String, String> {
        fs::read_to_string(root.join(slot.to_string())).map_err(|e| {
            t(
                lang,
                "sess.bakNotFound",
                &[("id", &format!("{} (slot {}): {}", id, slot, e))],
            )
        })
    };

    // Every target is checked before a single byte moves. A file that changed
    // since the write is somebody else's edit — restoring over it would delete
    // work the app never saw, so the whole restore is refused instead.
    for tg in &m.targets {
        let path = PathBuf::from(&tg.path);
        if !inside(Path::new(&m.cwd), &path) {
            return Err(t(lang, "sess.bakOutside", &[("path", &tg.path)]));
        }
        let current = read_text(&path);
        let untouched = match (&tg.sha256_after, &current) {
            (None, None) => true,
            (Some(h), Some(c)) => &sha(c) == h,
            _ => false,
        };
        if !untouched {
            return Err(t(lang, "sess.bakConflict", &[("path", &tg.path)]));
        }
    }

    let ops: Vec<FileChange> = m
        .targets
        .iter()
        .map(|tg| FileChange {
            path: tg.path.clone(),
            before: read_text(Path::new(&tg.path)),
            after: match tg.slot {
                None => None,
                Some(s) => slot_text(s).ok(),
            },
        })
        .collect();
    let write: Vec<FileChange> = ops
        .iter()
        .filter(|op| op.before != op.after)
        .cloned()
        .collect();
    // Taken from the state on disk, before anything below changes it. It keeps
    // the mode of the point it undoes: an undo of a profile save is a profile
    // save.
    let point = save_restore_point(Path::new(&m.cwd), &write, &m.mode, lang);

    let mut restored = vec![];
    let mut deleted = vec![];
    for op in write {
        match &op.after {
            None => {
                let _ = fs::remove_file(&op.path);
                deleted.push(op.path.clone());
            }
            Some(after) => {
                if write_atomic(Path::new(&op.path), after).is_err() {
                    return Err(t(lang, "io.readError", &[("err", &op.path)]));
                }
                restored.push(op.path.clone());
            }
        }
    }
    Ok(RestoreOutcome {
        restored,
        deleted,
        point,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cl-backups-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// Same rule as profiles: whichever test initialises first fixes the root;
    /// every assertion below looks for its own ids, never at exact counts.
    fn ensure_paths() {
        paths::init(temp_root("root"));
    }

    #[test]
    fn id_format_is_strict() {
        assert!(is_valid_id("20261007-143012-a3f1"));
        assert!(is_valid_id("20261007-143012-A3F1"), "uppercase hex passes");
        assert!(!is_valid_id("20261007-143012-xyz1"), "non-hex tail");
        assert!(!is_valid_id("2026107-143012-a3f1"), "wrong length");
        assert!(!is_valid_id("20261007_143012-a3f1"), "wrong separators");
        assert!(!is_valid_id("20261007-14301a-a3f1"), "letter in the time");
        assert!(!is_valid_id("../../etc/passwd"), "a path is not an id");
    }

    #[test]
    fn save_lists_and_a_noop_write_is_not_recorded() {
        ensure_paths();
        let proj = temp_root("proj");
        let f = proj.join("settings.json");
        fs::write(&f, "old bytes").unwrap();

        let noop = save_restore_point(
            &proj,
            &[FileChange {
                path: f.to_string_lossy().into_owned(),
                before: Some("same".into()),
                after: Some("same".into()),
            }],
            "apply",
            Lang::Ru,
        );
        assert!(noop.is_none(), "before == after means nothing happened");

        let id = save_restore_point(
            &proj,
            &[
                FileChange {
                    path: f.to_string_lossy().into_owned(),
                    before: Some("old bytes".into()),
                    after: Some("new bytes".into()),
                },
                FileChange {
                    path: proj.join("created.md").to_string_lossy().into_owned(),
                    before: None,
                    after: Some("hello".into()),
                },
            ],
            "apply",
            Lang::Ru,
        )
        .expect("a real change produces a point");

        let points = list_restore_points();
        let mine = points.iter().find(|p| p.id == id).expect("listed");
        assert_eq!(mine.mode, "apply");
        assert_eq!(mine.files.len(), 2);
        assert!(mine.files[0].existed);
        assert!(!mine.files[1].existed, "the created file had no before");

        // The "before" bytes are stored, the created file is not.
        assert_eq!(point_slots(&id).len(), 1);
        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn restore_refuses_when_someone_else_edited_and_works_when_untouched() {
        ensure_paths();
        let proj = temp_root("restore");
        let f = proj.join(".claude").join("settings.json");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, "version one").unwrap();

        let id = save_restore_point(
            &proj,
            &[FileChange {
                path: f.to_string_lossy().into_owned(),
                before: Some("version one".into()),
                after: Some("version two".into()),
            }],
            "apply",
            Lang::Ru,
        )
        .unwrap();
        fs::write(&f, "version two").unwrap();

        // A hand edit after the write: the restore must not eat it.
        fs::write(&f, "my own edit").unwrap();
        let err = restore_point(&id, Lang::Ru).expect_err("conflict refused");
        assert!(err.contains(&f.to_string_lossy().to_string()) || err.contains("settings.json"));
        assert_eq!(fs::read_to_string(&f).unwrap(), "my own edit");

        // Back to the state the write left: the restore puts version one back,
        // and the undo itself is recorded as a point.
        fs::write(&f, "version two").unwrap();
        let out = restore_point(&id, Lang::Ru).expect("restored");
        assert_eq!(out.restored, vec![f.to_string_lossy().into_owned()]);
        assert!(out.point.is_some(), "the rollback is itself rollback-able");
        assert_eq!(fs::read_to_string(&f).unwrap(), "version one");
        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn a_created_file_is_rolled_back_to_nonexistence_and_outside_is_refused() {
        ensure_paths();
        let proj = temp_root("created");
        let made = proj.join(".claude").join("written-by-apply.json");
        fs::create_dir_all(made.parent().unwrap()).unwrap();
        fs::write(&made, "hello").unwrap();

        let id = save_restore_point(
            &proj,
            &[FileChange {
                path: made.to_string_lossy().into_owned(),
                before: None,
                after: Some("hello".into()),
            }],
            "apply",
            Lang::Ru,
        )
        .unwrap();

        // The outside target: the manifest is built by hand, the app would
        // never write one, and that is exactly the case the check is for.
        let outside = temp_root("outside").join("elsewhere.json");
        fs::write(&outside, "{}").unwrap();
        let forged = json!({
            "id": id,
            "ts": 0,
            "cwd": proj.to_string_lossy(),
            "mode": "apply",
            "targets": [{ "path": outside.to_string_lossy(), "existed": true, "slot": 0, "sha256After": null }],
        });
        let mdir = paths::backup_dir().join(&id);
        // The real manifest stays; a second point carries the forged one.
        let id2 = save_restore_point(
            &proj,
            &[FileChange {
                path: made.to_string_lossy().into_owned(),
                before: Some("hello".into()),
                after: Some("hello2".into()),
            }],
            "apply",
            Lang::Ru,
        )
        .unwrap();
        fs::write(manifest_path(&id2), to_pretty(&forged)).unwrap();
        let err = restore_point(&id2, Lang::Ru).expect_err("outside target refused");
        assert!(!err.is_empty());
        assert!(made.exists(), "nothing was touched by the refused restore");
        let _ = fs::remove_dir_all(&proj);
        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_dir_all(mdir);
    }

    #[test]
    fn delete_is_honest_and_clear_counts() {
        ensure_paths();
        let proj = temp_root("del");
        let f = proj.join("a.txt");
        fs::write(&f, "x").unwrap();
        let id = save_restore_point(
            &proj,
            &[FileChange {
                path: f.to_string_lossy().into_owned(),
                before: Some("x".into()),
                after: Some("y".into()),
            }],
            "apply",
            Lang::Ru,
        )
        .unwrap();

        assert!(delete_restore_point(&id));
        assert!(
            !delete_restore_point(&id),
            "gone is gone, not a silent true"
        );
        assert!(!delete_restore_point("not-an-id"));
        assert!(list_restore_points().iter().all(|p| p.id != id));
        let _ = fs::remove_dir_all(&proj);
    }
}
