//! Is a Claude Code session running right now?
//!
//! It matters for exactly one write: `~/.claude.json`. A running session holds
//! the parsed file in memory and writes it back when it exits. The app does not
//! refuse the write — the human may know better — it says so.
//!
//! Windows only: `tasklist` ships with the OS. Anywhere else the answer is
//! "no session seen", which is the honest reading of a check we cannot perform.

pub fn claude_sessions_running() -> usize {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq claude.exe", "/NH"])
        .output();
    let out = match out {
        Ok(o) => o,
        Err(_) => return 0,
    };
    // "No tasks are running" is the empty answer; every hit names the image.
    String::from_utf8_lossy(&out.stdout)
        .to_lowercase()
        .matches("claude.exe")
        .count()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_check_answers_without_panicking() {
        // The count depends on the machine (a session may genuinely be open);
        // what the app needs is a number, never a crash.
        let _ = super::claude_sessions_running();
    }
}
