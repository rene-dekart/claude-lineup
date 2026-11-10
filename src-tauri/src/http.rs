//! The only HTTP the app does: the skillselion index, the official MCP
//! registry, raw.githubusercontent. Blocking, one GET at a time — the same
//! shape as the TS original's `fetch` with an AbortSignal timeout.

use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

// One agent for the whole app: its pool keeps HTTPS connections alive, so the
// second request to a host skips the TLS handshake the first one paid for.
// The timeout is per-request — callers legitimately use different ones.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| ureq::AgentBuilder::new().build())
}

fn status_error(res: &ureq::Response) -> String {
    format!("{} {}", res.status(), res.status_text())
}

pub fn get_json(url: &str, headers: &[(&str, &str)], timeout_ms: u64) -> Result<Value, String> {
    let mut req = agent().get(url).timeout(Duration::from_millis(timeout_ms));
    for (k, v) in headers {
        req = req.set(k, v);
    }
    let res = req.call().map_err(|e| e.to_string())?;
    if res.status() >= 300 {
        return Err(status_error(&res));
    }
    let text = res.into_string().map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub fn get_text(url: &str, timeout_ms: u64) -> Result<String, String> {
    let res = agent()
        .get(url)
        .timeout(Duration::from_millis(timeout_ms))
        .call()
        .map_err(|e| e.to_string())?;
    if res.status() >= 300 {
        return Err(status_error(&res));
    }
    res.into_string().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn offline_is_an_error_not_a_panic() {
        // Port 1 on localhost answers nothing; the error must come back as text.
        let r = super::get_json(
            "http://127.0.0.1:1/nope",
            &[("accept", "application/json")],
            2_000,
        );
        assert!(r.is_err());
    }
}
